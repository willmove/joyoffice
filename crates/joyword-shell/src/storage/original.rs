//! Original-file replacement (W07 gate, W11 publish strategy).
//!
//! `save_original` re-runs conflict verification, then requires
//! [`ExportPlan::origin_bytes`](joyword_docx::ExportPlan::origin_bytes) to be
//! the stamped package bytes. A missing origin or a different package refuses
//! before any write. Unobservable or unpreservable metadata is listed before
//! the caller chooses a copy or explicitly accepts replacement. Every publish
//! path ends by reading the replaced file back; only that read produces the
//! receipt and its refreshed stamp. `save_original_with_lock_token`
//! can skip the caller's own JoyWord lock and then hits the same checks.
//! `save_export_copy` stays the verified fallback.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(any(unix, windows))]
use std::fs::{self, File};
#[cfg(any(unix, windows))]
use std::io::Write;

use super::DocumentVersion;
use super::{ExportPlan, ExportTarget};

#[cfg(unix)]
use super::error::MetadataItem;
use super::error::{ReplacementBlocker, SaveOriginalError, SourceStage};
#[cfg(any(unix, windows))]
use super::source::{SourceIdentity, confirm_published};
use super::source::{SourceStamp, check_source_with_lock_token};
#[cfg(any(unix, windows))]
use super::{locks, sync_directory, verify_bytes};

/// Persistence state of a completed original replacement. Reused from the
/// C03 copy path: `FileSynced` means the published file accepted a sync;
/// `FileAndDirectorySynced` adds the containing directory. Neither claims
/// power-loss testing.
pub type OriginalDurability = super::CopyDurability;

/// Receipt for a completed original replacement. It can only be produced by
/// the real replacement write path after reading the exchanged file back: its
/// stamp holds the identity and bytes of that read, never values copied from
/// the plan. An error after publication must carry the receipt instead of
/// pretending nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveOriginalReceipt {
    path: PathBuf,
    version: DocumentVersion,
    durability: OriginalDurability,
    // Boxed so errors that carry a receipt stay small.
    stamp: Box<SourceStamp>,
}

impl SaveOriginalReceipt {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn version(&self) -> DocumentVersion {
        self.version
    }
    pub fn durability(&self) -> OriginalDurability {
        self.durability
    }
    /// Length of the published file as read back.
    pub fn bytes(&self) -> u64 {
        self.stamp.len()
    }
    /// Stamp of the published file, from its read-back. Later checks and
    /// saves of this file must use it: the stamp passed to `save_original`
    /// now describes the replaced object.
    pub fn stamp(&self) -> &SourceStamp {
        &self.stamp
    }
    pub fn into_stamp(self) -> SourceStamp {
        *self.stamp
    }
}

/// What this build can do with one metadata aspect of a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataCapability {
    /// Observable, and an approved `std` write path restores it on the new
    /// inode, where the replace steps verify it before and after the rename.
    /// A value the process may not apply (an owner it cannot give a file to)
    /// is refused before publication. This is a statement about capability,
    /// not an executed replacement.
    Preservable,
    /// Observable, but no approved dependency can restore it; replacement is
    /// refused.
    ObservableNotPreservable,
    /// Not even observable with approved dependencies (ACLs, extended
    /// attributes, provenance marks), so silent loss could not be detected;
    /// replacement is refused.
    Unobservable,
}

/// Per-item capability matrix for original replacement on the current
/// platform build. It grounds the conservative refusal in facts a test can
/// assert; it never authorizes a write by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriginalReplacementCapabilities {
    pub permissions: MetadataCapability,
    pub ownership: MetadataCapability,
    pub acl: MetadataCapability,
    pub extended_attributes: MetadataCapability,
    pub creation_time: MetadataCapability,
    pub links: MetadataCapability,
    pub provenance_marks: MetadataCapability,
}

