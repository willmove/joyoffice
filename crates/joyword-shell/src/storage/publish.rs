//! The Unix original-replacement write path (W07 follow-up, ARCHITECTURE §5).
//!
//! Order: private temporary file in the source's directory → write the plan
//! → restore owner/group, permission bits and (macOS) creation time → sync →
//! byte and metadata read-back → full conflict re-verification → hard-link
//! the current source object under a private backup name → prove that link
//! names the verified object with its exact bytes → `rename` the temporary
//! file over the source path → bounded read-back of the published object,
//! which alone yields the receipt and refreshed stamp → remove the backup →
//! sync the directory.
//!
//! Before the rename every failure leaves the verified original in place and
//! removes the temporary file and backup link. After it nothing is rolled
//! back or retried: an unconfirmed publication keeps the backup, and a
//! confirmed one carries its receipt. One race is not closed: another
//! writer replacing the source between the last object check and the
//! `rename(2)` call is overwritten (the backup still holds the verified
//! original). Closing it needs `renamex_np(RENAME_SWAP)` or
//! `renameat2(RENAME_EXCHANGE)`, which std does not expose.
//!
//! Callers decide whether replacement is allowed at all (origin binding and
//! the metadata gate in `original.rs`); this module only executes it.

use std::fs::{self, File, Permissions};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use joyword_docx::ExportPlan;

use super::error::{
    ConflictDetail, MetadataItem, Publication, ReplaceStage, ReplacementBlocker, SaveOriginalError,
};
use super::original::{OriginalDurability, SaveOriginalReceipt};
use super::source::{SourceIdentity, SourceStamp, check_source, confirm_published};
use super::{locks, sync_directory, verify};

/// Metadata the replacement must carry, taken from the verified source.
#[derive(Clone, Debug)]
pub(crate) struct Expected {
    mode: u32,
    uid: u32,
    gid: u32,
    #[cfg(target_os = "macos")]
    created: Option<std::time::SystemTime>,
}

impl Expected {
    /// Owner, group and permission bits come from the stamp's identity
    /// (verified equal to the disk by the caller's `check_source`); creation
    /// time from `metadata` of the same object. Any later change to them
    /// moves the status-change time, which the recheck before the backup
    /// compares against the stamp.
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
    pub(crate) fn of(source: &SourceStamp, metadata: &fs::Metadata) -> Self {
        let unix = source.identity().unix();
        Self {
            mode: unix.mode() & 0o7777,
            uid: unix.uid(),
            gid: unix.gid(),
            #[cfg(target_os = "macos")]
            created: metadata.created().ok(),
        }
    }

    /// First metadata item `metadata` does not carry as expected.
    fn mismatch(&self, metadata: &fs::Metadata) -> Option<MetadataItem> {
        if metadata.uid() != self.uid || metadata.gid() != self.gid {
            return Some(MetadataItem::Ownership);
        }
        if metadata.mode() & 0o7777 != self.mode {
            return Some(MetadataItem::Permissions);
        }
        if metadata.nlink() != 1 {
            return Some(MetadataItem::Links);
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::darwin::fs::MetadataExt as _;
            if metadata.st_flags() != 0 {
                return Some(MetadataItem::FileFlags);
            }
            if self.created.is_some() && metadata.created().ok() != self.created {
                return Some(MetadataItem::CreationTime);
            }
        }
        None
    }
}

/// Replace the source with the plan. See the module documentation.
pub(crate) fn replace(
    plan: &ExportPlan,
    source: &SourceStamp,
    expected: &Expected,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    replace_inner(plan, source, expected, |_, _| Ok(()))
}

