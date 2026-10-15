//! W07 integration: bounded source reads, source stamps and every conflict
//! class the task card requires — same-size rewrites with restored times,
//! replacement/recreation, aliases, symlinks/hard links, non-regular files,
//! Office/JoyWord locks (present or undecidable) and read-budget limits.

use std::fs;
use std::path::{Path, PathBuf};

use joyoffice_opc::Budget;
use joyword_shell::storage::{
    ConflictDetail, LockKind, LockState, SaveOriginalError, SourceStage, check_source, open_source,
};

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn write_source(dir: &Path, name: &str) -> (PathBuf, Vec<u8>) {
    let bytes = fixture();
    let path = dir.join(name);
    fs::write(&path, &bytes).unwrap();
    (path, bytes)
}

#[cfg(unix)]
fn modified_of(path: &Path) -> std::time::SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[cfg(unix)]
fn restore_modified(path: &Path, time: std::time::SystemTime) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_times(
        std::fs::FileTimes::new()
            .set_modified(time)
            .set_accessed(time),
    )
    .unwrap();
}

#[test]
fn open_captures_exact_bytes_and_later_checks_stay_green() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx");
    let opened = open_source(&path, &Budget::default()).unwrap();
    assert_eq!(opened.bytes(), bytes.as_slice());
    assert_eq!(opened.stamp().len(), bytes.len() as u64);
    assert_eq!(opened.stamp().path(), fs::canonicalize(&path).unwrap());
    check_source(opened.stamp()).unwrap();
    check_source(opened.stamp()).unwrap();
}

#[test]
fn missing_paths_report_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let error = open_source(&dir.path().join("missing.docx"), &Budget::default()).unwrap_err();
    assert_eq!(error.stage(), Some(SourceStage::Resolve));
    assert_eq!(
        error.io_error().map(|e| e.kind()),
        Some(std::io::ErrorKind::NotFound)
    );
}

#[cfg(unix)]
#[test]
fn directories_and_devices_are_not_regular_files() {
    let dir = tempfile::tempdir().unwrap();
    for path in [dir.path().to_path_buf(), PathBuf::from("/dev/null")] {
        let error = open_source(&path, &Budget::default()).unwrap_err();
        assert!(
            matches!(error, SaveOriginalError::NotARegularFile { .. }),
            "{:?} {:?}",
            path,
            error
        );
    }
}

#[cfg(not(unix))]
#[test]
fn directories_are_not_regular_files() {
    let dir = tempfile::tempdir().unwrap();
    let error = open_source(dir.path(), &Budget::default()).unwrap_err();
    assert!(matches!(error, SaveOriginalError::NotARegularFile { .. }));
}

#[test]
fn deleted_source_is_a_missing_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_source(dir.path(), "report.docx");
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    fs::remove_file(&path).unwrap();
    assert_eq!(
        check_source(&stamp).unwrap_err().conflict(),
        Some(ConflictDetail::Missing)
    );
}

#[cfg(unix)]
#[test]
fn same_size_rewrite_with_restored_mtime_is_not_missed() {
    let dir = tempfile::tempdir().unwrap();
    let (path, mut bytes) = write_source(dir.path(), "report.docx");
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let mtime = modified_of(&path);
    // Flip bytes without changing the length, then put mtime back. ctime
    // cannot be forged, so identity catches it; the exact-byte reread stays
    // behind as defense in depth for coarser file systems.
    for byte in bytes.iter_mut() {
        *byte ^= 1;
    }
    fs::write(&path, &bytes).unwrap();
    restore_modified(&path, mtime);
    assert_eq!(modified_of(&path), mtime);
    let error = check_source(&stamp).unwrap_err();
    assert_eq!(
        error.conflict(),
        Some(ConflictDetail::TimesChanged).or(Some(ConflictDetail::ContentChanged))
    );
}

#[cfg(unix)]
#[test]
fn mtime_only_change_is_still_a_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_source(dir.path(), "report.docx");
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    restore_modified(
        &path,
        modified_of(&path) + std::time::Duration::from_secs(3600),
    );
    assert_eq!(
        check_source(&stamp).unwrap_err().conflict(),
        Some(ConflictDetail::TimesChanged)
    );
}

