//! T110: unconfirmed original save lists metadata losses and leaves the source
//! untouched. Confirmation on a fully observed file replaces the bytes and
//! keeps the metadata this build can restore. The default path writes a copy.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::Budget;
use joyword_docx::ExportPlan;
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::{
    MetadataItem, ReplacementBlocker, open_source, save_default_copy, save_original,
};
#[cfg(unix)]
use joyword_shell::storage::{OriginalChoice, save_original_with_choice};

fn fixture() -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/docx/DOCX-MIN-001.docx"),
    )
    .unwrap()
}

fn edited_plan(bytes: &[u8]) -> ExportPlan {
    let mut session = DocxSession::open(bytes, &Budget::default()).unwrap();
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
    session.export_plan().unwrap()
}

#[test]
fn declining_replacement_lists_metadata_and_keeps_the_source() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/bin/chmod")
            .args(["+a", "everyone deny delete", path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "chmod acl: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let plan = edited_plan(&bytes);
    let error = save_original(&plan, &stamp).unwrap_err();
    let blockers = error.blockers().expect("losses");
    assert!(blockers.contains(&ReplacementBlocker::ConfirmationRequired));
    #[cfg(target_os = "macos")]
    assert!(
        blockers.contains(&ReplacementBlocker::UnpreservableMetadata(
            MetadataItem::Acl
        ))
    );
    #[cfg(not(target_os = "macos"))]
    assert!(
        blockers.contains(&ReplacementBlocker::UnpreservableMetadata(
            MetadataItem::CreationTime
        )) || blockers.contains(&ReplacementBlocker::UnobservableMetadata(
            MetadataItem::CreationTime
        )) || blockers.contains(&ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl))
    );
    assert!(error.published().is_none());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[cfg(windows)]
#[test]
fn declining_on_windows_keeps_the_source_and_provenance_stream() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let stream = format!("{}:Zone.Identifier", path.display());
    fs::write(&stream, b"[ZoneTransfer]\r\nZoneId=3\r\n").unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let plan = edited_plan(&bytes);
    let error = save_original(&plan, &stamp).unwrap_err();
    let blockers = error.blockers().expect("losses");
    assert!(blockers.contains(&ReplacementBlocker::ConfirmationRequired));
    assert!(
        !blockers.contains(&ReplacementBlocker::UnpreservableMetadata(
            MetadataItem::ProvenanceMarks
        ))
    );
    assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl)));
    assert!(error.published().is_none());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        fs::read(&stream).unwrap(),
        b"[ZoneTransfer]\r\nZoneId=3\r\n"
    );
}

#[cfg(unix)]
#[test]
fn confirming_a_fully_observed_file_replaces_bytes_and_keeps_metadata() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o640);
    fs::set_permissions(&path, permissions).unwrap();
    let before = fs::metadata(&path).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let plan = edited_plan(&bytes);
    assert_eq!(plan.origin_bytes(), Some(bytes.as_slice()));
    let receipt = save_original_with_choice(&plan, &stamp, None, OriginalChoice::Confirm).unwrap();
    assert_eq!(fs::read(&path).unwrap(), plan.bytes());
    assert_eq!(receipt.bytes(), plan.bytes().len() as u64);
    let after = fs::metadata(&path).unwrap();
    assert_eq!(after.mode() & 0o777, 0o640);
    assert_eq!((after.uid(), after.gid()), (before.uid(), before.gid()));
    #[cfg(target_os = "macos")]
    assert_eq!(after.created().unwrap(), before.created().unwrap());
    let reopened = DocxSession::open(&fs::read(&path).unwrap(), &Budget::default()).unwrap();
    assert!(
        reopened
            .session()
            .document()
            .body()
            .blocks
            .iter()
            .any(|block| {
                block.as_paragraph().is_some_and(|paragraph| {
                    paragraph.plain_text(|_| String::new()).contains("原件")
                })
            })
    );
}

#[test]
fn the_default_path_writes_a_new_copy_and_leaves_the_source() {
    let bytes = fixture();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.docx");
    fs::write(&path, &bytes).unwrap();
    let stamp = open_source(&path, &Budget::default()).unwrap().into_stamp();
    let plan = edited_plan(&bytes);
    let copy = dir.path().join("report-copy.docx");
    let saved = save_default_copy(&plan, &stamp, &copy).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read(saved.receipt.path()).unwrap(), plan.bytes());
    assert!(saved.receipt.path() != path);
}

/// A volume classification refusal (network share, non-NTFS/ReFS filesystem,
/// missing capability flag, unrecognized drive) surfaces through the Decline
/// precheck as an observation failure carrying the structured reason. The
/// Windows observation produces it from real volume facts; this test
/// synthesizes the same public error shape on every platform, so the
/// precheck contract the native prompt consumes is checked off-Windows too.
#[test]
fn a_volume_refused_precheck_reports_replacement_unavailable_not_confirmable_losses() {
    use joyword_shell::storage::{SaveOriginalError, VolumeRejection};
    use std::io;
    let path = PathBuf::from("/net/report.docx");
    for reason in [
        VolumeRejection::Remote,
        VolumeRejection::UnknownDriveType,
        VolumeRejection::UnsupportedFileSystem,
        VolumeRejection::NoPersistentAcls,
        VolumeRejection::NoNamedStreams,
        VolumeRejection::NoHardLinks,
    ] {
        let error = SaveOriginalError::ObservationFailed {
            path: path.clone(),
            source: io::Error::new(io::ErrorKind::Unsupported, reason),
        };
        assert!(error.is_observation_failure(), "{reason:?}");
        assert_eq!(error.volume_rejection(), Some(reason));
        // A refused volume is not a confirmable metadata-loss list; the
        // prompt must treat replacement as unavailable.
        assert!(error.blockers().is_none());
        assert!(error.published().is_none());
    }
    // An ordinary observation failure keeps its current meaning: a plain
    // error, with no volume reason attached.
    let plain = SaveOriginalError::ObservationFailed {
        path: path.clone(),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "acl read failed"),
    };
    assert!(plain.is_observation_failure());
    assert_eq!(plain.volume_rejection(), None);
    assert!(plain.blockers().is_none());
}
