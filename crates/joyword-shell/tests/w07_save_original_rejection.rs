//! W07 integration: `save_original` is a real gate. It re-runs conflict
//! verification itself, validates plan/source consistency, requires the plan
//! to originate byte-exactly from the stamped package, and while any
//! replacement blocker stands it writes nothing — no bytes, no metadata
//! change, no temporary files, no locks — and never lets a plan from one
//! document (or one version) overwrite another file.

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
    edited_plan_with(id, "安全保存🙂 ")
}

fn edited_plan_with(id: u8, text: &str) -> ExportPlan {
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
                text: text.into(),
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

/// Platforms without a vetted way to see every metadata item refuse even a
/// correctly bound plan, before writing anything.
#[cfg(not(target_os = "macos"))]
#[test]
fn a_bound_plan_is_refused_where_metadata_cannot_be_vouched_for() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    let before = fs::metadata(&path).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let error = save_original(&edited_plan(1), &stamp).unwrap_err();
    let blockers = error.blockers().expect("expected durable blockers");
    assert!(!blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
    assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl)));
    assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(
        MetadataItem::ExtendedAttributes
    )));
    assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(
        MetadataItem::ProvenanceMarks
    )));
    #[cfg(not(unix))]
    assert!(blockers.contains(&ReplacementBlocker::UnsupportedPlatform));
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
    let own = DocxSession::open(&fixture(2), &Budget::default()).unwrap();
    let other = DocxSession::open(&fixture(1), &Budget::default())
        .unwrap()
        .export_plan()
        .unwrap();
    assert_eq!(own.session().version(), other.version());
    for plan in [other, edited_plan(1)] {
        let error = save_original(&plan, &stamp).unwrap_err();
        let blockers = error.blockers().expect("expected durable blockers");
        assert!(blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
        assert_untouched(&path, &bytes, 1);
    }
}

#[test]
fn a_plan_from_an_older_version_of_the_same_file_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    let old = open_source(&path, &Budget::default()).unwrap();
    let plan = edited_plan(1);
    assert!(plan.originates_from(old.bytes()));
    // Another application saves a new version of the file.
    let other_save = edited_plan_with(1, "another editor ").bytes().to_vec();
    assert_ne!(other_save, bytes);
    fs::write(&path, &other_save).unwrap();
    // The old stamp sees the change; a fresh stamp sees a different package.
    assert!(
        save_original(&plan, old.stamp())
            .unwrap_err()
            .conflict()
            .is_some()
    );
    let fresh = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let error = save_original(&plan, &fresh).unwrap_err();
    assert!(
        error
            .blockers()
            .unwrap()
            .contains(&ReplacementBlocker::OriginBindingUnproven)
    );
    assert_untouched(&path, &other_save, 1);
}

#[test]
fn a_read_only_source_is_not_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    let writable = fs::metadata(&path).unwrap().permissions();
    let mut permissions = writable.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let error = save_original(&edited_plan(1), &stamp).unwrap_err();
    assert!(
        error
            .blockers()
            .unwrap()
            .contains(&ReplacementBlocker::ReadOnlySource)
    );
    assert_untouched(&path, &bytes, 1);
    // Let the temporary directory be removed everywhere.
    fs::set_permissions(&path, writable).unwrap();
}

#[test]
fn a_plan_larger_than_the_stamp_read_budget_is_refused_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let (path, bytes) = write_source(dir.path(), "report.docx", 1);
    let plan = edited_plan(1);
    assert!(plan.bytes().len() > bytes.len());
    let tight = Budget {
        max_total_bytes: bytes.len() as u64,
        ..Budget::default()
    };
    let stamp = open_source(&path, &tight).unwrap().into_stamp();
    let error = save_original(&plan, &stamp).unwrap_err();
    assert_eq!(error.stage(), Some(SourceStage::Validate));
    assert_eq!(
        error.io_error().map(|e| e.kind()),
        Some(std::io::ErrorKind::FileTooLarge)
    );
    assert_untouched(&path, &bytes, 1);
}

/// macOS observes extended attributes, ACLs, provenance marks and file
/// flags; a source carrying any of them is refused before writing.
#[cfg(target_os = "macos")]
#[test]
fn present_metadata_that_cannot_be_reproduced_is_refused() {
    use std::process::Command;
    let cases: [(&str, &[&str], MetadataItem); 4] = [
        (
            "/usr/bin/xattr",
            &["-w", "com.example.joyword", "1"],
            MetadataItem::ExtendedAttributes,
        ),
        (
            "/usr/bin/xattr",
            &["-w", "com.apple.quarantine", "0081;65f00000;Safari;"],
            MetadataItem::ProvenanceMarks,
        ),
        (
            "/bin/chmod",
            &["+a", "group:everyone deny delete"],
            MetadataItem::Acl,
        ),
        ("/usr/bin/chflags", &["hidden"], MetadataItem::FileFlags),
    ];
    for (tool, args, item) in cases {
        let dir = tempfile::tempdir().unwrap();
        let (path, bytes) = write_source(dir.path(), "report.docx", 1);
        assert!(
            Command::new(tool)
                .args(args)
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
        let error = save_original(&edited_plan(1), &stamp).unwrap_err();
        assert_eq!(
            error.blockers(),
            Some(&[ReplacementBlocker::UnpreservableMetadata(item)][..]),
            "{tool} {args:?}"
        );
        assert_untouched(&path, &bytes, 1);
        if item == MetadataItem::Acl {
            // Let the temporary directory be removed.
            assert!(
                Command::new("/bin/chmod")
                    .args(["-N"])
                    .arg(&path)
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
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