#[cfg(unix)]
#[test]
fn delete_and_recreate_with_identical_bytes_is_an_inode_change() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx");
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    fs::remove_file(&path).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        check_source(&stamp).unwrap_err().conflict(),
        Some(ConflictDetail::InodeChanged)
    );
}

#[cfg(unix)]
#[test]
fn symlinked_request_binds_the_target_and_retargeting_is_detected() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let (first, bytes) = write_source(dir.path(), "first.docx");
    let second = dir.path().join("second.docx");
    fs::write(&second, &bytes).unwrap();
    let alias = dir.path().join("alias.docx");
    symlink(&first, &alias).unwrap();
    let stamp = open_source(&alias, &Budget::default())
        .unwrap()
        .into_stamp();
    assert_eq!(stamp.path(), fs::canonicalize(&first).unwrap());
    check_source(&stamp).unwrap();
    fs::remove_file(&alias).unwrap();
    symlink(&second, &alias).unwrap();
    assert_eq!(
        check_source(&stamp).unwrap_err().conflict(),
        Some(ConflictDetail::PathResolutionChanged)
    );
}

#[cfg(unix)]
#[test]
fn hard_link_alias_writes_are_detected_through_the_stamped_path() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx");
    let alias = dir.path().join("alias.docx");
    fs::hard_link(&path, &alias).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    // Same inode observed through the other name: same size, rewritten bytes.
    fs::write(&alias, &bytes).unwrap();
    assert_eq!(
        check_source(&stamp).unwrap_err().conflict(),
        Some(ConflictDetail::TimesChanged)
    );
}

#[test]
fn office_owner_files_block_open_and_precheck() {
    let dir = tempfile::tempdir().unwrap();
    let canonical = fs::canonicalize(dir.path()).unwrap();
    let (path, _) = write_source(dir.path(), "report.docx");
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    // Word spells owner files "~$name" and truncates the front of long base
    // names; both spellings must be caught, and neither may be deleted.
    for owner in ["~$report.docx", "~$ort.docx"] {
        let owner_path = dir.path().join(owner);
        fs::write(&owner_path, b"owner").unwrap();
        let error = open_source(&path, &Budget::default()).unwrap_err();
        assert_eq!(
            error.lock(),
            Some((LockKind::OfficeOwnerFile, LockState::Present)),
            "{owner}"
        );
        let error = check_source(&stamp).unwrap_err();
        assert_eq!(
            error.lock(),
            Some((LockKind::OfficeOwnerFile, LockState::Present)),
            "{owner}"
        );
        assert_eq!(error.path(), &canonical.join(owner));
        fs::remove_file(&owner_path).unwrap();
    }
    check_source(&stamp).unwrap();
}

#[test]
fn foreign_owner_files_do_not_block_unrelated_documents() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_source(dir.path(), "report.docx");
    fs::write(dir.path().join("~$ledger.docx"), b"other document's lock").unwrap();
    open_source(&path, &Budget::default()).unwrap();
}

#[test]
fn joyword_lock_files_block_open_and_precheck() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_source(dir.path(), "report.docx");
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let lock = dir.path().join(".report.docx.joyword-lock");
    fs::write(&lock, b"").unwrap();
    let error = open_source(&path, &Budget::default()).unwrap_err();
    assert_eq!(
        error.lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    let error = check_source(&stamp).unwrap_err();
    assert_eq!(
        error.lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    assert_eq!(
        error.path(),
        &fs::canonicalize(dir.path())
            .unwrap()
            .join(".report.docx.joyword-lock")
    );
    fs::remove_file(&lock).unwrap();
    check_source(&stamp).unwrap();
}

#[cfg(unix)]
#[test]
fn undecidable_lock_state_is_reported_as_unknown_conflict() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore<'a>(&'a Path);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            fs::set_permissions(self.0, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_source(dir.path(), "report.docx");
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let guard = Restore(dir.path());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o000)).unwrap();
    let error = check_source(&stamp).unwrap_err();
    drop(guard);
    let (kind, state) = error.lock().expect("expected a lock-state conflict");
    assert_eq!(state, LockState::Unknown);
    assert_eq!(kind, LockKind::OfficeOwnerFile);
}

#[test]
fn oversized_sources_are_refused_under_a_small_budget() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_source(dir.path(), "report.docx");
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
