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

use joyword_docx::{ExportPlan, ExportTarget};
use joyword_model::DocumentVersion;

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
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn unix_matrix_is_honest_about_std_only_capabilities() {
        let caps = original_replacement_capabilities();
        assert_eq!(caps.permissions, MetadataCapability::Preservable);
        assert_eq!(caps.ownership, MetadataCapability::Preservable);
        assert_eq!(caps.acl, MetadataCapability::ObservableNotPreservable);
        assert_eq!(caps.extended_attributes, MetadataCapability::Preservable);
        assert_eq!(
            caps.creation_time,
            if cfg!(target_os = "macos") {
                MetadataCapability::Preservable
            } else {
                MetadataCapability::ObservableNotPreservable
            }
        );
        assert_eq!(caps.links, MetadataCapability::ObservableNotPreservable);
        assert_eq!(caps.provenance_marks, MetadataCapability::Preservable);
    }

    #[cfg(windows)]
    #[test]
    fn windows_matrix_reflects_replace_file_capabilities() {
        let caps = original_replacement_capabilities();
        assert_eq!(caps.permissions, MetadataCapability::Preservable);
        assert_eq!(caps.ownership, MetadataCapability::Unobservable);
        assert_eq!(caps.acl, MetadataCapability::Unobservable);
        assert_eq!(caps.extended_attributes, MetadataCapability::Unobservable);
        assert_eq!(caps.creation_time, MetadataCapability::Preservable);
        assert_eq!(caps.links, MetadataCapability::ObservableNotPreservable);
        assert_eq!(caps.provenance_marks, MetadataCapability::Preservable);
    }

    #[test]
    fn metadata_blockers_no_longer_include_the_origin_gap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.docx");
        std::fs::write(&path, b"0123456789").unwrap();
        let source = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let blockers = listed_blockers(
            false,
            source.identity().nlink(),
            &[],
            OriginalChoice::Decline,
        );
        assert!(!blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
        assert!(!blockers.contains(&ReplacementBlocker::ReadOnlySource));
        assert!(blockers.contains(&ReplacementBlocker::ConfirmationRequired));
        assert!(
            listed_blockers(true, 1, &[], OriginalChoice::Confirm)
                .contains(&ReplacementBlocker::ReadOnlySource)
        );
        assert!(
            listed_blockers(false, 2, &[], OriginalChoice::Confirm)
                .contains(&ReplacementBlocker::LinkedSourceSemantics)
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_observation_failure_refuses_even_a_confirmed_replacement() {
        let bytes = fixture();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, &bytes).unwrap();
        let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let plan = edited_plan(&bytes);
        let error =
            save_original_inner(&plan, &stamp, None, OriginalChoice::Confirm, |stage, _| {
                if stage == SourceStage::ObserveMetadata {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "metadata observation failed",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert!(error.is_observation_failure());
        assert!(error.published().is_none());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_replacement_copies_a_user_extended_attribute() {
        let bytes = fixture();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, &bytes).unwrap();
        let name = if cfg!(target_os = "macos") {
            "joyword.m1"
        } else {
            "user.joyword-m1"
        };
        xattr::set(&path, name, b"kept").unwrap();
        let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let plan = edited_plan(&bytes);
        save_original_with_choice(&plan, &stamp, None, OriginalChoice::Confirm).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
        assert_eq!(
            xattr::get(&path, name).unwrap().as_deref(),
            Some(&b"kept"[..])
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn confirmed_replacement_skips_security_xattrs_and_keeps_user_xattrs() {
        let bytes = fixture();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, &bytes).unwrap();
        xattr::set(&path, "user.joyword-m1", b"kept").unwrap();
        xattr::set(&path, "user.security.note", b"also-kept").unwrap();
        let marker = b"v";
        let planted = xattr::set(&path, "security.selinux", marker).is_ok();
        let observed = super::super::metadata::observe(&path).unwrap();
        assert!(
            observed
                .xattrs
                .iter()
                .any(|(name, value)| name == "user.joyword-m1" && value == b"kept")
        );
        assert!(
            observed
                .xattrs
                .iter()
                .all(|(name, _)| !super::super::metadata::is_kernel_security_xattr(name))
        );
        assert!(
            super::super::metadata::kernel_xattrs_are_security_namespace(&observed.kernel_xattrs)
        );
        if planted {
            assert!(
                observed
                    .kernel_xattrs
                    .iter()
                    .any(|name| name == "security.selinux")
            );
        }
        let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let plan = edited_plan(&bytes);
        save_original_with_choice(&plan, &stamp, None, OriginalChoice::Confirm).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
        assert_eq!(
            xattr::get(&path, "user.joyword-m1").unwrap().as_deref(),
            Some(&b"kept"[..])
        );
        assert_eq!(
            xattr::get(&path, "user.security.note").unwrap().as_deref(),
            Some(&b"also-kept"[..])
        );
        if planted {
            assert_ne!(
                xattr::get(&path, "security.selinux").unwrap().as_deref(),
                Some(marker.as_slice())
            );
        }
    }

    #[cfg(windows)]
    fn windows_fixture() -> Vec<u8> {
        std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
        )
        .unwrap()
    }

    #[cfg(windows)]
    fn windows_plan(bytes: &[u8]) -> joyword_docx::ExportPlan {
        use joyoffice_opc::Budget;
        use joyword_engine::{Command, Transaction};
        use joyword_model::{Bias, Position, ScalarOffset};

        let mut session = crate::docx::DocxSession::open(bytes, &Budget::default()).unwrap();
        let document = session.session().document();
        let at = Position {
            story: document.body_id(),
            paragraph: document.body().blocks.get(0).unwrap().id(),
            offset: ScalarOffset(0),
            bias: Bias::After,
        };
        session
            .apply(Transaction::new(
                session.session().version(),
                vec![Command::InsertText {
                    at,
                    text: "发布".into(),
                }],
            ))
            .unwrap();
        session.export_plan().unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn windows_failures_before_replace_keep_the_source_and_remove_backups() {
        let bytes = windows_fixture();
        let plan = windows_plan(&bytes);
        for stage in [
            SourceStage::CreateTemporary,
            SourceStage::Write,
            SourceStage::RestorePermissions,
            SourceStage::SyncFile,
            SourceStage::Verify,
            SourceStage::Backup,
            SourceStage::VerifyBackup,
            SourceStage::Publish,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("report.docx");
            std::fs::write(&path, &bytes).unwrap();
            let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
                .unwrap()
                .into_stamp();
            let error =
                save_original_inner(&plan, &stamp, None, OriginalChoice::Confirm, |at, _| {
                    if at == stage {
                        Err(io::Error::new(io::ErrorKind::StorageFull, "injected"))
                    } else {
                        Ok(())
                    }
                })
                .unwrap_err();
            assert_eq!(error.stage(), Some(stage), "{stage}");
            assert!(error.published().is_none());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "{stage}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_replace_restores_a_writer_raced_after_the_final_identity_check() {
        fn race(path: &Path) -> io::Result<()> {
            let other = path.with_file_name("other-writer.tmp");
            std::fs::write(&other, b"raced-writer")?;
            std::fs::rename(other, path)
        }
        struct ClearHook;
        impl Drop for ClearHook {
            fn drop(&mut self) {
                super::PRE_REPLACE.with(|cell| cell.set(None));
            }
        }
        let _clear = ClearHook;
        super::PRE_REPLACE.with(|cell| cell.set(Some(race)));

        let bytes = windows_fixture();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, &bytes).unwrap();
        let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let plan = windows_plan(&bytes);
        let error =
            save_original_inner(&plan, &stamp, None, OriginalChoice::Confirm, |_, _| Ok(()))
                .unwrap_err();
        assert_eq!(
            error.conflict(),
            Some(super::super::ConflictDetail::InodeChanged)
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"raced-writer");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_unconfirmed_publication_keeps_the_original_backup() {
        let bytes = windows_fixture();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, &bytes).unwrap();
        let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let plan = windows_plan(&bytes);
        let error =
            save_original_inner(&plan, &stamp, None, OriginalChoice::Confirm, |stage, _| {
                if stage == SourceStage::Confirm {
                    Err(io::Error::new(io::ErrorKind::StorageFull, "injected"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.stage(), Some(SourceStage::Confirm));
        assert!(error.published().is_none());
        assert_eq!(
            std::fs::read(error.retained_backup().unwrap()).unwrap(),
            bytes
        );
        assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 3);
    }

    #[cfg(unix)]
    fn fixture() -> Vec<u8> {
        std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn edited_plan(bytes: &[u8]) -> joyword_docx::ExportPlan {
        use joyoffice_opc::Budget;
        use joyword_engine::{Command, Transaction};
        use joyword_model::{Bias, Position, ScalarOffset};

        let mut session = crate::docx::DocxSession::open(bytes, &Budget::default()).unwrap();
        let document = session.session().document();
        let at = Position {
            story: document.body_id(),
            paragraph: document.body().blocks.get(0).unwrap().id(),
            offset: ScalarOffset(0),
            bias: Bias::After,
        };
        session
            .apply(Transaction::new(
                session.session().version(),
                vec![Command::InsertText {
                    at,
                    text: "发布".into(),
                }],
            ))
            .unwrap();
        session.export_plan().unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn direct_publish_replaces_bytes_restores_mode_and_reopens() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        use joyoffice_opc::Budget;

        let bytes = fixture();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.docx");
        std::fs::write(&path, &bytes).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o640);
        std::fs::set_permissions(&path, permissions).unwrap();
        let stamp = super::super::open_source(&path, &Budget::default())
            .unwrap()
            .into_stamp();
        let plan = edited_plan(&bytes);
        assert_eq!(plan.origin_bytes(), Some(bytes.as_slice()));
        assert_ne!(plan.bytes(), bytes.as_slice());
        let before = std::fs::metadata(&path).unwrap();
        let receipt = publish_original(&plan, &stamp, None, &[], |_, _| Ok(())).unwrap();
        assert_eq!(receipt.path(), stamp.path());
        assert_eq!(receipt.version(), plan.version());
        assert_eq!(
            receipt.durability(),
            super::super::CopyDurability::FileAndDirectorySynced
        );
        assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
        let after = std::fs::metadata(&path).unwrap();
        assert_eq!(after.mode() & 0o777, 0o640);
        assert_eq!((after.uid(), after.gid()), (before.uid(), before.gid()));
        assert_ne!(after.ino(), before.ino());
        #[cfg(target_os = "macos")]
        assert_eq!(after.created().unwrap(), before.created().unwrap());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        // The receipt's stamp is the read-back of the new file.
        assert_eq!(receipt.bytes(), plan.bytes().len() as u64);
        assert_eq!(receipt.stamp().path(), stamp.path());
        super::super::check_source(receipt.stamp()).unwrap();
        let reopened =
            crate::docx::DocxSession::open(&std::fs::read(&path).unwrap(), &Budget::default())
                .unwrap();
        assert_eq!(reopened.backing().original_bytes(), plan.bytes());
        let fresh = super::super::open_source(&path, &Budget::default()).unwrap();
        assert_eq!(fresh.bytes(), plan.bytes());
        assert!(super::super::check_source(&stamp).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn publish_failures_before_rename_keep_the_source_and_remove_the_backup() {
        let bytes = fixture();
        let plan = edited_plan(&bytes);
        for stage in [
            SourceStage::CreateTemporary,
            SourceStage::Write,
            SourceStage::SyncFile,
            SourceStage::Verify,
            SourceStage::RestorePermissions,
            SourceStage::Backup,
            SourceStage::VerifyBackup,
            SourceStage::Publish,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("report.docx");
            std::fs::write(&path, &bytes).unwrap();
            let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
                .unwrap()
                .into_stamp();
            let error = publish_original(&plan, &stamp, None, &[], |at, file| {
                if at != stage {
                    return Ok(());
                }
                if at == SourceStage::Verify {
                    let mut corrupted = plan.bytes().to_vec();
                    corrupted[0] ^= 0xff;
                    std::fs::write(file, corrupted)?;
                    return Ok(());
                }
                Err(std::io::Error::new(
                    std::io::ErrorKind::StorageFull,
                    "injected",
                ))
            })
            .unwrap_err();
            assert_eq!(error.stage(), Some(stage), "{stage}");
            assert!(error.published().is_none());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "{stage}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn failures_after_rename_report_the_published_receipt() {
        let bytes = fixture();
        let plan = edited_plan(&bytes);
        for stage in [SourceStage::Cleanup, SourceStage::SyncDirectory] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("report.docx");
            std::fs::write(&path, &bytes).unwrap();
            let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
                .unwrap()
                .into_stamp();
            let error = publish_original(&plan, &stamp, None, &[], |at, _| {
                if at == stage {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "injected",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.stage(), Some(stage));
            let receipt = error.published().expect("published");
            assert_eq!(receipt.version(), plan.version());
            assert_eq!(
                receipt.durability(),
                super::super::CopyDurability::FileSynced
            );
            assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
            super::super::check_source(receipt.stamp()).unwrap();
        }
    }

    /// One fresh source and a bound edited plan for the race and read-back
    /// tests below.
    #[cfg(unix)]
    struct Case {
        dir: tempfile::TempDir,
        path: PathBuf,
        bytes: Vec<u8>,
        stamp: SourceStamp,
        plan: ExportPlan,
    }

    #[cfg(unix)]
    impl Case {
        fn new() -> Self {
            let bytes = fixture();
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("report.docx");
            std::fs::write(&path, &bytes).unwrap();
            let stamp = super::super::open_source(&path, &joyoffice_opc::Budget::default())
                .unwrap()
                .into_stamp();
            let plan = edited_plan(&bytes);
            Self {
                dir,
                path,
                bytes,
                stamp,
                plan,
            }
        }

        fn publish(
            &self,
            before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
        ) -> Result<SaveOriginalReceipt, SaveOriginalError> {
            publish_original(&self.plan, &self.stamp, None, &[], before)
        }

        fn entries(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(self.dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    /// Another writer's file renamed over `path`: a different object.
    #[cfg(unix)]
    fn replace_with_other(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let other = path.with_file_name("other-writer.tmp");
        std::fs::write(&other, bytes)?;
        std::fs::rename(other, path)
    }

    #[cfg(unix)]
    #[test]
    fn a_replacement_after_the_backup_link_is_caught() {
        let case = Case::new();
        let error = case
            .publish(|at, _| {
                if at == SourceStage::VerifyBackup {
                    // Identical bytes, different object.
                    replace_with_other(&case.path, &case.bytes)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(
            error.conflict(),
            Some(super::super::ConflictDetail::InodeChanged)
        );
        assert_eq!(std::fs::read(&case.path).unwrap(), case.bytes);
        assert_eq!(case.entries(), ["report.docx"]);
    }

    #[cfg(unix)]
    #[test]
    fn an_append_after_the_backup_link_is_caught() {
        let case = Case::new();
        let error = case
            .publish(|at, _| {
                if at == SourceStage::VerifyBackup {
                    let mut file = File::options().append(true).open(&case.path)?;
                    file.write_all(b"appended")?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.conflict().is_some(), "{error}");
        assert!(std::fs::read(&case.path).unwrap().ends_with(b"appended"));
        assert_eq!(case.entries(), ["report.docx"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_replacement_just_before_the_rename_is_caught() {
        let case = Case::new();
        let error = case
            .publish(|at, _| {
                if at == SourceStage::Publish {
                    replace_with_other(&case.path, b"another writer")?;
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(
            error.conflict(),
            Some(super::super::ConflictDetail::InodeChanged)
        );
        assert_eq!(std::fs::read(&case.path).unwrap(), b"another writer");
        assert_eq!(case.entries(), ["report.docx"]);
    }

    /// The identity check passed, then another writer replaced the target.
    /// Exchange observes the displaced inode and swaps the names back.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn exchange_restores_a_file_replaced_after_the_identity_check() {
        let case = Case::new();
        fn race(path: &Path) -> io::Result<()> {
            let other = path.with_file_name("other-writer.tmp");
            std::fs::write(&other, b"raced-writer")?;
            std::fs::rename(&other, path)?;
            Ok(())
        }
        struct ClearHook;
        impl Drop for ClearHook {
            fn drop(&mut self) {
                super::PRE_EXCHANGE.with(|cell| cell.set(None));
            }
        }
        let _clear = ClearHook;
        super::PRE_EXCHANGE.with(|cell| cell.set(Some(race)));
        let error = case.publish(|_, _| Ok(())).unwrap_err();
        assert_eq!(
            error.conflict(),
            Some(super::super::ConflictDetail::InodeChanged)
        );
        assert!(matches!(
            std::fs::read(&case.path).as_deref(),
            Ok(b"raced-writer")
        ));
        assert_eq!(case.entries(), ["report.docx"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_lock_appearing_during_the_save_blocks_publication_and_stays() {
        for lock in ["~$report.docx", ".report.docx.joyword-lock"] {
            let case = Case::new();
            let lock_path = case.path.with_file_name(lock);
            let error = case
                .publish(|at, _| {
                    if at == SourceStage::VerifyBackup {
                        std::fs::write(&lock_path, b"owner")?;
                    }
                    Ok(())
                })
                .unwrap_err();
            assert!(error.lock().is_some(), "{lock}: {error}");
            assert_eq!(std::fs::read(&case.path).unwrap(), case.bytes);
            assert_eq!(std::fs::read(&lock_path).unwrap(), b"owner");
            assert_eq!(case.entries().len(), 2);
        }
    }

    #[cfg(unix)]
    #[test]
    fn metadata_drift_on_the_temporary_is_refused_before_publication() {
        use std::os::unix::fs::PermissionsExt;
        let case = Case::new();
        let error = case
            .publish(|at, path| {
                if at == SourceStage::Verify {
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(
            error.blockers(),
            Some(
                &[ReplacementBlocker::UnpreservableMetadata(
                    MetadataItem::Permissions
                )][..]
            )
        );
        assert_eq!(std::fs::read(&case.path).unwrap(), case.bytes);
        assert_eq!(case.entries(), ["report.docx"]);
    }

    #[cfg(unix)]
    #[test]
    fn an_owner_the_process_cannot_give_is_refused() {
        use std::os::unix::fs::MetadataExt;
        let case = Case::new();
        if std::fs::metadata(&case.path).unwrap().uid() == 0 {
            // Root may give files away, so the refusal cannot be provoked.
            return;
        }
        let mut expected = Expected::of(&case.stamp).unwrap();
        expected.uid = expected.uid.wrapping_add(1);
        let temporary = tempfile::NamedTempFile::new_in(case.dir.path()).unwrap();
        let error = expected
            .restore(temporary.as_file(), &case.path, temporary.path(), &[])
            .unwrap_err();
        assert_eq!(
            error.blockers(),
            Some(
                &[ReplacementBlocker::UnpreservableMetadata(
                    MetadataItem::Ownership
                )][..]
            )
        );
        assert_eq!(std::fs::read(&case.path).unwrap(), case.bytes);
    }

    #[cfg(unix)]
    #[test]
    fn an_unconfirmed_publication_keeps_the_backup_and_issues_no_receipt() {
        let case = Case::new();
        let error = case
            .publish(|at, _| {
                if at == SourceStage::Confirm {
                    Err(io::Error::new(io::ErrorKind::StorageFull, "injected"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.stage(), Some(SourceStage::Confirm));
        assert!(error.published().is_none());
        let backup = error.retained_backup().expect("backup kept");
        assert_eq!(std::fs::read(backup).unwrap(), case.bytes);
        assert_eq!(std::fs::read(&case.path).unwrap(), case.plan.bytes());
        assert_eq!(case.entries().len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn receipt_and_stamp_come_only_from_the_published_object() {
        // Same object, other bytes: not confirmed.
        let case = Case::new();
        let error = case
            .publish(|at, path| {
                if at == SourceStage::Confirm {
                    let mut bytes = std::fs::read(path)?;
                    bytes[3] ^= 1;
                    File::options().write(true).open(path)?.write_all(&bytes)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.published().is_none());
        assert_eq!(
            std::fs::read(error.retained_backup().unwrap()).unwrap(),
            case.bytes
        );

        // The plan's exact bytes under another object: not confirmed either.
        let case = Case::new();
        let plan = case.plan.bytes().to_vec();
        let error = case
            .publish(|at, path| {
                if at == SourceStage::Confirm {
                    replace_with_other(path, &plan)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.published().is_none());
        assert!(error.retained_backup().is_some());
        assert_eq!(std::fs::read(&case.path).unwrap(), plan);
    }
}
