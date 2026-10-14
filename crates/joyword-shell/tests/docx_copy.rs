use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

use joyoffice_opc::Budget;
use joyword_docx::ExportPlan;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::{CopyDurability, SaveCopyStage, save_export_copy};

fn fixture(id: u8) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}
fn edit(session: &mut DocxSession, text: &str) {
    let d = session.session().document();
    let at = Position {
        story: d.body_id(),
        paragraph: d.body().blocks.get(0).unwrap().id(),
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
fn plan() -> ExportPlan {
    DocxSession::open(&fixture(1), &Budget::default())
        .unwrap()
        .export_plan()
        .unwrap()
}

#[test]
fn edit_export_publish_reopen_preserves_source_and_session() {
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("source.docx");
    let destination = dir.path().join("new.docx");
    let original = fixture(1);
    fs::write(&source_path, &original).unwrap();
    let mut session =
        DocxSession::open(&fs::read(&source_path).unwrap(), &Budget::default()).unwrap();
    edit(&mut session, "落盘副本🙂 &<> ");
    let plan = session.export_plan().unwrap();
    let version = session.session().version();
    let receipt = save_export_copy(&plan, &destination).unwrap();
    assert_eq!(receipt.version(), version);
    assert_eq!(receipt.bytes(), plan.bytes().len() as u64);
    assert_eq!(fs::read(receipt.path()).unwrap(), plan.bytes());
    let reopened = DocxSession::open(&fs::read(&destination).unwrap(), &Budget::default()).unwrap();
    assert!(
        session
            .session()
            .document()
            .semantic_difference(reopened.session().document())
            .is_none()
    );
    assert_eq!(fs::read(&source_path).unwrap(), original);
    assert_eq!(session.backing().original_bytes(), original);
    assert_eq!(session.session().version(), version);
    assert!(session.session().can_undo());
    assert!(session.unchanged_package_bytes().is_err());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    #[cfg(unix)]
    assert_eq!(receipt.durability(), CopyDurability::FileAndDirectorySynced);
    #[cfg(not(unix))]
    assert_eq!(receipt.durability(), CopyDurability::FileSynced);
}

#[test]
fn unchanged_restricted_corpus_can_only_be_copied_exactly() {
    let dir = tempfile::tempdir().unwrap();
    for id in 1..=5 {
        let bytes = fixture(id);
        let session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        let plan = session.export_plan().unwrap();
        let receipt = save_export_copy(&plan, &dir.path().join(format!("copy-{id}.docx"))).unwrap();
        assert_eq!(fs::read(receipt.path()).unwrap(), bytes);
        assert_eq!(receipt.version(), session.session().version());
        assert!(!session.session().can_undo());
    }
}

#[test]
fn existing_file_directory_and_hard_link_are_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let plan = plan();
    let file = dir.path().join("file.docx");
    fs::write(&file, b"existing original").unwrap();
    let directory = dir.path().join("directory.docx");
    fs::create_dir(&directory).unwrap();
    let link = dir.path().join("hardlink.docx");
    fs::hard_link(&file, &link).unwrap();
    for path in [&file, &directory, &link] {
        let error = save_export_copy(&plan, path).unwrap_err();
        assert_eq!(error.stage(), SaveCopyStage::Prepare);
        assert_eq!(error.io_error().kind(), std::io::ErrorKind::AlreadyExists);
        assert!(error.published().is_none());
    }
    assert_eq!(fs::read(&file).unwrap(), b"existing original");
    assert_eq!(fs::read(&link).unwrap(), b"existing original");
    assert!(directory.is_dir());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
}

#[cfg(unix)]
#[test]
fn symbolic_links_including_dangling_destinations_are_refused() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let plan = plan();
    let original = dir.path().join("original.docx");
    fs::write(&original, b"original").unwrap();
    let linked = dir.path().join("linked.docx");
    symlink(&original, &linked).unwrap();
    let dangling = dir.path().join("dangling.docx");
    symlink(dir.path().join("missing.docx"), &dangling).unwrap();
    for path in [&linked, &dangling] {
        let error = save_export_copy(&plan, path).unwrap_err();
        assert_eq!(error.io_error().kind(), std::io::ErrorKind::AlreadyExists);
        assert!(fs::symlink_metadata(path).unwrap().file_type().is_symlink());
    }
    assert_eq!(fs::read(&original).unwrap(), b"original");
}

#[test]
fn eight_concurrent_writers_publish_exactly_one_complete_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("race.docx");
    let plan = Arc::new(plan());
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let target = target.clone();
            let plan = plan.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                save_export_copy(&plan, &target)
            })
        })
        .collect();
    let mut successes = 0;
    for handle in handles {
        match handle.join().unwrap() {
            Ok(receipt) => {
                successes += 1;
                assert_eq!(receipt.version(), plan.version());
            }
            Err(error) => {
                assert_eq!(error.io_error().kind(), std::io::ErrorKind::AlreadyExists);
                assert!(error.published().is_none());
            }
        }
    }
    assert_eq!(successes, 1);
    assert_eq!(fs::read(&target).unwrap(), plan.bytes());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn invalid_paths_and_mismatched_type_leave_no_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let plan = plan();
    for name in [
        "no-extension",
        "wrong.docm",
        "missing/out.docx",
        "../missing-c03-dir/out.docx",
    ] {
        let error = save_export_copy(&plan, &dir.path().join(name)).unwrap_err();
        assert_eq!(error.stage(), SaveCopyStage::Prepare);
        assert!(error.published().is_none());
    }
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    let receipt = save_export_copy(&plan, &dir.path().join("UPPER.DOCX")).unwrap();
    assert_eq!(fs::read(receipt.path()).unwrap(), plan.bytes());
}

#[test]
fn saving_an_older_plan_reports_its_version_and_leaves_current_edit_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = DocxSession::open(&fixture(1), &Budget::default()).unwrap();
    edit(&mut session, "first ");
    let old = session.export_plan().unwrap();
    edit(&mut session, "second ");
    let current = session.session().version();
    let receipt = save_export_copy(&old, &dir.path().join("old.docx")).unwrap();
    assert_eq!(receipt.version(), old.version());
    assert_ne!(receipt.version(), current);
    assert_eq!(session.session().version(), current);
    assert!(session.session().can_undo());
    let reopened =
        DocxSession::open(&fs::read(receipt.path()).unwrap(), &Budget::default()).unwrap();
    assert!(
        session
            .session()
            .document()
            .semantic_difference(reopened.session().document())
            .is_some()
    );
    session.undo().unwrap();
    assert!(
        session
            .session()
            .document()
            .semantic_difference(reopened.session().document())
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn published_copy_has_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let receipt = save_export_copy(&plan(), &dir.path().join("private.docx")).unwrap();
    assert_eq!(
        fs::metadata(receipt.path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
