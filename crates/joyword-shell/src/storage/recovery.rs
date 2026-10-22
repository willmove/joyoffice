//! Production text checkpoints (T104), not rich-document or undo recovery.
//!
//! Two published slots retain the last valid generation throughout replacement.
//! As in experiments/recovery-journal, written bytes must be synced and verified
//! before publication. File locks serialize cooperating local processes; detached
//! requests also compare the generation observed when their snapshot was taken.
//! The private directory must not be mutated by non-cooperating writers. CRC is
//! corruption detection, not authentication. No power-loss guarantee is implied.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use joyoffice_opc::{Budget, ZipArchive, ZipError};

const MAGIC: &[u8; 8] = b"JWREC001";
const MAGIC_V2: &[u8; 8] = b"JWREC002";
const SLOTS: [&str; 2] = ["recovery-a.jwr", "recovery-b.jwr"];
const LOCK: &str = "recovery.lock";
// Hard allocation/IO ceilings, including for legacy files. Check before copying.
const MAX_BASELINE: usize = 96 * 1024 * 1024;
const MAX_TEXT: usize = 32 * 1024 * 1024;
const MAX_RECORD: usize = MAX_BASELINE + MAX_TEXT + 49;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovered {
    pub baseline: Vec<u8>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryCheckpoint {
    /// Zero denotes the read-only legacy format.
    pub generation: u64,
    /// None for legacy or callers of the unversioned compatibility API.
    pub version: Option<u64>,
    pub recovered: Recovered,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryStage {
    Prepare,
    Lock,
    Read,
    CreateTemporary,
    Write,
    Flush,
    SyncFile,
    Verify,
    Publish,
    SyncDirectory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryReceipt {
    pub path: PathBuf,
    pub generation: u64,
    pub version: Option<u64>,
    /// False on Windows: this implementation cannot sync the directory there.
    /// True means an OS sync request succeeded, not a tested power-loss result.
    pub directory_synced: bool,
}

#[derive(Debug)]
pub struct RecoveryError {
    pub stage: RecoveryStage,
    pub source: io::Error,
    /// Some means publication completed, even if directory synchronization failed.
    pub published: Option<RecoveryReceipt>,
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "recovery at {:?}: {}", self.stage, self.source)?;
        if self.published.is_some() {
            f.write_str(" (checkpoint already published)")?;
        }
        Ok(())
    }
}
impl std::error::Error for RecoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Capture this before detaching a snapshot/job, not when its old result lands.
/// A request can publish only while its observed generation is still current.
#[derive(Clone, Debug)]
pub struct RecoveryRequest {
    directory: PathBuf,
    expected: u64,
    version: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Record {
    generation: u64,
    version: Option<u64>,
    // A published tombstone invalidates both old slots and outstanding requests.
    recovered: Option<Recovered>,
}

struct Selected {
    record: Option<Record>,
    slot: Option<usize>,
    degraded: bool,
}
impl Selected {
    fn generation(&self) -> u64 {
        self.record.as_ref().map_or(0, |record| record.generation)
    }
}

pub fn package_is_encrypted(bytes: &[u8]) -> bool {
    matches!(
        ZipArchive::new(bytes, &Budget::default()),
        Err(ZipError::Unsupported(message)) if message.contains("encrypted")
    )
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn validate(baseline: &[u8], text: &str) -> io::Result<()> {
    validate_lengths(baseline.len(), text.len())?;
    if package_is_encrypted(baseline) {
        return Err(invalid(
            "encrypted documents are not written as plaintext recovery copies",
        ));
    }
    Ok(())
}

fn validate_lengths(baseline: usize, text: usize) -> io::Result<()> {
    let total = baseline.checked_add(text).and_then(|n| n.checked_add(49));
    if baseline > MAX_BASELINE || text > MAX_TEXT || total.is_none_or(|n| n > MAX_RECORD) {
        return Err(invalid("recovery byte budget"));
    }
    Ok(())
}

/// Compatibility entry point. New callers should bind a document version with
/// `commit_versioned`, or prepare a request before dispatching asynchronous IO.
/// Encrypted packages and over-budget inputs produce no files, including locks.
pub fn commit(directory: &Path, baseline: &[u8], text: &str) -> io::Result<PathBuf> {
    validate(baseline, text)?;
    let request = prepare(directory, None)?;
    publish(&request, baseline, text)
        .map(|receipt| receipt.path)
        .map_err(io::Error::other)
}

pub fn commit_versioned(
    directory: &Path,
    version: u64,
    baseline: &[u8],
    text: &str,
) -> Result<RecoveryReceipt, RecoveryError> {
    validate(baseline, text).map_err(|source| failure(RecoveryStage::Prepare, source))?;
    let request =
        prepare(directory, Some(version)).map_err(|source| failure(RecoveryStage::Read, source))?;
    publish(&request, baseline, text)
}

/// Read-only preparation. A missing directory represents generation zero.
pub fn prepare(directory: &Path, version: Option<u64>) -> io::Result<RecoveryRequest> {
    let _lock = reader_lock(directory)?;
    let selected = select(directory)?;
    require_writable(&selected)?;
    Ok(RecoveryRequest {
        directory: directory.to_path_buf(),
        expected: selected.generation(),
        version,
    })
}

pub fn publish(
    request: &RecoveryRequest,
    baseline: &[u8],
    text: &str,
) -> Result<RecoveryReceipt, RecoveryError> {
    publish_inner(request, Some((baseline, text)), |_, _| Ok(()))
}

fn failure(stage: RecoveryStage, source: io::Error) -> RecoveryError {
    RecoveryError {
        stage,
        source,
        published: None,
    }
}

fn require_writable(selected: &Selected) -> io::Result<()> {
    if selected.degraded {
        // We cannot trust the missing generation number of a damaged header.
        // Refuse writes rather than recycle a generation or destroy evidence.
        return Err(invalid(
            "damaged recovery slot: recover to a new directory before writing",
        ));
    }
    Ok(())
}

fn publish_inner(
    request: &RecoveryRequest,
    content: Option<(&[u8], &str)>,
    mut before: impl FnMut(RecoveryStage, &Path) -> io::Result<()>,
) -> Result<RecoveryReceipt, RecoveryError> {
    let directory = &request.directory;
    let step = |stage, result: io::Result<()>| result.map_err(|source| failure(stage, source));
    step(
        RecoveryStage::Prepare,
        before(RecoveryStage::Prepare, directory),
    )?;
    if let Some((baseline, text)) = content {
        step(RecoveryStage::Prepare, validate(baseline, text))?;
    }
    step(RecoveryStage::Lock, before(RecoveryStage::Lock, directory))?;
    let _lock = writer_lock(directory).map_err(|e| failure(RecoveryStage::Lock, e))?;
    step(RecoveryStage::Read, before(RecoveryStage::Read, directory))?;
    let selected = select(directory).map_err(|e| failure(RecoveryStage::Read, e))?;
    step(RecoveryStage::Read, require_writable(&selected))?;
    if selected.generation() != request.expected {
        return Err(failure(
            RecoveryStage::Publish,
            io::Error::new(
                io::ErrorKind::WouldBlock,
                "stale recovery request; newer generation exists",
            ),
        ));
    }
    let generation = selected.generation().checked_add(1).ok_or_else(|| {
        failure(
            RecoveryStage::Prepare,
            invalid("recovery generation overflow"),
        )
    })?;
    let target = directory.join(SLOTS[selected.slot.map_or(0, |slot| 1 - slot)]);
    let record = Record {
        generation,
        version: content.and(request.version),
        recovered: content.map(|(baseline, text)| Recovered {
            baseline: baseline.to_vec(),
            text: text.to_owned(),
        }),
    };
    let payload = encode(&record);
    step(
        RecoveryStage::CreateTemporary,
        before(RecoveryStage::CreateTemporary, directory),
    )?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".joyword-recovery-pending-")
        .tempfile_in(directory)
        .map_err(|e| failure(RecoveryStage::CreateTemporary, e))?;
    step(
        RecoveryStage::Write,
        before(RecoveryStage::Write, temporary.path()).and_then(|()| temporary.write_all(&payload)),
    )?;
    step(
        RecoveryStage::Flush,
        before(RecoveryStage::Flush, temporary.path()).and_then(|()| temporary.flush()),
    )?;
    step(
        RecoveryStage::SyncFile,
        before(RecoveryStage::SyncFile, temporary.path())
            .and_then(|()| temporary.as_file().sync_all()),
    )?;
    step(
        RecoveryStage::Verify,
        before(RecoveryStage::Verify, temporary.path())
            .and_then(|()| super::verify_bytes(temporary.as_file_mut(), &payload))
            .and_then(|()| {
                // Reopen the actual file and parse it through the recovery decoder.
                if read_record(temporary.path(), false)? != Some(record) {
                    return Err(invalid("recovery readback differs"));
                }
                Ok(())
            }),
    )?;
    step(
        RecoveryStage::Publish,
        before(RecoveryStage::Publish, temporary.path()),
    )?;
    // persist uses same-directory atomic replacement; never truncate a slot.
    temporary
        .persist(&target)
        .map_err(|e| failure(RecoveryStage::Publish, e.error))?;
    let mut receipt = RecoveryReceipt {
        path: target,
        generation,
        version: request.version,
        directory_synced: false,
    };
    before(RecoveryStage::SyncDirectory, directory)
        .and_then(|()| super::sync_directory(directory))
        .map_err(|source| RecoveryError {
            stage: RecoveryStage::SyncDirectory,
            source,
            published: Some(receipt.clone()),
        })?;
    receipt.directory_synced = cfg!(unix);
    Ok(receipt)
}

/// Explicit discard publishes a tombstone instead of unlinking in an order that
/// could resurrect an older slot after interruption. Old requests are rejected.
/// Legacy bytes are never modified; the newer published tombstone supersedes them.
/// A damaged journal must be recovered to a new directory before further writes.
pub fn clear(directory: &Path) -> io::Result<()> {
    let request = prepare(directory, None)?;
    publish_inner(&request, None, |_, _| Ok(())).map_err(io::Error::other)?;
    Ok(())
}

pub fn recover(directory: &Path) -> io::Result<Recovered> {
    recover_checkpoint(directory).map(|checkpoint| checkpoint.recovered)
}

pub fn recover_checkpoint(directory: &Path) -> io::Result<RecoveryCheckpoint> {
    let _lock = reader_lock(directory)?;
    let record = select(directory)?
        .record
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no published recovery"))?;
    let recovered = record
        .recovered
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "recovery explicitly discarded"))?;
    Ok(RecoveryCheckpoint {
        generation: record.generation,
        version: record.version,
        recovered,
    })
}

