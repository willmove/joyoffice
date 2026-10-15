//! W10: `plan_export` binds each plan to the backing's original package bytes.
//! The binding is those bytes, not the document version and not the patched
//! archive. Callers cannot install a different origin.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{ExportMode, ExportPlan, ExportWarning, ImportResult, W, import, plan_export};
use joyword_model::{Block, Document, ScalarOffset};

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn fixture(id: u8) -> Vec<u8> {
    std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

fn open(bytes: &[u8]) -> ImportResult {
    import(bytes, &Budget::default()).unwrap()
}

fn plan_of(bytes: &[u8]) -> (ImportResult, ExportPlan) {
    let source = open(bytes);
    let plan = plan_export(
        &source.backing,
        &source.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    (source, plan)
}

fn insert_prefix(model: &mut Document) {
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(0), "绑定")
            .unwrap();
    });
}

#[test]
fn unchanged_plan_origin_is_the_opened_package() {
    let bytes = fixture(1);
    let (source, plan) = plan_of(&bytes);
    assert_eq!(plan.mode(), ExportMode::OriginalPackage);
    assert!(plan.warnings().is_empty());
    assert!(plan.changed_parts().is_empty());
    assert_eq!(plan.origin_bytes(), Some(bytes.as_slice()));
    assert_eq!(plan.origin_bytes(), Some(source.backing.original_bytes()));
    assert_eq!(plan.bytes(), bytes.as_slice());
    assert_eq!(plan.version(), source.document.version());
}

#[test]
fn same_version_different_packages_have_different_origins() {
    let alpha = package("Alpha");
    let beta = package("Beta");
    let (_, left) = plan_of(&alpha);
    let (_, right) = plan_of(&beta);
    assert_eq!(left.version(), right.version());
    assert_ne!(left.origin_bytes(), right.origin_bytes());
    assert_eq!(left.origin_bytes(), Some(alpha.as_slice()));
    assert_eq!(right.origin_bytes(), Some(beta.as_slice()));
    assert_ne!(left.origin_bytes(), Some(beta.as_slice()));
}

#[test]
fn truncated_or_repacked_bytes_are_not_the_origin() {
    let bytes = package("Alpha");
    let (_, plan) = plan_of(&bytes);
    let truncated = &bytes[..bytes.len() - 1];
    assert_ne!(plan.origin_bytes(), Some(truncated));
    assert!(import(truncated, &Budget::default()).is_err());

    let repacked = package("Alpha!");
    assert_eq!(plan.origin_bytes().unwrap().len(), bytes.len());
    assert_ne!(plan.origin_bytes(), Some(repacked.as_slice()));
}

#[test]
fn edited_output_keeps_the_original_package_as_origin() {
    let bytes = package("Alpha");
    let (mut source, before) = plan_of(&bytes);
    let version = source.document.version();
    insert_prefix(&mut source.document);
    let edited = plan_export(
        &source.backing,
        &source.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(edited.version(), version);
    assert_eq!(before.version(), version);
    assert_eq!(edited.mode(), ExportMode::TextPatch);
    assert_eq!(edited.warnings(), &[ExportWarning::ZipMetadataRegenerated]);
    assert_eq!(edited.origin_bytes(), Some(bytes.as_slice()));
    assert_eq!(edited.origin_bytes(), before.origin_bytes());
    assert_ne!(edited.bytes(), edited.origin_bytes().unwrap());
    assert_ne!(edited.bytes(), before.bytes());

    let cloned = edited.clone();
    assert_eq!(cloned.origin_bytes(), edited.origin_bytes());
    assert_eq!(cloned.bytes(), edited.bytes());
    assert_eq!(cloned.version(), edited.version());
    assert_eq!(cloned.mode(), edited.mode());
    assert_eq!(cloned.warnings(), edited.warnings());

    let rebound = plan_export(
        &open(edited.bytes()).backing,
        &open(edited.bytes()).document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(rebound.origin_bytes(), Some(edited.bytes()));
    assert_ne!(rebound.origin_bytes(), Some(bytes.as_slice()));
}