/// Permission bits and, on Unix, owner and group (`fchown`) are preservable,
/// as is the creation time on macOS (`FileTimesExt::set_created`). User
/// extended attributes and provenance marks are copied with the pinned
/// `xattr` crate when they fit the size cap. Linux `security.*` names are
/// kernel-managed: they are observed and omitted from the restore set, and
/// they are not a replacement blocker. Windows `ReplaceFileW` preserves
/// creation time, DACLs and named streams without asking us to ignore merge
/// failures. Only the creation time, link count and provenance stream that
/// this build actually observes are marked accordingly; unobserved metadata
/// remains in the prompt.
pub fn original_replacement_capabilities() -> OriginalReplacementCapabilities {
    OriginalReplacementCapabilities {
        permissions: MetadataCapability::Preservable,
        #[cfg(unix)]
        ownership: MetadataCapability::Preservable,
        #[cfg(not(unix))]
        ownership: MetadataCapability::Unobservable,
        #[cfg(unix)]
        acl: MetadataCapability::ObservableNotPreservable,
        #[cfg(not(unix))]
        acl: MetadataCapability::Unobservable,
        #[cfg(unix)]
        extended_attributes: MetadataCapability::Preservable,
        #[cfg(not(unix))]
        extended_attributes: MetadataCapability::Unobservable,
        #[cfg(any(target_os = "macos", windows))]
        creation_time: MetadataCapability::Preservable,
        #[cfg(not(any(target_os = "macos", windows)))]
        creation_time: MetadataCapability::ObservableNotPreservable,
        #[cfg(unix)]
        links: MetadataCapability::ObservableNotPreservable,
        #[cfg(windows)]
        links: MetadataCapability::ObservableNotPreservable,
        #[cfg(all(not(unix), not(windows)))]
        links: MetadataCapability::Unobservable,
        #[cfg(any(unix, windows))]
        provenance_marks: MetadataCapability::Preservable,
        #[cfg(all(not(unix), not(windows)))]
        provenance_marks: MetadataCapability::Unobservable,
    }
}

/// Whether the caller has explicitly confirmed original replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginalChoice {
    /// Leave the source bytes unchanged and report metadata losses.
    Decline,
    /// Replace the source after observation succeeds.
    Confirm,
}

fn listed_blockers(
    read_only: bool,
    nlink: u64,
    losses: &[ReplacementBlocker],
    choice: OriginalChoice,
) -> Vec<ReplacementBlocker> {
    let mut hard = Vec::new();
    if read_only {
        hard.push(ReplacementBlocker::ReadOnlySource);
    }
    if nlink > 1 {
        hard.push(ReplacementBlocker::LinkedSourceSemantics);
    }
    if !hard.is_empty() {
        return hard;
    }
    match choice {
        OriginalChoice::Confirm => Vec::new(),
        OriginalChoice::Decline => {
            let mut blockers = vec![ReplacementBlocker::ConfirmationRequired];
            blockers.extend(losses.iter().copied());
            blockers
        }
    }
}

/// Replace the opened source file with a verified export plan.
///
/// This is the unconfirmed entry point: the source bytes stay in place and
/// the error lists metadata that cannot be preserved or observed. Call
/// [`save_original_with_choice`] with [`OriginalChoice::Confirm`] to replace.
pub fn save_original(
    plan: &ExportPlan,
    source: &SourceStamp,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    save_original_with_choice(plan, source, None, OriginalChoice::Decline)
}

/// [`save_original`] for the session that created the JoyWord lock.
///
/// `owner_token` is compared with the lock file and then discarded. It does
/// not change the origin check, the metadata blockers, or the file. Callers
/// without a token use [`save_original`], which still treats every JoyWord
/// lock as a conflict.
pub fn save_original_with_lock_token(
    plan: &ExportPlan,
    source: &SourceStamp,
    owner_token: Option<&str>,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    save_original_with_choice(plan, source, owner_token, OriginalChoice::Decline)
}

/// [`save_original`] with an explicit confirmation choice.
///
/// [`OriginalChoice::Decline`] does not write the source. [`OriginalChoice::Confirm`]
/// replaces it after observation succeeds. Read-only files and extra hard links
/// still refuse. An observation failure refuses even when confirmed.
pub fn save_original_with_choice(
    plan: &ExportPlan,
    source: &SourceStamp,
    owner_token: Option<&str>,
    choice: OriginalChoice,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    save_original_inner(plan, source, owner_token, choice, |_, _| Ok(()))
}