fn regular(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid("recovery path is not a regular file"));
    }
    Ok(())
}

fn reader_lock(directory: &Path) -> io::Result<Option<File>> {
    let path = directory.join(LOCK);
    match regular(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        result => result?,
    }
    let file = File::open(path)?;
    file.try_lock_shared().map_err(io::Error::from)?;
    Ok(Some(file))
}

fn writer_lock(directory: &Path) -> io::Result<File> {
    fs::create_dir_all(directory)?;
    let path = directory.join(LOCK);
    match regular(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        result => result?,
    }
    // Never delete the lock file: another process may still hold its inode.
    // The OS releases the lock on process death; no stale lock-file recovery.
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().map_err(io::Error::from)?;
    Ok(file)
}

fn select(directory: &Path) -> io::Result<Selected> {
    let mut selected = Selected {
        record: None,
        slot: None,
        degraded: false,
    };
    for (slot, name) in SLOTS.iter().enumerate() {
        let candidate = match read_record(&directory.join(name), false) {
            Ok(record) => record,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
                ) =>
            {
                selected.degraded = true;
                None
            }
            Err(e) => return Err(e),
        };
        if let Some(record) = candidate {
            if let Some(previous) = &selected.record {
                if record.generation == previous.generation && record != *previous {
                    return Err(invalid(
                        "conflicting recovery records at the same generation",
                    ));
                }
                if record.generation <= previous.generation {
                    continue;
                }
            }
            selected.record = Some(record);
            selected.slot = Some(slot);
        }
    }
    if selected.record.is_none() {
        if selected.degraded {
            // Do not silently resurrect the legacy file behind damaged v2 slots.
            return Err(invalid("no valid published recovery generation"));
        }
        selected.record = read_record(&directory.join("recovery.jwr"), true)?;
    }
    Ok(selected)
}

