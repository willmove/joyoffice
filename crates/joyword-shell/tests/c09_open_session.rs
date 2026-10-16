//! C09-A/C09-B: `open_source` becomes a bound session, and Save As is only
//! `save_export_copy`. The bound path and dirty flag stay on the opened file.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::Budget;
use joyword_model::Document;
use joyword_shell::command::{
    FILE_OPEN, FILE_SAVE_COPY, MenuEntry, MenuKind, Platform, QUIT, Registry,
};
use joyword_shell::docx::{BoundDocx, BoundOpenError, BoundSaveError, DocxSession};
use joyword_shell::input::{InputLimits, TextInput};
use joyword_shell::storage::{LockKind, LockState, SaveCopyStage, SaveOriginalError, open_source};

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn plain(document: &Document) -> String {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn lock_conflict(error: SaveOriginalError) -> (LockKind, LockState) {
    error.lock().expect("lock conflict")
}

#[test]
fn open_installs_origin_and_a_joyword_lock_that_blocks_a_second_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let original = fixture();
    fs::write(&path, &original).unwrap();
    let bound = BoundDocx::open(&path, &Budget::default()).unwrap();
    let canonical = fs::canonicalize(&path).unwrap();
    assert_eq!(bound.source_path(), canonical.as_path());
    assert_eq!(bound.stamp().path(), canonical.as_path());
    assert_eq!(
        bound.lock_path(),
        canonical
            .parent()
            .unwrap()
            .join(".report.docx.joyword-lock")
    );
    let token = fs::read_to_string(bound.lock_path()).unwrap();
    assert!(token.starts_with("joyword-lock 1\n"), "{token}");
    assert!(token.contains(&canonical.display().to_string()));
    let plan = bound.session().export_plan().unwrap();
    assert_eq!(plan.origin_bytes(), Some(original.as_slice()));
    assert_eq!(plan.bytes(), original.as_slice());
    let again = open_source(bound.source_path(), &Budget::default()).unwrap_err();
    assert_eq!(
        lock_conflict(again),
        (LockKind::JoyWordLockFile, LockState::Present)
    );
    let lock_path = bound.lock_path().to_path_buf();
    drop(bound);
    assert!(!lock_path.exists());
    let reopened = open_source(&path, &Budget::default()).unwrap();
    assert_eq!(reopened.bytes(), original.as_slice());
}

#[test]
fn failed_open_creates_no_lock_and_installs_no_session() {
    let dir = tempfile::tempdir().unwrap();
    let junk = dir.path().join("notes.bin");
    fs::write(&junk, b"not a package").unwrap();
    let error = match BoundDocx::open_default(&junk) {
        Err(error) => error,
        Ok(_) => panic!("junk bytes installed a session"),
    };
    assert!(matches!(error, BoundOpenError::Import(_)), "{error}");
    assert!(!dir.path().join(".notes.bin.joyword-lock").exists());

    let path = dir.path().join("report.docx");
    fs::write(&path, fixture()).unwrap();
    fs::write(dir.path().join("~$report.docx"), b"owner").unwrap();
    let error = match BoundDocx::open_default(&path) {
        Err(error) => error,
        Ok(_) => panic!("an Office owner file installed a session"),
    };
    match error {
        BoundOpenError::Source(error) => {
            assert_eq!(
                error.lock(),
                Some((LockKind::OfficeOwnerFile, LockState::Present))
            );
        }
        other => panic!("expected the Office owner file, got {other}"),
    }
    assert!(!dir.path().join(".report.docx.joyword-lock").exists());
    assert_eq!(fs::read(&path).unwrap(), fixture());
}

#[test]
fn save_copy_keeps_the_original_binding_and_dirty_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let original = fixture();
    fs::write(&path, &original).unwrap();
    let mut input = TextInput::new(
        BoundDocx::open_default(&path).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    assert!(!input.has_unsaved_changes());
    let clean = dir.path().join("clean-copy.docx");
    input.backend().save_copy(&clean).unwrap();
    assert!(!input.has_unsaved_changes());
    assert_eq!(
        input.backend().source_path(),
        fs::canonicalize(&path).unwrap()
    );
    assert_eq!(fs::read(&path).unwrap(), original);

    input.replace(None, "C09新增").unwrap();
    assert!(input.has_unsaved_changes());
    let version = input.backend().session().session().version();
    let source = input.backend().source_path().to_path_buf();
    let dest = dir.path().join("edited-copy.docx");
    let receipt = input.backend().save_copy(&dest).unwrap();
    assert_eq!(input.backend().source_path(), source.as_path());
    assert_eq!(input.backend().session().session().version(), version);
    assert!(input.has_unsaved_changes());
    assert!(input.backend().session().session().can_undo());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(input.backend().lock_path().exists());

    let plan = input.backend().session().export_plan().unwrap();
    assert_eq!(plan.origin_bytes(), Some(original.as_slice()));
    assert_ne!(plan.bytes(), original.as_slice());
    assert_eq!(fs::read(receipt.path()).unwrap(), plan.bytes());
    let reopened = DocxSession::open(&fs::read(&dest).unwrap(), &Budget::default()).unwrap();
    assert!(plain(reopened.session().document()).contains("C09新增"));
    assert!(
        input
            .backend()
            .session()
            .session()
            .document()
            .semantic_difference(reopened.session().document())
            .is_none()
    );
    let copy_plan = reopened.export_plan().unwrap();
    assert_eq!(copy_plan.origin_bytes(), Some(plan.bytes()));
    assert_ne!(copy_plan.origin_bytes(), Some(original.as_slice()));
}

