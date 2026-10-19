//! Bounded source reads with a private, unforgeable source stamp (W07).
//!
//! `open_source` reads a document file under the caller's OPC budget and
//! captures, in the same window, both the exact bytes and the file-system
//! identity of the object that produced them. Identity observations before
//! and after the read must agree, or no baseline is issued at all. The stamp
//! has private fields and no public constructor: it can only come from a
//! completed verified read.
//!
//! The stamp compares exact bytes, not digests, so same-size/same-mtime
//! content changes are always detected on a later check. The baseline read
//! itself also re-reads the bytes: on Linux 5.4, mtime and ctime advance in
//! jiffies, so a same-length rewrite between stat and open can leave device,
//! inode, size and timestamps unchanged. What the stamp does not do is freeze
//! the file: the world can change right after `open_source` returns, which is
//! why `check_source` re-verifies and `save_original` verifies again on its
//! own instead of trusting earlier calls.

use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use joyoffice_opc::Budget;

use super::error::{ConflictDetail, SaveOriginalError, SourceStage};
use super::locks;

/// File-system facts of the object a stamp is bound to. Unix records the
/// raw stat identity. Windows additionally records the stable volume/file ID
/// through Win32 so an atomic replacement can prove which object was displaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SourceIdentity {
    len: u64,
    #[cfg(unix)]
    unix: UnixIdentity,
    #[cfg(not(unix))]
    modified: std::time::SystemTime,
    #[cfg(windows)]
    windows: WindowsIdentity,
}

#[cfg(unix)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnixIdentity {
    dev: u64,
    ino: u64,
    mode: u32,
    nlink: u64,
    uid: u32,
    gid: u32,
    mtime_secs: i64,
    mtime_nanos: i64,
    ctime_secs: i64,
    ctime_nanos: i64,
    /// Birth time when the platform reports one. Absent when the filesystem
    /// has no birth timestamp. Compared only as an extra identity signal.
    birth: Option<std::time::SystemTime>,
}

#[cfg(unix)]
impl UnixIdentity {
    pub(crate) fn nlink(&self) -> u64 {
        self.nlink
    }
    pub(crate) fn mode(&self) -> u32 {
        self.mode
    }
    pub(crate) fn uid(&self) -> u32 {
        self.uid
    }
    pub(crate) fn gid(&self) -> u32 {
        self.gid
    }
}

#[cfg(windows)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WindowsIdentity {
    volume_serial_number: u32,
    file_index: u64,
    nlink: u32,
    attributes: u32,
    creation_time: u64,
    last_write_time: u64,
}

#[cfg(windows)]
fn windows_error(error: winsafe::co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}

