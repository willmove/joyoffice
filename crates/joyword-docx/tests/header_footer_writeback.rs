//! Header and footer part writeback (ECMA-376 Part 1 §17.11.2, §17.11.3).
//!
//! Text edits use the same paragraph operations as the engine story path
//! (`remove` then `insert_text`).

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ExportBinding, ExportError, ExportMode, import, plan_export};
use joyword_model::{
    Block, Document, HeaderFooterKind, Hyperlink, ScalarOffset, StoryId, StoryKind,
};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const HEADER_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const FOOTER_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
const HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

fn archive(items: &[(&str, &str)]) -> Vec<u8> {
    let owned: Vec<(&str, Vec<u8>)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes().to_vec()))
        .collect();
    let packed: Vec<(&str, &[u8], Method)> = owned
        .iter()
        .map(|(name, data)| (*name, data.as_slice(), Method::Stored))
        .collect();
    write_archive(&packed).unwrap()
}

fn content_types(overrides: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>{overrides}</Types>"#
    )
}

fn story_part(local: &str, inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:{local} xmlns:w="{W}" xmlns:r="{R}">{inner}</w:{local}>"#
    )
}

fn document(rels_inner: &str, body: &str) -> (String, String) {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{REL}">{rels_inner}</Relationships>"#
    );
    (document, rels)
}

fn sect(inner: &str) -> String {
    format!(
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/>{inner}</w:sectPr>"#
    )
}

fn part_bytes(package: &[u8], name: &str) -> Vec<u8> {
    Package::read(package, &Budget::default())
        .unwrap()
        .part(name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .data
        .as_ref()
        .to_vec()
}

fn replace_text(document: &mut Document, story: StoryId, text: &str) {
    document
        .story_mut(story)
        .unwrap()
        .blocks
        .update(0, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            let paragraph = Arc::make_mut(paragraph);
            let end = ScalarOffset(paragraph.len());
            paragraph.remove(ScalarOffset(0)..end).unwrap();
            if !text.is_empty() {
                paragraph.insert_text(ScalarOffset(0), text).unwrap();
            }
        });
}