fn save_original_inner(
    plan: &ExportPlan,
    source: &SourceStamp,
    owner_token: Option<&str>,
    choice: OriginalChoice,
    mut before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    let path = source.path();
    let extension = match plan.target() {
        ExportTarget::Docx => "docx",
        ExportTarget::Dotx => "dotx",
        ExportTarget::Docm => "docm",
        ExportTarget::Dotm => "dotm",
    };
    let matches = path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case(extension));
    if !matches {
        return Err(SaveOriginalError::io(
            SourceStage::Validate,
            path,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "source extension must match the export plan's package type",
            ),
        ));
    }
    // The published file is read back under the stamp's cap.
    if plan.bytes().len() as u64 > source.budget_limit() {
        return Err(SaveOriginalError::io(
            SourceStage::Validate,
            path,
            io::Error::new(
                io::ErrorKind::FileTooLarge,
                "export plan exceeds the source read budget",
            ),
        ));
    }
    // Full re-verification inside this call; earlier check_source passes are
    // not treated as an atomic compare-and-swap against later races.
    check_source_with_lock_token(source, owner_token)?;
    match plan.origin_bytes() {
        None => {
            return Err(SaveOriginalError::ReplacementUnavailable {
                path: path.to_path_buf(),
                blockers: vec![ReplacementBlocker::OriginBindingUnproven],
            });
        }
        Some(origin) if origin != source.bytes() => {
            return Err(SaveOriginalError::OriginMismatch {
                path: path.to_path_buf(),
            });
        }
        Some(_) => {}
    }
    let read_only = std::fs::metadata(path)
        .map_err(|e| SaveOriginalError::io(SourceStage::Stat, path, e))?
        .permissions()
        .readonly();
    if let Err(error) = before(SourceStage::ObserveMetadata, path) {
        return Err(SaveOriginalError::observation(path, error));
    }
    let observed = super::metadata::observe(path)
        .map_err(|error| SaveOriginalError::observation(path, error))?;
    if !super::metadata::kernel_xattrs_are_security_namespace(&observed.kernel_xattrs) {
        return Err(SaveOriginalError::observation(
            path,
            io::Error::other(
                "extended attribute classification included a name outside security.*",
            ),
        ));
    }
    let blockers = listed_blockers(
        read_only,
        source.identity().nlink(),
        &observed.losses,
        choice,
    );
    if !blockers.is_empty() {
        return Err(SaveOriginalError::ReplacementUnavailable {
            path: path.to_path_buf(),
            blockers,
        });
    }
    publish_original(plan, source, owner_token, &observed.xattrs, before)
}

/// Same-directory replace. Callers must already have proved origin equality
/// and an empty blocker list. Tests also call this directly to exercise the
/// steps without pretending the public matrix currently allows them.
fn publish_original(
    plan: &ExportPlan,
    source: &SourceStamp,
    owner_token: Option<&str>,
    xattrs: &[(std::ffi::OsString, Vec<u8>)],
    before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    #[cfg(unix)]
    {
        publish_original_unix(plan, source, owner_token, xattrs, before)
    }
    #[cfg(windows)]
    {
        publish_original_windows(plan, source, owner_token, xattrs, before)
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        let _ = (plan, owner_token, xattrs, before);
        Err(SaveOriginalError::ReplacementUnavailable {
            path: source.path().to_path_buf(),
            blockers: vec![ReplacementBlocker::AtomicReplaceUnverified],
        })
    }
}

#[cfg(windows)]
fn win32_error(error: winsafe::co::ERROR) -> io::Error {
    io::Error::from_raw_os_error(error.raw() as i32)
}

#[cfg(windows)]
fn win32_path(path: &Path) -> io::Result<&str> {
    path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Win32 path is not valid Unicode",
        )
    })
}

#[cfg(windows)]
fn replace_file(target: &Path, replacement: &Path, backup: &Path) -> io::Result<()> {
    winsafe::ReplaceFile(
        win32_path(target)?,
        win32_path(replacement)?,
        Some(win32_path(backup)?),
        winsafe::co::REPLACEFILE::default(),
    )
    .map_err(win32_error)
}

#[cfg(windows)]
type PreReplace = fn(&Path) -> io::Result<()>;

#[cfg(windows)]
thread_local! {
    static PRE_REPLACE: std::cell::Cell<Option<PreReplace>> = const { std::cell::Cell::new(None) };
}

#[cfg(windows)]
fn pre_replace_hook(target: &Path) -> io::Result<()> {
    PRE_REPLACE.with(|cell| {
        if let Some(hook) = cell.take() {
            hook(target)
        } else {
            Ok(())
        }
    })
}

#[cfg(windows)]
struct RemoveFileOnDrop {
    path: PathBuf,
    armed: bool,
}

#[cfg(windows)]
impl RemoveFileOnDrop {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(windows)]
impl Drop for RemoveFileOnDrop {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(windows)]
fn reserve_windows_name(parent: &Path, prefix: &str) -> io::Result<PathBuf> {
    let slot = tempfile::Builder::new()
        .prefix(prefix)
        .tempfile_in(parent)?;
    let path = slot.path().to_path_buf();
    slot.close()?;
    Ok(path)
}

#[cfg(windows)]
fn confirm_windows_backup(backup: &Path, source: &SourceStamp) -> Result<(), SaveOriginalError> {
    let target = source.path();
    let identity = SourceIdentity::capture_path(backup)
        .map_err(|e| SaveOriginalError::io(SourceStage::VerifyBackup, target, e))?;
    if !source.identity().same_object(&identity) {
        return Err(SaveOriginalError::conflict_error(
            target,
            super::error::ConflictDetail::InodeChanged,
        ));
    }
    let mut file = File::open(backup)
        .map_err(|e| SaveOriginalError::io(SourceStage::VerifyBackup, target, e))?;
    match verify_bytes(&mut file, source.bytes()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => Err(
            SaveOriginalError::conflict_error(target, super::error::ConflictDetail::ContentChanged),
        ),
        Err(error) => Err(SaveOriginalError::io(
            SourceStage::VerifyBackup,
            target,
            error,
        )),
    }
}