#[cfg(windows)]
fn file_time(value: winsafe::FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

#[cfg(all(test, unix))]
thread_local! {
    static COARSE_IDENTITY_TIMES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Collapse mtime, ctime and birth to one value so a test can reproduce a
/// jiffy clock. Linux 5.4 stores those times at tick granularity; a fast
/// same-length rewrite then leaves the metadata identity unchanged.
#[cfg(all(test, unix))]
fn coarse_identity_times() -> bool {
    COARSE_IDENTITY_TIMES.with(std::cell::Cell::get)
}

#[cfg(all(test, unix))]
struct CoarseIdentityGuard;

#[cfg(all(test, unix))]
impl CoarseIdentityGuard {
    fn enter() -> Self {
        COARSE_IDENTITY_TIMES.with(|cell| cell.set(true));
        Self
    }
}

#[cfg(all(test, unix))]
impl Drop for CoarseIdentityGuard {
    fn drop(&mut self) {
        COARSE_IDENTITY_TIMES.with(|cell| cell.set(false));
    }
}

#[cfg(unix)]
fn unix_times(metadata: &fs::Metadata) -> (i64, i64, i64, i64, Option<std::time::SystemTime>) {
    use std::os::unix::fs::MetadataExt;
    // Tests opt in to a jiffy clock. Production keeps the kernel's timestamps.
    #[cfg(test)]
    if coarse_identity_times() {
        return (0, 0, 0, 0, None);
    }
    (
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        metadata.created().ok(),
    )
}

impl SourceIdentity {
    #[cfg(not(windows))]
    pub(crate) fn capture(metadata: &fs::Metadata) -> Self {
        Self {
            len: metadata.len(),
            #[cfg(unix)]
            unix: {
                use std::os::unix::fs::MetadataExt;
                let (mtime_secs, mtime_nanos, ctime_secs, ctime_nanos, birth) =
                    unix_times(metadata);
                UnixIdentity {
                    dev: metadata.dev(),
                    ino: metadata.ino(),
                    mode: metadata.mode(),
                    nlink: metadata.nlink(),
                    uid: metadata.uid(),
                    gid: metadata.gid(),
                    mtime_secs,
                    mtime_nanos,
                    ctime_secs,
                    ctime_nanos,
                    birth,
                }
            },
            #[cfg(not(unix))]
            modified: metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn capture_path(path: &Path) -> io::Result<Self> {
        fs::metadata(path).map(|metadata| Self::capture(&metadata))
    }

    #[cfg(windows)]
    pub(crate) fn capture_path(path: &Path) -> io::Result<Self> {
        use winsafe::prelude::*;

        let metadata = fs::metadata(path)?;
        let name = path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Win32 path is not valid Unicode",
            )
        })?;
        let (file, _) = winsafe::HFILE::CreateFile(
            name,
            winsafe::co::GENERIC::READ,
            Some(
                winsafe::co::FILE_SHARE::READ
                    | winsafe::co::FILE_SHARE::WRITE
                    | winsafe::co::FILE_SHARE::DELETE,
            ),
            None,
            winsafe::co::DISPOSITION::OPEN_EXISTING,
            winsafe::co::FILE_ATTRIBUTE::NORMAL,
            None,
            None,
            None,
        )
        .map_err(windows_error)?;
        let mut info = winsafe::BY_HANDLE_FILE_INFORMATION::default();
        file.GetFileInformationByHandle(&mut info)
            .map_err(windows_error)?;
        Ok(Self {
            len: metadata.len(),
            modified: metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            windows: WindowsIdentity {
                volume_serial_number: info.dwVolumeSerialNumber,
                file_index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
                nlink: info.nNumberOfLinks,
                attributes: info.dwFileAttributes.raw(),
                creation_time: file_time(info.ftCreationTime),
                last_write_time: file_time(info.ftLastWriteTime),
            },
        })
    }

    pub(crate) fn capture_handle(path: &Path, file: &File) -> io::Result<Self> {
        #[cfg(not(windows))]
        {
            let _ = path;
            file.metadata().map(|metadata| Self::capture(&metadata))
        }
        #[cfg(windows)]
        {
            let _ = file;
            Self::capture_path(path)
        }
    }

    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    #[cfg(unix)]
    pub(crate) fn unix(&self) -> &UnixIdentity {
        &self.unix
    }

    pub(crate) fn nlink(&self) -> u64 {
        #[cfg(unix)]
        {
            self.unix.nlink()
        }
        #[cfg(windows)]
        {
            u64::from(self.windows.nlink)
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            1
        }
    }

    #[cfg(windows)]
    pub(crate) fn creation_time(&self) -> u64 {
        self.windows.creation_time
    }

    /// Same volume and file ID. Link count and timestamps are ignored because
    /// creating a backup can change them without changing the object.
    pub(crate) fn same_object(&self, other: &Self) -> bool {
        #[cfg(unix)]
        {
            self.unix.dev == other.unix.dev && self.unix.ino == other.unix.ino
        }
        #[cfg(windows)]
        {
            self.windows.volume_serial_number == other.windows.volume_serial_number
                && self.windows.file_index == other.windows.file_index
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            self.len == other.len && self.modified == other.modified
        }
    }

    /// First observable difference against another identity, as a conflict.
    pub(crate) fn difference(&self, other: &Self) -> Option<ConflictDetail> {
        #[cfg(unix)]
        {
            let (a, b) = (&self.unix, &other.unix);
            if a.ino != b.ino || a.dev != b.dev {
                return Some(ConflictDetail::InodeChanged);
            }
            if a.mode != b.mode {
                return Some(ConflictDetail::ModeChanged);
            }
            if a.nlink != b.nlink {
                return Some(ConflictDetail::LinkCountChanged);
            }
            if a.mtime_secs != b.mtime_secs
                || a.mtime_nanos != b.mtime_nanos
                || a.ctime_secs != b.ctime_secs
                || a.ctime_nanos != b.ctime_nanos
                || a.birth != b.birth
            {
                return Some(ConflictDetail::TimesChanged);
            }
        }
        #[cfg(windows)]
        {
            if self.windows.volume_serial_number != other.windows.volume_serial_number
                || self.windows.file_index != other.windows.file_index
            {
                return Some(ConflictDetail::InodeChanged);
            }
            if self.windows.attributes != other.windows.attributes {
                return Some(ConflictDetail::ModeChanged);
            }
            if self.windows.nlink != other.windows.nlink {
                return Some(ConflictDetail::LinkCountChanged);
            }
            if self.windows.creation_time != other.windows.creation_time
                || self.windows.last_write_time != other.windows.last_write_time
                || self.modified != other.modified
            {
                return Some(ConflictDetail::TimesChanged);
            }
        }
        #[cfg(all(not(unix), not(windows)))]
        if self.modified != other.modified {
            return Some(ConflictDetail::TimesChanged);
        }
        if self.len != other.len {
            return Some(ConflictDetail::SizeChanged);
        }
        None
    }
}

/// Private, unforgeable identity of one verified source read. Only
/// `open_source` and the read-back of a confirmed original replacement
/// construct stamps; callers can read the bound path and length but never
/// synthesize a stamp from raw values.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceStamp {
    /// The path exactly as requested, for re-resolution on later checks.
    given_path: PathBuf,
    /// Fully resolved path (no symlinks, absolute) the bytes came from.
    canonical_path: PathBuf,
    /// Directories scanned for owner/lock files: the canonical file's
    /// directory plus the request path's directory when different.
    lock_directories: Vec<PathBuf>,
    identity: SourceIdentity,
    /// Byte cap of the verified read; later re-reads reuse the same cap.
    budget_limit: u64,
    /// The exact bytes of the verified read, shared with `OpenedSource`.
    bytes: Arc<[u8]>,
}

