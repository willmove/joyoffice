//! W12: structural export of fully mapped paragraph and run properties.
//!
//! A split, merge, insertion or removal may touch a body paragraph whose
//! `pPr`/`rPr` the model represents completely and that holds no preserved
//! fragment. New paragraphs reuse those source element bytes. Unmapped
//! properties, unknown attributes and preserved markup are still refused.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{
    ExportError, ExportMode, ExportPlan, ImportResult, PackageBacking, W, import, plan_export,
};
use joyword_model::{Block, Document, ParaProps, PreservedId, RunProps, ScalarOffset};

const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const MAIN: &str = "/word/document.xml";
const BODY_SECT: &str = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#;
const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="bin" ContentType="application/octet-stream"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn document_xml(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}" xmlns:mc="{MC}" xmlns:w14="{W14}" xmlns:x="urn:w12:unknown" mc:Ignorable="w14"><w:body>{body}{BODY_SECT}</w:body></w:document>"#
    )
}

fn package(body: &str) -> Vec<u8> {
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.invalid/w12" TargetMode="External"/></Relationships>"#;
    let document = document_xml(body);
    let items: Vec<(&str, &[u8], Method)> = vec![
        (
            "[Content_Types].xml",
            CONTENT_TYPES.as_bytes(),
            Method::Deflated,
        ),
        ("_rels/.rels", PACKAGE_RELS.as_bytes(), Method::Deflated),
        ("word/document.xml", document.as_bytes(), Method::Deflated),
        (
            "word/_rels/document.xml.rels",
            rels.as_bytes(),
            Method::Deflated,
        ),
    ];
    write_archive(&items).unwrap()
}

fn package_document(document: &str) -> Vec<u8> {
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#;
    let items: Vec<(&str, &[u8], Method)> = vec![
        (
            "[Content_Types].xml",
            CONTENT_TYPES.as_bytes(),
            Method::Deflated,
        ),
        ("_rels/.rels", PACKAGE_RELS.as_bytes(), Method::Deflated),
        ("word/document.xml", document.as_bytes(), Method::Deflated),
        (
            "word/_rels/document.xml.rels",
            rels.as_bytes(),
            Method::Deflated,
        ),
    ];
    write_archive(&items).unwrap()
}

fn open(bytes: &[u8]) -> ImportResult {
    import(bytes, &Budget::default()).unwrap()
}

fn texts(document: &Document) -> Vec<String> {
    document
        .body()
        .blocks
        .iter()
        .map(|block| {
            block
                .as_paragraph()
                .expect("body paragraph")
                .plain_text(|_| "\u{FFFC}".into())
        })
        .collect()
}

fn split(model: &mut Document, index: usize, at: usize) {
    let id = model.ids().node();
    let body = model.body_id();
    let blocks = &mut model.story_mut(body).unwrap().blocks;
    let tail = blocks.update(index, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph {index}");
        };
        Arc::make_mut(paragraph)
            .split_off(ScalarOffset(at), id)
            .unwrap()
    });
    blocks.insert(index + 1, tail.into());
}

fn merge(model: &mut Document, index: usize) {
    let body = model.body_id();
    let blocks = &mut model.story_mut(body).unwrap().blocks;
    let Block::Paragraph(next) = blocks.remove(index + 1) else {
        panic!("paragraph {}", index + 1);
    };
    blocks.update(index, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph {index}");
        };
        Arc::make_mut(paragraph).append(Arc::unwrap_or_clone(next));
    });
}

fn remove(model: &mut Document, index: usize) {
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.remove(index);
}

fn insert(model: &mut Document, index: usize, at: usize, text: &str) {
    let body = model.body_id();
    model
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph {index}");
            };
            Arc::make_mut(paragraph)
                .insert_text(ScalarOffset(at), text)
                .unwrap();
        });
}

fn export(source: &ImportResult, model: &Document) -> Result<ExportPlan, ExportError> {
    plan_export(&source.backing, &model.snapshot(), &Budget::default())
}

fn main_xml(bytes: &[u8]) -> String {
    let package = Package::read(bytes, &Budget::default()).unwrap();
    String::from_utf8(package.part(MAIN).unwrap().data.to_vec()).unwrap()
}

