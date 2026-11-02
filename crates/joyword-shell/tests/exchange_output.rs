use joyword_shell::{
    exchange::{self, Format},
    exchange_output::{WriteError, write_new},
};
use std::sync::atomic::AtomicBool;

#[test]
fn cancelled_and_unconfirmed_exports_leave_no_files_or_directories() {
    let dir = tempfile::tempdir().unwrap();
    let plan = exchange::plan("中文😀".as_bytes(), Format::Txt, Format::Html).unwrap();
    let path = dir.path().join("result.html");
    assert!(matches!(
        write_new(&plan, &path, false, &AtomicBool::new(false)),
        Err(WriteError::NeedsConfirmation)
    ));
    assert!(matches!(
        write_new(&plan, &path, true, &AtomicBool::new(true)),
        Err(WriteError::Cancelled)
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn shared_publisher_keeps_existing_outputs_and_enforces_target_type() {
    let dir = tempfile::tempdir().unwrap();
    let plan = exchange::plan(b"source", Format::Txt, Format::Html).unwrap();
    let cancel = AtomicBool::new(false);
    let wrong = dir.path().join("result.docx");
    assert!(matches!(
        write_new(&plan, &wrong, true, &cancel),
        Err(WriteError::InvalidExtension)
    ));
    let path = dir.path().join("result.html");
    std::fs::write(&path, "existing document").unwrap();
    assert!(write_new(&plan, &path, true, &cancel).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "existing document");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    let fresh = dir.path().join("new.html");
    write_new(&plan, &fresh, true, &cancel).unwrap();
    assert_eq!(std::fs::read(fresh).unwrap(), plan.bytes());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn committed_model_exports_are_identical_to_the_cli_import_path() {
    let input = "中文😀 **bold**".as_bytes();
    let docx = exchange::plan(input, Format::Markdown, Format::Docx).unwrap();
    let session = joyword_shell::docx::DocxSession::open_default(docx.bytes()).unwrap();
    let snapshot = session.session().document();
    let before = snapshot.version();
    for target in [Format::Markdown, Format::Html, Format::Rtf, Format::Txt] {
        let desktop = exchange::plan_document(snapshot, target).unwrap();
        let cli = exchange::plan(docx.bytes(), Format::Docx, target).unwrap();
        assert_eq!(desktop.bytes(), cli.bytes(), "{target:?}");
        assert_eq!(desktop.losses, cli.losses);
    }
    assert_eq!(session.session().document().version(), before);
    assert_eq!(session.unchanged_package_bytes().unwrap(), docx.bytes());
}