impl fmt::Debug for SourceStamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceStamp")
            .field("given_path", &self.given_path)
            .field("canonical_path", &self.canonical_path)
            .field("identity", &self.identity)
            .field("budget_limit", &self.budget_limit)
            .field("bytes", &format!("<{} bytes>", self.bytes.len()))
            .finish()
    }
}

impl SourceStamp {
    /// Resolved path of the file this stamp is bound to.
    pub fn path(&self) -> &Path {
        &self.canonical_path
    }
    /// Length of the verified baseline in bytes. A real DOCX source is
    /// never empty, but an empty file stamps as empty rather than failing
    /// here; import refuses it later.
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }
    /// Whether the captured baseline is zero bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub(crate) fn given_path(&self) -> &Path {
        &self.given_path
    }
    pub(crate) fn identity(&self) -> &SourceIdentity {
        &self.identity
    }
    pub(crate) fn budget_limit(&self) -> u64 {
        self.budget_limit
    }
    /// Exact bytes captured by the verified source read or published read-back.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn lock_directories(&self) -> &[PathBuf] {
        &self.lock_directories
    }
}

/// Result of one verified bounded read: the bytes and the stamp captured in
/// the same window. The bytes are what `DocxSession::open` should receive;
/// the stamp is what later conflict checks and (once available) original
/// replacement are bound to.
pub struct OpenedSource {
    bytes: Arc<[u8]>,
    stamp: SourceStamp,
}

impl OpenedSource {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn stamp(&self) -> &SourceStamp {
        &self.stamp
    }
    /// Discard the bytes and keep the stamp for later conflict checks.
    pub fn into_stamp(self) -> SourceStamp {
        self.stamp
    }
}

impl fmt::Debug for OpenedSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenedSource")
            .field("bytes", &format!("<{} bytes>", self.bytes.len()))
            .field("stamp", &self.stamp)
            .finish()
    }
}