fn content(document: &Document, backing: &PackageBacking) -> Vec<String> {
    let kept = |handle: Option<PreservedId>| {
        handle.map(|handle| backing.fragment_bytes(handle).unwrap().to_vec())
    };
    document
        .body()
        .blocks
        .iter()
        .map(|block| {
            let paragraph = block.as_paragraph().expect("body paragraph");
            let mut props = ParaProps::clone(&paragraph.props);
            let props_kept = kept(props.preserved.take());
            let mut mark = RunProps::clone(&paragraph.mark_props);
            let mark_kept = kept(mark.preserved.take());
            let text = paragraph.plain_text(|_| "\u{FFFC}".into());
            let mut chars = text.chars();
            let mut groups: Vec<(String, Option<Vec<u8>>, String)> = Vec::new();
            for run in paragraph.runs() {
                let mut run_props = RunProps::clone(&run.props);
                let run_kept = kept(run_props.preserved.take());
                let format = format!("{:?} {run_props:?}", run.style);
                let text: String = chars.by_ref().take(run.len).collect();
                match groups.last_mut() {
                    Some((style, kept, existing)) if *style == format && *kept == run_kept => {
                        existing.push_str(&text);
                    }
                    _ => groups.push((format, run_kept, text)),
                }
            }
            format!(
                "{:?} {props:?} {props_kept:?} {mark:?} {mark_kept:?} {groups:?}",
                paragraph.style
            )
        })
        .collect()
}

fn verify(source: &ImportResult, model: &Document, plan: &ExportPlan) -> ImportResult {
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    assert_eq!(plan.version(), model.version());
    // W10: the plan stays bound to the caller's backing, not the patched archive.
    assert_eq!(plan.origin_bytes(), Some(source.backing.original_bytes()));
    assert_ne!(plan.bytes(), source.backing.original_bytes());
    let saved = open(plan.bytes());
    assert!(saved.editability.is_unrestricted());
    assert_eq!(texts(&saved.document), texts(model));
    assert_eq!(
        content(&saved.document, &saved.backing),
        content(model, &source.backing)
    );
    assert_eq!(
        saved
            .document
            .sections()
            .iter()
            .map(|section| section.last_block)
            .collect::<Vec<_>>(),
        model
            .sections()
            .iter()
            .map(|section| section.last_block)
            .collect::<Vec<_>>()
    );
    let fragments = |result: &ImportResult| {
        let mut all: Vec<Vec<u8>> = result
            .backing
            .fragments()
            .map(|(id, _)| result.backing.fragment_bytes(id).unwrap().to_vec())
            .collect();
        all.sort();
        all
    };
    assert_eq!(fragments(source), fragments(&saved));
    saved
}