// Same private boundary hook as the C03 copy path: deterministic failures and
// races around real IO for tests. Production always runs the real steps.
pub(crate) fn replace_inner(
    plan: &ExportPlan,
    source: &SourceStamp,
    expected: &Expected,
    mut before: impl FnMut(ReplaceStage, &Path) -> io::Result<()>,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    let target = source.path();
    let fail = |stage, error, publication, retained_backup| SaveOriginalError::Replace {
        stage,
        path: target.to_path_buf(),
        source: error,
        publication,
        retained_backup,
    };
    // Failure before anything was published, with no backup to report.
    let early = |stage| move |error| fail(stage, error, Publication::NotPublished, None);
    let refuse = |item| SaveOriginalError::ReplacementUnavailable {
        path: target.to_path_buf(),
        blockers: vec![ReplacementBlocker::UnpreservableMetadata(item)],
    };
    let parent = match target.parent() {
        Some(parent) => parent.to_path_buf(),
        None => {
            return Err(early(ReplaceStage::CreateTemporary)(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source has no directory",
            )));
        }
    };

    // Every early return below drops `temporary`, which removes it.
    before(ReplaceStage::CreateTemporary, &parent).map_err(early(ReplaceStage::CreateTemporary))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".joyword-save-")
        .tempfile_in(&parent)
        .map_err(early(ReplaceStage::CreateTemporary))?;

    before(ReplaceStage::Write, temporary.path())
        .and_then(|_| io::Write::write_all(temporary.as_file_mut(), plan.bytes()))
        .map_err(early(ReplaceStage::Write))?;

    before(ReplaceStage::RestoreMetadata, temporary.path())
        .map_err(early(ReplaceStage::RestoreMetadata))?;
    let file = temporary.as_file();
    let created = file
        .metadata()
        .map_err(early(ReplaceStage::RestoreMetadata))?;
    if created.uid() != expected.uid || created.gid() != expected.gid {
        // Giving a file away is a privilege; being refused it means the
        // replacement would change the owner, which is not a degraded save.
        std::os::unix::fs::fchown(file, Some(expected.uid), Some(expected.gid))
            .map_err(|_| refuse(MetadataItem::Ownership))?;
    }
    // After any ownership change, which may clear set-id bits.
    file.set_permissions(Permissions::from_mode(expected.mode))
        .map_err(early(ReplaceStage::RestoreMetadata))?;
    #[cfg(target_os = "macos")]
    if let Some(created) = expected.created {
        use std::os::darwin::fs::FileTimesExt as _;
        file.set_times(fs::FileTimes::new().set_created(created))
            .map_err(early(ReplaceStage::RestoreMetadata))?;
    }

    before(ReplaceStage::SyncFile, temporary.path())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(early(ReplaceStage::SyncFile))?;

    before(ReplaceStage::VerifyTemporary, temporary.path())
        .and_then(|_| verify(temporary.as_file_mut(), plan.bytes()))
        .map_err(early(ReplaceStage::VerifyTemporary))?;
    let written = temporary
        .as_file()
        .metadata()
        .map_err(early(ReplaceStage::VerifyTemporary))?;
    if let Some(item) = expected.mismatch(&written) {
        return Err(refuse(item));
    }
    let object = (written.dev(), written.ino());
    let at_path =
        fs::symlink_metadata(temporary.path()).map_err(early(ReplaceStage::VerifyTemporary))?;
    if (at_path.dev(), at_path.ino()) != object {
        return Err(early(ReplaceStage::VerifyTemporary)(io::Error::new(
            io::ErrorKind::InvalidData,
            "temporary path no longer names the written file",
        )));
    }
    // A directory default ACL or a quarantining process could have given
    // the new file metadata the original does not have.
    #[cfg(target_os = "macos")]
    match super::observe::observe(temporary.path()) {
        Ok(extended) => {
            if let Some(item) = extended.present_items().first() {
                return Err(refuse(*item));
            }
        }
        Err(e) => {
            return Err(early(ReplaceStage::VerifyTemporary)(e));
        }
    }

    // Everything above can take time; verify the source again in full.
    before(ReplaceStage::Recheck, target).map_err(early(ReplaceStage::Recheck))?;
    check_source(source)?;

    before(ReplaceStage::Backup, target).map_err(early(ReplaceStage::Backup))?;
    // Cleanup is manual from here: the link must survive any failure after
    // the rename, and a link that cannot be removed must be reported.
    let backup = tempfile::Builder::new()
        .prefix(".joyword-backup-")
        .disable_cleanup(true)
        .make_in(&parent, |path| fs::hard_link(target, path))
        .map_err(early(ReplaceStage::Backup))?
        .path()
        .to_path_buf();
    // Removes the backup before publication; names it if it stayed behind.
    let abandon = || fs::remove_file(&backup).err().map(|_| backup.clone());
    let refused = |error: SaveOriginalError, stage| match abandon() {
        None => error,
        // The refusal stands, but the link to the original stayed.
        Some(retained) => fail(
            stage,
            io::Error::other(error.to_string()),
            Publication::NotPublished,
            Some(retained),
        ),
    };

    if let Err(e) = before(ReplaceStage::VerifyBackup, &backup) {
        return Err(fail(
            ReplaceStage::VerifyBackup,
            e,
            Publication::NotPublished,
            abandon(),
        ));
    }
    match verify_backup(source, &backup) {
        Ok(()) => {}
        Err(BackupFailure::Io(e)) => {
            return Err(fail(
                ReplaceStage::VerifyBackup,
                e,
                Publication::NotPublished,
                abandon(),
            ));
        }
        Err(BackupFailure::Refused(error)) => {
            return Err(refused(error, ReplaceStage::VerifyBackup));
        }
    }

    // Last object check before the rename: the path must still name the
    // verified original. What happens between this check and rename(2)
    // itself is the race the module documentation describes.
    if let Err(e) = before(ReplaceStage::Publish, temporary.path()) {
        return Err(fail(
            ReplaceStage::Publish,
            e,
            Publication::NotPublished,
            abandon(),
        ));
    }
    match fs::symlink_metadata(target) {
        Ok(current) if (current.dev(), current.ino()) == source.identity().unix().object() => {}
        Ok(_) => {
            return Err(refused(
                SaveOriginalError::conflict_error(target, ConflictDetail::InodeChanged),
                ReplaceStage::Publish,
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(refused(
                SaveOriginalError::conflict_error(target, ConflictDetail::Missing),
                ReplaceStage::Publish,
            ));
        }
        Err(e) => {
            return Err(fail(
                ReplaceStage::Publish,
                e,
                Publication::NotPublished,
                abandon(),
            ));
        }
    }
    if let Err(e) = temporary.persist(target) {
        // `e.file` is dropped here, removing the unpublished temporary file.
        return Err(fail(
            ReplaceStage::Publish,
            e.error,
            Publication::NotPublished,
            abandon(),
        ));
    }

    // The rename has happened. The backup stays until the published object
    // is confirmed by reading it back.
    let confirmed = before(ReplaceStage::Confirm, target)
        .and_then(|_| confirm_published(source, object, plan.bytes()))
        .and_then(|(stamp, metadata)| match expected.mismatch(&metadata) {
            None => Ok(stamp),
            Some(item) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("published file does not carry the expected {item:?}"),
            )),
        });
    let stamp = match confirmed {
        Ok(stamp) => stamp,
        Err(e) => {
            return Err(fail(
                ReplaceStage::Confirm,
                e,
                Publication::Unconfirmed,
                Some(backup),
            ));
        }
    };
    let receipt = SaveOriginalReceipt::new(
        target.to_path_buf(),
        plan.version(),
        OriginalDurability::FileSynced,
        stamp,
    );

    if let Err(e) =
        before(ReplaceStage::RemoveBackup, &backup).and_then(|_| fs::remove_file(&backup))
    {
        let retained = fs::symlink_metadata(&backup).is_ok().then_some(backup);
        return Err(fail(
            ReplaceStage::RemoveBackup,
            e,
            Publication::Confirmed(Box::new(receipt)),
            retained,
        ));
    }

    before(ReplaceStage::SyncDirectory, &parent)
        .and_then(|_| sync_directory(&parent))
        .map_err(|e| {
            fail(
                ReplaceStage::SyncDirectory,
                e,
                Publication::Confirmed(Box::new(receipt.clone())),
                None,
            )
        })?;
    Ok(receipt.with_durability(OriginalDurability::FileAndDirectorySynced))
}

