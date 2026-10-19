//! Simple `w:hyperlink` import and export (ECMA-376 Part 1 §17.16.22).

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{DiagnosticKind, ExportError, ExportMode, import, plan_export};
use joyword_model::{Block, Hyperlink, OpenRefusal, ScalarOffset, open_target};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

fn content_types() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_owned()
}

fn package(document: &str, rels: &str) -> Vec<u8> {
    let package_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let names = [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
    ];
    let owned = [
        content_types().into_bytes(),
        package_rels.into_bytes(),
        document.as_bytes().to_vec(),
        rels.as_bytes().to_vec(),
    ];
    let items: Vec<(&str, &[u8], Method)> = names
        .iter()
        .zip(owned.iter())
        .map(|(name, data)| (*name, data.as_slice(), Method::Stored))
        .collect();
    write_archive(&items).unwrap()
}

fn document(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    )
}

fn rels(id: &str, target: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{REL}"><Relationship Id="{id}" Type="{HYPERLINK}" Target="{target}" TargetMode="External"/></Relationships>"#
    )
}

fn part_text(bytes: &[u8], name: &str) -> String {
    let package = Package::read(bytes, &Budget::default()).unwrap();
    let part = package
        .part(name)
        .unwrap_or_else(|| panic!("missing {name}"));
    String::from_utf8(part.data.to_vec()).unwrap()
}

fn first_link(result: &joyword_docx::ImportResult) -> Hyperlink {
    let Block::Paragraph(paragraph) = result.document.body().blocks.get(0).unwrap() else {
        panic!("paragraph");
    };
    paragraph.link_at(0).cloned().expect("link")
}

