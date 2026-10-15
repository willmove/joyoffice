//! W07 integration: `save_original` is a real conservative gate. It re-runs
//! conflict verification itself, validates plan/source consistency, and
//! while replacement blockers stand it must write nothing — no bytes, no
//! metadata change, no temporary files, no locks — and never let a plan
//! from one document overwrite another document's file.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::Budget;
use joyword_docx::ExportPlan;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::{
    MetadataItem, ReplacementBlocker, SourceStage, open_source, save_original,
};

fn fixture(id: u8) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn edited_plan(id: u8) -> ExportPlan {
    let mut session = DocxSession::open(&fixture(id), &Budget::default()).unwrap();
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
                text: "安全保存🙂 ".into(),
            }],
        ))
        .unwrap();
    session.export_plan().unwrap()
}

fn write_source(dir: &std::path::Path, name: &str, id: u8) -> (PathBuf, Vec<u8>) {
    let bytes = fixture(id);
    let path = dir.join(name);
    fs::write(&path, &bytes).unwrap();
    (path, bytes)
}

fn assert_untouched(path: &std::path::Path, bytes: &[u8], entries: usize) {
    assert_eq!(fs::read(path).unwrap(), bytes);
    let metadata = fs::metadata(path).unwrap();
    assert_eq!(metadata.len(), bytes.len() as u64);
    assert_eq!(
        fs::read_dir(path.parent().unwrap()).unwrap().count(),
        entries
    );
}

#[test]
fn replacement_is_refused_with_durable_blockers_and_zero_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    let before = fs::metadata(&path).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let error = save_original(&edited_plan(1), &stamp).unwrap_err();
    let blockers = error.blockers().expect("expected durable blockers");
    assert!(blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
    assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl)));
    assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(
        MetadataItem::ExtendedAttributes
    )));
    assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(
        MetadataItem::ProvenanceMarks
    )));
    assert!(
        blockers.contains(&ReplacementBlocker::UnpreservableMetadata(
            MetadataItem::CreationTime
        ))
    );
    let after = fs::metadata(&path).unwrap();
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_untouched(&path, &bytes, 1);
}

#[test]
fn a_plan_from_another_document_cannot_overwrite_a_source() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "mine.docx", 2);
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    // Both plans are first-version exports of unedited sessions: equal
    // version numbers must not suggest they belong to this source.
    let other = DocxSession::open(&fixture(1), &Budget::default())
        .unwrap()
        .export_plan()
        .unwrap();
    let error = save_original(&other, &stamp).unwrap_err();
    assert!(error.blockers().is_some());
    assert_untouched(&path, &bytes, 1);
}

#[test]
fn save_original_reruns_the_full_conflict_verification_itself() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    // Replacement by identical bytes is only observable where the stamp
    // records the object (Unix); elsewhere identity is length and mtime.
    #[cfg(unix)]
    {
        use joyword_shell::storage::ConflictDetail;
        let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
        fs::remove_file(&path).unwrap();
        fs::write(&path, &bytes).unwrap();
        let error = save_original(&edited_plan(1), &stamp).unwrap_err();
        // ext4 and tmpfs may reuse the freed inode number at once.
        assert!(
            matches!(
                error.conflict(),
                Some(ConflictDetail::InodeChanged | ConflictDetail::TimesChanged)
            ),
            "{:?}",
            error.conflict()
        );
        assert_eq!(error.blockers(), None);
    }
    #[cfg(not(unix))]
    let _ = &bytes;

    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    fs::write(dir.path().join("~$report.docx"), b"owner").unwrap();
    let error = save_original(&edited_plan(1), &stamp).unwrap_err();
    assert_eq!(
        error.lock().map(|(kind, _)| kind),
        Some(joyword_shell::storage::LockKind::OfficeOwnerFile)
    );
    assert_eq!(error.blockers(), None);
}

#[test]
fn plan_type_and_source_extension_must_agree() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "template.dotx", 1);
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let error = save_original(&edited_plan(1), &stamp).unwrap_err();
    assert_eq!(error.stage(), Some(SourceStage::Validate));
    assert_eq!(
        error.io_error().map(|e| e.kind()),
        Some(std::io::ErrorKind::InvalidInput)
    );
    assert_untouched(&path, &bytes, 1);
}

#[cfg(unix)]
#[test]
fn hard_linked_sources_add_the_link_semantics_blocker() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    fs::hard_link(&path, dir.path().join("alias.docx")).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let error = save_original(&edited_plan(1), &stamp).unwrap_err();
    let blockers = error.blockers().unwrap();
    assert!(blockers.contains(&ReplacementBlocker::LinkedSourceSemantics));
    assert_untouched(&path, &bytes, 2);
}

#[test]
fn stamp_bytes_reopen_as_a_real_docx_session() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    let opened = open_source(&path, &Budget::default()).unwrap();
    let session = DocxSession::open(opened.bytes(), &Budget::default()).unwrap();
    assert_eq!(session.backing().original_bytes(), bytes.as_slice());
}