#[cfg(windows)]
fn verify_windows_metadata(
    source: &SourceStamp,
    published: &SourceStamp,
    xattrs: &[(std::ffi::OsString, Vec<u8>)],
) -> io::Result<()> {
    use std::io::Read;

    if source.identity().creation_time() != published.identity().creation_time() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "published file creation time differs from the source",
        ));
    }
    for (name, expected) in xattrs {
        if name != "Zone.Identifier" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unexpected Windows metadata stream",
            ));
        }
        let stream = format!("{}:Zone.Identifier", source.path().display());
        let mut bytes = Vec::new();
        File::open(stream)?
            .take(expected.len() as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes != *expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "published Zone.Identifier differs from the source",
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn publish_original_windows(
    plan: &ExportPlan,
    source: &SourceStamp,
    owner_token: Option<&str>,
    xattrs: &[(std::ffi::OsString, Vec<u8>)],
    mut before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    let target = source.path();
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let fail = |stage, error: io::Error| SaveOriginalError::io(stage, target, error);
    before(SourceStage::CreateTemporary, parent)
        .map_err(|e| fail(SourceStage::CreateTemporary, e))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".joyword-replace-")
        .tempfile_in(parent)
        .map_err(|e| fail(SourceStage::CreateTemporary, e))?;
    before(SourceStage::Write, temporary.path())
        .and_then(|_| temporary.write_all(plan.bytes()))
        .map_err(|e| fail(SourceStage::Write, e))?;
    before(SourceStage::RestorePermissions, temporary.path())
        .map_err(|e| fail(SourceStage::RestorePermissions, e))?;
    before(SourceStage::SyncFile, temporary.path())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|e| fail(SourceStage::SyncFile, e))?;
    before(SourceStage::Verify, temporary.path())
        .and_then(|_| verify_bytes(temporary.as_file_mut(), plan.bytes()))
        .map_err(|e| fail(SourceStage::Verify, e))?;
    let written =
        SourceIdentity::capture_path(temporary.path()).map_err(|e| fail(SourceStage::Verify, e))?;

    before(SourceStage::Backup, target).map_err(|e| fail(SourceStage::Backup, e))?;
    check_source_with_lock_token(source, owner_token)?;
    let backup_path = reserve_windows_name(parent, ".joyword-replace-bak-")
        .map_err(|e| fail(SourceStage::Backup, e))?;
    fs::hard_link(target, &backup_path).map_err(|e| fail(SourceStage::Backup, e))?;
    let mut backup = RemoveFileOnDrop::new(backup_path.clone());
    let displaced_path = reserve_windows_name(parent, ".joyword-replace-old-")
        .map_err(|e| fail(SourceStage::Backup, e))?;
    let mut displaced = RemoveFileOnDrop::new(displaced_path.clone());
    before(SourceStage::VerifyBackup, &backup_path)
        .map_err(|e| fail(SourceStage::VerifyBackup, e))?;
    confirm_windows_backup(&backup_path, source)?;
    let file_name = target.file_name().unwrap_or_default();
    locks::check_directories(source.lock_directories(), file_name, owner_token)?;

    before(SourceStage::Publish, temporary.path()).map_err(|e| fail(SourceStage::Publish, e))?;
    let live = SourceIdentity::capture_path(target).map_err(|e| fail(SourceStage::Publish, e))?;
    if !source.identity().same_object(&live) {
        return Err(SaveOriginalError::conflict_error(
            target,
            super::error::ConflictDetail::InodeChanged,
        ));
    }
    confirm_windows_backup(&backup_path, source)?;
    let (file, temporary_path) = temporary
        .keep()
        .map_err(|error| fail(SourceStage::Publish, error.error))?;
    drop(file);
    let mut temporary_cleanup = RemoveFileOnDrop::new(temporary_path.clone());
    pre_replace_hook(target).map_err(|e| fail(SourceStage::Publish, e))?;
    if let Err(error) = replace_file(target, &temporary_path, &displaced_path) {
        let unchanged = SourceIdentity::capture_path(target)
            .ok()
            .is_some_and(|identity| source.identity().same_object(&identity))
            && fs::read(target).ok().as_deref() == Some(source.bytes());
        if unchanged {
            return Err(fail(SourceStage::Publish, error));
        }
        backup.disarm();
        displaced.disarm();
        temporary_cleanup.disarm();
        return Err(SaveOriginalError::PublishedUnconfirmed {
            path: target.to_path_buf(),
            source: error,
            backup: backup_path,
        });
    }
    temporary_cleanup.disarm();

    let displaced_matches = SourceIdentity::capture_path(&displaced_path)
        .ok()
        .is_some_and(|identity| source.identity().same_object(&identity))
        && fs::read(&displaced_path).ok().as_deref() == Some(source.bytes());
    if !displaced_matches {
        let restore_backup = reserve_windows_name(parent, ".joyword-restore-new-")
            .map_err(|e| fail(SourceStage::Publish, e))?;
        match replace_file(target, &displaced_path, &restore_backup) {
            Ok(()) => {
                let _ = fs::remove_file(restore_backup);
                return Err(SaveOriginalError::conflict_error(
                    target,
                    super::error::ConflictDetail::InodeChanged,
                ));
            }
            Err(error) => {
                backup.disarm();
                displaced.disarm();
                let _ = fs::remove_file(restore_backup);
                return Err(SaveOriginalError::PublishedUnconfirmed {
                    path: target.to_path_buf(),
                    source: error,
                    backup: backup_path,
                });
            }
        }
    }

    let confirmed = before(SourceStage::Confirm, target)
        .and_then(|_| confirm_published(source, &written, plan.bytes()))
        .and_then(|(stamp, _metadata)| {
            verify_windows_metadata(source, &stamp, xattrs).map(|()| stamp)
        });
    let stamp = match confirmed {
        Ok(stamp) => stamp,
        Err(error) => {
            backup.disarm();
            displaced.disarm();
            return Err(SaveOriginalError::PublishedUnconfirmed {
                path: target.to_path_buf(),
                source: error,
                backup: backup_path,
            });
        }
    };
    let receipt = SaveOriginalReceipt {
        path: target.to_path_buf(),
        version: plan.version(),
        durability: super::CopyDurability::FileSynced,
        stamp: Box::new(stamp),
    };
    before(SourceStage::Cleanup, &backup_path)
        .and_then(|_| fs::remove_file(&backup_path))
        .and_then(|_| fs::remove_file(&displaced_path))
        .map_err(|error| {
            backup.disarm();
            displaced.disarm();
            SaveOriginalError::io_published(SourceStage::Cleanup, target, error, receipt.clone())
        })?;
    backup.disarm();
    displaced.disarm();
    before(SourceStage::SyncDirectory, parent)
        .and_then(|_| sync_directory(parent))
        .map_err(|error| {
            SaveOriginalError::io_published(
                SourceStage::SyncDirectory,
                target,
                error,
                receipt.clone(),
            )
        })?;
    Ok(receipt)
}