fn read_record(path: &Path, legacy: bool) -> io::Result<Option<Record>> {
    match regular(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        result => result?,
    }
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_RECORD as u64 {
        return Err(invalid("recovery file budget"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD {
        return Err(invalid("recovery file budget"));
    }
    decode(&bytes, legacy).map(Some)
}

fn encode(record: &Record) -> Vec<u8> {
    let mut bytes = MAGIC_V2.to_vec();
    bytes.extend_from_slice(&record.generation.to_le_bytes());
    bytes.push(if record.recovered.is_none() {
        2
    } else {
        u8::from(record.version.is_some())
    });
    bytes.extend_from_slice(&record.version.unwrap_or(0).to_le_bytes());
    if let Some(recovered) = &record.recovered {
        write_blob(&mut bytes, &recovered.baseline);
        write_blob(&mut bytes, recovered.text.as_bytes());
    }
    bytes.extend_from_slice(&crc32fast::hash(&bytes).to_le_bytes());
    bytes
}

fn decode(bytes: &[u8], legacy: bool) -> io::Result<Record> {
    let magic = if legacy { MAGIC } else { MAGIC_V2 };
    if bytes.len() < 12 || bytes.len() > MAX_RECORD || &bytes[..8] != magic {
        return Err(invalid("recovery header or budget"));
    }
    let (body, crc) = bytes.split_at(bytes.len() - 4);
    let expected = u32::from_le_bytes(crc.try_into().map_err(|_| invalid("recovery checksum"))?);
    if crc32fast::hash(body) != expected {
        return Err(invalid("recovery checksum"));
    }
    let mut cursor = &body[8..];
    let mut record = Record {
        generation: 0,
        version: None,
        recovered: None,
    };
    if !legacy {
        record.generation = read_u64(&mut cursor)?;
        if record.generation == 0 {
            return Err(invalid("zero recovery generation"));
        }
        let (&kind, rest) = cursor
            .split_first()
            .ok_or_else(|| invalid("recovery kind"))?;
        cursor = rest;
        let version = read_u64(&mut cursor)?;
        match kind {
            0 if version == 0 => {}
            1 => record.version = Some(version),
            2 if version == 0 && cursor.is_empty() => return Ok(record),
            _ => return Err(invalid("recovery kind or version")),
        }
    }
    let baseline = read_blob(&mut cursor, MAX_BASELINE)?;
    let text = read_blob(&mut cursor, MAX_TEXT)?;
    if !cursor.is_empty() {
        return Err(invalid("trailing recovery bytes"));
    }
    let text = String::from_utf8(text).map_err(|_| invalid("recovery text"))?;
    // Also reject encrypted bytes from legacy/untrusted recovery files before
    // Office::recover_from can write a recovered package or apply plaintext.
    validate(&baseline, &text)?;
    record.recovered = Some(Recovered { baseline, text });
    Ok(record)
}

/// Where a running editor journals a commit. An explicit directory wins.
/// Otherwise the journal lives under the per-user configuration directory.
pub fn directory_from(explicit: Option<&Path>, config_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit.filter(|path| !path.as_os_str().is_empty()) {
        return Some(path.to_path_buf());
    }
    config_dir.map(|dir| dir.join("recovery"))
}

/// `JOYWORD_RECOVERY_DIR` when set, otherwise the active profile directory.
pub fn directory() -> Option<PathBuf> {
    let explicit = std::env::var_os("JOYWORD_RECOVERY_DIR").map(PathBuf::from);
    let config = crate::diag::resolve().ok().map(|paths| paths.config_dir);
    directory_from(explicit.as_deref(), config.as_deref())
}

fn write_blob(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn read_u64(input: &mut &[u8]) -> io::Result<u64> {
    let bytes = input
        .get(..8)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "recovery integer"))?;
    let value = u64::from_le_bytes(bytes.try_into().map_err(|_| invalid("recovery integer"))?);
    *input = &input[8..];
    Ok(value)
}

fn read_blob(input: &mut &[u8], budget: usize) -> io::Result<Vec<u8>> {
    let len = usize::try_from(read_u64(input)?).map_err(|_| invalid("recovery length overflow"))?;
    if len > budget {
        return Err(invalid("recovery blob budget"));
    }
    let bytes = input
        .get(..len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "recovery blob"))?;
    let result = bytes.to_vec();
    *input = &input[len..];
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_rotates_and_falls_back_from_a_corrupt_latest() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let first = commit(dir.path(), b"PK", "one")?;
        let second = commit(dir.path(), b"PK", "two")?;
        assert_ne!(first, second, "two independent published generations");
        assert_eq!(recover(dir.path())?.text, "two");
        let third = commit(dir.path(), b"PK", "three")?;
        assert_eq!(third, first, "rotation reuses only the older slot");
        assert_eq!(recover_checkpoint(dir.path())?.generation, 3);
        fs::write(third, b"torn")?;
        assert_eq!(recover(dir.path())?.text, "two");
        // Fail closed on further writes: a corrupt header cannot supply a safe
        // monotonic generation, and the last valid generation must survive.
        assert!(commit(dir.path(), b"PK", "four").is_err());
        assert_eq!(recover(dir.path())?.text, "two");
        fs::write(second, b"also torn")?;
        assert!(recover(dir.path()).is_err());
        Ok(())
    }

    fn legacy(baseline: &[u8], text: &[u8]) -> Vec<u8> {
        let mut bytes = MAGIC.to_vec();
        write_blob(&mut bytes, baseline);
        write_blob(&mut bytes, text);
        bytes.extend_from_slice(&crc32fast::hash(&bytes).to_le_bytes());
        bytes
    }

    #[test]
    fn legacy_trailing_bytes_are_rejected_even_with_a_valid_crc() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let mut bytes = legacy(b"PK", b"text");
        bytes.truncate(bytes.len() - 4);
        bytes.push(0);
        bytes.extend_from_slice(&crc32fast::hash(&bytes).to_le_bytes());
        fs::write(dir.path().join("recovery.jwr"), bytes)?;
        assert!(recover(dir.path()).is_err());
        Ok(())
    }

    #[test]
    fn superseded_save_ticket_does_not_publish() -> Result<(), Box<dyn std::error::Error>> {
        use crate::command::{ArgValue, EDIT_INSERT, FILE_OPEN};
        use crate::host::Office;
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source.docx");
        fs::write(
            &source,
            include_bytes!("../../../../tests/fixtures/docx/DOCX-MIN-001.docx"),
        )?;
        let mut office = Office::new();
        office
            .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
            .map_err(|e| io::Error::other(format!("{e:?}")))?;
        office
            .invoke(EDIT_INSERT, &[("text", ArgValue::Text("N".into()))])
            .map_err(|e| io::Error::other(format!("{e:?}")))?;
        let stale_path = dir.path().join("stale.docx");
        let error = |e: crate::command::ExecuteError| io::Error::other(e.to_string());
        let stale = office.start_save(&stale_path).map_err(error)?;
        let _newer = office
            .start_save(&dir.path().join("newer.docx"))
            .map_err(error)?;
        assert!(!office.finish_save(stale).map_err(error)?);
        assert!(!stale_path.exists());
        Ok(())
    }

    fn record(generation: u64, text: &str) -> Record {
        Record {
            generation,
            version: Some(generation),
            recovered: Some(Recovered {
                baseline: b"PK".to_vec(),
                text: text.into(),
            }),
        }
    }

    #[test]
    fn real_io_faults_have_stages_and_keep_the_last_published_generation()
    -> Result<(), Box<dyn std::error::Error>> {
        for stage in [
            RecoveryStage::Prepare,
            RecoveryStage::Lock,
            RecoveryStage::Read,
            RecoveryStage::CreateTemporary,
            RecoveryStage::Write,
            RecoveryStage::Flush,
            RecoveryStage::SyncFile,
            RecoveryStage::Verify,
            RecoveryStage::Publish,
            RecoveryStage::SyncDirectory,
        ] {
            let dir = tempfile::tempdir()?;
            commit(dir.path(), b"PK", "one")?;
            commit(dir.path(), b"PK", "two")?;
            let request = prepare(dir.path(), Some(3))?;
            let result = publish_inner(&request, Some((b"PK", "three")), |at, path| {
                if at == stage {
                    if at == RecoveryStage::Write {
                        fs::write(path, b"partial")?;
                    }
                    Err(io::Error::new(
                        io::ErrorKind::StorageFull,
                        "injected IO failure",
                    ))
                } else {
                    Ok(())
                }
            });
            let error = result
                .err()
                .ok_or_else(|| invalid("fault unexpectedly succeeded"))?;
            assert_eq!(error.stage, stage);
            assert!(error.to_string().contains(&format!("{stage:?}")));
            if stage == RecoveryStage::SyncDirectory {
                let receipt = error
                    .published
                    .ok_or_else(|| invalid("missing published receipt"))?;
                assert_eq!(receipt.generation, 3);
                assert!(!receipt.directory_synced);
                assert_eq!(recover(dir.path())?.text, "three");
                fs::write(receipt.path, b"corrupt after publication")?;
            } else {
                assert!(error.published.is_none());
            }
            assert_eq!(recover(dir.path())?.text, "two");
        }
        Ok(())
    }

    #[test]
    fn readback_rejects_corruption_truncation_and_growth_before_publish()
    -> Result<(), Box<dyn std::error::Error>> {
        for mutation in 0..3 {
            let dir = tempfile::tempdir()?;
            commit(dir.path(), b"PK", "previous")?;
            let request = prepare(dir.path(), Some(2))?;
            let result = publish_inner(&request, Some((b"PK", "latest")), |stage, path| {
                if stage == RecoveryStage::Verify {
                    let mut bytes = fs::read(path)?;
                    match mutation {
                        0 => bytes[10] ^= 1,
                        1 => bytes.truncate(12),
                        _ => bytes.push(0),
                    }
                    fs::write(path, bytes)?;
                }
                Ok(())
            });
            let error = result.err().ok_or_else(|| invalid("readback succeeded"))?;
            assert_eq!(error.stage, RecoveryStage::Verify);
            assert!(error.published.is_none());
            assert_eq!(recover(dir.path())?.text, "previous");
        }
        Ok(())
    }

    #[test]
    fn real_rename_error_retains_previous_slot() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        commit(dir.path(), b"PK", "previous")?;
        let request = prepare(dir.path(), Some(2))?;
        let result = publish_inner(&request, Some((b"PK", "latest")), |stage, _| {
            if stage == RecoveryStage::Publish {
                fs::create_dir(dir.path().join(SLOTS[1]))?;
            }
            Ok(())
        });
        let error = result.err().ok_or_else(|| invalid("rename succeeded"))?;
        assert_eq!(error.stage, RecoveryStage::Publish);
        assert!(error.published.is_none());
        assert_eq!(recover(dir.path())?.text, "previous");
        Ok(())
    }

    #[test]
    fn legacy_is_read_only_through_failed_and_successful_migration()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("recovery.jwr");
        let bytes = legacy(b"PK", b"legacy");
        fs::write(&path, &bytes)?;
        assert_eq!(recover_checkpoint(dir.path())?.generation, 0);
        let request = prepare(dir.path(), Some(9))?;
        for stage in [
            RecoveryStage::Write,
            RecoveryStage::SyncFile,
            RecoveryStage::Verify,
            RecoveryStage::Publish,
        ] {
            assert!(
                publish_inner(&request, Some((b"PK", "migrated")), |at, _| {
                    if at == stage {
                        Err(io::Error::other("migration fault"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
            );
            assert_eq!(recover(dir.path())?.text, "legacy");
            assert_eq!(fs::read(&path)?, bytes);
        }
        let receipt = publish(&request, b"PK", "migrated")?;
        assert_eq!(receipt.generation, 1);
        assert_eq!(receipt.directory_synced, cfg!(unix));
        assert_eq!(recover_checkpoint(dir.path())?.version, Some(9));
        assert_eq!(fs::read(&path)?, bytes);
        fs::write(receipt.path, b"broken")?;
        assert!(
            recover(dir.path()).is_err(),
            "must not fall back behind a published v2 slot"
        );
        assert_eq!(fs::read(path)?, bytes);
        Ok(())
    }

    #[test]
    fn pending_and_tmp_are_never_candidates_and_equal_generation_conflicts_fail() -> io::Result<()>
    {
        let dir = tempfile::tempdir()?;
        let bytes = encode(&record(99, "not published"));
        for name in [
            "recovery.jwr.tmp",
            "recovery-pending.jwr",
            ".joyword-recovery-pending-crashed",
        ] {
            fs::write(dir.path().join(name), &bytes)?;
        }
        assert_eq!(
            recover(dir.path()).err().map(|e| e.kind()),
            Some(io::ErrorKind::NotFound)
        );
        let a = dir.path().join(SLOTS[0]);
        let b = dir.path().join(SLOTS[1]);
        fs::write(&a, encode(&record(7, "same")))?;
        fs::write(&b, encode(&record(7, "same")))?;
        assert_eq!(recover(dir.path())?.text, "same");
        fs::write(&b, encode(&record(7, "different")))?;
        let error = recover(dir.path())
            .err()
            .ok_or_else(|| invalid("conflict accepted"))?;
        assert!(error.to_string().contains("same generation"));
        assert!(commit(dir.path(), b"PK", "overwrite conflict").is_err());
        Ok(())
    }

    #[test]
    fn stale_requests_concurrent_publishers_and_discard_cannot_regress_generation()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let stale = prepare(dir.path(), Some(1))?;
        let later = prepare(dir.path(), Some(2))?;
        publish(&later, b"PK", "N+1")?;
        let error = publish(&stale, b"PK", "N")
            .err()
            .ok_or_else(|| invalid("stale accepted"))?;
        assert_eq!(error.stage, RecoveryStage::Publish);
        assert_eq!(recover_checkpoint(dir.path())?.version, Some(2));
        let concurrent = prepare(dir.path(), Some(3))?;
        let barrier = std::sync::Barrier::new(2);
        let results = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                publish(&concurrent, b"PK", "first")
            });
            let second = scope.spawn(|| {
                barrier.wait();
                publish(&concurrent, b"PK", "second")
            });
            (first.join(), second.join())
        });
        let first = results
            .0
            .map_err(|_| io::Error::other("publisher panicked"))?;
        let second = results
            .1
            .map_err(|_| io::Error::other("publisher panicked"))?;
        assert_ne!(first.is_ok(), second.is_ok());
        assert_eq!(recover_checkpoint(dir.path())?.generation, 2);
        let winner = recover(dir.path())?;
        let lock = writer_lock(dir.path())?;
        assert!(publish(&concurrent, b"PK", "locked").is_err());
        drop(lock);
        assert_eq!(recover(dir.path())?, winner);
        clear(dir.path())?;
        assert!(publish(&concurrent, b"PK", "after discard").is_err());
        assert!(recover(dir.path()).is_err());
        Ok(())
    }

    #[test]
    fn bounded_decoder_rejects_lengths_overflow_utf8_truncation_and_generation_overflow()
    -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("recovery.jwr");
        let valid = legacy(b"PK", b"text");
        for end in 0..valid.len() {
            fs::write(&path, &valid[..end])?;
            assert!(recover(dir.path()).is_err(), "truncation {end}");
        }
        let mut overflow = MAGIC.to_vec();
        overflow.extend_from_slice(&u64::MAX.to_le_bytes());
        overflow.extend_from_slice(&crc32fast::hash(&overflow).to_le_bytes());
        fs::write(&path, overflow)?;
        assert!(recover(dir.path()).is_err());
        fs::write(&path, legacy(b"PK", &[0xff]))?;
        assert!(recover(dir.path()).is_err());
        let oversized = File::create(&path)?;
        oversized.set_len(MAX_RECORD as u64 + 1)?;
        drop(oversized);
        assert!(recover(dir.path()).is_err());
        assert!(validate_lengths(usize::MAX, 1).is_err());
        assert!(validate_lengths(MAX_BASELINE + 1, 0).is_err());
        assert!(validate_lengths(0, MAX_TEXT + 1).is_err());
        let absent = dir.path().join("over-budget");
        assert!(commit(&absent, b"PK", &"x".repeat(MAX_TEXT + 1)).is_err());
        assert!(!absent.exists());
        fs::write(dir.path().join(SLOTS[0]), encode(&record(u64::MAX, "last")))?;
        assert!(commit(dir.path(), b"PK", "overflow").is_err());
        assert_eq!(recover_checkpoint(dir.path())?.generation, u64::MAX);
        Ok(())
    }

    fn encrypted_fixture() -> Vec<u8> {
        let mut bytes =
            include_bytes!("../../../../tests/fixtures/docx/DOCX-MIN-001.docx").to_vec();
        for i in 0..bytes.len().saturating_sub(10) {
            if &bytes[i..i + 4] == b"PK\x03\x04" {
                bytes[i + 6] |= 1;
            }
            if &bytes[i..i + 4] == b"PK\x01\x02" {
                bytes[i + 8] |= 1;
            }
        }
        bytes
    }

    #[test]
    fn encryption_is_refused_on_write_and_on_legacy_or_v2_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let encrypted = encrypted_fixture();
        assert!(package_is_encrypted(&encrypted));
        let absent = dir.path().join("absent");
        assert!(commit(&absent, &encrypted, "plaintext").is_err());
        assert!(commit_versioned(&absent, 2, &encrypted, "plaintext").is_err());
        assert!(!absent.exists());
        let request = prepare(&absent, Some(1))?;
        let error = publish(&request, &encrypted, "plaintext")
            .err()
            .ok_or_else(|| invalid("encrypted accepted"))?;
        assert_eq!(error.stage, RecoveryStage::Prepare);
        assert!(!absent.exists());
        fs::write(
            dir.path().join("recovery.jwr"),
            legacy(&encrypted, b"plaintext"),
        )?;
        assert!(crate::host::Office::recover_from(dir.path()).is_err());
        assert!(!dir.path().join("recovered.docx").exists());
        let malicious = Record {
            generation: 1,
            version: Some(1),
            recovered: Some(Recovered {
                baseline: encrypted,
                text: "plaintext".into(),
            }),
        };
        fs::write(dir.path().join(SLOTS[0]), encode(&malicious))?;
        assert!(crate::host::Office::recover_from(dir.path()).is_err());
        assert!(!dir.path().join("recovered.docx").exists());
        Ok(())
    }

    #[test]
    fn detached_save_n_completion_keeps_n_plus_one_dirty_and_recoverable()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::command::{ArgValue, EDIT_INSERT, FILE_OPEN};
        use crate::host::Office;
        let dir = tempfile::tempdir()?;
        let journal = dir.path().join("journal");
        let source = dir.path().join("source.docx");
        fs::write(
            &source,
            include_bytes!("../../../../tests/fixtures/docx/DOCX-MIN-001.docx"),
        )?;
        let mut office = Office::new();
        office.set_recovery_directory(journal.clone());
        let invoke_error = |e| io::Error::other(format!("{e:?}"));
        office
            .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
            .map_err(invoke_error)?;
        office
            .invoke(EDIT_INSERT, &[("text", ArgValue::Text("N".into()))])
            .map_err(invoke_error)?;
        let id = office.active_id().ok_or_else(|| invalid("no active id"))?;
        let job = office
            .active()
            .ok_or_else(|| invalid("no input"))?
            .save_original_job();
        office
            .invoke(EDIT_INSERT, &[("text", ArgValue::Text("N+1".into()))])
            .map_err(invoke_error)?;
        let newer = recover_checkpoint(&journal)?;
        let output = job.run(crate::storage::OriginalChoice::Confirm);
        let receipt = office
            .apply_original_save(id, output)
            .ok_or_else(|| invalid("no save owner"))??;
        assert_eq!(receipt.version(), job.version());
        assert!(office.is_dirty(id));
        assert!(
            newer
                .version
                .is_some_and(|version| version > receipt.version().get())
        );
        assert_eq!(recover_checkpoint(&journal)?, newer);
        let restored =
            Office::recover_from(&journal).map_err(|e| io::Error::other(e.to_string()))?;
        assert_eq!(
            restored
                .active()
                .ok_or_else(|| invalid("no restored input"))?
                .view()
                .index()
                .text(),
            newer.recovered.text
        );
        Ok(())
    }

    #[test]
    fn an_explicit_directory_wins_over_the_config_dir() {
        let chosen = directory_from(
            Some(Path::new("/var/joyword-recovery")),
            Some(Path::new("/cfg")),
        );
        assert_eq!(chosen, Some(PathBuf::from("/var/joyword-recovery")));
    }

    #[test]
    fn the_config_dir_is_used_when_no_directory_is_explicit() {
        let chosen = directory_from(Some(Path::new("")), Some(Path::new("/cfg")));
        assert_eq!(chosen, Some(PathBuf::from("/cfg/recovery")));
        assert_eq!(directory_from(None, None), None);
    }

    #[test]
    fn clearing_removes_a_committed_journal() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        commit(dir.path(), b"PK", "edited")?;
        let stale = prepare(dir.path(), Some(2))?;
        clear(dir.path())?;
        assert!(
            recover(dir.path()).is_err(),
            "discarded journal must not restore"
        );
        assert!(publish(&stale, b"PK", "resurrect").is_err());
        clear(dir.path())?;
        commit(dir.path(), b"PK", "new session")?;
        assert_eq!(recover_checkpoint(dir.path())?.generation, 4);
        Ok(())
    }
}
