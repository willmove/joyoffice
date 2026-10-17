//! W10: a real open → edit → `export_plan` chain exposes the backing bytes
//! through `origin_bytes`. `save_export_copy` still publishes the plan output.
//! `save_original` still refuses; this task does not implement replacement.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{ExportMode, W};
use joyword_engine::{Command, Transaction};
use joyword_model::{Bias, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::storage::{ReplacementBlocker, open_source, save_export_copy, save_original};

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn fixture(id: u8) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn package(text: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body><w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p></w:body></w:document>"#
    );
    write_archive(&[
        (
            "[Content_Types].xml",
            CONTENT_TYPES.as_bytes(),
            Method::Deflated,
        ),
        ("_rels/.rels", PACKAGE_RELS.as_bytes(), Method::Deflated),
        ("word/document.xml", document.as_bytes(), Method::Deflated),
    ])
    .unwrap()
}

fn edited_session(bytes: &[u8]) -> DocxSession {
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
                text: "来源".into(),
            }],
        ))
        .unwrap();
    session
}

#[test]
fn session_plan_origin_matches_the_opened_file() {
    for id in 1..=5 {
        let bytes = fixture(id);
        let session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        let plan = session.export_plan().unwrap();
        assert_eq!(plan.origin_bytes(), Some(bytes.as_slice()));
        assert_eq!(
            plan.origin_bytes(),
            Some(session.backing().original_bytes())
        );
        assert_eq!(plan.bytes(), bytes.as_slice());
        assert_eq!(plan.mode(), ExportMode::OriginalPackage);
        let cloned = plan.clone();
        assert_eq!(cloned.origin_bytes(), plan.origin_bytes());
    }
}

#[test]
fn edited_sessions_keep_origin_when_versions_match() {
    let alpha = package("Alpha");
    let beta = package("Beta");
    let left = edited_session(&alpha);
    let right = edited_session(&beta);
    let left_plan = left.export_plan().unwrap();
    let right_plan = right.export_plan().unwrap();
    assert_eq!(left_plan.version(), right_plan.version());
    let unedited = DocxSession::open(&alpha, &Budget::default()).unwrap();
    assert_ne!(
        left_plan.version(),
        unedited.export_plan().unwrap().version()
    );
    assert_eq!(left_plan.origin_bytes(), Some(alpha.as_slice()));
    assert_eq!(right_plan.origin_bytes(), Some(beta.as_slice()));
    assert_ne!(left_plan.origin_bytes(), right_plan.origin_bytes());
    assert_ne!(left_plan.bytes(), left_plan.origin_bytes().unwrap());
    assert_eq!(left_plan.mode(), ExportMode::TextPatch);
    assert_eq!(
        left.backing().original_bytes(),
        left_plan.origin_bytes().unwrap()
    );
}

#[test]
fn open_source_bytes_round_trip_into_the_plan_origin() {
    let bytes = package("打开");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opened.docx");
    fs::write(&path, &bytes).unwrap();
    let opened = open_source(&path, &Budget::default()).unwrap();
    let session = DocxSession::open(opened.bytes(), &Budget::default()).unwrap();
    let plan = session.export_plan().unwrap();
    assert_eq!(plan.origin_bytes(), Some(opened.bytes()));
    assert_eq!(
        plan.origin_bytes(),
        Some(fs::read(&path).unwrap().as_slice())
    );
    let truncated = &bytes[..bytes.len() - 1];
    assert_ne!(plan.origin_bytes(), Some(truncated));
    assert!(DocxSession::open(truncated, &Budget::default()).is_err());
}

#[test]
fn save_export_copy_writes_output_and_save_original_still_refuses() {
    let bytes = package("副本");
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.docx");
    fs::write(&source, &bytes).unwrap();
    let opened = open_source(&source, &Budget::default()).unwrap();
    let session = edited_session(opened.bytes());
    let plan = session.export_plan().unwrap();
    assert_eq!(plan.origin_bytes(), Some(bytes.as_slice()));
    assert_ne!(plan.bytes(), bytes.as_slice());

    let copy = dir.path().join("copy.docx");
    let receipt = save_export_copy(&plan, &copy).unwrap();
    assert_eq!(receipt.bytes(), plan.bytes().len() as u64);
    assert_eq!(fs::read(&copy).unwrap(), plan.bytes());
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(plan.origin_bytes(), Some(bytes.as_slice()));

    let stamp = opened.into_stamp();
    let error = save_original(&plan, &stamp).unwrap_err();
    let blockers = error.blockers().expect("replacement still blocked");
    assert!(!blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
    assert!(blockers.contains(&ReplacementBlocker::ConfirmationRequired));
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        2,
        "refusal must not leave a temporary file beside source and copy"
    );
}