/// Metadata the replacement must carry, from the verified source.
#[cfg(unix)]
struct Expected {
    mode: u32,
    uid: u32,
    gid: u32,
    #[cfg(target_os = "macos")]
    created: Option<std::time::SystemTime>,
}

#[cfg(unix)]
impl Expected {
    /// Mode, owner and group from the stamp, which `check_source` just
    /// matched against the disk; the creation time from the same file now.
    /// Later changes to any of them move the status-change time, which the
    /// check before the backup compares against the stamp.
    fn of(source: &SourceStamp) -> io::Result<Self> {
        let unix = source.identity().unix();
        #[cfg(target_os = "macos")]
        let created = fs::metadata(source.path())?.created().ok();
        Ok(Self {
            mode: unix.mode() & 0o777,
            uid: unix.uid(),
            gid: unix.gid(),
            #[cfg(target_os = "macos")]
            created,
        })
    }

    /// First item `metadata` does not carry as expected.
    fn mismatch(&self, metadata: &fs::Metadata) -> Option<MetadataItem> {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != self.uid || metadata.gid() != self.gid {
            return Some(MetadataItem::Ownership);
        }
        if metadata.mode() & 0o777 != self.mode {
            return Some(MetadataItem::Permissions);
        }
        if metadata.nlink() != 1 {
            return Some(MetadataItem::Links);
        }
        #[cfg(target_os = "macos")]
        if self.created.is_some() && metadata.created().ok() != self.created {
            return Some(MetadataItem::CreationTime);
        }
        None
    }

