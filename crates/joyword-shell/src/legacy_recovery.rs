use crate::reader::{Reader, check_cancel, guarded};
use joyword_engine::doc::{Document, Pos, StoryRef, para};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

const SLOTS: [&str; 2] = ["recovery-a.jwr", "recovery-b.jwr"];
const SINGLE: &str = "recovery.jwr";
const MAX_PACKAGE: usize = 96 << 20;
const MAX_TEXT: usize = 32 << 20;
const MAX_RECORD: usize = MAX_PACKAGE + MAX_TEXT + 49;
const OBJECT_MARKER: char = '\u{fdd0}';

/// Locate the prior desktop's published journals without creating or changing any files.
pub fn legacy_recovery_directory() -> Option<PathBuf> {
    let directory = std::env::var_os("JOYWORD_RECOVERY_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| crate::prefs::default_config_dir().map(|root| root.join("recovery")))?;
    [SINGLE, SLOTS[0], SLOTS[1]]
        .iter()
        .any(|name| fs::symlink_metadata(directory.join(name)).is_ok_and(|m| m.is_file()))
        .then_some(directory)
}

#[derive(PartialEq, Eq)]
struct Record {
    generation: u64,
    version: Option<u64>,
    content: Option<Checkpoint>,
}

#[derive(PartialEq, Eq)]
struct Checkpoint {
    baseline: Vec<u8>,
    text: String,
    snapshot: Option<Vec<u8>>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// Filesystem uncertainty is not corrupt journal data: selection must stop,
// rather than fall back to an older generation that may have been discarded.
fn open_regular(path: &Path) -> io::Result<Option<File>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => open_checked_regular(path, &meta).map(Some),
        Ok(_) => Err(io::Error::other("recovery path is not a regular file")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

// Keep the metadata-to-open boundary explicit so replacement can be tested
// deterministically without racing threads or timing-sensitive sleeps.
fn open_checked_regular(path: &Path, before: &fs::Metadata) -> io::Result<File> {
    #[cfg(unix)]
    let file: File = {
        use rustix::fs::{Mode, OFlags, open};
        open(
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?
        .into()
    };
    #[cfg(not(unix))]
    let file = File::open(path)?;
    let opened = file.metadata()?;
    let after = fs::symlink_metadata(path)?;
    if !before.is_file() || !opened.is_file() || !after.is_file() {
        return Err(io::Error::other(
            "recovery path changed or is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let identity = |m: &fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        if identity(before) != identity(&opened) || identity(before) != identity(&after) {
            return Err(io::Error::other("recovery file changed while opening"));
        }
    }
    #[cfg(not(unix))]
    if before.len() != opened.len()
        || before.len() != after.len()
        || before.modified()? != opened.modified()?
        || before.modified()? != after.modified()?
    {
        return Err(io::Error::other("recovery file changed while opening"));
    }
    Ok(file)
}

fn blob<'a>(input: &mut &'a [u8], limit: usize) -> io::Result<&'a [u8]> {
    let len = input
        .split_off(..8)
        .and_then(|n| n.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or_else(|| invalid("truncated recovery length"))?;
    if len > limit as u64 {
        return Err(invalid("recovery blob exceeds budget"));
    }
    input
        .split_off(..len as usize)
        .ok_or_else(|| invalid("truncated recovery blob"))
}

fn decode(bytes: &[u8], single: bool) -> io::Result<Record> {
    if bytes.len() > MAX_RECORD {
        return Err(invalid("recovery file exceeds budget"));
    }
    let (body, crc) = bytes
        .split_last_chunk::<4>()
        .ok_or_else(|| invalid("truncated recovery header"))?;
    let (magic, mut input) = body
        .split_first_chunk::<8>()
        .ok_or_else(|| invalid("truncated recovery header"))?;
    if !matches!(
        (single, magic),
        (true, b"JWREC001") | (false, b"JWREC002" | b"JWREC003")
    ) {
        return Err(invalid("unsupported recovery header"));
    }
    if crc32fast::hash(body) != u32::from_le_bytes(*crc) {
        return Err(invalid("recovery checksum mismatch"));
    }
    let (generation, version) = if single {
        (0, None)
    } else {
        let header = input
            .split_off(..17)
            .ok_or_else(|| invalid("truncated recovery generation"))?;
        let generation = u64::from_le_bytes(
            header[..8]
                .try_into()
                .map_err(|_| invalid("recovery generation"))?,
        );
        let version = u64::from_le_bytes(
            header[9..]
                .try_into()
                .map_err(|_| invalid("recovery version"))?,
        );
        if generation == 0 {
            return Err(invalid("zero recovery generation"));
        }
        let version = match header[8] {
            0 if version == 0 => None,
            1 => Some(version),
            2 if version == 0 && input.is_empty() => {
                return Ok(Record {
                    generation,
                    version: None,
                    content: None,
                });
            }
            _ => return Err(invalid("invalid recovery kind or version")),
        };
        (generation, version)
    };
    let baseline = blob(&mut input, MAX_PACKAGE)?;
    let text = std::str::from_utf8(blob(&mut input, MAX_TEXT)?)
        .map_err(|_| invalid("recovery text is not UTF-8"))?;
    let snapshot = if magic == b"JWREC003" {
        let bytes = blob(&mut input, MAX_PACKAGE)?;
        if bytes.is_empty() {
            return Err(invalid("empty recovery snapshot"));
        }
        Some(bytes)
    } else {
        None
    };
    if !input.is_empty() {
        return Err(invalid("trailing recovery bytes"));
    }
    for package in std::iter::once(baseline).chain(snapshot) {
        if joyword_docx::is_encrypted(package)
            || matches!(
                joyoffice_opc::ZipArchive::new(package, &Default::default()),
                Err(joyoffice_opc::ZipError::Unsupported(message)) if message.contains("encrypted")
            )
        {
            return Err(invalid("encrypted recovery package"));
        }
    }
    Ok(Record {
        generation,
        version,
        content: Some(Checkpoint {
            baseline: baseline.to_vec(),
            text: text.to_owned(),
            snapshot: snapshot.map(<[u8]>::to_vec),
        }),
    })
}

/// Validate the bytes of one legacy record without consulting the filesystem.
pub fn validate_legacy_recovery_record(bytes: &[u8], single: bool) -> io::Result<()> {
    decode(bytes, single).map(|_| ())
}

fn read_record(path: &Path, single: bool) -> io::Result<Option<Record>> {
    let Some(file) = open_regular(path)? else {
        return Ok(None);
    };
    if file.metadata()?.len() > MAX_RECORD as u64 {
        return Err(invalid("recovery file exceeds budget"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD {
        return Err(invalid("recovery file exceeds budget"));
    }
    decode(&bytes, single).map(Some)
}

fn select(path: &Path, cancel: &AtomicBool) -> Result<(Checkpoint, bool), String> {
    let directory = if path.is_dir() {
        path
    } else {
        let name = path.file_name().and_then(|name| name.to_str());
        if !name.is_some_and(|name| name == SINGLE || SLOTS.contains(&name)) {
            return Err("Select a legacy journal directory or recovery[-a|-b].jwr".into());
        }
        path.parent().unwrap_or(Path::new("."))
    };
    // Hold the existing writer's lock while inspecting both slots. Never create a lock.
    let lock = open_regular(&directory.join("recovery.lock")).map_err(|e| e.to_string())?;
    if let Some(file) = &lock {
        file.try_lock_shared()
            .map_err(|e| format!("Cannot lock the legacy journal for reading: {e}"))?;
    }
    let result = select_locked(directory, cancel);
    if let Some(file) = lock {
        let _ = file.unlock();
    }
    result
}

fn select_locked(directory: &Path, cancel: &AtomicBool) -> Result<(Checkpoint, bool), String> {
    let mut selected: Option<Record> = None;
    let mut damaged = Vec::new();
    for name in SLOTS {
        check_cancel(cancel)?;
        match read_record(&directory.join(name), false) {
            Ok(Some(candidate)) => {
                if let Some(current) = &selected
                    && candidate.generation == current.generation
                    && &candidate != current
                {
                    return Err("Conflicting recovery records at the same generation".into());
                }
                if selected
                    .as_ref()
                    .is_none_or(|current| candidate.generation > current.generation)
                {
                    selected = Some(candidate);
                }
            }
            Ok(None) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
                ) =>
            {
                damaged.push(format!("{name}: {e}"));
            }
            Err(e) => return Err(format!("{name}: {e}")),
        }
    }
    if selected.is_none() {
        if !damaged.is_empty() {
            return Err(format!(
                "No valid published recovery generation ({})",
                damaged.join(", ")
            ));
        }
        selected =
            read_record(&directory.join(SINGLE), true).map_err(|e| format!("{SINGLE}: {e}"))?;
    }
    let record = selected.ok_or("No published recovery checkpoint")?;
    let content = record.content.ok_or("Recovery was explicitly discarded")?;
    Ok((content, !damaged.is_empty()))
}

struct BodyText {
    text: String,
    starts: Vec<(usize, Pos)>,
}

fn projected_chars(
    paragraph: &joyword_engine::doc::Paragraph,
) -> impl Iterator<Item = (usize, char)> {
    let mut objects = paragraph.objects.iter();
    paragraph
        .text
        .char_indices()
        .filter_map(move |(offset, ch)| {
            let projected = match ch {
                para::OBJ if objects.next().is_some_and(para::InlineObject::is_marker) => {
                    return None;
                }
                para::OBJ | para::LINE_BREAK | para::PAGE_BREAK | para::COLUMN_BREAK => {
                    OBJECT_MARKER
                }
                ch => ch,
            };
            Some((offset, projected))
        })
}

impl BodyText {
    fn new(doc: &Document) -> Result<Self, String> {
        let mut result = Self {
            text: String::new(),
            starts: Vec::new(),
        };
        for path in doc.para_paths(StoryRef::Body) {
            let paragraph = doc
                .para(StoryRef::Body, &path)
                .ok_or("Missing recovery paragraph")?;
            if !result.starts.is_empty() {
                result.text.push('\n');
            }
            result
                .starts
                .push((result.text.len(), Pos::new(StoryRef::Body, path, 0)));
            for (_, projected) in projected_chars(paragraph) {
                result.text.push(projected);
                if result.text.len() > MAX_TEXT {
                    return Err("Recovery body text exceeds budget".into());
                }
            }
        }
        Ok(result)
    }

    fn position(&self, doc: &Document, offset: usize) -> Result<Pos, String> {
        let index = self.starts.partition_point(|(start, _)| *start <= offset);
        let (start, position) = self
            .starts
            .get(index.saturating_sub(1))
            .ok_or("Missing recovery position")?;
        let paragraph = doc.para_at(position).ok_or("Missing recovery paragraph")?;
        let mut relative = offset - start;
        for (byte, ch) in projected_chars(paragraph) {
            if relative == 0 {
                return Ok(Pos {
                    off: byte,
                    ..position.clone()
                });
            }
            relative = relative
                .checked_sub(ch.len_utf8())
                .ok_or("Invalid recovery character boundary")?;
        }
        if relative != 0 {
            return Err("Recovery range does not match the baseline".into());
        }
        Ok(Pos {
            off: paragraph.len(),
            ..position.clone()
        })
    }
}

fn diff<'a>(before: &str, after: &'a str) -> (usize, usize, &'a str) {
    let prefix: usize = before
        .chars()
        .zip(after.chars())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum();
    let suffix: usize = before[prefix..]
        .chars()
        .rev()
        .zip(after[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum();
    (
        prefix,
        before.len() - suffix,
        &after[prefix..after.len() - suffix],
    )
}

fn replay(reader: &mut Reader, current: BodyText, text: &str) -> Result<(), String> {
    let mut edits = Vec::new();
    let mut lines = text.split('\n');
    // When paragraph boundaries are stable, disjoint local edits can preserve
    // the table cells and inline objects between them.
    for (index, old) in current.text.split('\n').enumerate() {
        let Some(new) = lines.next() else {
            edits.clear();
            break;
        };
        if old != new {
            let (start, end, insert) = diff(old, new);
            if (start == 0 && end == old.len()) || edits.len() == 128 {
                edits.clear();
                break;
            }
            let base = current.starts[index].0;
            edits.push((base + start, base + end, insert));
        }
    }
    if edits.len() < 2 || lines.next().is_some() {
        edits = vec![diff(&current.text, text)];
    }
    let positions = edits
        .into_iter()
        .map(|(start, end, replacement)| {
            if current.text[start..end].contains(OBJECT_MARKER)
                || replacement.contains(OBJECT_MARKER)
            {
                return Err("Text-only recovery cannot replay changes to inline objects".into());
            }
            let crossed = current
                .starts
                .partition_point(|(offset, _)| *offset <= start)
                ..current.starts.partition_point(|(offset, _)| *offset <= end);
            let start = current.position(&reader.session.doc, start)?;
            let end = current.position(&reader.session.doc, end)?;
            let parent = start.path.parent();
            if parent != end.path.parent()
                || current.starts[crossed]
                    .iter()
                    .any(|(_, pos)| pos.path.parent() != parent)
            {
                return Err("Text-only recovery cannot replay across table cells".into());
            }
            Ok((start, end, replacement))
        })
        .collect::<Result<Vec<_>, String>>()?;
    for (start, end, replacement) in positions.into_iter().rev() {
        reader.select_positions(start, end)?;
        reader.replace_text(None, replacement)?;
    }
    if BodyText::new(&reader.session.doc)?.text != text {
        return Err("Text recovery replay does not match checkpoint".into());
    }
    Ok(())
}

impl Reader {
    /// Recover an independent document from a legacy journal directory or published slot.
    pub fn recover_legacy(path: &Path, cancel: &AtomicBool) -> Result<Self, String> {
        // Recovery nests the DOCX importer and the engine's edit/layout path.
        // The Windows GPUI worker pool can use the 1 MiB default stack.
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("joyword-legacy-recovery".into())
                .stack_size(8 * 1024 * 1024)
                .spawn_scoped(scope, || Self::read_legacy_checkpoint(path, cancel))
                .map_err(|e| format!("Cannot start legacy recovery worker: {e}"))?
                .join()
                .map_err(|_| "Legacy recovery worker failed".to_owned())?
        })
    }

    fn read_legacy_checkpoint(path: &Path, cancel: &AtomicBool) -> Result<Self, String> {
        guarded(|| {
            let (checkpoint, degraded) = select(path, cancel)?;
            check_cancel(cancel)?;
            let rich = checkpoint.snapshot.is_some();
            let package = checkpoint.snapshot.unwrap_or(checkpoint.baseline);
            let mut reader = Self::from_bytes("Recovered.docx", &package, cancel)
                .map_err(|e| format!("Legacy recovery DOCX: {e}"))?;
            let body = BodyText::new(&reader.session.doc)?;
            if body.text == checkpoint.text
                || (rich && joyword_docx::legacy_inspection::matches_recovery_body(&package, &checkpoint.text))
            {
                reader.recovered_package = Some((reader.session.rev(), Arc::from(package)));
            } else if rich {
                return Err("Recovery snapshot body text mismatch".into());
            } else {
                // Re-exporting a text checkpoint needs to preserve the baseline
                // structure. Refuse known unsupported content instead of
                // silently dropping it during regeneration.
                let baseline = joyword_docx::legacy_inspection::project(&package, 0)?;
                if baseline.read_only || baseline.unsupported_body {
                    return Err("Text-only recovery baseline contains protected or unsupported content".into());
                }
                replay(&mut reader, body, &checkpoint.text)?;
            }
            check_cancel(cancel)?;
            reader.session.dirty = true;
            reader.recovery_notice = Some(if degraded {
                "A legacy recovery slot is damaged. An earlier valid checkpoint was opened. Save as a new DOCX."
            } else {
                "Legacy checkpoint opened. Save as a new DOCX to keep the recovered document."
            });
            Ok(reader)
        }).map_err(|e| format!("Legacy recovery: {e}"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn journal(baseline: &[u8], text: &str, snapshot: Option<&[u8]>) -> Vec<u8> {
        let mut record = if snapshot.is_some() {
            b"JWREC003".to_vec()
        } else {
            b"JWREC002".to_vec()
        };
        record.extend_from_slice(&1_u64.to_le_bytes());
        record.extend_from_slice(&[0; 9]);
        for bytes in [baseline, text.as_bytes()].into_iter().chain(snapshot) {
            record.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            record.extend_from_slice(bytes);
        }
        record.extend_from_slice(&crc32fast::hash(&record).to_le_bytes());
        record
    }

    fn assert_refused_without_changing_source(baseline: &[u8], text: &str, diagnostic: &str) {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.docx");
        let slot = dir.path().join("recovery-a.jwr");
        let lock = dir.path().join("recovery.lock");
        let record = journal(baseline, text, None);
        fs::write(&source, baseline).unwrap();
        fs::write(&slot, &record).unwrap();
        fs::write(&lock, b"existing lock").unwrap();
        let result = Reader::recover_legacy(&slot, &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(result.err().unwrap().contains(diagnostic));
        assert_eq!(fs::read(&source).unwrap(), baseline);
        assert_eq!(fs::read(&slot).unwrap(), record);
        assert_eq!(fs::read(&lock).unwrap(), b"existing lock");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[test]
    fn refuses_text_checkpoint_removing_or_inserting_an_inline_object() {
        let baseline = include_bytes!("../../../assets/samples/mixed-media.docx");
        let reader = Reader::from_bytes("fixture.docx", baseline, &AtomicBool::new(false)).unwrap();
        let current = BodyText::new(&reader.session.doc).unwrap();
        assert!(current.text.contains(OBJECT_MARKER));
        let removed = current.text.replacen(OBJECT_MARKER, "", 1);
        let inserted = format!("{}{OBJECT_MARKER}", current.text);
        for target in [removed, inserted] {
            assert_refused_without_changing_source(baseline, &target, "inline objects");
        }
    }

    #[test]
    fn refuses_text_checkpoint_joining_different_table_cells() {
        let baseline = include_bytes!("../../../assets/samples/mixed-media.docx");
        let reader = Reader::from_bytes("fixture.docx", baseline, &AtomicBool::new(false)).unwrap();
        let current = BodyText::new(&reader.session.doc).unwrap();
        let boundary = current
            .starts
            .windows(2)
            .find(|pair| {
                let left = pair[0].1.path.parent();
                let right = pair[1].1.path.parent();
                !left.is_empty() && !right.is_empty() && left != right
            })
            .unwrap()[1]
            .0;
        let mut target = current.text;
        assert_eq!(&target[boundary - 1..boundary], "\n");
        target.remove(boundary - 1);
        assert_refused_without_changing_source(baseline, &target, "across table cells");
    }

    #[test]
    fn rich_object_snapshot_is_preserved_exactly_and_mismatch_is_refused() {
        let baseline = include_bytes!("../../../assets/samples/mixed-media.docx");
        let cancel = AtomicBool::new(false);
        let reader = Reader::from_bytes("fixture.docx", baseline, &cancel).unwrap();
        let text = BodyText::new(&reader.session.doc).unwrap().text;
        assert!(text.contains(OBJECT_MARKER));
        for matched in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let slot = dir.path().join("recovery-a.jwr");
            let saved = dir.path().join("recovered.docx");
            let checkpoint = if matched {
                text.clone()
            } else {
                format!("{text}!")
            };
            let record = journal(baseline, &checkpoint, Some(baseline));
            fs::write(&slot, &record).unwrap();
            let recovered = Reader::recover_legacy(&slot, &cancel);
            if matched {
                recovered
                    .unwrap()
                    .save_job(saved.clone(), true)
                    .unwrap()
                    .run()
                    .unwrap();
                assert_eq!(fs::read(&saved).unwrap(), baseline);
            } else {
                assert!(recovered.is_err());
                assert!(
                    recovered
                        .err()
                        .unwrap()
                        .contains("snapshot body text mismatch")
                );
                assert!(!saved.exists());
            }
            assert_eq!(fs::read(&slot).unwrap(), record);
        }
    }

    #[test]
    fn replays_disjoint_changes_around_objects_and_table_cells() {
        let cancel = AtomicBool::new(false);
        let baseline = include_bytes!("../../../assets/samples/mixed-media.docx");
        let reader = Reader::from_bytes("fixture.docx", baseline, &cancel).unwrap();
        let current = BodyText::new(&reader.session.doc).unwrap();
        assert!(current.text.contains(OBJECT_MARKER));
        let target = current.text.replacen("阅读验证", "恢复😀验证", 1).replacen(
            "R2 阅读器",
            "R8 恢复📝阅读器",
            1,
        );
        let media = reader.session.doc.media.clone();
        let dir = tempfile::tempdir().unwrap();
        let slot = dir.path().join("recovery-a.jwr");
        let record = journal(baseline, &target, None);
        fs::write(&slot, &record).unwrap();
        let reader = Reader::recover_legacy(&slot, &cancel).unwrap();
        assert_eq!(BodyText::new(&reader.session.doc).unwrap().text, target);
        assert_eq!(reader.session.doc.media, media);
        let saved = dir.path().join("recovered.docx");
        reader.save_job(saved.clone(), true).unwrap().run().unwrap();
        let roundtrip = joyword_docx::read(&fs::read(saved).unwrap()).unwrap();
        assert_eq!(BodyText::new(&roundtrip).unwrap().text, target);
        assert_eq!(roundtrip.media, media);
        assert_eq!(fs::read(slot).unwrap(), record);
    }

    #[test]
    fn regular_recovery_open_keeps_missing_and_handle_read_contract() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("recovery-a.jwr");
        assert!(open_regular(&path)?.is_none());
        fs::write(&path, b"original")?;
        let mut file = open_regular(&path)?.ok_or_else(|| invalid("file missing"))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"original");
        assert!(open_regular(dir.path()).is_err());
        assert_eq!(fs::read(&path)?, b"original");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn checked_recovery_open_rejects_replacement_after_metadata() -> io::Result<()> {
        use rustix::fs::{CWD, Mode, mkfifoat};
        use std::os::unix::fs::{FileTypeExt, symlink};
        for mode in 0..3 {
            let dir = tempfile::tempdir()?;
            let path = dir.path().join("recovery-a.jwr");
            let original = dir.path().join("original.jwr");
            let target = dir.path().join("replacement.jwr");
            fs::write(&path, b"original")?;
            fs::write(&target, b"replaced")?;
            let before = fs::symlink_metadata(&path)?;
            fs::rename(&path, &original)?;
            match mode {
                0 => symlink(&target, &path)?,
                1 => mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR)?,
                _ => fs::copy(&target, &path).map(|_| ())?,
            }
            // The old File::open path follows the symlink or reads the new inode.
            // Do not run the old FIFO open: it blocks without a writer.
            if mode != 1 {
                assert_eq!(fs::read(&path)?, b"replaced");
            }
            let error = open_checked_regular(&path, &before)
                .err()
                .ok_or_else(|| invalid("replacement accepted"))?;
            assert!(!matches!(
                error.kind(),
                io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof
            ));
            assert_eq!(fs::read(&original)?, b"original");
            assert_eq!(fs::read(&target)?, b"replaced");
            if mode == 1 {
                assert!(fs::symlink_metadata(&path)?.file_type().is_fifo());
            }
            assert_eq!(fs::read_dir(dir.path())?.count(), 3);
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn nonregular_slot_must_not_resurrect_an_older_generation() -> io::Result<()> {
        use rustix::fs::{CWD, Mode, mkfifoat};
        use std::os::unix::fs::symlink;
        let baseline = include_bytes!("../../../assets/samples/mixed-media.docx");
        for mode in 0..3 {
            let dir = tempfile::tempdir()?;
            let old = journal(baseline, "older text", None);
            fs::write(dir.path().join(SLOTS[0]), &old)?;
            let suspect = dir.path().join(SLOTS[1]);
            match mode {
                0 => fs::create_dir(&suspect)?,
                1 => mkfifoat(CWD, &suspect, Mode::RUSR | Mode::WUSR)?,
                _ => symlink(dir.path().join(SLOTS[0]), &suspect)?,
            }
            let result = select_locked(dir.path(), &AtomicBool::new(false));
            assert!(
                result.is_err(),
                "must not fall back through nonregular slot {mode}"
            );
            assert_eq!(fs::read(dir.path().join(SLOTS[0]))?, old);
            assert_eq!(fs::read_dir(dir.path())?.count(), 2);
        }
        Ok(())
    }

    #[test]
    fn checksum_damage_still_allows_reported_older_generation_fallback() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let baseline = include_bytes!("../../../assets/samples/mixed-media.docx");
        let old = journal(baseline, "older text", None);
        fs::write(dir.path().join(SLOTS[0]), &old)?;
        let mut corrupt = journal(baseline, "newer text", None);
        if let Some(byte) = corrupt.last_mut() {
            *byte ^= 1;
        }
        fs::write(dir.path().join(SLOTS[1]), &corrupt)?;
        let (checkpoint, damaged) =
            select_locked(dir.path(), &AtomicBool::new(false)).map_err(io::Error::other)?;
        assert_eq!(checkpoint.text, "older text");
        assert!(damaged);
        assert_eq!(fs::read(dir.path().join(SLOTS[1]))?, corrupt);
        Ok(())
    }
}
