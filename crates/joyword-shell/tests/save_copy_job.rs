//! J3／P0-3: Save As takes a detached job of one committed version, so the
//! export plan, its verification and the file IO can run on another thread
//! while editing continues. The copy never marks the session saved.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::Budget;
use joyword_model::Document;
use joyword_shell::docx::{BoundDocx, BoundSaveError, DocxSession, SaveCopyJob};
use joyword_shell::input::{InputLimits, TextInput};
use joyword_shell::storage::SaveCopyStage;

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

fn reopened(bytes: &[u8]) -> String {
    plain(
        DocxSession::open(bytes, &Budget::default())
            .unwrap()
            .session()
            .document(),
    )
}

fn detached<T: Send + Sync + 'static>(_: &T) {}

#[test]
fn a_copy_job_writes_its_own_version_on_another_thread_while_typing_continues() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let original = fixture();
    fs::write(&path, &original).unwrap();
    let mut input = TextInput::new(
        BoundDocx::open_default(&path).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    input.replace(None, "保存前").unwrap();
    let requested = input.backend().session().session().version();
    // Taking the job is all the UI thread does; it holds no borrow of the session.
    let job: SaveCopyJob = input.backend().save_copy_job();
    detached(&job);
    assert_eq!(job.version(), requested);

    let destination = dir.path().join("copy.docx");
    let worker = {
        let destination = destination.clone();
        std::thread::spawn(move || job.run(&destination))
    };
    // Typing continues while the copy is planned, verified and written.
    input.replace(None, "保存中").unwrap();
    let typed = input.backend().session().session().version();
    let receipt = worker.join().unwrap().unwrap();

    assert_eq!(receipt.version(), requested);
    assert_ne!(typed, requested);
    let written = reopened(&fs::read(receipt.path()).unwrap());
    assert!(written.contains("保存前"), "{written}");
    assert!(!written.contains("保存中"), "{written}");
    // Dirty is still bound to the opened version, and the binding is unchanged.
    assert!(input.has_unsaved_changes());
    assert_eq!(input.backend().session().session().version(), typed);
    assert_eq!(
        input.backend().source_path(),
        fs::canonicalize(&path).unwrap()
    );
    assert!(input.backend().lock_path().exists());
    assert_eq!(fs::read(&path).unwrap(), original);
    // A job taken now holds the typed version.
    assert_eq!(input.backend().save_copy_job().version(), typed);
}

#[test]
fn a_copy_job_of_a_clean_session_writes_the_original_and_failures_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    let original = fixture();
    fs::write(&path, &original).unwrap();
    let mut input = TextInput::new(
        BoundDocx::open_default(&path).unwrap(),
        InputLimits::default(),
    )
    .unwrap();
    let clean = input.backend().save_copy_job();
    let receipt = clean.run(&dir.path().join("clean.docx")).unwrap();
    assert_eq!(fs::read(receipt.path()).unwrap(), original);
    assert!(!input.has_unsaved_changes());

    input.replace(None, "仍未写回").unwrap();
    let version = input.backend().session().session().version();
    let taken = dir.path().join("taken.docx");
    fs::write(&taken, b"keep").unwrap();
    let job = input.backend().save_copy_job();
    let error = std::thread::spawn(move || job.run(&taken))
        .join()
        .unwrap()
        .unwrap_err();
    match error {
        BoundSaveError::Copy(error) => {
            assert_eq!(error.stage(), SaveCopyStage::Prepare);
            assert_eq!(error.io_error().kind(), std::io::ErrorKind::AlreadyExists);
            assert!(error.published().is_none());
        }
        BoundSaveError::Export(error) => panic!("export failed: {error}"),
    }
    assert_eq!(fs::read(dir.path().join("taken.docx")).unwrap(), b"keep");
    assert_eq!(input.backend().session().session().version(), version);
    assert!(input.has_unsaved_changes());
    assert_eq!(fs::read(&path).unwrap(), original);
}