    /// Apply owner and group (only when they differ), then permission bits
    /// (an owner change may clear them), then the creation time and the
    /// extended attributes observed on the source. An owner the process may
    /// not give the file to is a refusal, not an IO error.
    fn restore(
        &self,
        file: &File,
        target: &Path,
        xattr_path: &Path,
        xattrs: &[(std::ffi::OsString, Vec<u8>)],
    ) -> Result<(), SaveOriginalError> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let fail = |e| SaveOriginalError::io(SourceStage::RestorePermissions, target, e);
        let current = file.metadata().map_err(fail)?;
        if (current.uid(), current.gid()) != (self.uid, self.gid) {
            std::os::unix::fs::fchown(file, Some(self.uid), Some(self.gid))
                .map_err(|_| refuse(target, MetadataItem::Ownership))?;
        }
        file.set_permissions(fs::Permissions::from_mode(self.mode))
            .map_err(fail)?;
        #[cfg(target_os = "macos")]
        if let Some(created) = self.created {
            use std::os::darwin::fs::FileTimesExt as _;
            file.set_times(fs::FileTimes::new().set_created(created))
                .map_err(fail)?;
        }
        super::metadata::restore_xattrs(xattr_path, xattrs).map_err(|item| refuse(target, item))?;
        Ok(())
    }
}

/// A single metadata item that cannot be reproduced on the replacement.
#[cfg(unix)]
fn refuse(target: &Path, item: MetadataItem) -> SaveOriginalError {
    SaveOriginalError::ReplacementUnavailable {
        path: target.to_path_buf(),
        blockers: vec![ReplacementBlocker::UnpreservableMetadata(item)],
    }
}

#[cfg(unix)]
fn publish_original_unix(
    plan: &ExportPlan,
    source: &SourceStamp,
    owner_token: Option<&str>,
    xattrs: &[(std::ffi::OsString, Vec<u8>)],
    mut before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    let target = source.path();
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let fail = |stage, error: io::Error| SaveOriginalError::io(stage, target, error);
    let expected = Expected::of(source).map_err(|e| fail(SourceStage::Stat, e))?;
    before(SourceStage::CreateTemporary, parent)
        .map_err(|e| fail(SourceStage::CreateTemporary, e))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".joyword-replace-")
        .tempfile_in(parent)
        .map_err(|e| fail(SourceStage::CreateTemporary, e))?;
    before(SourceStage::Write, temporary.path()).map_err(|e| fail(SourceStage::Write, e))?;
    temporary
        .write_all(plan.bytes())
        .map_err(|e| fail(SourceStage::Write, e))?;
    // Metadata goes on before the sync, so the sync covers it too.
    before(SourceStage::RestorePermissions, temporary.path())
        .map_err(|e| fail(SourceStage::RestorePermissions, e))?;
    expected.restore(temporary.as_file(), target, temporary.path(), xattrs)?;
    before(SourceStage::SyncFile, temporary.path()).map_err(|e| fail(SourceStage::SyncFile, e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| fail(SourceStage::SyncFile, e))?;
    before(SourceStage::Verify, temporary.path()).map_err(|e| fail(SourceStage::Verify, e))?;
    verify_bytes(temporary.as_file_mut(), plan.bytes())
        .map_err(|e| fail(SourceStage::Verify, e))?;
    let written = temporary
        .as_file()
        .metadata()
        .map_err(|e| fail(SourceStage::Verify, e))?;
    if let Some(item) = expected.mismatch(&written) {
        return Err(refuse(target, item));
    }
    let written = SourceIdentity::capture(&written);

    before(SourceStage::Backup, target).map_err(|e| fail(SourceStage::Backup, e))?;
    let current =
        SourceIdentity::capture(&fs::metadata(target).map_err(|e| fail(SourceStage::Backup, e))?);
    if let Some(detail) = source.identity().difference(&current) {
        return Err(SaveOriginalError::conflict_error(target, detail));
    }
    let backup_path = reserve_backup_name(parent).map_err(|e| fail(SourceStage::Backup, e))?;
    if let Err(error) = fs::hard_link(target, &backup_path) {
        let _ = fs::remove_file(&backup_path);
        return Err(fail(SourceStage::Backup, error));
    }
    let abort_backup = |error: SaveOriginalError| {
        let _ = fs::remove_file(&backup_path);
        error
    };
    if let Err(error) = before(SourceStage::VerifyBackup, &backup_path) {
        return Err(abort_backup(fail(SourceStage::VerifyBackup, error)));
    }
    // A lock that appeared while the replacement was prepared still wins.
    let file_name = target.file_name().unwrap_or_default();
    if let Err(error) = locks::check_directories(source.lock_directories(), file_name, owner_token)
    {
        return Err(abort_backup(error));
    }
    if let Err(error) = confirm_backup(&backup_path, source) {
        return Err(abort_backup(error));
    }

    if let Err(error) = before(SourceStage::Publish, temporary.path()) {
        return Err(abort_backup(fail(SourceStage::Publish, error)));
    }
    // Last object check before the exchange. A replacement that lands after
    // this check is undone: the exchange is atomic, and the inode that lands
    // on the temporary name must still be the verified source.
    match fs::symlink_metadata(target) {
        Ok(live)
            if source
                .identity()
                .same_object(&SourceIdentity::capture(&live)) => {}
        Ok(_) => {
            return Err(abort_backup(SaveOriginalError::conflict_error(
                target,
                super::error::ConflictDetail::InodeChanged,
            )));
        }
        Err(error) => return Err(abort_backup(fail(SourceStage::Publish, error))),
    }
    let kept = match temporary.keep() {
        Ok(kept) => kept,
        Err(error) => {
            return Err(abort_backup(fail(SourceStage::Publish, error.error)));
        }
    };
    let (file, tmp_path) = kept;
    drop(file);
    publish_exchange(source, target, &tmp_path, &backup_path)?;

    // The target name now refers to the new bytes. Until the read-back below
    // confirms them, the backup (the previous inode) is kept.
    let confirmed = before(SourceStage::Confirm, target)
        .and_then(|_| confirm_published(source, &written, plan.bytes()))
        .and_then(|(stamp, metadata)| match expected.mismatch(&metadata) {
            None => Ok(stamp),
            Some(item) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("published file does not carry the expected {item:?}"),
            )),
        });
    let stamp = match confirmed {
        Ok(stamp) => stamp,
        Err(error) => {
            return Err(SaveOriginalError::PublishedUnconfirmed {
                path: target.to_path_buf(),
                source: error,
                backup: backup_path,
            });
        }
    };
    let receipt = SaveOriginalReceipt {
        path: target.to_path_buf(),
        version: plan.version(),
        durability: super::CopyDurability::FileSynced,
        stamp: Box::new(stamp),
    };
    if let Err(error) =
        before(SourceStage::Cleanup, &backup_path).and_then(|_| fs::remove_file(&backup_path))
    {
        return Err(SaveOriginalError::io_published(
            SourceStage::Cleanup,
            target,
            error,
            receipt,
        ));
    }
    if let Err(error) =
        before(SourceStage::SyncDirectory, parent).and_then(|_| sync_directory(parent))
    {
        return Err(SaveOriginalError::io_published(
            SourceStage::SyncDirectory,
            target,
            error,
            receipt,
        ));
    }
    Ok(SaveOriginalReceipt {
        durability: super::CopyDurability::FileAndDirectorySynced,
        ..receipt
    })
}