fn refused(result: Result<ExportPlan, ExportError>) -> &'static str {
    match result {
        Err(ExportError::Unsupported { reason, .. }) => reason,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

const JC: &str = r#"<w:p w:rsidR="00A1B2C3" w14:paraId="1A2B3C4D"><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>甲乙</w:t></w:r></w:p>"#;
const BOLD: &str =
    r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">AB</w:t></w:r></w:p>"#;

#[test]
fn split_copies_paragraph_and_run_property_bytes() {
    let mark = r#"<w:p><w:pPr><w:rPr><w:i/></w:rPr></w:pPr><w:r><w:t>段落标记</w:t></w:r></w:p>"#;
    let style =
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>标题</w:t></w:r></w:p>"#;
    let bytes = package(&format!("{JC}{BOLD}{mark}{style}"));
    let source = open(&bytes);
    let ppr = r#"<w:pPr><w:jc w:val="center"/></w:pPr>"#;
    let rpr = r#"<w:rPr><w:b/></w:rPr>"#;
    let mark_rpr = r#"<w:rPr><w:i/></w:rPr>"#;
    let pstyle = r#"<w:pPr><w:pStyle w:val="Heading1"/></w:pPr>"#;
    for (index, at, property) in [(0, 1, ppr), (1, 1, rpr), (2, 2, mark_rpr), (3, 1, pstyle)] {
        let mut model = source.document.clone();
        split(&mut model, index, at);
        let plan = export(&source, &model).unwrap();
        verify(&source, &model, &plan);
        let xml = main_xml(plan.bytes());
        assert_eq!(xml.matches(property).count(), 2, "{property} in {xml}");
    }
    // The head keeps its identifiers; the new paragraph does not copy them.
    let mut model = source.document.clone();
    split(&mut model, 0, 1);
    let xml = main_xml(export(&source, &model).unwrap().bytes());
    assert_eq!(xml.matches(r#"w:rsidR="00A1B2C3""#).count(), 1);
    assert_eq!(xml.matches("w14:paraId=").count(), 1);
    assert_eq!(source.backing.original_bytes(), bytes);
}

#[test]
fn split_at_edges_inserts_an_empty_paragraph_with_the_same_properties() {
    let bytes = package(JC);
    let source = open(&bytes);
    for at in [0, 2] {
        let mut model = source.document.clone();
        split(&mut model, 0, at);
        let plan = export(&source, &model).unwrap();
        verify(&source, &model, &plan);
        let xml = main_xml(plan.bytes());
        assert_eq!(xml.matches(r#"<w:jc w:val="center"/>"#).count(), 2, "{xml}");
        if at == 2 {
            assert!(
                xml.contains(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr></w:p>"#),
                "{xml}"
            );
        } else {
            assert!(
                xml.contains(r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t xml:space="preserve">甲乙</w:t></w:r></w:p>"#),
                "{xml}"
            );
        }
    }
}

#[test]
fn merge_and_delete_keep_the_survivor_property_bytes() {
    let right = r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>右</w:t></w:r></w:p>"#;
    let right_2 = r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>边</w:t></w:r></w:p>"#;
    let italic = r#"<w:p><w:r><w:rPr><w:i w:val="true"/></w:rPr><w:t>斜</w:t></w:r></w:p>"#;
    let bytes = package(&format!("{right}{right_2}{italic}{BOLD}"));
    let source = open(&bytes);
    let mut model = source.document.clone();
    merge(&mut model, 0);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    assert_eq!(xml.matches(r#"<w:jc w:val="right"/>"#).count(), 1);
    assert!(xml.contains("右边") || (xml.contains('右') && xml.contains('边')));
    // Different run properties: the survivor is rebuilt from the source bytes.
    let mut model = source.document.clone();
    merge(&mut model, 2);
    assert_eq!(texts(&model)[2], "斜AB");
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    assert!(
        xml.contains(r#"<w:rPr><w:i w:val="true"/></w:rPr>"#),
        "{xml}"
    );
    assert!(xml.contains(r#"<w:rPr><w:b/></w:rPr>"#), "{xml}");
    let mut model = source.document.clone();
    remove(&mut model, 0);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    assert!(!xml.contains(">右<"), "{xml}");
    assert_eq!(xml.matches(r#"w:val="right""#).count(), 1, "{xml}");
}

#[test]
fn split_between_run_properties_keeps_each_side() {
    let both = r#"<w:p><w:pPr><w:spacing w:before="240" w:line="360" w:lineRule="auto"/></w:pPr><w:r><w:rPr><w:b/><w:sz w:val="28"/></w:rPr><w:t>粗</w:t></w:r><w:r><w:rPr><w:i/><w:color w:val="FF0000"/></w:rPr><w:t>斜</w:t></w:r></w:p>"#;
    let bytes = package(both);
    let source = open(&bytes);
    let mut model = source.document.clone();
    split(&mut model, 0, 1);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    assert_eq!(
        xml.matches(r#"<w:spacing w:before="240" w:line="360" w:lineRule="auto"/>"#)
            .count(),
        2,
        "{xml}"
    );
    assert!(
        xml.contains(r#"<w:rPr><w:b/><w:sz w:val="28"/></w:rPr>"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<w:rPr><w:i/><w:color w:val="FF0000"/></w:rPr>"#),
        "{xml}"
    );
}

#[test]
fn mapped_indent_fonts_language_numbering_and_explicit_off_round_trip() {
    let paragraph = r#"<w:p><w:pPr><w:ind w:left="420" w:firstLineChars="200"/><w:numPr><w:ilvl w:val="1"/><w:numId w:val="3"/></w:numPr><w:keepNext/></w:pPr><w:r><w:rPr><w:b w:val="0"/><w:rFonts w:ascii="Times New Roman" w:eastAsia="宋体" w:hint="eastAsia"/><w:lang w:val="en-US" w:eastAsia="zh-CN"/><w:u w:val="single"/></w:rPr><w:t>格式</w:t></w:r></w:p>"#;
    let bytes = package(paragraph);
    let source = open(&bytes);
    let mut model = source.document.clone();
    split(&mut model, 0, 1);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    for property in [
        r#"<w:ind w:left="420" w:firstLineChars="200"/>"#,
        r#"<w:numPr><w:ilvl w:val="1"/><w:numId w:val="3"/></w:numPr>"#,
        "<w:keepNext/>",
        r#"<w:b w:val="0"/>"#,
        r#"w:eastAsia="宋体""#,
        r#"w:eastAsia="zh-CN""#,
        r#"<w:u w:val="single"/>"#,
    ] {
        assert_eq!(
            xml.matches(property).count(),
            2,
            "{property} missing in {xml}"
        );
    }
}

#[test]
fn unicode_and_escapes_in_a_formatted_paragraph() {
    let bytes = package(BOLD);
    let source = open(&bytes);
    let mut model = source.document.clone();
    let pieces = ["中文", "👨‍👩‍👧‍👦", "e\u{301}", "<&>", "\t制表"];
    // Insert into the bold run, then split that run off, so the tail keeps it.
    for (index, text) in pieces.iter().enumerate() {
        let end = model
            .body()
            .blocks
            .get(index)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .len();
        insert(&mut model, index, end, text);
        split(&mut model, index, end);
    }
    let plan = export(&source, &model).unwrap();
    let saved = verify(&source, &model, &plan);
    assert_eq!(texts(&saved.document)[1..], pieces);
    let xml = main_xml(plan.bytes());
    assert!(xml.contains("&lt;&amp;&gt;"), "{xml}");
    assert!(xml.contains(r#"<w:rPr><w:b/></w:rPr>"#), "{xml}");
    assert_eq!(plan.bytes(), export(&source, &model).unwrap().bytes());
}

#[test]
fn mixed_document_exports_only_the_safe_paragraph() {
    let unsafe_p = r#"<w:p w14:paraId="AABBCCDD"><w:pPr><w:jc w:val="both"/><w:shd w:fill="AABBCC"/></w:pPr><w:r><w:rPr><w:b/><w:highlight w:val="yellow"/></w:rPr><w:t>保留</w:t></w:r></w:p>"#;
    let bytes = package(&format!("{BOLD}{unsafe_p}"));
    let source = open(&bytes);
    let mut model = source.document.clone();
    split(&mut model, 0, 1);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    assert!(xml.contains(unsafe_p), "{xml}");
    assert_eq!(xml.matches(r#"<w:rPr><w:b/></w:rPr>"#).count(), 2, "{xml}");
    let mut model = source.document.clone();
    split(&mut model, 1, 1);
    assert_eq!(
        refused(export(&source, &model)),
        "paragraph holds preserved markup"
    );
    assert_eq!(source.backing.original_bytes(), bytes);
}

#[test]
fn unexplained_properties_are_refused_without_touching_the_backing() {
    let cases = [
        (
            r#"<w:p><w:pPr><w:jc w:val="center" w:extra="1"/></w:pPr><w:r><w:t>AB</w:t></w:r></w:p>"#,
            "structural paragraph markup has unknown attributes",
        ),
        (
            r#"<w:p><w:r><w:rPr><w:b w:val="1"/><w:position w:val="6"/></w:rPr><w:t>AB</w:t></w:r></w:p>"#,
            "paragraph holds preserved markup",
        ),
        (
            r#"<w:p><w:r><w:rPr><w:bCs/></w:rPr><w:t>AB</w:t></w:r></w:p>"#,
            "paragraph holds preserved markup",
        ),
        (
            r#"<w:p><w:pPr><w:jc w:val="thaiDistribute"/></w:pPr><w:r><w:t>AB</w:t></w:r></w:p>"#,
            "paragraph holds preserved markup",
        ),
        (
            r#"<w:p xmlns:w2="{W}"><w2:pPr><w2:jc w2:val="center"/></w2:pPr><w:r><w:t>AB</w:t></w:r></w:p>"#,
            "property markup uses a prefix that cannot be copied safely",
        ),
    ];
    for (paragraph, expected) in cases {
        let paragraph = paragraph.replace("{W}", W);
        let bytes = package(&format!(
            "{paragraph}{}",
            r#"<w:p><w:r><w:t>后</w:t></w:r></w:p>"#
        ));
        let source = open(&bytes);
        for operation in 0..3 {
            let mut model = source.document.clone();
            match operation {
                0 => split(&mut model, 0, 1),
                1 => merge(&mut model, 0),
                _ => remove(&mut model, 0),
            }
            assert_eq!(
                refused(export(&source, &model)),
                expected,
                "{paragraph} operation {operation}"
            );
        }
        assert_eq!(source.backing.original_bytes(), bytes);
    }
}

#[test]
fn prefixed_and_default_namespaces_copy_property_bytes() {
    for (open_tags, close, prefix) in [
        (
            format!(r#"<ns0:document xmlns:ns0="{W}"><ns0:body>"#),
            "</ns0:body></ns0:document>",
            "ns0:",
        ),
        (
            // Attributes are never in the default namespace, so `w` is bound
            // for `w:val` while elements stay unprefixed.
            format!(r#"<document xmlns="{W}" xmlns:w="{W}"><body>"#),
            "</body></document>",
            "",
        ),
    ] {
        let attr = if prefix.is_empty() { "w:" } else { prefix };
        let paragraph = format!(
            r#"<{prefix}p><{prefix}pPr><{prefix}jc {attr}val="center"/></{prefix}pPr><{prefix}r><{prefix}rPr><{prefix}b/></{prefix}rPr><{prefix}t>甲乙</{prefix}t></{prefix}r></{prefix}p>"#
        );
        let document =
            format!(r#"<?xml version="1.0" encoding="UTF-8"?>{open_tags}{paragraph}{close}"#);
        let bytes = package_document(&document);
        let source = open(&bytes);
        let mut model = source.document.clone();
        split(&mut model, 0, 1);
        let plan = export(&source, &model).unwrap();
        let saved = open(plan.bytes());
        assert_eq!(texts(&saved.document), texts(&model), "{prefix}");
        assert_eq!(
            content(&saved.document, &saved.backing),
            content(&model, &source.backing),
            "{prefix}"
        );
        let xml = main_xml(plan.bytes());
        let ppr = format!(r#"<{prefix}pPr><{prefix}jc {attr}val="center"/></{prefix}pPr>"#);
        let rpr = format!(r#"<{prefix}rPr><{prefix}b/></{prefix}rPr>"#);
        assert_eq!(xml.matches(&ppr).count(), 2, "{xml}");
        assert_eq!(xml.matches(&rpr).count(), 2, "{xml}");
    }
}

#[test]
fn plans_stay_immutable_when_a_later_edit_is_refused() {
    let bytes = package(&format!(
        "{BOLD}{}",
        r#"<w:p><w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr><w:t>注</w:t></w:r></w:p>"#
    ));
    let source = open(&bytes);
    let mut model = source.document.clone();
    split(&mut model, 0, 1);
    let plan = export(&source, &model).unwrap();
    let first = plan.bytes().to_vec();
    split(&mut model, 2, 0);
    assert!(export(&source, &model).is_err());
    assert_eq!(plan.bytes(), first);
    assert_eq!(source.backing.original_bytes(), bytes);
    assert_eq!(plan.mode(), ExportMode::TextPatch);
}

#[test]
fn new_formatted_markup_is_bounded_by_the_part_budget() {
    let bytes = package(JC);
    let parsed = Package::read(&bytes, &Budget::default()).unwrap();
    let main = parsed.part(MAIN).unwrap().data.len() as u64;
    let source = open(&bytes);
    let mut model = source.document.clone();
    for _ in 0..32 {
        split(&mut model, 0, 0);
    }
    let tight = Budget {
        max_part_bytes: main + 400,
        ..Budget::default()
    };
    let source_tight = import(&bytes, &tight).unwrap();
    assert!(matches!(
        plan_export(&source_tight.backing, &model.snapshot(), &tight),
        Err(ExportError::Budget { .. })
    ));
    assert!(export(&source, &model).is_ok());
}
