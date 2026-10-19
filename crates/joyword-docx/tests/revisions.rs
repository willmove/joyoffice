//! T300: move pairs survive a settings patch and a rewritten paragraph.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{ExportMode, import, plan_export};
use joyword_model::{Block, RevisionKind, ScalarOffset};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;

fn package(body: &str, settings: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}">{settings}</w:settings>"#
    );
    let items = [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", document.as_str()),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS),
        ("word/settings.xml", settings.as_str()),
    ];
    let stored: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

fn move_body() -> &'static str {
    concat!(
        r#"<w:p>"#,
        r#"<w:moveFromRangeStart w:id="1" w:author="A" w:name="m" w:date="2020-01-02T03:04:05Z"/>"#,
        r#"<w:moveFrom w:id="2" w:author="A" w:date="2020-01-02T03:04:05Z"><w:r><w:delText>旧</w:delText></w:r></w:moveFrom>"#,
        r#"<w:moveFromRangeEnd w:id="3" w:author="A" w:name="m" w:date="2020-01-02T03:04:05Z"/>"#,
        r#"<w:moveToRangeStart w:id="4" w:author="A" w:name="m" w:date="2020-01-02T03:04:05Z"/>"#,
        r#"<w:moveTo w:id="5" w:author="A" w:date="2020-01-02T03:04:05Z"><w:r><w:t>新</w:t></w:r></w:moveTo>"#,
        r#"<w:moveToRangeEnd w:id="6" w:author="A" w:name="m" w:date="2020-01-02T03:04:05Z"/>"#,
        r#"</w:p>"#,
    )
}

fn kinds(document: &joyword_model::Document) -> Vec<RevisionKind> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| run.revision.as_ref().map(|mark| mark.kind))
        .collect()
}

fn texts(document: &joyword_model::Document) -> Vec<String> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

#[test]
fn a_settings_patch_keeps_the_move_pair() {
    let bytes = package(move_body(), "<w:trackRevisions/>");
    let source = import(&bytes, &Budget::default()).unwrap();
    assert_eq!(source.document.move_markers().len(), 4);
    assert_eq!(
        kinds(&source.document),
        vec![RevisionKind::MoveFrom, RevisionKind::MoveTo]
    );
    assert!(!source.editability.is_read_only());

    let mut turned_off = source.document.clone();
    turned_off.settings_mut().track_revisions = Some(false);
    let patched = plan_export(&source.backing, &turned_off.snapshot(), &Budget::default()).unwrap();
    assert_eq!(patched.mode(), ExportMode::TextPatch);
    assert!(
        patched
            .changed_parts()
            .iter()
            .any(|part| part.as_str().ends_with("settings.xml"))
    );
    assert!(
        patched
            .changed_parts()
            .iter()
            .all(|part| !part.as_str().ends_with("document.xml"))
    );
    let again = import(patched.bytes(), &Budget::default()).unwrap();
    assert_eq!(again.document.settings().track_revisions, Some(false));
    assert_eq!(again.document.move_markers().len(), 4);
    assert_eq!(
        kinds(&again.document),
        vec![RevisionKind::MoveFrom, RevisionKind::MoveTo]
    );
    assert!(
        again
            .document
            .move_markers()
            .iter()
            .all(|marker| marker.name.as_ref() == "m")
    );
    assert!(turned_off.semantic_difference(&again.document).is_none());
}

#[test]
fn rewriting_a_move_paragraph_keeps_both_ends() {
    let bytes = package(move_body(), "<w:trackRevisions/>");
    let source = import(&bytes, &Budget::default()).unwrap();
    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .push_text("尾", None, Arc::default())
            .unwrap();
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(texts(&saved.document), vec!["旧新尾".to_string()]);
    assert_eq!(saved.document.move_markers().len(), 4);
    assert_eq!(
        kinds(&saved.document),
        vec![RevisionKind::MoveFrom, RevisionKind::MoveTo]
    );
    assert!(model.semantic_difference(&saved.document).is_none());
}

#[test]
fn an_accepted_move_exports_without_leaving_half_the_pair() {
    let bytes = package(move_body(), "<w:trackRevisions/>");
    let source = import(&bytes, &Budget::default()).unwrap();
    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        let paragraph = Arc::make_mut(paragraph);
        paragraph
            .set_revision(ScalarOffset(1)..ScalarOffset(2), None)
            .unwrap();
        paragraph.remove(ScalarOffset(0)..ScalarOffset(1)).unwrap();
    });
    model.move_markers_mut().clear();
    assert_eq!(texts(&model), vec!["新".to_string()]);
    assert!(kinds(&model).is_empty());

    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(texts(&saved.document), vec!["新".to_string()]);
    assert!(saved.document.move_markers().is_empty());
    assert!(kinds(&saved.document).is_empty());
    assert!(model.semantic_difference(&saved.document).is_none());
    assert!(!saved.editability.is_read_only());
}
