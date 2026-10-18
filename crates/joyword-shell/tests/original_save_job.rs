use std::fs;
use std::path::{Path, PathBuf};

use joyoffice_opc::Budget;
use joyword_shell::command::{ArgValue, DOCUMENT_SWITCH, FILE_SAVE, InvokeError};
use joyword_shell::docx::{BoundDocx, DocxSession};
use joyword_shell::host::Office;
use joyword_shell::input::{InputLimits, TextInput};
use joyword_shell::storage::OriginalChoice;

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn open(path: &Path) -> TextInput<BoundDocx> {
    TextInput::new(
        BoundDocx::open_default(path).unwrap(),
        InputLimits::default(),
    )
    .unwrap()
}

fn text(bytes: &[u8]) -> String {
    let session = DocxSession::open(bytes, &Budget::default()).unwrap();
    session
        .session()
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn late_original_save_rebinds_without_rolling_back_later_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, fixture()).unwrap();
    let mut input = open(&path);
    input.replace(None, "早").unwrap();
    let job = input.save_original_job();
    let saved_version = job.version();
    input.replace(None, "晚").unwrap();
    let current = input.view().index().text().to_owned();

    let output = job.run(OriginalChoice::Confirm);
    let receipt = input.apply_original_save(output).unwrap();
    assert_eq!(receipt.version(), saved_version);
    assert_eq!(input.saved_version(), saved_version);
    assert!(input.has_unsaved_changes());
    assert_eq!(input.view().index().text(), current);
    assert!(current.contains("晚"));

    let on_disk = fs::read(&path).unwrap();
    assert!(!text(&on_disk).contains("晚"));
    assert_eq!(
        input.backend().session().backing().original_bytes(),
        on_disk
    );
    assert_eq!(input.backend().stamp().bytes(), on_disk);

    let second = input.save_original(OriginalChoice::Confirm).unwrap();
    assert!(!input.has_unsaved_changes());
    assert_eq!(fs::read(&path).unwrap(), second.stamp().bytes());
    assert!(text(&fs::read(&path).unwrap()).contains("晚"));

    input.undo().unwrap();
    assert!(input.has_unsaved_changes());
    input.save_original(OriginalChoice::Confirm).unwrap();
    assert!(!input.has_unsaved_changes());
    assert!(!text(&fs::read(&path).unwrap()).contains("晚"));
}

#[test]
fn office_routes_a_late_result_to_the_original_document() {
    let dir = tempfile::tempdir().unwrap();
    let first_path = dir.path().join("first.docx");
    let second_path = dir.path().join("second.docx");
    fs::write(&first_path, fixture()).unwrap();
    fs::write(&second_path, fixture()).unwrap();
    let second_bytes = fs::read(&second_path).unwrap();

    let mut office = Office::new();
    let mut first = open(&first_path);
    first.replace(None, "原件保存").unwrap();
    let first_id = office.attach(first);
    let job = office.active().unwrap().save_original_job();
    let second_id = office.attach(open(&second_path));
    assert_ne!(first_id, second_id);
    assert_eq!(office.active_id(), Some(second_id));

    let output = job.run(OriginalChoice::Confirm);
    let receipt = office
        .apply_original_save(first_id, output)
        .expect("first document still exists")
        .unwrap();
    assert_eq!(receipt.version(), job.version());
    assert_eq!(fs::read(&second_path).unwrap(), second_bytes);
    assert!(text(&fs::read(&first_path).unwrap()).contains("原件保存"));

    office
        .invoke(DOCUMENT_SWITCH, &[("id", ArgValue::Id(first_id.0))])
        .unwrap();
    assert!(!office.active().unwrap().has_unsaved_changes());
    assert_eq!(
        office
            .active()
            .unwrap()
            .backend()
            .session()
            .backing()
            .original_bytes(),
        receipt.stamp().bytes()
    );
}

#[test]
fn registry_save_requires_explicit_confirmation_and_marks_only_that_version_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, fixture()).unwrap();
    let mut input = open(&path);
    input.replace(None, "命令保存").unwrap();
    let mut office = Office::new();
    let id = office.attach(input);

    assert_eq!(
        office.invoke(FILE_SAVE, &[]),
        Err(InvokeError::PendingInput {
            missing: vec!["choice"]
        })
    );
    office
        .invoke(FILE_SAVE, &[("choice", ArgValue::Text("confirm".into()))])
        .unwrap();
    assert!(!office.is_dirty(id));
    assert!(text(&fs::read(&path).unwrap()).contains("命令保存"));
}