#[cfg(unix)]
fn reserve_backup_name(parent: &Path) -> io::Result<PathBuf> {
    let slot = tempfile::Builder::new()
        .prefix(".joyword-replace-bak-")
        .tempfile_in(parent)?;
    let path = slot.path().to_path_buf();
    slot.close()?;
    Ok(path)
}

/// The backup must still be the stamped bytes, and the directory entry must
/// still be that same object. Link count and ctime move when the backup
/// hard link is created, so they are not compared here.
#[cfg(unix)]
fn confirm_backup(backup: &Path, source: &SourceStamp) -> Result<(), SaveOriginalError> {
    let target = source.path();
    let metadata = fs::metadata(backup)
        .map_err(|e| SaveOriginalError::io(SourceStage::VerifyBackup, target, e))?;
    if metadata.len() != source.bytes().len() as u64 {
        return Err(SaveOriginalError::conflict_error(
            target,
            super::error::ConflictDetail::SizeChanged,
        ));
    }
    let mut file = File::open(backup)
        .map_err(|e| SaveOriginalError::io(SourceStage::VerifyBackup, target, e))?;
    match verify_bytes(&mut file, source.bytes()) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            return Err(SaveOriginalError::conflict_error(
                target,
                super::error::ConflictDetail::ContentChanged,
            ));
        }
        Err(error) => {
            return Err(SaveOriginalError::io(
                SourceStage::VerifyBackup,
                target,
                error,
            ));
        }
    }
    let live = fs::metadata(target)
        .map_err(|e| SaveOriginalError::io(SourceStage::VerifyBackup, target, e))?;
    if !source
        .identity()
        .same_object(&SourceIdentity::capture(&live))
    {
        return Err(SaveOriginalError::conflict_error(
            target,
            super::error::ConflictDetail::InodeChanged,
        ));
    }
    Ok(())
}