/// The whole-file read cap derived from the import budget. A package larger
/// than the total uncompressed budget could never import under that budget,
/// so refusing it here is consistent, and the read is never unbounded.
fn read_cap(budget: &Budget) -> u64 {
    budget.max_total_bytes
}

/// Read `path` while `expected` still describes it. The file is closed before
/// return so a later test hook can replace the bytes.
fn sample_bytes(
    path: &Path,
    expected: &SourceIdentity,
    limit: u64,
) -> Result<Vec<u8>, SaveOriginalError> {
    let fail = |error| SaveOriginalError::io(SourceStage::Stat, path, error);
    let file = File::open(path).map_err(fail)?;
    let opened = SourceIdentity::capture_handle(path, &file).map_err(fail)?;
    if opened != *expected {
        return Err(SaveOriginalError::UnstableRead {
            path: path.to_path_buf(),
        });
    }
    let mut bytes = Vec::new();
    (&file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(fail)?;
    if bytes.len() as u64 > limit {
        return Err(SaveOriginalError::SourceTooLarge {
            path: path.to_path_buf(),
            limit,
        });
    }
    let after_handle = SourceIdentity::capture_handle(path, &file).map_err(fail)?;
    let after_path = SourceIdentity::capture_path(path).map_err(fail)?;
    if after_handle != *expected || after_path != *expected || bytes.len() as u64 != expected.len()
    {
        return Err(SaveOriginalError::UnstableRead {
            path: path.to_path_buf(),
        });
    }
    Ok(bytes)
}

/// Byte-for-byte compare of an open file with `expected`, without keeping a
/// second copy. A longer file, including one that crosses `limit`, does not
/// match.
fn contents_match(file: &File, expected: &[u8], limit: u64) -> io::Result<bool> {
    let mut reader = file.take(limit.saturating_add(1));
    let mut buf = [0u8; 8192];
    let mut offset = 0usize;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            return Ok(offset == expected.len());
        }
        let Some(end) = offset.checked_add(n) else {
            return Ok(false);
        };
        if end > expected.len() || buf[..n] != expected[offset..end] {
            return Ok(false);
        }
        offset = end;
    }
}

/// Read a source document under `budget`, capturing its bytes and identity.
///
/// Refuses non-regular files, oversized files, existing or undecidable
/// Office/JoyWord locks, and reads whose identity observations disagree.
/// Symlinked request paths are followed; the stamp binds the resolved
/// object, and later checks re-resolve the request path to detect
/// retargeting.
pub fn open_source(path: &Path, budget: &Budget) -> Result<OpenedSource, SaveOriginalError> {
    open_source_inner(path, budget, |_, _| Ok(()))
}

