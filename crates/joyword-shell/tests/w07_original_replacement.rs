//! W07 follow-up: real original replacement through the public API on the
//! verified platform (macOS, local APFS). A plan bound to the stamped package
//! replaces the file; the receipt and refreshed stamp come from reading the
//! published file back; permission bits, owner, creation time and symbolic
//! links survive; the old stamp and the pre-save session no longer match.
//! Other platforms refuse before writing (see w07_save_original_rejection).
#![cfg(target_os = "macos")]

use std::fs;
use std::os::darwin::fs::MetadataExt as _;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use joyoffice_opc::Budget;
use joyword_docx::{ExportMode, ExportPlan};
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::{
    CopyDurability, OpenedSource, ReplacementBlocker, check_source, open_source, save_original,
};

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn insert(session: &mut DocxSession, text: &str) {
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
                text: text.into(),
            }],
        ))
        .unwrap();
}

/// The C08 flow: the session is opened from the very bytes the stamp read.
fn open_session(opened: &OpenedSource) -> DocxSession {
    DocxSession::open(opened.bytes(), &Budget::default()).unwrap()
}

fn first_paragraph(bytes: &[u8]) -> String {
    let session = DocxSession::open(bytes, &Budget::default()).unwrap();
    let document = session.session().document();
    document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new())
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_bound_plan_replaces_the_original_and_is_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = fs::canonicalize(dir.path()).unwrap().join("report.docx");
    fs::write(&path, fixture()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let before = fs::metadata(&path).unwrap();

    let opened = open_source(&path, &Budget::default()).unwrap();
    let mut session = open_session(&opened);
    insert(&mut session, "原件替换🙂 ");
    let plan = session.export_plan().unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);

    let receipt = save_original(&plan, opened.stamp()).unwrap();
    assert_eq!(receipt.path(), path);
    assert_eq!(receipt.version(), session.session().version());
    assert_eq!(receipt.bytes(), plan.bytes().len() as u64);
    assert_eq!(receipt.durability(), CopyDurability::FileAndDirectorySynced);

    // The file holds the plan and re-imports with the edit.
    let saved = fs::read(&path).unwrap();
    assert_eq!(saved, plan.bytes());
    assert!(first_paragraph(&saved).starts_with("原件替换🙂 "));
    assert_eq!(entries(dir.path()), ["report.docx"]);

    // A new object carrying the original's owner, mode and creation time.
    let after = fs::metadata(&path).unwrap();
    assert_ne!(after.ino(), before.ino());
    assert_eq!((after.uid(), after.gid()), (before.uid(), before.gid()));
    assert_eq!(after.mode() & 0o7777, 0o640);
    assert_eq!(after.st_birthtime(), before.st_birthtime());
    assert_eq!(after.st_birthtime_nsec(), before.st_birthtime_nsec());
    assert_eq!(after.nlink(), 1);

    // Stamps: the refreshed one matches the disk, the old one does not.
    check_source(receipt.stamp()).unwrap();
    assert_eq!(receipt.stamp().len(), saved.len() as u64);
    assert!(
        check_source(opened.stamp())
            .unwrap_err()
            .conflict()
            .is_some()
    );
    assert!(
        save_original(&plan, opened.stamp())
            .unwrap_err()
            .conflict()
            .is_some()
    );
}

#[test]
fn an_unedited_session_rewrites_identical_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let bytes = fixture();
    fs::write(&path, &bytes).unwrap();
    let opened = open_source(&path, &Budget::default()).unwrap();
    let plan = open_session(&opened).export_plan().unwrap();
    assert_eq!(plan.mode(), ExportMode::OriginalPackage);
    let receipt = save_original(&plan, opened.stamp()).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    check_source(receipt.stamp()).unwrap();
}

#[test]
fn a_symlinked_request_path_writes_through_the_link() {
    let dir = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(dir.path()).unwrap();
    fs::create_dir(root.join("real")).unwrap();
    let target = root.join("real/report.docx");
    fs::write(&target, fixture()).unwrap();
    let link = root.join("alias.docx");
    std::os::unix::fs::symlink("real/report.docx", &link).unwrap();

    let opened = open_source(&link, &Budget::default()).unwrap();
    let mut session = open_session(&opened);
    insert(&mut session, "via link ");
    let plan = session.export_plan().unwrap();
    let receipt = save_original(&plan, opened.stamp()).unwrap();

    assert_eq!(receipt.path(), target);
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(fs::read_link(&link).unwrap(), Path::new("real/report.docx"));
    assert_eq!(fs::read(&link).unwrap(), plan.bytes());
    assert_eq!(entries(&root), ["alias.docx", "real"]);
    assert_eq!(entries(&root.join("real")), ["report.docx"]);
    check_source(receipt.stamp()).unwrap();
}

#[test]
fn the_next_save_needs_a_session_rebased_on_the_saved_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, fixture()).unwrap();
    let opened = open_source(&path, &Budget::default()).unwrap();
    let mut session = open_session(&opened);
    insert(&mut session, "first ");
    let first = session.export_plan().unwrap();
    let receipt = save_original(&first, opened.stamp()).unwrap();

    // The same session still exports from the pre-save package.
    insert(&mut session, "second ");
    let second: ExportPlan = session.export_plan().unwrap();
    let error = save_original(&second, receipt.stamp()).unwrap_err();
    assert!(
        error
            .blockers()
            .unwrap()
            .contains(&ReplacementBlocker::OriginBindingUnproven)
    );
    assert_eq!(fs::read(&path).unwrap(), first.bytes());

    // Re-opening the saved file gives a session whose plans bind again.
    let reopened = open_source(&path, &Budget::default()).unwrap();
    let mut rebased = open_session(&reopened);
    insert(&mut rebased, "second ");
    let plan = rebased.export_plan().unwrap();
    save_original(&plan, reopened.stamp()).unwrap();
    assert!(first_paragraph(&fs::read(&path).unwrap()).starts_with("second first "));
}
