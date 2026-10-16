//! W11: `save_original` compares `origin_bytes()` with the stamped package
//! after its own conflict check. A different package is refused with no
//! write. A matching origin still refuses on this build because ACL, extended
//! attributes, provenance marks, ownership and creation time cannot be
//! preserved with the approved dependencies. The Unix replace steps are
//! covered by the storage unit tests, which call the publisher directly.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::Budget;
use joyword_docx::ExportPlan;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::{MetadataItem, ReplacementBlocker, open_source, save_original};

fn fixture(id: u8) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn plan_from(bytes: &[u8], edit: bool) -> ExportPlan {
    let mut session = DocxSession::open(bytes, &Budget::default()).unwrap();
    if edit {
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
                    text: "原件".into(),
                }],
            ))
            .unwrap();
    }
    session.export_plan().unwrap()
}

#[test]
fn same_version_different_packages_are_refused_with_no_write() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = fixture(2);
    let path = dir.path().join("mine.docx");
    fs::write(&path, &bytes).unwrap();
    let before = fs::metadata(&path).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let other = plan_from(&fixture(1), false);
    let error = save_original(&other, &stamp).unwrap_err();
    assert!(error.is_origin_mismatch());
    assert!(error.published().is_none());
    assert_eq!(other.version(), plan_from(&bytes, false).version());
    assert_ne!(other.origin_bytes(), Some(bytes.as_slice()));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        fs::metadata(&path).unwrap().modified().unwrap(),
        before.modified().unwrap()
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn matching_origin_still_refuses_unpreservable_metadata_before_any_write() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = fixture(1);
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let plan = plan_from(&bytes, true);
    assert_eq!(plan.origin_bytes(), Some(bytes.as_slice()));
    assert_ne!(plan.bytes(), bytes.as_slice());
    let error = save_original(&plan, &stamp).unwrap_err();
    let blockers = error.blockers().expect("metadata still blocks replacement");
    assert!(!blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
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
    assert!(
        blockers.contains(&ReplacementBlocker::UnpreservableMetadata(
            MetadataItem::Ownership
        )) || blockers.contains(&ReplacementBlocker::UnobservableMetadata(
            MetadataItem::Ownership
        ))
    );
    assert!(error.published().is_none());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn a_lock_file_still_refuses_before_origin_or_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = fixture(1);
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    fs::write(dir.path().join("~$report.docx"), b"owner").unwrap();
    let error = save_original(&plan_from(&bytes, true), &stamp).unwrap_err();
    assert!(error.lock().is_some());
    assert!(error.blockers().is_none());
    assert!(!error.is_origin_mismatch());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn oversized_sources_never_reach_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.docx");
    fs::write(&path, fixture(1)).unwrap();
    let budget = Budget {
        max_total_bytes: 8,
        ..Budget::default()
    };
    let error = open_source(&path, &budget).unwrap_err();
    assert!(matches!(
        error,
        joyword_shell::storage::SaveOriginalError::SourceTooLarge { .. }
    ));
}

#[cfg(unix)]
#[test]
fn a_symlink_request_does_not_replace_the_link_or_its_target() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = fixture(1);
    let real = dir.path().join("real.docx");
    fs::write(&real, &bytes).unwrap();
    let link = dir.path().join("link.docx");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let stamp = open_source(&link, &Budget::default()).unwrap().into_stamp();
    assert_eq!(stamp.path(), real.canonicalize().unwrap().as_path());
    let error = save_original(&plan_from(&bytes, true), &stamp).unwrap_err();
    assert!(error.blockers().is_some());
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&real).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn hard_linked_sources_still_refuse_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = fixture(1);
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let alias = dir.path().join("alias.docx");
    fs::hard_link(&path, &alias).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let error = save_original(&plan_from(&bytes, true), &stamp).unwrap_err();
    let blockers = error.blockers().unwrap();
    assert!(blockers.contains(&ReplacementBlocker::LinkedSourceSemantics));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read(&alias).unwrap(), bytes);
}
