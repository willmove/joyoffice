//! Document storage: verified export copies and the safe-save pipeline.
//!
//! [`save_export_copy`] (C03/T103, T106) publishes a verified export as a
//! NEW file. Existing destinations (including dangling symlinks) are never
//! overwritten. The chosen directory must support hard links. No fallback
//! writes partial bytes to the destination. Session state and source
//! metadata are untouched. A failure after publication carries the receipt;
//! callers must not pretend nothing was written, blindly retry, or mark a
//! newer session saved.
//!
//! W07 adds the original-file pipeline: [`open_source`] captures a bounded
//! read with an unforgeable [`SourceStamp`], [`check_source`] prechecks
//! conflicts (locks, retargeting, identity, exact bytes), and
//! [`save_original`] re-verifies conflicts, then requires
//! `ExportPlan::origin_bytes` to equal the stamped package.
//! [`save_original_with_lock_token`] skips only the JoyWord lock whose
//! contents equal the caller's token; it does not delete that file.
//! Metadata that this build cannot preserve still refuses before any write. The Unix
//! replace strategy itself (temporary file, sync, read-back, backup hard
//! link, rename) lives behind that gate. `save_export_copy` remains the
//! verified fallback when replacement is refused.

mod error;
mod locks;
mod original;
mod source;

pub use error::{
    ConflictDetail, LockKind, LockState, MetadataItem, ReplacementBlocker, SaveOriginalError,
    SourceStage,
};
pub use original::{
    MetadataCapability, OriginalDurability, OriginalReplacementCapabilities, SaveOriginalReceipt,
    original_replacement_capabilities, save_original, save_original_with_lock_token,
};
pub use source::{
    OpenedSource, SourceStamp, check_source, check_source_with_lock_token, open_source,
};

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use joyword_docx::{ExportPlan, ExportTarget};
use joyword_model::DocumentVersion;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveCopyStage {
    Prepare,
    CreateTemporary,
    Write,
    SyncFile,
    Verify,
    Publish,
    Cleanup,
    SyncDirectory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyDurability {
    /// The file and parent directory both accepted sync requests. This does
    /// not claim testing of power loss or the device's write cache.
    FileAndDirectorySynced,
    /// Publication occurred; no parent sync has completed.
    FileSynced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveCopyReceipt {
    path: PathBuf,
    version: DocumentVersion,
    bytes: u64,
    durability: CopyDurability,
}

impl SaveCopyReceipt {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn version(&self) -> DocumentVersion {
        self.version
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn durability(&self) -> CopyDurability {
        self.durability
    }
}

#[derive(Debug)]
pub struct SaveCopyError {
    stage: SaveCopyStage,
    destination: PathBuf,
    source: io::Error,
    published: Option<SaveCopyReceipt>,
}

impl SaveCopyError {
    pub fn stage(&self) -> SaveCopyStage {
        self.stage
    }
    pub fn destination(&self) -> &Path {
        &self.destination
    }
    pub fn io_error(&self) -> &io::Error {
        &self.source
    }
    /// Some means a complete file was published. Failure concerned cleanup
    /// or directory sync; the destination must not be deleted automatically.
    pub fn published(&self) -> Option<&SaveCopyReceipt> {
        self.published.as_ref()
    }
}

impl fmt::Display for SaveCopyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DOCX export copy {} at {:?}: {}",
            self.destination.display(),
            self.stage,
            self.source
        )?;
        if self.published.is_some() {
            f.write_str(" (complete file already published)")?;
        }
        Ok(())
    }
}
impl std::error::Error for SaveCopyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Write/sync/verify a private temporary file in the destination directory,
/// then publish a hard link with no overwrite. The resulting version receipt
/// does not change document origin, backing, undo history or dirty state.
///
/// Directories, files and symlinks already at the target are refused. The
/// extension must agree with the verified plan (docx/dotx/docm/dotm).
/// Windows reports FileSynced because directory fsync is not provided by this
/// implementation. Unix directory-sync errors report an already-published
/// receipt. Remote filesystems and actual power loss require separate tests.
pub fn save_export_copy(
    plan: &ExportPlan,
    destination: &Path,
) -> Result<SaveCopyReceipt, SaveCopyError> {
    save_copy_inner(plan, destination, |_, _| Ok(()))
}