#[test]
fn save_copy_refuses_an_existing_path_and_a_wrong_extension() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let original = fixture();
    fs::write(&path, &original).unwrap();
    let mut input = TextInput::new(
        BoundDocx::open_default(&path).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    input.replace(None, "仍未写回").unwrap();
    let source = input.backend().source_path().to_path_buf();
    let version = input.backend().session().session().version();

    let existing = dir.path().join("taken.docx");
    fs::write(&existing, b"keep").unwrap();
    match input.backend().save_copy(&existing).unwrap_err() {
        BoundSaveError::Copy(error) => {
            assert_eq!(error.stage(), SaveCopyStage::Prepare);
            assert_eq!(error.io_error().kind(), std::io::ErrorKind::AlreadyExists);
            assert!(error.published().is_none());
        }
        BoundSaveError::Export(error) => panic!("export failed: {error}"),
    }
    assert_eq!(fs::read(&existing).unwrap(), b"keep");

    let wrong = dir.path().join("copy.txt");
    match input.backend().save_copy(&wrong).unwrap_err() {
        BoundSaveError::Copy(error) => {
            assert_eq!(error.stage(), SaveCopyStage::Prepare);
            assert_eq!(error.io_error().kind(), std::io::ErrorKind::InvalidInput);
            assert!(error.published().is_none());
        }
        BoundSaveError::Export(error) => panic!("export failed: {error}"),
    }
    assert!(!wrong.exists());
    assert_eq!(input.backend().source_path(), source.as_path());
    assert_eq!(input.backend().session().session().version(), version);
    assert!(input.has_unsaved_changes());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(input.backend().lock_path().exists());
}

#[test]
fn a_copy_of_one_open_file_does_not_include_another() {
    let dir = tempfile::tempdir().unwrap();
    let path_a = dir.path().join("a.docx");
    let path_b = dir.path().join("b.docx");
    let original = fixture();
    fs::write(&path_a, &original).unwrap();
    fs::write(&path_b, &original).unwrap();
    let mut first = TextInput::new(
        BoundDocx::open_default(&path_a).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    let mut second = TextInput::new(
        BoundDocx::open_default(&path_b).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    first.replace(None, "甲文件独有").unwrap();
    second.replace(None, "乙文件独有").unwrap();
    let dest = dir.path().join("a-copy.docx");
    first.backend().save_copy(&dest).unwrap();
    let copy = DocxSession::open(&fs::read(&dest).unwrap(), &Budget::default()).unwrap();
    let text = plain(copy.session().document());
    assert!(text.contains("甲文件独有"), "{text}");
    assert!(!text.contains("乙文件独有"), "{text}");
    assert_eq!(fs::read(&path_b).unwrap(), original);
    assert_eq!(fs::read(&path_a).unwrap(), original);
    assert_ne!(first.backend().lock_path(), second.backend().lock_path());
    assert!(first.backend().lock_path().exists());
    assert!(second.backend().lock_path().exists());
    drop(first);
    assert!(second.backend().lock_path().exists());
    assert_eq!(
        second.backend().source_path(),
        fs::canonicalize(&path_b).unwrap()
    );
}

#[test]
fn file_menu_offers_open_and_save_copy_but_not_save() {
    let registry = Registry::builtin();
    assert!(registry.find("file.save").is_none());
    assert_eq!(
        registry
            .find("file.open")
            .unwrap()
            .shortcut
            .for_platform(Platform::Linux),
        Some("ctrl-o")
    );
    assert_eq!(
        registry
            .find("file.save_copy")
            .unwrap()
            .shortcut
            .for_platform(Platform::MacOs),
        Some("cmd-shift-s")
    );
    let menus = registry.menus(Platform::Linux);
    let file = menus
        .iter()
        .find(|menu| menu.kind == MenuKind::File)
        .unwrap();
    // Outside macOS there is no application menu; Quit ends File.
    assert_eq!(
        file.entries,
        vec![
            MenuEntry::Command(FILE_OPEN),
            MenuEntry::Command(FILE_SAVE_COPY),
            MenuEntry::Separator,
            MenuEntry::Command(QUIT),
        ]
    );
    assert!(menus.iter().all(|menu| menu.kind != MenuKind::App));
}