// Same private boundary hook pattern as the C03 copy path: deterministic
// failures around real IO for tests; production always executes real steps.
fn open_source_inner(
    path: &Path,
    budget: &Budget,
    mut before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
) -> Result<OpenedSource, SaveOriginalError> {
    before(SourceStage::Resolve, path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Resolve, path, e))?;
    let canonical_path =
        fs::canonicalize(path).map_err(|e| SaveOriginalError::io(SourceStage::Resolve, path, e))?;
    let file_name = canonical_path
        .file_name()
        .map(OsStr::to_owned)
        .unwrap_or_default();
    let mut lock_directories = vec![
        canonical_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/")),
    ];
    // A lock may also sit next to the alias the requester used (Word locks
    // beside the path it opened). Record that directory when it resolves.
    if let Some(parent) = path.parent()
        && let Ok(resolved) = fs::canonicalize(parent)
        && !lock_directories.contains(&resolved)
    {
        lock_directories.push(resolved);
    }
    before(SourceStage::LockScan, &canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::LockScan, &canonical_path, e))?;
    // Opening never exempts a lock, including one this process created.
    locks::check_directories(&lock_directories, &file_name, None)?;
    before(SourceStage::Stat, &canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Stat, &canonical_path, e))?;
    let before_metadata = fs::metadata(&canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Stat, &canonical_path, e))?;
    if !before_metadata.is_file() {
        return Err(SaveOriginalError::NotARegularFile {
            path: canonical_path,
        });
    }
    let before_identity = SourceIdentity::capture_path(&canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Stat, &canonical_path, e))?;
    let limit = read_cap(budget);
    if before_identity.len() > limit {
        return Err(SaveOriginalError::SourceTooLarge {
            path: canonical_path,
            limit,
        });
    }
    // First content sample, still inside the stat window and before the
    // open hook. A same-length rewrite that does not advance coarse
    // timestamps is visible here as a byte mismatch on the next read.
    let sampled = sample_bytes(&canonical_path, &before_identity, limit)?;
    before(SourceStage::Open, &canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Open, &canonical_path, e))?;
    let mut file = File::open(&canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Open, &canonical_path, e))?;
    let opened_identity = SourceIdentity::capture_handle(&canonical_path, &file)
        .map_err(|e| SaveOriginalError::io(SourceStage::Stat, &canonical_path, e))?;
    if opened_identity != before_identity {
        return Err(SaveOriginalError::UnstableRead {
            path: canonical_path,
        });
    }
    before(SourceStage::Read, &canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Read, &canonical_path, e))?;
    let matches_sample = contents_match(&file, &sampled, limit)
        .map_err(|e| SaveOriginalError::io(SourceStage::Read, &canonical_path, e))?;
    if !matches_sample {
        return Err(SaveOriginalError::UnstableRead {
            path: canonical_path,
        });
    }
    before(SourceStage::Restat, &canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Restat, &canonical_path, e))?;
    let after_handle = SourceIdentity::capture_handle(&canonical_path, &file)
        .map_err(|e| SaveOriginalError::io(SourceStage::Restat, &canonical_path, e))?;
    let after_path = SourceIdentity::capture_path(&canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Restat, &canonical_path, e))?;
    if after_handle != before_identity
        || after_path != before_identity
        || after_handle.len() != sampled.len() as u64
    {
        return Err(SaveOriginalError::UnstableRead {
            path: canonical_path,
        });
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|e| SaveOriginalError::io(SourceStage::Restat, &canonical_path, e))?;
    let matches_again = contents_match(&file, &sampled, limit)
        .map_err(|e| SaveOriginalError::io(SourceStage::Restat, &canonical_path, e))?;
    if !matches_again {
        return Err(SaveOriginalError::UnstableRead {
            path: canonical_path,
        });
    }
    let bytes: Arc<[u8]> = sampled.into();
    Ok(OpenedSource {
        stamp: SourceStamp {
            given_path: path.to_path_buf(),
            canonical_path,
            lock_directories,
            identity: before_identity,
            budget_limit: limit,
            bytes: bytes.clone(),
        },
        bytes,
    })
}

/// Conflict precheck for a previously captured stamp.
///
/// Re-runs the lock scan, re-resolves the request path, compares the current
/// file-system identity and re-reads the bytes for exact comparison. A pass
/// is only a precheck: the world can change again immediately, so callers
/// must not treat this plus a later write as an atomic compare-and-swap.
pub fn check_source(source: &SourceStamp) -> Result<(), SaveOriginalError> {
    check_source_with_lock_token(source, None)
}

/// [`check_source`] that skips one JoyWord lock.
///
/// `owner_token` matches only when the lock file is a regular file and its
/// entire contents are that token. An empty token, a different token, a
/// directory, and an Office `~$` file stay conflicts. The lock file is not
/// deleted or rewritten. [`open_source`] does not call this.
pub fn check_source_with_lock_token(
    source: &SourceStamp,
    owner_token: Option<&str>,
) -> Result<(), SaveOriginalError> {
    let canonical_path = source.path();
    let file_name = canonical_path
        .file_name()
        .map(OsStr::to_owned)
        .unwrap_or_default();
    locks::check_directories(&source.lock_directories, &file_name, owner_token)?;
    let resolved = match fs::canonicalize(source.given_path()) {
        Ok(resolved) => resolved,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(SaveOriginalError::conflict_error(
                canonical_path,
                ConflictDetail::Missing,
            ));
        }
        Err(e) => {
            return Err(SaveOriginalError::io(
                SourceStage::Resolve,
                source.given_path(),
                e,
            ));
        }
    };
    if resolved != canonical_path {
        return Err(SaveOriginalError::conflict_error(
            canonical_path,
            ConflictDetail::PathResolutionChanged,
        ));
    }
    verify_current(source)
}