/// Publish by exchanging the temporary file with the target, then drop the
/// displaced name. On macOS this is `renamex_np(RENAME_SWAP)`; on Linux,
/// `renameat2(RENAME_EXCHANGE)`. Both names must already exist. If the inode
/// that comes back is not the verified source, the names are swapped back and
/// the replacement is refused.
#[cfg(unix)]
fn publish_exchange(
    source: &SourceStamp,
    target: &Path,
    tmp_path: &Path,
    backup_path: &Path,
) -> Result<(), SaveOriginalError> {
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = source;
    let fail = |error: io::Error| SaveOriginalError::io(SourceStage::Publish, target, error);
    let abort_backup = |error: SaveOriginalError| {
        let _ = fs::remove_file(backup_path);
        error
    };
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Err(error) = pre_exchange_hook(target) {
        let _ = fs::remove_file(tmp_path);
        return Err(abort_backup(fail(error)));
    }
    if let Err(error) = exchange_names(tmp_path, target) {
        let _ = fs::remove_file(tmp_path);
        return Err(abort_backup(fail(error)));
    }
    // `rename` consumes the temporary name. Exchange leaves the previous
    // target inode there, and that name has to be removed after the check.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let displaced = match fs::symlink_metadata(tmp_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                return Err(SaveOriginalError::PublishedUnconfirmed {
                    path: target.to_path_buf(),
                    source: error,
                    backup: backup_path.to_path_buf(),
                });
            }
        };
        if !source
            .identity()
            .same_object(&SourceIdentity::capture(&displaced))
        {
            return Err(restore_displaced(
                target,
                tmp_path,
                backup_path,
                abort_backup,
            ));
        }
        if let Err(error) = fs::remove_file(tmp_path) {
            if exchange_names(tmp_path, target).is_ok() {
                let _ = fs::remove_file(tmp_path);
                return Err(abort_backup(fail(error)));
            }
            return Err(SaveOriginalError::PublishedUnconfirmed {
                path: target.to_path_buf(),
                source: error,
                backup: backup_path.to_path_buf(),
            });
        }
    }
    Ok(())
}

#[cfg(all(unix, any(target_os = "linux", target_os = "macos")))]
fn restore_displaced(
    target: &Path,
    tmp_path: &Path,
    backup_path: &Path,
    abort_backup: impl Fn(SaveOriginalError) -> SaveOriginalError,
) -> SaveOriginalError {
    match exchange_names(tmp_path, target) {
        Ok(()) => {
            let _ = fs::remove_file(tmp_path);
            abort_backup(SaveOriginalError::conflict_error(
                target,
                super::error::ConflictDetail::InodeChanged,
            ))
        }
        Err(error) => {
            // The unexpected file is the only name at `tmp_path`. Leave it.
            SaveOriginalError::PublishedUnconfirmed {
                path: target.to_path_buf(),
                source: error,
                backup: backup_path.to_path_buf(),
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn exchange_names(from: &Path, to: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(io::Error::from)
}

/// Other Unix targets have no reviewed exchange flag. `rename` still publishes
/// only after the identity check above; it does not restore a displaced inode.
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn exchange_names(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
type PreExchange = fn(&Path) -> io::Result<()>;

#[cfg(any(target_os = "linux", target_os = "macos"))]
thread_local! {
    static PRE_EXCHANGE: std::cell::Cell<Option<PreExchange>> = const { std::cell::Cell::new(None) };
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn pre_exchange_hook(target: &Path) -> io::Result<()> {
    PRE_EXCHANGE.with(|cell| {
        if let Some(hook) = cell.take() {
            hook(target)
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
mod migration_tests {
    use super::*;
    fn plan(source: &SourceStamp) -> ExportPlan {
        let mut doc = joyword_docx::read(source.bytes()).unwrap();
        let mut session = joyword_engine::Session::new(doc);
        session.run("select.all", &serde_json::json!({})).unwrap();
        session
            .run(
                "text.insert",
                &serde_json::json!({"text":"Fault injection 中文","raw":true}),
            )
            .unwrap();
        doc = session.doc;
        ExportPlan::build(&doc, 1, Some(std::sync::Arc::from(source.bytes()))).unwrap()
    }
    #[test]
    fn injected_original_save_failures_preserve_source() {
        for stage in [
            SourceStage::CreateTemporary,
            SourceStage::Write,
            SourceStage::SyncFile,
            SourceStage::Verify,
            SourceStage::Backup,
            SourceStage::VerifyBackup,
            SourceStage::Publish,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("source.docx");
            let bytes = crate::reader::WELCOME;
            std::fs::write(&path, bytes).unwrap();
            let source = super::super::open_source(&path, &Default::default())
                .unwrap()
                .into_stamp();
            let p = plan(&source);
            let result =
                save_original_inner(&p, &source, None, OriginalChoice::Confirm, |at, _| {
                    if at == stage {
                        Err(io::Error::other("injected storage failure"))
                    } else {
                        Ok(())
                    }
                });
            assert!(result.is_err(), "{stage:?}");
            assert_eq!(std::fs::read(&path).unwrap(), bytes, "{stage:?}");
        }
    }
}
