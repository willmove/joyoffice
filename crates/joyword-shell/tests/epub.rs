use joyword_shell::epub::{self, Metadata, WriteError};
use std::sync::atomic::AtomicBool;
#[test]
fn new_epub_is_read_back_and_existing_targets_never_change() {
    let doc = joyword_model::Document::new_blank();
    let plan = epub::export(
        &doc,
        &Metadata {
            title: "Blank",
            language: "und",
            modified: "2026-10-30T00:00:00Z",
        },
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.epub");
    epub::write_new(&plan, &path, &AtomicBool::new(false)).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
    assert!(epub::write_new(&plan, &path, &AtomicBool::new(false)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), plan.bytes());
    assert!(matches!(
        epub::write_new(&plan, &dir.path().join("bad.docx"), &AtomicBool::new(false)),
        Err(WriteError::InvalidExtension)
    ));
    assert!(matches!(
        epub::write_new(
            &plan,
            &dir.path().join("cancel.epub"),
            &AtomicBool::new(true)
        ),
        Err(WriteError::Cancelled)
    ));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
