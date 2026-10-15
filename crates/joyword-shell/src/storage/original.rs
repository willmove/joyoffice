//! Original-file replacement (W07 gate, W11 publish strategy).
//!
//! `save_original` re-runs conflict verification, then requires
//! [`ExportPlan::origin_bytes`](joyword_docx::ExportPlan::origin_bytes) to be
//! the stamped package bytes. A missing origin or a different package refuses
//! before any write. Metadata this build cannot preserve still refuses before
//! any write as well: the Unix replace steps below run only when that list is
//! empty. `save_original_with_lock_token` can skip the caller's own JoyWord
//! lock and then hits the same checks. `save_export_copy` stays the verified fallback.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::fs::{self, File};
#[cfg(unix)]
use std::io::Write;

use joyword_docx::{ExportPlan, ExportTarget};
use joyword_model::DocumentVersion;

use super::error::{MetadataItem, ReplacementBlocker, SaveOriginalError, SourceStage};
#[cfg(unix)]
use super::source::SourceIdentity;
use super::source::{SourceStamp, check_source_with_lock_token};
#[cfg(unix)]
use super::{sync_directory, verify_bytes};

/// Persistence state of a completed original replacement. Reused from the
/// C03 copy path: `FileSynced` means the published file accepted a sync;
/// `FileAndDirectorySynced` adds the containing directory. Neither claims
/// power-loss testing.
pub type OriginalDurability = super::CopyDurability;

/// Receipt for a completed original replacement. It can only be produced by
/// the real replacement write path; while that path is unavailable
/// `save_original` never constructs one, and an error after publication must
/// carry the receipt instead of pretending nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveOriginalReceipt {
    path: PathBuf,
    version: DocumentVersion,
    durability: OriginalDurability,
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
}

/// What this build can do with one metadata aspect of a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataCapability {
    /// Observable, and an approved write path (`std::fs::set_permissions`
    /// or the platform read-only attribute) can restore it on a new inode.
    /// This is a statement about capability, not an executed replacement.
    Preservable,
    /// Observable, but no approved dependency can restore it (ownership,
    /// creation time); replacement is refused.
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

/// The capability matrix of this build. `std`-only, safe Rust, no new
/// dependencies: permission bits are the only preservable item; ACLs,
/// extended attributes and provenance marks are not even observable, which
/// is why original replacement refuses rather than risking silent loss.
/// Windows entries describe the compiled target, not a tested one.
pub fn original_replacement_capabilities() -> OriginalReplacementCapabilities {
    OriginalReplacementCapabilities {
        permissions: MetadataCapability::Preservable,
        #[cfg(unix)]
        ownership: MetadataCapability::ObservableNotPreservable,
        #[cfg(not(unix))]
        ownership: MetadataCapability::Unobservable,
        acl: MetadataCapability::Unobservable,
        extended_attributes: MetadataCapability::Unobservable,
        creation_time: MetadataCapability::ObservableNotPreservable,
        links: MetadataCapability::ObservableNotPreservable,
        provenance_marks: MetadataCapability::Unobservable,
    }
}

/// Metadata blockers for this source. Origin binding is not one of them:
/// `save_original` compares `origin_bytes()` with the stamp separately.
fn replacement_blockers(source: &SourceStamp) -> Vec<ReplacementBlocker> {
    let caps = original_replacement_capabilities();
    let mut blockers = Vec::new();
    for (item, capability) in [
        (MetadataItem::Permissions, caps.permissions),
        (MetadataItem::Ownership, caps.ownership),
        (MetadataItem::Acl, caps.acl),
        (MetadataItem::ExtendedAttributes, caps.extended_attributes),
        (MetadataItem::CreationTime, caps.creation_time),
        (MetadataItem::Links, caps.links),
        (MetadataItem::ProvenanceMarks, caps.provenance_marks),
    ] {
        match capability {
            MetadataCapability::Preservable => {}
            MetadataCapability::ObservableNotPreservable => {
                blockers.push(ReplacementBlocker::UnpreservableMetadata(item));
            }
            MetadataCapability::Unobservable => {
                blockers.push(ReplacementBlocker::UnobservableMetadata(item));
            }
        }
    }
    if source.identity().nlink() > 1 {
        blockers.push(ReplacementBlocker::LinkedSourceSemantics);
    }
    blockers
}

/// Replace the opened source file with a verified export plan.
///
/// Re-runs the full conflict verification itself (locks, path resolution,
/// identity, exact bytes). The plan's `origin_bytes()` must then equal the
/// stamped package; a missing or different origin refuses with no write.
/// Capability blockers also refuse before any write. Only an empty blocker
/// list continues into the same-directory temporary file, sync, read-back,
/// backup hard link and rename. A failure before that rename leaves the
/// source bytes in place. A failure after it returns the error with
/// [`SaveOriginalError::published`] set.
pub fn save_original(
    plan: &ExportPlan,
    source: &SourceStamp,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    save_original_with_lock_token(plan, source, None)
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
    save_original_inner(plan, source, owner_token, |_, _| Ok(()))
}