#[test]
fn an_external_hyperlink_imports_without_being_fetched() {
    let bytes = package(
        &document(
            r#"<w:p><w:hyperlink r:id="rId2"><w:r><w:t>项目说明</w:t></w:r></w:hyperlink></w:p>"#,
        ),
        &rels("rId2", "https://example.invalid/joyword"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let link = first_link(&result);
    assert_eq!(link.relationship.as_deref(), Some("rId2"));
    assert_eq!(
        link.external.as_deref(),
        Some("https://example.invalid/joyword")
    );
    assert!(open_target(&link).is_ok());
    let paragraph = result.document.body().blocks.get(0).unwrap().id();
    assert!(result.editability.can_edit(paragraph));
    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(plan.mode(), ExportMode::OriginalPackage);
    assert_eq!(plan.bytes(), bytes.as_slice());
}

#[test]
fn javascript_and_file_targets_import_and_are_not_openable() {
    let bytes = package(
        &document(r#"<w:p><w:hyperlink r:id="rId2"><w:r><w:t>go</w:t></w:r></w:hyperlink></w:p>"#),
        &rels("rId2", "javascript:alert(1)"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let link = first_link(&result);
    assert_eq!(link.external.as_deref(), Some("javascript:alert(1)"));
    assert_eq!(open_target(&link), Err(OpenRefusal::Scheme));

    let file = package(
        &document(r#"<w:p><w:hyperlink r:id="rId2"><w:r><w:t>go</w:t></w:r></w:hyperlink></w:p>"#),
        &rels("rId2", "file://fileserver.invalid/share"),
    );
    let result = import(&file, &Budget::default()).unwrap();
    assert_eq!(open_target(&first_link(&result)), Err(OpenRefusal::Scheme));
}

#[test]
fn an_anchor_imports_and_is_not_a_url() {
    let bytes = package(
        &document(
            r#"<w:p><w:hyperlink w:anchor="top"><w:r><w:t>top</w:t></w:r></w:hyperlink></w:p>"#,
        ),
        &format!(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"/>"#),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let link = first_link(&result);
    assert!(link.external.is_none());
    assert_eq!(link.anchor.as_deref(), Some("top"));
    assert_eq!(open_target(&link), Err(OpenRefusal::NoExternalTarget));
}

#[test]
fn a_tooltip_stays_preserved_and_locks_the_paragraph() {
    let bytes = package(
        &document(
            r#"<w:p><w:hyperlink r:id="rId2" w:tooltip="hint"><w:r><w:t>go</w:t></w:r></w:hyperlink></w:p>"#,
        ),
        &rels("rId2", "https://example.invalid/joyword"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let Block::Paragraph(paragraph) = result.document.body().blocks.get(0).unwrap() else {
        panic!("paragraph");
    };
    assert!(paragraph.link_at(0).is_none());
    assert!(!result.editability.can_edit(paragraph.id()));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| matches!(item.kind, DiagnosticKind::Preserved { .. }))
    );
}

#[test]
fn editing_link_text_keeps_the_target_and_a_new_target_is_appended() {
    let bytes = package(
        &document(
            r#"<w:p><w:hyperlink r:id="rId2"><w:r><w:t>项目说明</w:t></w:r></w:hyperlink></w:p>"#,
        ),
        &rels("rId2", "https://example.invalid/joyword"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let mut edited = result.document.clone();
    let body = edited.body_id();
    edited.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(4), "!")
            .unwrap();
    });
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let xml = part_text(plan.bytes(), "/word/document.xml");
    assert!(xml.contains("项目说明!"), "{xml}");
    assert!(xml.contains("w:hyperlink"), "{xml}");
    assert!(xml.contains("r:id=\"rId2\""), "{xml}");
    let rels = part_text(plan.bytes(), "/word/_rels/document.xml.rels");
    assert!(rels.contains("https://example.invalid/joyword"), "{rels}");

    let mut retargeted = result.document.clone();
    let body = retargeted.body_id();
    retargeted
        .story_mut(body)
        .unwrap()
        .blocks
        .update(0, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            let link =
                Hyperlink::for_edit(Some("https://example.invalid/new"), Some("anchor")).unwrap();
            Arc::make_mut(paragraph)
                .set_links(ScalarOffset(0)..ScalarOffset(4), Some(Arc::new(link)))
                .unwrap();
        });
    let plan = plan_export(&result.backing, &retargeted.snapshot(), &Budget::default()).unwrap();
    let xml = part_text(plan.bytes(), "/word/document.xml");
    let rels = part_text(plan.bytes(), "/word/_rels/document.xml.rels");
    assert!(xml.contains("w:hyperlink"), "{xml}");
    assert!(xml.contains("w:anchor=\"anchor\""), "{xml}");
    assert!(rels.contains("https://example.invalid/new"), "{rels}");
    assert!(
        rels.contains("https://example.invalid/joyword"),
        "the original relationship stays in the part: {rels}"
    );
}

#[test]
fn removing_a_hyperlink_does_not_leave_the_wrapper() {
    let bytes = package(
        &document(r#"<w:p><w:hyperlink r:id="rId2"><w:r><w:t>go</w:t></w:r></w:hyperlink></w:p>"#),
        &rels("rId2", "https://example.invalid/joyword"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let mut edited = result.document.clone();
    let body = edited.body_id();
    edited.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .set_links(ScalarOffset(0)..ScalarOffset(2), None)
            .unwrap();
    });
    match plan_export(&result.backing, &edited.snapshot(), &Budget::default()) {
        Ok(plan) => {
            let xml = part_text(plan.bytes(), "/word/document.xml");
            assert!(!xml.contains("w:hyperlink"), "{xml}");
            assert!(xml.contains(">go<"), "{xml}");
        }
        Err(ExportError::Unsupported { reason, .. }) => {
            assert!(
                reason.contains("hyperlink"),
                "a link that cannot be written is refused, not dropped: {reason}"
            );
        }
        Err(other) => panic!("{other:?}"),
    }
}

fn one_cell_table(body: &str) -> String {
    format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:p>{body}</w:p></w:tc></w:tr></w:tbl>"#
    )
}

fn cell_link(result: &joyword_docx::ImportResult) -> Hyperlink {
    let Block::Table(table) = result.document.body().blocks.get(0).unwrap() else {
        panic!("typed table");
    };
    table.rows[0].cells[0]
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .link_at(0)
        .cloned()
        .expect("cell hyperlink")
}

fn edit_cell_text(model: &mut joyword_model::Document, text: &str) {
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        Arc::make_mut(table).rows[0].cells[0]
            .blocks
            .update(0, |block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("paragraph");
                };
                Arc::make_mut(paragraph)
                    .push_text(text, None, Arc::default())
                    .unwrap();
            });
    });
}

#[test]
fn a_table_cell_hyperlink_round_trips_after_a_cell_edit() {
    let bytes = package(
        &document(&one_cell_table(
            r#"<w:hyperlink r:id="rId2"><w:r><w:t>条款</w:t></w:r></w:hyperlink>"#,
        )),
        &rels("rId2", "https://example.invalid/terms"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let link = cell_link(&result);
    assert_eq!(link.relationship.as_deref(), Some("rId2"));
    assert_eq!(
        link.external.as_deref(),
        Some("https://example.invalid/terms")
    );
    let Block::Table(table) = result.document.body().blocks.get(0).unwrap() else {
        panic!("typed table");
    };
    let paragraph = table.rows[0].cells[0].blocks.get(0).unwrap().id();
    assert!(result.editability.can_edit(paragraph));

    let mut edited = result.document.clone();
    edit_cell_text(&mut edited, "附则");
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let xml = part_text(plan.bytes(), "/word/document.xml");
    assert!(xml.contains("附则"), "{xml}");
    assert!(xml.contains("w:hyperlink"), "{xml}");
    assert!(xml.contains("r:id=\"rId2\""), "{xml}");
    let rels = part_text(plan.bytes(), "/word/_rels/document.xml.rels");
    assert!(rels.contains("https://example.invalid/terms"), "{rels}");
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(edited.semantic_difference(&saved.document), None);
}

#[test]
fn a_new_hyperlink_target_in_a_cell_appends_a_relationship() {
    let bytes = package(
        &document(&one_cell_table(
            r#"<w:hyperlink r:id="rId2"><w:r><w:t>go</w:t></w:r></w:hyperlink>"#,
        )),
        &rels("rId2", "https://example.invalid/old"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let mut retargeted = result.document.clone();
    let body = retargeted.body_id();
    retargeted
        .story_mut(body)
        .unwrap()
        .blocks
        .update(0, |block| {
            let Block::Table(table) = block else {
                panic!("table");
            };
            Arc::make_mut(table).rows[0].cells[0]
                .blocks
                .update(0, |block| {
                    let Block::Paragraph(paragraph) = block else {
                        panic!("paragraph");
                    };
                    let link =
                        Hyperlink::for_edit(Some("https://example.invalid/new"), None).unwrap();
                    Arc::make_mut(paragraph)
                        .set_links(ScalarOffset(0)..ScalarOffset(2), Some(Arc::new(link)))
                        .unwrap();
                });
        });
    let plan = plan_export(&result.backing, &retargeted.snapshot(), &Budget::default()).unwrap();
    let xml = part_text(plan.bytes(), "/word/document.xml");
    let rels = part_text(plan.bytes(), "/word/_rels/document.xml.rels");
    assert!(xml.contains("w:hyperlink"), "{xml}");
    assert!(rels.contains("https://example.invalid/new"), "{rels}");
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    let link = cell_link(&saved);
    assert_eq!(
        link.external.as_deref(),
        Some("https://example.invalid/new")
    );
    assert_eq!(retargeted.semantic_difference(&saved.document), None);
}

#[test]
fn an_anchor_only_cell_hyperlink_round_trips() {
    let bytes = package(
        &document(&one_cell_table(
            r#"<w:hyperlink w:anchor="bookmark1"><w:r><w:t>跳</w:t></w:r></w:hyperlink>"#,
        )),
        &format!(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"/>"#),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let link = cell_link(&result);
    assert!(link.external.is_none());
    assert_eq!(link.anchor.as_deref(), Some("bookmark1"));

    let mut edited = result.document.clone();
    edit_cell_text(&mut edited, "转");
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let xml = part_text(plan.bytes(), "/word/document.xml");
    assert!(xml.contains(r#"w:anchor="bookmark1""#), "{xml}");
    assert!(
        !xml.contains("r:id="),
        "anchor-only links write no r:id: {xml}"
    );
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(edited.semantic_difference(&saved.document), None);
}

#[test]
fn a_complex_hyperlink_keeps_the_table_opaque() {
    let bytes = package(
        &document(&one_cell_table(
            r#"<w:hyperlink r:id="rId2" w:tooltip="hint"><w:r><w:t>go</w:t></w:r></w:hyperlink>"#,
        )),
        &rels("rId2", "https://example.invalid/joyword"),
    );
    let result = import(&bytes, &Budget::default()).unwrap();
    let block = result.document.body().blocks.get(0).unwrap();
    assert!(
        matches!(block, Block::Opaque(opaque) if &*opaque.preserved.element == "w:tbl"),
        "a cell link outside the subset preserves the whole table"
    );
    assert!(!result.editability.can_edit(block.id()));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| matches!(item.kind, DiagnosticKind::Preserved { .. }))
    );
}