fn story_text(document: &Document, story: StoryId) -> String {
    let story = document.story(story).unwrap();
    story
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| "\u{FFFC}".into()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn story_of(document: &Document, kind: HeaderFooterKind, header: bool) -> StoryId {
    let section = &document.sections()[0];
    let list = if header {
        &section.headers
    } else {
        &section.footers
    };
    list.iter()
        .find(|(item, _)| *item == kind)
        .map(|(_, id)| *id)
        .unwrap_or_else(|| panic!("missing {kind:?}"))
}

#[test]
fn edited_header_and_footer_text_round_trips_and_keeps_relationships() {
    let header = story_part("hdr", r#"<w:p><w:r><w:t>H</w:t></w:r></w:p>"#);
    let footer = story_part("ftr", r#"<w:p><w:r><w:t>F</w:t></w:r></w:p>"#);
    let references = r#"<w:headerReference w:type="default" r:id="rId2"/><w:footerReference w:type="default" r:id="rId3"/>"#;
    let (document, rels) = document(
        &format!(
            r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/><Relationship Id="rId3" Type="{FOOTER_REL}" Target="footer1.xml"/>"#
        ),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(references)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &header),
        ("word/footer1.xml", &footer),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(result.editability.is_unrestricted());
    let unchanged = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(unchanged.mode(), ExportMode::OriginalPackage);
    assert_eq!(unchanged.bytes(), bytes.as_slice());

    let header_id = story_of(&result.document, HeaderFooterKind::Default, true);
    let footer_id = story_of(&result.document, HeaderFooterKind::Default, false);
    let mut edited = result.document.clone();
    replace_text(&mut edited, header_id, "页眉");
    replace_text(&mut edited, footer_id, "页脚");
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default())
        .expect("header and footer writeback");
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let changed: Vec<_> = plan
        .changed_parts()
        .iter()
        .map(|part| part.as_str().to_string())
        .collect();
    assert!(
        changed.iter().any(|part| part == "/word/header1.xml"),
        "{changed:?}"
    );
    assert!(
        changed.iter().any(|part| part == "/word/footer1.xml"),
        "{changed:?}"
    );
    assert!(
        !changed.iter().any(|part| part.ends_with(".rels")),
        "text edits do not rewrite relationship parts: {changed:?}"
    );
    let header_xml = String::from_utf8(part_bytes(plan.bytes(), "/word/header1.xml")).unwrap();
    let footer_xml = String::from_utf8(part_bytes(plan.bytes(), "/word/footer1.xml")).unwrap();
    assert!(header_xml.contains("页眉"), "{header_xml}");
    assert!(footer_xml.contains("页脚"), "{footer_xml}");
    assert_eq!(
        part_bytes(plan.bytes(), "/word/_rels/document.xml.rels"),
        rels.as_bytes()
    );
    let again = import(plan.bytes(), &Budget::default()).unwrap();
    let again_header = story_of(&again.document, HeaderFooterKind::Default, true);
    let again_footer = story_of(&again.document, HeaderFooterKind::Default, false);
    assert_eq!(story_text(&again.document, again_header), "页眉");
    assert_eq!(story_text(&again.document, again_footer), "页脚");
    assert_eq!(again.document.sections()[0].headers.len(), 1);
    assert_eq!(again.document.sections()[0].footers.len(), 1);

    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    let binding =
        ExportBinding::rebind(saved.backing, &edited.snapshot(), &Budget::default()).unwrap();
    replace_text(&mut edited, header_id, "页眉二");
    let second = binding
        .plan(&edited.snapshot(), &Budget::default())
        .expect("rebound header writeback");
    let rebound = import(second.bytes(), &Budget::default()).unwrap();
    let rebound_header = story_of(&rebound.document, HeaderFooterKind::Default, true);
    assert_eq!(story_text(&rebound.document, rebound_header), "页眉二");
    assert_eq!(
        story_text(
            &rebound.document,
            story_of(&rebound.document, HeaderFooterKind::Default, false)
        ),
        "页脚"
    );
}

#[test]
fn a_body_edit_copies_an_unchanged_header_part() {
    let header = story_part("hdr", r#"<w:p><w:r><w:t>Keep</w:t></w:r></w:p>"#);
    let (document, rels) = document(
        &format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(r#"<w:headerReference w:type="default" r:id="rId2"/>"#)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &header),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    let mut edited = result.document.clone();
    let body = edited.body_id();
    replace_text(&mut edited, body, "正文");
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    assert_eq!(
        part_bytes(plan.bytes(), "/word/header1.xml"),
        header.as_bytes()
    );
    assert!(
        !plan
            .changed_parts()
            .iter()
            .any(|part| part.as_str() == "/word/header1.xml")
    );
}

#[test]
fn first_and_even_writeback_keeps_the_other_part_and_an_empty_story() {
    let first = story_part("hdr", "<w:p/>");
    let even = story_part("hdr", r#"<w:p><w:r><w:t>Even</w:t></w:r></w:p>"#);
    let default_header = story_part("hdr", r#"<w:p><w:r><w:t>Default</w:t></w:r></w:p>"#);
    let references = r#"<w:headerReference w:type="first" r:id="rId2"/><w:headerReference w:type="even" r:id="rId3"/><w:headerReference w:type="default" r:id="rId4"/><w:titlePg/>"#;
    let (document, rels) = document(
        &format!(
            r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/><Relationship Id="rId3" Type="{HEADER_REL}" Target="header2.xml"/><Relationship Id="rId4" Type="{HEADER_REL}" Target="header3.xml"/>"#
        ),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(references)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header2.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/header3.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &first),
        ("word/header2.xml", &even),
        ("word/header3.xml", &default_header),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(result.editability.is_unrestricted());
    let first_id = story_of(&result.document, HeaderFooterKind::First, true);
    let even_id = story_of(&result.document, HeaderFooterKind::Even, true);
    let default_id = story_of(&result.document, HeaderFooterKind::Default, true);
    assert_eq!(story_text(&result.document, first_id), "");
    assert_ne!(first_id, default_id);
    let mut edited = result.document.clone();
    replace_text(&mut edited, default_id, "已改");
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    assert_eq!(
        part_bytes(plan.bytes(), "/word/header1.xml"),
        first.as_bytes()
    );
    assert_eq!(
        part_bytes(plan.bytes(), "/word/header2.xml"),
        even.as_bytes()
    );
    let written = String::from_utf8(part_bytes(plan.bytes(), "/word/header3.xml")).unwrap();
    assert!(written.contains("已改"), "{written}");
    assert!(!written.contains("Even"), "{written}");
    let again = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(
        story_text(
            &again.document,
            story_of(&again.document, HeaderFooterKind::First, true)
        ),
        ""
    );
    assert_eq!(
        story_text(
            &again.document,
            story_of(&again.document, HeaderFooterKind::Even, true)
        ),
        "Even"
    );
    assert_eq!(
        story_text(
            &again.document,
            story_of(&again.document, HeaderFooterKind::Default, true)
        ),
        "已改"
    );
    assert_eq!(
        again
            .document
            .stories()
            .filter(|story| story.kind == StoryKind::Header)
            .count(),
        3
    );
    let _ = even_id;
}

#[test]
fn an_empty_header_part_gains_plain_text_without_a_new_part() {
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr xmlns:w="{W}" xmlns:r="{R}"/>"#
    );
    let (document, rels) = document(
        &format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(r#"<w:headerReference w:type="default" r:id="rId2"/>"#)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &header),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    let header_id = story_of(&result.document, HeaderFooterKind::Default, true);
    assert_eq!(story_text(&result.document, header_id), "");
    let mut edited = result.document.clone();
    replace_text(&mut edited, header_id, "页眉");
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let names: Vec<_> = Package::read(plan.bytes(), &Budget::default())
        .unwrap()
        .parts()
        .map(|part| part.name.as_str().to_string())
        .collect();
    assert_eq!(
        names.iter().filter(|name| name.contains("header")).count(),
        1,
        "{names:?}"
    );
    let written = String::from_utf8(part_bytes(plan.bytes(), "/word/header1.xml")).unwrap();
    assert!(written.contains("页眉"), "{written}");
    assert!(written.contains("w:hdr"), "{written}");
    let again = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(
        story_text(
            &again.document,
            story_of(&again.document, HeaderFooterKind::Default, true)
        ),
        "页眉"
    );
}

#[test]
fn a_page_field_header_is_refused_instead_of_dropping_the_field() {
    let header = story_part(
        "hdr",
        r#"<w:p><w:r><w:t>H</w:t></w:r><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p>"#,
    );
    let (document, rels) = document(
        &format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(r#"<w:headerReference w:type="default" r:id="rId2"/>"#)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &header),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(result.editability.is_unrestricted());
    let header_id = story_of(&result.document, HeaderFooterKind::Default, true);
    let mut edited = result.document.clone();
    replace_text(&mut edited, header_id, "页眉");
    let error = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap_err();
    assert!(matches!(error, ExportError::Unsupported { .. }), "{error}");
}

#[test]
fn a_header_table_is_kept_when_the_paragraph_text_changes() {
    let header = story_part(
        "hdr",
        r#"<w:p><w:r><w:t>H</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>CELL</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    );
    let (document, rels) = document(
        &format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(r#"<w:headerReference w:type="default" r:id="rId2"/>"#)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &header),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(result.editability.is_unrestricted());
    let header_id = story_of(&result.document, HeaderFooterKind::Default, true);
    let mut edited = result.document.clone();
    replace_text(&mut edited, header_id, "页眉");
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let written = String::from_utf8(part_bytes(plan.bytes(), "/word/header1.xml")).unwrap();
    assert!(written.contains("页眉"), "{written}");
    assert!(written.contains("<w:tbl>"), "{written}");
    assert!(written.contains("CELL"), "{written}");
}

#[test]
fn header_link_text_keeps_the_relationship_and_a_new_target_is_appended() {
    let header = story_part(
        "hdr",
        r#"<w:p><w:hyperlink r:id="rId2"><w:r><w:t>项目说明</w:t></w:r></w:hyperlink></w:p>"#,
    );
    let header_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="{REL}"><Relationship Id="rId2" Type="{HYPERLINK}" Target="https://example.invalid/joyword" TargetMode="External"/></Relationships>"#
    );
    let (document, rels) = document(
        &format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(r#"<w:headerReference w:type="default" r:id="rId2"/>"#)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &header),
        ("word/_rels/header1.xml.rels", &header_rels),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    let header_id = story_of(&result.document, HeaderFooterKind::Default, true);
    let mut edited = result.document.clone();
    edited
        .story_mut(header_id)
        .unwrap()
        .blocks
        .update(0, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            Arc::make_mut(paragraph)
                .insert_text(ScalarOffset(4), "!")
                .unwrap();
        });
    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let xml = String::from_utf8(part_bytes(plan.bytes(), "/word/header1.xml")).unwrap();
    assert!(xml.contains("项目说明!"), "{xml}");
    assert!(xml.contains("r:id=\"rId2\""), "{xml}");
    assert_eq!(
        part_bytes(plan.bytes(), "/word/_rels/header1.xml.rels"),
        header_rels.as_bytes()
    );
    assert_eq!(
        part_bytes(plan.bytes(), "/word/_rels/document.xml.rels"),
        rels.as_bytes()
    );

    let mut retargeted = result.document.clone();
    retargeted
        .story_mut(header_id)
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
    let rels_xml =
        String::from_utf8(part_bytes(plan.bytes(), "/word/_rels/header1.xml.rels")).unwrap();
    assert!(
        rels_xml.contains("https://example.invalid/new"),
        "{rels_xml}"
    );
    assert!(
        rels_xml.contains("https://example.invalid/joyword"),
        "{rels_xml}"
    );
    assert!(
        plan.changed_parts()
            .iter()
            .any(|part| part.as_str() == "/word/_rels/header1.xml.rels")
    );
    assert_eq!(
        part_bytes(plan.bytes(), "/word/_rels/document.xml.rels"),
        rels.as_bytes()
    );
}

#[test]
fn adding_a_header_paragraph_is_refused() {
    let header = story_part("hdr", r#"<w:p><w:r><w:t>H</w:t></w:r></w:p>"#);
    let (document, rels) = document(
        &format!(r#"<Relationship Id="rId2" Type="{HEADER_REL}" Target="header1.xml"/>"#),
        &format!(
            r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{}"#,
            sect(r#"<w:headerReference w:type="default" r:id="rId2"/>"#)
        ),
    );
    let types = content_types(
        r#"<Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/header1.xml", &header),
    ]);
    let result = import(&bytes, &Budget::default()).unwrap();
    let mut edited = result.document.clone();
    let header_id = story_of(&edited, HeaderFooterKind::Default, true);
    let paragraph = joyword_model::Paragraph::new(edited.ids().node());
    edited
        .story_mut(header_id)
        .unwrap()
        .blocks
        .push(paragraph.into());
    let error = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap_err();
    assert!(matches!(error, ExportError::Unsupported { .. }), "{error}");
}