enum BackupFailure {
    Io(io::Error),
    Refused(SaveOriginalError),
}

/// Prove that `backup` names the object the stamp verified, with its exact
/// bytes, and that no lock appeared meanwhile.
fn verify_backup(source: &SourceStamp, backup: &Path) -> Result<(), BackupFailure> {
    let file_name = source.path().file_name().unwrap_or_default();
    locks::check_directories(source.lock_directories(), file_name)
        .map_err(BackupFailure::Refused)?;
    let file = File::open(backup).map_err(BackupFailure::Io)?;
    let linked = SourceIdentity::capture(&file.metadata().map_err(BackupFailure::Io)?);
    if let Some(detail) = source.identity().linked_difference(&linked) {
        return Err(BackupFailure::Refused(SaveOriginalError::conflict_error(
            source.path(),
            detail,
        )));
    }
    let mut bytes = Vec::with_capacity(source.bytes().len());
    (&file)
        .take(source.budget_limit().saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(BackupFailure::Io)?;
    if bytes != source.bytes() {
        return Err(BackupFailure::Refused(SaveOriginalError::conflict_error(
            source.path(),
            ConflictDetail::ContentChanged,
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::DocxSession;
    use crate::storage::{OpenedSource, open_source};
    use joyoffice_opc::Budget;
    use joyword_engine::{Command, Transaction};
    use joyword_model::{Bias, Position, ScalarOffset};
    use std::path::PathBuf;

    const PRE_PUBLICATION: [ReplaceStage; 9] = [
        ReplaceStage::CreateTemporary,
        ReplaceStage::Write,
        ReplaceStage::RestoreMetadata,
        ReplaceStage::SyncFile,
        ReplaceStage::VerifyTemporary,
        ReplaceStage::Recheck,
        ReplaceStage::Backup,
        ReplaceStage::VerifyBackup,
        ReplaceStage::Publish,
    ];

    struct Fixture {
        dir: tempfile::TempDir,
        path: PathBuf,
        original: Vec<u8>,
        stamp: SourceStamp,
        plan: ExportPlan,
        expected: Expected,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = fs::canonicalize(dir.path()).unwrap().join("report.docx");
            let original = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
            )
            .unwrap();
            fs::write(&path, &original).unwrap();
            fs::set_permissions(&path, Permissions::from_mode(0o640)).unwrap();
            let opened = open_source(&path, &Budget::default()).unwrap();
            let plan = edited_plan(&opened);
            let stamp = opened.into_stamp();
            let expected = Expected::of(&stamp, &fs::metadata(&path).unwrap());
            Self {
                dir,
                path,
                original,
                stamp,
                plan,
                expected,
            }
        }

        fn run(
            &self,
            before: impl FnMut(ReplaceStage, &Path) -> io::Result<()>,
        ) -> Result<SaveOriginalReceipt, SaveOriginalError> {
            replace_inner(&self.plan, &self.stamp, &self.expected, before)
        }

        fn entries(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        /// Same object, bytes, mode, mtime and link count as when stamped,
        /// and nothing else left in the directory.
        fn assert_original_kept(&self) {
            assert_eq!(fs::read(&self.path).unwrap(), self.original);
            let now = fs::symlink_metadata(&self.path).unwrap();
            let then = self.stamp.identity().unix();
            assert_eq!((now.dev(), now.ino()), then.object());
            assert_eq!(now.mode(), then.mode());
            assert_eq!(now.nlink(), 1);
            assert_eq!((now.mtime(), now.mtime_nsec()), then.mtime());
            assert_eq!(self.entries(), ["report.docx"]);
        }
    }

    fn edited_plan(opened: &OpenedSource) -> ExportPlan {
        let mut session = DocxSession::open(opened.bytes(), &Budget::default()).unwrap();
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
                    text: "原件替换 ".into(),
                }],
            ))
            .unwrap();
        session.export_plan().unwrap()
    }

    fn inject(stage: ReplaceStage) -> impl FnMut(ReplaceStage, &Path) -> io::Result<()> {
        move |at, path| {
            if at != stage {
                return Ok(());
            }
            if at == ReplaceStage::Write {
                fs::write(path, b"partial bytes")?;
            }
            Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "injected failure",
            ))
        }
    }

    /// Another writer's file, renamed over `path` (a different object).
    fn replace_with_other(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let other = path.with_file_name("other-writer.tmp");
        fs::write(&other, bytes)?;
        fs::rename(other, path)
    }

    #[test]
    fn success_publishes_reads_back_and_leaves_nothing_behind() {
        let fixture = Fixture::new();
        let receipt = fixture.run(|_, _| Ok(())).unwrap();
        assert_eq!(receipt.path(), fixture.path);
        assert_eq!(receipt.version(), fixture.plan.version());
        assert_eq!(receipt.bytes(), fixture.plan.bytes().len() as u64);
        assert_eq!(
            receipt.durability(),
            OriginalDurability::FileAndDirectorySynced
        );
        assert_eq!(fs::read(&fixture.path).unwrap(), fixture.plan.bytes());
        assert_eq!(fixture.entries(), ["report.docx"]);
        let now = fs::metadata(&fixture.path).unwrap();
        assert_eq!(now.mode() & 0o7777, 0o640);
        assert_eq!(now.nlink(), 1);
        assert_ne!(
            (now.dev(), now.ino()),
            fixture.stamp.identity().unix().object()
        );
        // The refreshed stamp describes the file on disk; the old one does not.
        check_source(receipt.stamp()).unwrap();
        assert_eq!(receipt.stamp().bytes(), fs::read(&fixture.path).unwrap());
        assert!(
            check_source(&fixture.stamp)
                .unwrap_err()
                .conflict()
                .is_some()
        );
    }

    #[test]
    fn every_failure_before_publication_keeps_the_original() {
        for stage in PRE_PUBLICATION {
            let fixture = Fixture::new();
            let error = fixture.run(inject(stage)).unwrap_err();
            assert_eq!(error.replace_stage(), Some(stage), "{error}");
            assert!(matches!(
                error.publication(),
                Some(Publication::NotPublished)
            ));
            assert_eq!(error.retained_backup(), None);
            assert!(error.published().is_none());
            fixture.assert_original_kept();
            // A backup link was created and removed from VerifyBackup on,
            // which may advance the status-change time; reopening recovers.
            match check_source(&fixture.stamp) {
                Ok(()) => {}
                Err(e) => {
                    assert!(stage >= ReplaceStage::VerifyBackup, "{stage:?}: {e}");
                    assert_eq!(e.conflict(), Some(ConflictDetail::TimesChanged));
                    let reopened = open_source(&fixture.path, &Budget::default()).unwrap();
                    assert!(fixture.plan.originates_from(reopened.bytes()));
                }
            }
        }
    }

    #[test]
    fn readback_corruption_of_the_temporary_file_is_never_published() {
        for truncate in [false, true] {
            let fixture = Fixture::new();
            let plan = fixture.plan.bytes().to_vec();
            let error = fixture
                .run(|at, path| {
                    if at == ReplaceStage::VerifyTemporary {
                        let mut bytes = plan.clone();
                        if truncate {
                            bytes.truncate(10);
                        } else {
                            bytes[7] ^= 1;
                        }
                        // Rewrite in place so the object stays the same.
                        let mut file = File::options().write(true).open(path)?;
                        file.set_len(0)?;
                        io::Write::write_all(&mut file, &bytes)?;
                    }
                    Ok(())
                })
                .unwrap_err();
            assert_eq!(error.replace_stage(), Some(ReplaceStage::VerifyTemporary));
            assert_eq!(
                error.io_error().map(io::Error::kind),
                Some(io::ErrorKind::InvalidData)
            );
            fixture.assert_original_kept();
        }
    }

    #[test]
    fn metadata_drift_on_the_temporary_is_refused_before_publication() {
        let fixture = Fixture::new();
        let error = fixture
            .run(|at, path| {
                if at == ReplaceStage::VerifyTemporary {
                    fs::set_permissions(path, Permissions::from_mode(0o666))?;
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
        fixture.assert_original_kept();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn an_attribute_gained_by_the_temporary_is_refused_before_publication() {
        let fixture = Fixture::new();
        let error = fixture
            .run(|at, path| {
                if at == ReplaceStage::VerifyTemporary {
                    let status = std::process::Command::new("/usr/bin/xattr")
                        .args(["-w", "com.example.gained", "1"])
                        .arg(path)
                        .status()?;
                    assert!(status.success());
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(
            error.blockers(),
            Some(
                &[ReplacementBlocker::UnpreservableMetadata(
                    MetadataItem::ExtendedAttributes
                )][..]
            )
        );
        fixture.assert_original_kept();
    }

    #[test]
    fn an_owner_the_process_cannot_give_is_refused() {
        let mut fixture = Fixture::new();
        let ours = fs::metadata(&fixture.path).unwrap().uid();
        if ours == 0 {
            // Root may give files away, so the refusal cannot be provoked.
            return;
        }
        fixture.expected.uid = ours.wrapping_add(1);
        let error = fixture.run(|_, _| Ok(())).unwrap_err();
        assert_eq!(
            error.blockers(),
            Some(
                &[ReplacementBlocker::UnpreservableMetadata(
                    MetadataItem::Ownership
                )][..]
            )
        );
        fixture.assert_original_kept();
    }

    #[test]
    fn a_rewrite_before_the_recheck_is_a_conflict_and_the_rewrite_stays() {
        let fixture = Fixture::new();
        let other: Vec<u8> = fixture.original.iter().map(|b| b ^ 0x55).collect();
        let error = fixture
            .run(|at, _| {
                if at == ReplaceStage::Recheck {
                    fs::write(&fixture.path, &other)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(error.conflict().is_some(), "{error}");
        assert_eq!(fs::read(&fixture.path).unwrap(), other);
        assert_eq!(fixture.entries(), ["report.docx"]);
    }

    #[test]
    fn a_replacement_racing_the_backup_link_is_caught_by_the_backup() {
        let fixture = Fixture::new();
        let error = fixture
            .run(|at, _| {
                if at == ReplaceStage::Backup {
                    // Identical bytes, different object.
                    replace_with_other(&fixture.path, &fixture.original)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.conflict(), Some(ConflictDetail::InodeChanged));
        assert_eq!(fs::read(&fixture.path).unwrap(), fixture.original);
        assert_ne!(
            {
                let m = fs::metadata(&fixture.path).unwrap();
                (m.dev(), m.ino())
            },
            fixture.stamp.identity().unix().object()
        );
        assert_eq!(fixture.entries(), ["report.docx"]);
    }

    #[test]
    fn an_append_after_the_backup_link_is_caught() {
        let fixture = Fixture::new();
        let error = fixture
            .run(|at, _| {
                if at == ReplaceStage::VerifyBackup {
                    let mut file = File::options().append(true).open(&fixture.path)?;
                    io::Write::write_all(&mut file, b"appended")?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(
            matches!(
                error.conflict(),
                Some(ConflictDetail::SizeChanged | ConflictDetail::TimesChanged)
            ),
            "{error}"
        );
        assert!(fs::read(&fixture.path).unwrap().ends_with(b"appended"));
        assert_eq!(fixture.entries(), ["report.docx"]);
    }

    #[test]
    fn a_replacement_just_before_the_rename_is_caught() {
        let fixture = Fixture::new();
        let error = fixture
            .run(|at, _| {
                if at == ReplaceStage::Publish {
                    replace_with_other(&fixture.path, b"another writer")?;
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.conflict(), Some(ConflictDetail::InodeChanged));
        assert_eq!(fs::read(&fixture.path).unwrap(), b"another writer");
        assert_eq!(fixture.entries(), ["report.docx"]);
    }

    #[test]
    fn a_lock_appearing_during_the_save_blocks_publication_and_stays() {
        for (stage, lock) in [
            (ReplaceStage::Recheck, "~$report.docx"),
            (ReplaceStage::VerifyBackup, ".report.docx.joyword-lock"),
        ] {
            let fixture = Fixture::new();
            let lock_path = fixture.path.with_file_name(lock);
            let error = fixture
                .run(|at, _| {
                    if at == stage {
                        fs::write(&lock_path, b"owner")?;
                    }
                    Ok(())
                })
                .unwrap_err();
            assert!(error.lock().is_some(), "{stage:?}: {error}");
            assert_eq!(fs::read(&fixture.path).unwrap(), fixture.original);
            assert_eq!(fs::read(&lock_path).unwrap(), b"owner");
            let mut expected = vec![lock.to_owned(), "report.docx".to_owned()];
            expected.sort();
            assert_eq!(fixture.entries(), expected);
        }
    }

    #[test]
    fn an_unconfirmed_publication_keeps_the_backup_and_issues_no_receipt() {
        let fixture = Fixture::new();
        let error = fixture.run(inject(ReplaceStage::Confirm)).unwrap_err();
        assert_eq!(error.replace_stage(), Some(ReplaceStage::Confirm));
        assert!(matches!(
            error.publication(),
            Some(Publication::Unconfirmed)
        ));
        assert!(error.published().is_none());
        let backup = error.retained_backup().unwrap();
        assert_eq!(fs::read(backup).unwrap(), fixture.original);
        assert_eq!(fs::read(&fixture.path).unwrap(), fixture.plan.bytes());
        assert_eq!(fixture.entries().len(), 2);
    }

    #[test]
    fn receipt_and_stamp_come_only_from_the_published_object() {
        // Same object, other bytes: not confirmed.
        let fixture = Fixture::new();
        let error = fixture
            .run(|at, path| {
                if at == ReplaceStage::Confirm {
                    let mut bytes = fs::read(path)?;
                    bytes[3] ^= 1;
                    let mut file = File::options().write(true).open(path)?;
                    io::Write::write_all(&mut file, &bytes)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error.publication(),
            Some(Publication::Unconfirmed)
        ));
        assert!(error.published().is_none());
        assert_eq!(
            fs::read(error.retained_backup().unwrap()).unwrap(),
            fixture.original
        );

        // The plan's exact bytes under another object: not confirmed either.
        let fixture = Fixture::new();
        let plan = fixture.plan.bytes().to_vec();
        let error = fixture
            .run(|at, path| {
                if at == ReplaceStage::Confirm {
                    replace_with_other(path, &plan)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error.publication(),
            Some(Publication::Unconfirmed)
        ));
        assert!(error.published().is_none());
        assert_eq!(fs::read(&fixture.path).unwrap(), plan);
    }

    #[test]
    fn failures_after_confirmation_carry_the_receipt() {
        for stage in [ReplaceStage::RemoveBackup, ReplaceStage::SyncDirectory] {
            let fixture = Fixture::new();
            let error = fixture.run(inject(stage)).unwrap_err();
            assert_eq!(error.replace_stage(), Some(stage));
            let receipt = error.published().expect("confirmed receipt");
            assert_eq!(receipt.version(), fixture.plan.version());
            assert_eq!(receipt.durability(), OriginalDurability::FileSynced);
            assert_eq!(fs::read(receipt.path()).unwrap(), fixture.plan.bytes());
            check_source(receipt.stamp()).unwrap();
            match stage {
                ReplaceStage::RemoveBackup => {
                    let backup = error.retained_backup().unwrap();
                    assert_eq!(fs::read(backup).unwrap(), fixture.original);
                }
                _ => {
                    assert_eq!(error.retained_backup(), None);
                    assert_eq!(fixture.entries(), ["report.docx"]);
                }
            }
        }
    }
}