// A private boundary hook allows deterministic failures around real IO, not
// fake in-memory success. Production always executes the real operations.
fn save_copy_inner(
    plan: &ExportPlan,
    destination: &Path,
    mut before: impl FnMut(SaveCopyStage, &Path) -> io::Result<()>,
) -> Result<SaveCopyReceipt, SaveCopyError> {
    let fail = |stage, source, published| SaveCopyError {
        stage,
        destination: destination.to_path_buf(),
        source,
        published,
    };
    before(SaveCopyStage::Prepare, destination)
        .map_err(|e| fail(SaveCopyStage::Prepare, e, None))?;
    let extension = match plan.target() {
        ExportTarget::Docx => "docx",
        ExportTarget::Dotx => "dotx",
        ExportTarget::Docm => "docm",
        ExportTarget::Dotm => "dotm",
    };
    if destination.file_name().is_none()
        || !destination
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case(extension))
    {
        return Err(fail(
            SaveCopyStage::Prepare,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination extension must match export type",
            ),
            None,
        ));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|e| fail(SaveCopyStage::Prepare, e, None))?;
    let target = parent.join(destination.file_name().expect("checked file name"));
    match fs::symlink_metadata(&target) {
        Ok(_) => {
            return Err(fail(
                SaveCopyStage::Prepare,
                io::Error::new(io::ErrorKind::AlreadyExists, "destination exists"),
                None,
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(fail(SaveCopyStage::Prepare, e, None)),
    }
    before(SaveCopyStage::CreateTemporary, &parent)
        .map_err(|e| fail(SaveCopyStage::CreateTemporary, e, None))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".joyword-export-")
        .tempfile_in(&parent)
        .map_err(|e| fail(SaveCopyStage::CreateTemporary, e, None))?;
    before(SaveCopyStage::Write, temporary.path())
        .and_then(|_| temporary.write_all(plan.bytes()))
        .map_err(|e| fail(SaveCopyStage::Write, e, None))?;
    before(SaveCopyStage::SyncFile, temporary.path())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|e| fail(SaveCopyStage::SyncFile, e, None))?;
    before(SaveCopyStage::Verify, temporary.path())
        .and_then(|_| verify_bytes(temporary.as_file_mut(), plan.bytes()))
        .map_err(|e| fail(SaveCopyStage::Verify, e, None))?;
    // The OS operation itself refuses an existing target, including one
    // created after the initial check. No check-then-truncate race.
    before(SaveCopyStage::Publish, temporary.path())
        .and_then(|_| fs::hard_link(temporary.path(), &target))
        .map_err(|e| fail(SaveCopyStage::Publish, e, None))?;
    let receipt = SaveCopyReceipt {
        path: target,
        version: plan.version(),
        bytes: plan.bytes().len() as u64,
        durability: CopyDurability::FileSynced,
    };
    before(SaveCopyStage::Cleanup, temporary.path())
        .and_then(|_| temporary.close())
        .map_err(|e| fail(SaveCopyStage::Cleanup, e, Some(receipt.clone())))?;
    before(SaveCopyStage::SyncDirectory, &parent)
        .and_then(|_| sync_directory(&parent))
        .map_err(|e| fail(SaveCopyStage::SyncDirectory, e, Some(receipt.clone())))?;
    Ok(SaveCopyReceipt {
        durability: if cfg!(unix) {
            CopyDurability::FileAndDirectorySynced
        } else {
            CopyDurability::FileSynced
        },
        ..receipt
    })
}

pub(crate) fn verify_bytes(file: &mut File, expected: &[u8]) -> io::Result<()> {
    if file.metadata()?.len() != expected.len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "temporary file length mismatch",
        ));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = [0u8; 64 * 1024];
    for chunk in expected.chunks(buffer.len()) {
        let read = &mut buffer[..chunk.len()];
        file.read_exact(read)?;
        if read != chunk {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "temporary file byte mismatch",
            ));
        }
    }
    // Also reject appended data if the size changed during read-back.
    if file.read(&mut buffer[..1])? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "temporary file grew during verification",
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn sync_directory(parent: &Path) -> io::Result<()> {
    File::open(parent)?.sync_all()
}
#[cfg(not(unix))]
pub(crate) fn sync_directory(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::DocxSession;
    use joyoffice_opc::Budget;

    fn plan() -> ExportPlan {
        let bytes = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
        DocxSession::open(bytes, &Budget::default())
            .unwrap()
            .export_plan()
            .unwrap()
    }

    #[test]
    fn each_prepublication_failure_leaves_destination_absent_and_cleans_temporary() {
        for stage in [
            SaveCopyStage::Prepare,
            SaveCopyStage::CreateTemporary,
            SaveCopyStage::Write,
            SaveCopyStage::SyncFile,
            SaveCopyStage::Verify,
            SaveCopyStage::Publish,
        ] {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("out.docx");
            let error = save_copy_inner(&plan(), &target, |at, path| {
                if at == stage {
                    if at == SaveCopyStage::Write {
                        fs::write(path, b"partial bytes")?;
                    }
                    Err(io::Error::new(
                        io::ErrorKind::StorageFull,
                        "injected failure",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.stage(), stage);
            assert!(error.published().is_none());
            assert!(!target.exists());
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn readback_rejects_same_length_corruption_and_truncation() {
        let plan = plan();
        for truncate in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("out.docx");
            let error = save_copy_inner(&plan, &target, |at, path| {
                if at == SaveCopyStage::Verify {
                    let mut bytes = plan.bytes().to_vec();
                    if truncate {
                        bytes.truncate(10);
                    } else {
                        bytes[5] ^= 1;
                    }
                    fs::write(path, bytes)?;
                }
                Ok(())
            })
            .unwrap_err();
            assert_eq!(error.stage(), SaveCopyStage::Verify);
            assert!(error.published().is_none());
            assert!(!target.exists());
        }
    }

    #[test]
    fn destination_created_between_check_and_publish_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.docx");
        let error = save_copy_inner(&plan(), &target, |stage, _| {
            if stage == SaveCopyStage::Publish {
                fs::write(&target, b"other writer")?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.stage(), SaveCopyStage::Publish);
        assert_eq!(error.io_error().kind(), io::ErrorKind::AlreadyExists);
        assert!(error.published().is_none());
        assert_eq!(fs::read(&target).unwrap(), b"other writer");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failures_after_publication_carry_complete_versioned_receipt() {
        for stage in [SaveCopyStage::Cleanup, SaveCopyStage::SyncDirectory] {
            let plan = plan();
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("out.docx");
            let error = save_copy_inner(&plan, &target, |at, _| {
                if at == stage {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "injected failure",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.stage(), stage);
            let receipt = error.published().unwrap();
            assert_eq!(receipt.version(), plan.version());
            assert_eq!(receipt.bytes(), plan.bytes().len() as u64);
            assert_eq!(receipt.durability(), CopyDurability::FileSynced);
            assert_eq!(fs::read(receipt.path()).unwrap(), plan.bytes());
        }
    }
}