/// Bounded, stable read-back of a just-renamed replacement at the stamp's
/// resolved path, producing the refreshed stamp for it.
///
/// The path must name `object` (the renamed temporary file, by device and
/// inode), observations before, during and after the read must agree, and
/// the bytes must equal `expected`. The new stamp keeps the request path,
/// lock directories and read cap of `source`; its identity and bytes come
/// only from this read of the file on disk, never from the plan. The
/// returned metadata is the handle's post-read observation.
pub(crate) fn confirm_published(
    source: &SourceStamp,
    object: &SourceIdentity,
    expected: &[u8],
) -> io::Result<(SourceStamp, fs::Metadata)> {
    let mismatch = |what| Err(io::Error::new(io::ErrorKind::InvalidData, what));
    let path = source.path();
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() {
        return mismatch("published path is not a regular file");
    }
    let identity = SourceIdentity::capture_path(path)?;
    if !identity.same_object(object) {
        return mismatch("published path names another object");
    }
    // Another writer can grow this same object before confirmation. Apply
    // the read cap before trusting its length as an allocation request.
    if identity.len() > source.budget_limit() || expected.len() as u64 > source.budget_limit() {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "published file exceeds the source read budget",
        ));
    }
    let file = File::open(path)?;
    if SourceIdentity::capture_handle(path, &file)? != identity {
        return mismatch("published object changed while it was opened");
    }
    let mut bytes = Vec::with_capacity(identity.len() as usize);
    (&file)
        .take(source.budget_limit().saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > source.budget_limit() {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "published file grew beyond the source read budget",
        ));
    }
    let after = file.metadata()?;
    if SourceIdentity::capture_handle(path, &file)? != identity
        || SourceIdentity::capture_path(path)? != identity
        || bytes.len() as u64 != identity.len()
    {
        return mismatch("published object changed during read-back");
    }
    if bytes != expected {
        return mismatch("published bytes differ from the plan");
    }
    let stamp = SourceStamp {
        given_path: source.given_path.clone(),
        canonical_path: path.to_path_buf(),
        lock_directories: source.lock_directories.clone(),
        identity,
        budget_limit: source.budget_limit,
        bytes: bytes.into(),
    };
    Ok((stamp, after))
}