fn save_original_inner(
    plan: &ExportPlan,
    source: &SourceStamp,
    owner_token: Option<&str>,
    before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
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
    let blockers = replacement_blockers(source);
    if !blockers.is_empty() {
        return Err(SaveOriginalError::ReplacementUnavailable {
            path: path.to_path_buf(),
            blockers,
        });
    }
    // Metadata blockers are non-empty on every current target, so this call
    // is the reviewed strategy for a later matrix that can actually clear
    // them. It is not a successful save today.
    publish_original(plan, source, before)
}

/// Same-directory replace. Callers must already have proved origin equality
/// and an empty blocker list. Tests also call this directly to exercise the
/// steps without pretending the public matrix currently allows them.
fn publish_original(
    plan: &ExportPlan,
    source: &SourceStamp,
    before: impl FnMut(SourceStage, &Path) -> io::Result<()>,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    #[cfg(unix)]
    {
        publish_original_unix(plan, source, before)
    }
    #[cfg(not(unix))]
    {
        let _ = (plan, before);
        Err(SaveOriginalError::ReplacementUnavailable {
            path: source.path().to_path_buf(),
            blockers: vec![ReplacementBlocker::AtomicReplaceUnverified],
        })
    }
}

#[cfg(unix)]
fn publish_original_unix(
    plan: &ExportPlan,
    source: &SourceStamp,
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
    before(SourceStage::Write, temporary.path()).map_err(|e| fail(SourceStage::Write, e))?;
    temporary
        .write_all(plan.bytes())
        .map_err(|e| fail(SourceStage::Write, e))?;
    before(SourceStage::SyncFile, temporary.path()).map_err(|e| fail(SourceStage::SyncFile, e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| fail(SourceStage::SyncFile, e))?;
    before(SourceStage::Verify, temporary.path()).map_err(|e| fail(SourceStage::Verify, e))?;
    verify_bytes(temporary.as_file_mut(), plan.bytes())
        .map_err(|e| fail(SourceStage::Verify, e))?;
    before(SourceStage::RestorePermissions, temporary.path())
        .map_err(|e| fail(SourceStage::RestorePermissions, e))?;
    restore_permissions(target, temporary.path())
        .map_err(|e| fail(SourceStage::RestorePermissions, e))?;

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
    if let Err(error) = confirm_backup(&backup_path, source) {
        return Err(abort_backup(error));
    }

    if let Err(error) = before(SourceStage::Publish, temporary.path()) {
        return Err(abort_backup(fail(SourceStage::Publish, error)));
    }
    let kept = match temporary.keep() {
        Ok(kept) => kept,
        Err(error) => {
            return Err(abort_backup(fail(SourceStage::Publish, error.error)));
        }
    };
    let (file, tmp_path) = kept;
    drop(file);
    if let Err(error) = fs::rename(&tmp_path, target) {
        let _ = fs::remove_file(&tmp_path);
        return Err(abort_backup(fail(SourceStage::Publish, error)));
    }
    // The target name now refers to the new bytes. Later failures must
    // carry the receipt; the backup still names the previous inode.
    let receipt = SaveOriginalReceipt {
        path: target.to_path_buf(),
        version: plan.version(),
        durability: super::CopyDurability::FileSynced,
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
fn restore_permissions(source: &Path, dest: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let mode = fs::metadata(source)?.mode() & 0o777;
    let mut permissions = fs::metadata(dest)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(dest, permissions)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn unix_matrix_is_honest_about_std_only_capabilities() {
        let caps = original_replacement_capabilities();
        assert_eq!(caps.permissions, MetadataCapability::Preservable);
        assert_eq!(caps.ownership, MetadataCapability::ObservableNotPreservable);
        assert_eq!(caps.acl, MetadataCapability::Unobservable);
        assert_eq!(caps.extended_attributes, MetadataCapability::Unobservable);
        assert_eq!(
            caps.creation_time,
            MetadataCapability::ObservableNotPreservable
        );
        assert_eq!(caps.links, MetadataCapability::ObservableNotPreservable);
        assert_eq!(caps.provenance_marks, MetadataCapability::Unobservable);
    }

    #[test]
    fn metadata_blockers_no_longer_include_the_origin_gap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.docx");
        std::fs::write(&path, b"0123456789").unwrap();
        let source = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let blockers = replacement_blockers(&source);
        assert!(!blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
        assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl)));
        assert!(blockers.len() > 1);
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
        let receipt = publish_original(&plan, &stamp, |_, _| Ok(())).unwrap();
        assert_eq!(receipt.path(), stamp.path());
        assert_eq!(receipt.version(), plan.version());
        assert_eq!(
            receipt.durability(),
            super::super::CopyDurability::FileAndDirectorySynced
        );
        assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o640);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
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
            let error = publish_original(&plan, &stamp, |at, file| {
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
            let error = publish_original(&plan, &stamp, |at, _| {
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
        }
    }
}
