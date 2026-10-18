//! C09 slice 2: a JoyWord lock is not a conflict for the token that owns it.
//! Other tokens and Office owner files still conflict. `open_source` never
//! uses the exemption, and nothing here deletes a lock file.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::Budget;
use joyword_shell::docx::BoundDocx;
use joyword_shell::storage::{
    LockKind, LockState, check_source, check_source_with_lock_token, open_source, save_original,
    save_original_with_lock_token,
};

const TOKEN: &str = "joyword-lock 1\npid 7\nseq 3\npath /tmp/report.docx\n";

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn plan() -> joyword_docx::ExportPlan {
    joyword_shell::docx::DocxSession::open(&fixture(), &Budget::default())
        .unwrap()
        .export_plan()
        .unwrap()
}

fn lock_file(dir: &std::path::Path) -> PathBuf {
    dir.join(".report.docx.joyword-lock")
}

#[test]
fn own_token_passes_the_lock_gate_and_leaves_the_file_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let bytes = fixture();
    fs::write(&path, &bytes).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let lock = lock_file(dir.path());
    fs::write(&lock, TOKEN).unwrap();

    assert_eq!(
        open_source(&path, &Budget::default()).unwrap_err().lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    assert_eq!(
        check_source(&stamp).unwrap_err().lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    assert_eq!(
        save_original(&plan(), &stamp).unwrap_err().lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    for token in [
        None,
        Some(""),
        Some("other-session\n"),
        Some(&TOKEN[..TOKEN.len() - 1]),
    ] {
        assert_eq!(
            check_source_with_lock_token(&stamp, token)
                .unwrap_err()
                .lock(),
            Some((LockKind::JoyWordLockFile, LockState::Present)),
            "{token:?}"
        );
    }
    fs::write(&lock, format!("{TOKEN}x")).unwrap();
    assert_eq!(
        check_source_with_lock_token(&stamp, Some(TOKEN))
            .unwrap_err()
            .lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    fs::write(&lock, TOKEN).unwrap();

    let mut same_length = TOKEN.as_bytes().to_vec();
    same_length[0] = b'x';
    fs::write(&lock, &same_length).unwrap();
    assert_eq!(
        check_source_with_lock_token(&stamp, Some(TOKEN))
            .unwrap_err()
            .lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    fs::write(&lock, TOKEN).unwrap();

    check_source_with_lock_token(&stamp, Some(TOKEN)).unwrap();
    let saved = save_original_with_lock_token(&plan(), &stamp, Some(TOKEN)).unwrap_err();
    assert!(saved.lock().is_none());
    assert!(saved.blockers().is_some());
    assert!(!saved.is_origin_mismatch());
    let foreign = {
        let raw = fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/docx/DOCX-MIN-002.docx"),
        )
        .unwrap();
        joyword_shell::docx::DocxSession::open(&raw, &Budget::default())
            .unwrap()
            .export_plan()
            .unwrap()
    };
    let mismatch = save_original_with_lock_token(&foreign, &stamp, Some(TOKEN)).unwrap_err();
    assert!(mismatch.is_origin_mismatch());
    assert!(mismatch.lock().is_none());
    assert_eq!(fs::read(&lock).unwrap(), TOKEN.as_bytes());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn an_office_owner_file_still_conflicts_next_to_an_owned_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, fixture()).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let lock = lock_file(dir.path());
    let office = dir.path().join("~$report.docx");
    fs::write(&lock, TOKEN).unwrap();
    fs::write(&office, b"owner").unwrap();
    let error = save_original_with_lock_token(&plan(), &stamp, Some(TOKEN)).unwrap_err();
    assert_eq!(
        error.lock(),
        Some((LockKind::OfficeOwnerFile, LockState::Present))
    );
    assert_eq!(fs::read(&lock).unwrap(), TOKEN.as_bytes());
    assert_eq!(fs::read(&office).unwrap(), b"owner");
}

#[test]
fn a_directory_using_the_lock_name_is_not_owned_and_is_not_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, fixture()).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let lock = lock_file(dir.path());
    fs::create_dir(&lock).unwrap();
    assert_eq!(
        check_source_with_lock_token(&stamp, Some(TOKEN))
            .unwrap_err()
            .lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    assert!(lock.is_dir());
}

#[test]
fn the_open_session_token_does_not_make_a_second_open_succeed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let bytes = fixture();
    fs::write(&path, &bytes).unwrap();
    let bound = BoundDocx::open_default(&path).unwrap();
    let token = fs::read_to_string(bound.lock_path()).unwrap();
    assert_eq!(
        open_source(bound.source_path(), &Budget::default())
            .unwrap_err()
            .lock(),
        Some((LockKind::JoyWordLockFile, LockState::Present))
    );
    check_source_with_lock_token(bound.stamp(), Some(&token)).unwrap();
    let error = save_original_with_lock_token(
        &bound.session().export_plan().unwrap(),
        bound.stamp(),
        Some(&token),
    )
    .unwrap_err();
    assert!(error.lock().is_none());
    assert!(error.blockers().is_some());
    assert_eq!(fs::read(bound.lock_path()).unwrap(), token.as_bytes());
    assert_eq!(fs::read(bound.source_path()).unwrap(), bytes);
}