/// Full state verification against the current file system. Used by
/// `check_source` and internally re-run by `save_original`.
pub(crate) fn verify_current(source: &SourceStamp) -> Result<(), SaveOriginalError> {
    let canonical_path = source.path();
    let metadata = fs::metadata(canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Stat, canonical_path, e))?;
    if !metadata.is_file() {
        return Err(SaveOriginalError::conflict_error(
            canonical_path,
            ConflictDetail::NotARegularFileAnymore,
        ));
    }
    let current = SourceIdentity::capture_path(canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Stat, canonical_path, e))?;
    if let Some(detail) = source.identity().difference(&current) {
        return Err(SaveOriginalError::conflict_error(canonical_path, detail));
    }
    let file = File::open(canonical_path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Open, canonical_path, e))?;
    let mut bytes = Vec::with_capacity(current.len() as usize);
    (&file)
        .take(source.budget_limit().saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| SaveOriginalError::io(SourceStage::Read, canonical_path, e))?;
    if bytes.len() as u64 > source.budget_limit() || bytes.as_slice() != source.bytes() {
        return Err(SaveOriginalError::conflict_error(
            canonical_path,
            ConflictDetail::ContentChanged,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_source(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    // Outside Unix the identity is length and modification time, so a
    // same-length rewrite within one clock tick is invisible to metadata;
    // the byte re-read above still refuses it. Unix metadata includes the
    // inode, but a jiffy clock can hide a same-length rewrite that keeps
    // the inode.
    #[cfg(unix)]
    #[test]
    fn identity_change_between_stat_and_open_is_not_a_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let error = open_source_inner(&path, &Budget::default(), |stage, _| {
            if stage == SourceStage::Open {
                fs::write(&path, b"9876543210")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, SaveOriginalError::UnstableRead { .. }));
    }

    /// Metadata timestamps are forced equal, which is what kernel 5.4 does
    /// for two writes inside one jiffy. The content re-read must still refuse.
    #[cfg(unix)]
    #[test]
    fn same_length_rewrite_with_coarse_timestamps_is_not_a_baseline() {
        let _coarse = CoarseIdentityGuard::enter();
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let error = open_source_inner(&path, &Budget::default(), |stage, _| {
            if stage == SourceStage::Open {
                fs::write(&path, b"9876543210")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, SaveOriginalError::UnstableRead { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn coarse_timestamps_still_accept_an_unchanged_file() {
        let _coarse = CoarseIdentityGuard::enter();
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let opened = open_source(&path, &Budget::default()).unwrap();
        assert_eq!(opened.bytes(), b"0123456789");
    }

    #[cfg(unix)]
    #[test]
    fn same_length_rewrite_before_restat_with_coarse_timestamps_is_not_a_baseline() {
        let _coarse = CoarseIdentityGuard::enter();
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let error = open_source_inner(&path, &Budget::default(), |stage, p| {
            if stage == SourceStage::Restat {
                fs::write(p, b"9876543210")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, SaveOriginalError::UnstableRead { .. }));
    }

    #[test]
    fn truncation_during_read_is_not_a_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let error = open_source_inner(&path, &Budget::default(), |stage, p| {
            if stage == SourceStage::Read {
                let file = File::options().write(true).open(p)?;
                file.set_len(4)?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, SaveOriginalError::UnstableRead { .. }));
    }

    #[cfg(not(unix))]
    #[test]
    fn length_change_between_stat_and_open_is_not_a_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let error = open_source_inner(&path, &Budget::default(), |stage, _| {
            if stage == SourceStage::Open {
                fs::write(&path, b"98765432100")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, SaveOriginalError::UnstableRead { .. }));
    }

    // Identical bytes under a new object: Unix sees the inode or ctime.
    #[cfg(unix)]
    #[test]
    fn replacement_after_read_before_restat_is_not_a_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let error = open_source_inner(&path, &Budget::default(), |stage, _| {
            if stage == SourceStage::Restat {
                fs::remove_file(&path)?;
                fs::write(&path, b"0123456789")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, SaveOriginalError::UnstableRead { .. }));
    }

    #[cfg(not(unix))]
    #[test]
    fn longer_replacement_after_read_before_restat_is_not_a_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let error = open_source_inner(&path, &Budget::default(), |stage, _| {
            if stage == SourceStage::Restat {
                fs::remove_file(&path)?;
                fs::write(&path, b"01234567890")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, SaveOriginalError::UnstableRead { .. }));
    }

    #[test]
    fn oversized_source_is_refused_before_reading() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        let budget = Budget {
            max_total_bytes: 8,
            ..Budget::default()
        };
        let error = open_source(&path, &budget).unwrap_err();
        match error {
            SaveOriginalError::SourceTooLarge { limit, .. } => assert_eq!(limit, 8),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn published_read_back_checks_the_budget_before_allocating_for_a_grown_inode() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"1234");
        let budget = Budget {
            max_total_bytes: 8,
            ..Budget::default()
        };
        let stamp = open_source(&path, &budget).unwrap().into_stamp();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(9)
            .unwrap();
        let error = confirm_published(&stamp, stamp.identity(), b"1234").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
        assert_eq!(fs::metadata(path).unwrap().len(), 9);
    }

    #[test]
    fn every_stage_can_fail_with_real_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), "a.docx", b"0123456789");
        for stage in [
            SourceStage::Resolve,
            SourceStage::LockScan,
            SourceStage::Stat,
            SourceStage::Open,
            SourceStage::Read,
            SourceStage::Restat,
        ] {
            let error = open_source_inner(&path, &Budget::default(), |at, _| {
                if at == stage {
                    Err(io::Error::new(io::ErrorKind::StorageFull, "injected"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.stage(), Some(stage));
            assert_eq!(
                error.io_error().map(io::Error::kind),
                Some(io::ErrorKind::StorageFull)
            );
        }
    }
}
