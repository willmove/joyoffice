//! W05: plain body paragraph splits, merges, insertions and removals in the
//! conservative DOCX export. Edits follow the engine's identity rules: a split
//! keeps the head's identity and source and gives the tail a new identity
//! without a source; a merge keeps the first paragraph. Hand-built original
//! packages only; see the W05 report for clauses.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{
    ExportError, ExportMode, ExportPlan, ImportResult, PackageBacking, W, import, plan_export,
};
use joyword_model::{
    Block, Document, OpaqueBlock, ParaProps, Paragraph, Preserved, PreservedId, RunProps,
    ScalarOffset,
};

const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const MAIN: &str = "/word/document.xml";
const BODY_SECT: &str = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#;
const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="bin" ContentType="application/octet-stream"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const BLOB: &[u8] = b"\x00\x01W05 unknown binary payload\xff";

fn document_xml(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}" xmlns:mc="{MC}" xmlns:w14="{W14}" xmlns:x="urn:w05:unknown" mc:Ignorable="w14"><w:body>{body}{BODY_SECT}</w:body></w:document>"#
    )
}

/// A package with a main document, an unrelated binary part, an external
/// relationship and optional settings.
fn package_with(body: &str, settings: Option<&str>, extra: &[(&str, &[u8])]) -> Vec<u8> {
    package_document(&document_xml(body), settings, extra)
}

fn package_document(document: &str, settings: Option<&str>, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.invalid/w05" TargetMode="External"/>"#,
    );
    let settings = settings.map(|inner| {
        rels.push_str(r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>"#);
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}">{inner}</w:settings>"#
        )
    });
    rels.push_str("</Relationships>");
    let mut items: Vec<(&str, &[u8], Method)> = vec![
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
        ("custom/blob.bin", BLOB, Method::Stored),
    ];
    if let Some(settings) = &settings {
        items.push(("word/settings.xml", settings.as_bytes(), Method::Deflated));
    }
    for (name, data) in extra {
        items.push((name, data, Method::Stored));
    }
    write_archive(&items).unwrap()
}

fn package(body: &str) -> Vec<u8> {
    package_with(body, None, &[])
}

fn p(text: &str) -> String {
    format!(r#"<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#)
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

/// SplitParagraph: the head keeps identity and source.
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

/// DeleteRange from the end of `index` to the start of the next paragraph.
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

fn edit(model: &mut Document, index: usize, change: impl FnOnce(&mut Paragraph)) {
    let body = model.body_id();
    model
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph {index}");
            };
            change(Arc::make_mut(paragraph));
        });
}

fn insert(model: &mut Document, index: usize, at: usize, text: &str) {
    edit(model, index, |paragraph| {
        paragraph.insert_text(ScalarOffset(at), text).unwrap();
    });
}

fn export(source: &ImportResult, model: &Document) -> Result<ExportPlan, ExportError> {
    plan_export(&source.backing, &model.snapshot(), &Budget::default())
}

fn main_xml(bytes: &[u8]) -> String {
    let package = Package::read(bytes, &Budget::default()).unwrap();
    String::from_utf8(package.part(MAIN).unwrap().data.to_vec()).unwrap()
}

/// Re-imports a plan and checks it against the edited model, the original
/// package and its preserved fragments.
fn verify(source: &ImportResult, model: &Document, plan: &ExportPlan) -> ImportResult {
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    assert_eq!(plan.version(), model.version());
    assert_eq!(plan.changed_parts().len(), 1);
    assert_eq!(plan.changed_parts()[0].as_str(), MAIN);
    let saved = open(plan.bytes());
    assert!(saved.editability.is_unrestricted());
    assert_eq!(texts(&saved.document), texts(model));
    assert_eq!(
        content(&saved.document, &saved.backing),
        content(model, &source.backing)
    );
    assert_eq!(
        saved.document.sections().len(),
        source.document.sections().len()
    );
    let before = source.backing.package();
    let after = saved.backing.package();
    assert_eq!(before.content_types_xml(), after.content_types_xml());
    assert_eq!(before.relationships(None), after.relationships(None));
    assert_eq!(before.parts().count(), after.parts().count());
    for part in before.parts() {
        let kept = after.part(part.name.as_str()).unwrap();
        assert_eq!(part.content_type, kept.content_type);
        assert_eq!(
            before.relationships(Some(&part.name)),
            after.relationships(Some(&part.name))
        );
        if part.name.as_str() != MAIN {
            assert_eq!(part.data, kept.data, "{}", part.name);
        }
    }
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
    let xml = main_xml(plan.bytes());
    assert_eq!(xml.matches(BODY_SECT).count(), 1);
    assert!(xml.ends_with(&format!("{BODY_SECT}</w:body></w:document>")));
    saved
}

/// Body paragraphs without identifiers, handles or run partitioning: style,
/// properties, the exact bytes each handle keeps, and text grouped by equal
/// formatting.
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
                    Some((f, k, t)) if *f == format && *k == run_kept => t.push_str(&text),
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

fn refused(result: Result<ExportPlan, ExportError>) -> &'static str {
    match result {
        Err(ExportError::Unsupported { reason, .. }) => reason,
        other => panic!("expected Unsupported, got {other:?}"),
    }
}

#[test]
fn split_at_start_middle_and_end_keeps_untouched_source_bytes() {
    let first =
        r#"<w:p w:rsidR="00A1B2C3" w14:paraId="1A2B3C4D"><w:r><w:t>甲乙丙丁</w:t></w:r></w:p>"#;
    let second = r#"<w:p w14:paraId="2A2B3C4D"><w:pPr><w:jc w:val="center"/><w:shd w:fill="AABBCC"/></w:pPr><w:r><w:rPr><w:b/><w:highlight w:val="yellow"/></w:rPr><w:t>格式段</w:t></w:r></w:p>"#;
    let third = p("尾段");
    let bytes = package(&format!("{first}{second}{third}"));
    let source = open(&bytes);
    assert!(source.editability.is_unrestricted());
    for (at, expected) in [
        (0, ["", "甲乙丙丁"]),
        (2, ["甲乙", "丙丁"]),
        (4, ["甲乙丙丁", ""]),
    ] {
        let mut model = source.document.clone();
        split(&mut model, 0, at);
        let plan = export(&source, &model).unwrap();
        let saved = verify(&source, &model, &plan);
        assert_eq!(texts(&saved.document)[..2], expected);
        let xml = main_xml(plan.bytes());
        // The head keeps its identifiers; the new paragraph has none.
        assert!(xml.contains(r#"<w:p w:rsidR="00A1B2C3" w14:paraId="1A2B3C4D">"#));
        assert_eq!(xml.matches("w14:paraId=").count(), 2);
        assert!(xml.contains(second), "formatted paragraph bytes");
        assert!(xml.contains(&third));
        // New markup uses the document's own prefix, as Word writes it.
        let new = if expected[1].is_empty() {
            "</w:p><w:p/>".to_owned()
        } else {
            format!(
                r#"<w:p><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
                expected[1]
            )
        };
        assert!(xml.contains(&new), "{xml}");
        assert!(!xml.contains("jwe:p") && !xml.contains("jwe:r"));
        assert_eq!(plan.bytes(), export(&source, &model).unwrap().bytes());
    }
    assert_eq!(source.backing.original_bytes(), bytes);
}

#[test]
fn first_middle_last_and_consecutive_new_paragraphs() {
    let bytes = package(&format!("{}{}{}", p("一二"), p("三四"), p("五六")));
    let source = open(&bytes);
    let mut model = source.document.clone();
    // Last paragraph: two Enters at its end, then text in the last new one.
    split(&mut model, 2, 2);
    split(&mut model, 3, 0);
    insert(&mut model, 4, 0, "新末");
    // Middle paragraph split twice in a row.
    split(&mut model, 1, 1);
    split(&mut model, 2, 0);
    // First paragraph split at its start.
    split(&mut model, 0, 0);
    assert_eq!(
        texts(&model),
        ["", "一二", "三", "", "四", "五六", "", "新末"]
    );
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
}

#[test]
fn merges_and_removals_drop_exactly_the_merged_source_elements() {
    let kept = r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>保留格式</w:t></w:r></w:p>"#;
    let bytes = package(&format!(
        r#"{}<w:p w:rsidR="00000001"><w:proofErr w:type="spellStart"/><w:r><w:t>B</w:t><w:lastRenderedPageBreak/><w:tab/><w:t>b</w:t></w:r><w:proofErr w:type="spellEnd"/></w:p>{}{}{kept}"#,
        p("A"),
        p("C"),
        p("D"),
    ));
    let source = open(&bytes);
    // Backspace at the start of B, then delete across C and into D.
    let mut model = source.document.clone();
    merge(&mut model, 0);
    assert_eq!(texts(&model)[0], "AB\tb");
    edit(&mut model, 0, |paragraph| {
        paragraph.remove(ScalarOffset(2)..ScalarOffset(4)).unwrap();
    });
    merge(&mut model, 0);
    merge(&mut model, 0);
    assert_eq!(texts(&model), ["ABCD", "保留格式"]);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    assert!(!xml.contains("00000001") && !xml.contains("proofErr"));
    assert!(xml.contains(kept));
    // A removed paragraph whose content was not merged anywhere.
    let mut model = source.document.clone();
    remove(&mut model, 2);
    remove(&mut model, 0);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
}

#[test]
fn new_paragraphs_before_the_first_surviving_source_block() {
    let bytes = package(&format!("{}{}", p("首"), p("次")));
    let source = open(&bytes);
    let mut model = source.document.clone();
    split(&mut model, 0, 0);
    split(&mut model, 1, 1);
    remove(&mut model, 0);
    assert_eq!(texts(&model), ["首", "", "次"]);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
    let xml = main_xml(plan.bytes());
    let body = xml.find("<w:body>").unwrap() + "<w:body>".len();
    assert!(xml[body..].starts_with(r#"<w:p><w:r><w:t xml:space="preserve">首</w:t></w:r></w:p>"#));
    // Every source paragraph removed: only new ones remain.
    let mut model = source.document.clone();
    merge(&mut model, 0);
    split(&mut model, 0, 0);
    remove(&mut model, 0);
    assert_eq!(texts(&model), ["首次"]);
    let plan = export(&source, &model).unwrap();
    verify(&source, &model, &plan);
}

#[test]
fn unicode_whitespace_tabs_and_xml_escapes_in_new_paragraphs() {
    let bytes = package(&p("起"));
    let source = open(&bytes);
    let mut model = source.document.clone();
    let pieces = [
        "  前后空白  ",
        "\t制表\t",
        "中文👨‍👩‍👧‍👦e\u{301}",
        "<&>\"'",
        "]]>",
        "\u{10FFFD}\u{E000}",
    ];
    for (index, text) in pieces.iter().enumerate() {
        let end = model_len(&model, index);
        split(&mut model, index, end);
        insert(&mut model, index + 1, 0, text);
    }
    let plan = export(&source, &model).unwrap();
    let saved = verify(&source, &model, &plan);
    assert_eq!(texts(&saved.document)[1..], pieces);
    // Splitting between the scalars of a ZWJ sequence or a combining mark
    // keeps every scalar; the model works in scalars, not graphemes.
    let mut model = source.document.clone();
    insert(&mut model, 0, 1, "👩‍💻e\u{301}");
    split(&mut model, 0, 3);
    split(&mut model, 1, 2);
    let plan = export(&source, &model).unwrap();
    let saved = verify(&source, &model, &plan);
    assert_eq!(texts(&saved.document).concat(), "起👩‍💻e\u{301}");
}

fn model_len(model: &Document, index: usize) -> usize {
    model
        .body()
        .blocks
        .get(index)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .len()
}

#[test]
fn text_edits_in_formatted_paragraphs_still_combine_with_plain_structure_edits() {
    let formatted = r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>粗体</w:t></w:r><w:r><w:t>正常</w:t></w:r></w:p>"#;
    let bytes = package(&format!("{formatted}{}", p("纯文本")));
    let source = open(&bytes);
    let mut model = source.document.clone();
    insert(&mut model, 0, 1, "改");
    split(&mut model, 1, 1);
    let plan = export(&source, &model).unwrap();
    let saved = verify(&source, &model, &plan);
    let first = saved.document.body().blocks.get(0).unwrap();
    assert_eq!(
        first.as_paragraph().unwrap().runs()[0].props.bold,
        Some(true)
    );
}

#[test]
fn section_boundaries_stay_on_their_paragraphs() {
    let section_end = r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="0" w:footer="0" w:gutter="0"/></w:sectPr></w:pPr><w:r><w:t>节一末</w:t></w:r></w:p>"#;
    let bytes = package(&format!(
        "{}{section_end}{}{}",
        p("节一"),
        p("节二"),
        p("节二末")
    ));
    let source = open(&bytes);
    assert_eq!(source.document.sections().len(), 2);
    // Plain paragraphs on either side of the boundary can change.
    let mut model = source.document.clone();
    split(&mut model, 0, 1);
    split(&mut model, 3, 1);
    split(&mut model, 5, 3);
    let plan = export(&source, &model).unwrap();
    let saved = verify(&source, &model, &plan);
    let boundary = saved.document.sections()[0].last_block.unwrap();
    let index = saved
        .document
        .body()
        .blocks
        .iter()
        .position(|block| block.id() == boundary)
        .unwrap();
    assert_eq!(index, 2);
    assert_eq!(texts(&saved.document)[index], "节一末");
    // The paragraph that ends a section is never split, merged or removed.
    for case in 0..4 {
        let mut model = source.document.clone();
        match case {
            0 => split(&mut model, 1, 3),
            1 => split(&mut model, 1, 0),
            2 => merge(&mut model, 1),
            _ => merge(&mut model, 0),
        }
        assert_eq!(
            refused(export(&source, &model)),
            "paragraph ends a section",
            "case {case}"
        );
    }
    let mut model = source.document.clone();
    remove(&mut model, 1);
    assert_eq!(refused(export(&source, &model)), "paragraph ends a section");
}

#[test]
fn structure_edits_refuse_formatting_objects_markup_and_unknown_attributes() {
    let cases = [
        (
            r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>AB</w:t></w:r></w:p>"#,
            "properties",
        ),
        (
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>AB</w:t></w:r></w:p>"#,
            "properties",
        ),
        (
            r#"<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t>AB</w:t></w:r></w:p>"#,
            "properties",
        ),
        (
            r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>AB</w:t></w:r></w:p>"#,
            "properties",
        ),
        (
            r#"<w:p><w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr><w:t>AB</w:t></w:r></w:p>"#,
            "properties",
        ),
        (
            r#"<w:p><w:r><w:t>A</w:t><w:br/><w:t>B</w:t></w:r></w:p>"#,
            "properties",
        ),
        (
            r#"<w:p><w:r><w:t>A</w:t></w:r><!--note--><w:r><w:t>B</w:t></w:r></w:p>"#,
            "paragraph contains comments or processing instructions",
        ),
        (
            r#"<w:p><w:r><w:t>A</w:t></w:r><?keep data?><w:r><w:t>B</w:t></w:r></w:p>"#,
            "paragraph contains comments or processing instructions",
        ),
        (
            r#"<w:p x:keep="yes"><w:r><w:t>AB</w:t></w:r></w:p>"#,
            "paragraph has unknown attributes",
        ),
        (
            r#"<w:p><w:r><w:t x:keep="yes">AB</w:t></w:r></w:p>"#,
            "text leaf has unknown attributes",
        ),
    ];
    for (paragraph, expected) in cases {
        let bytes = package(&format!("{paragraph}{}", p("后")));
        let source = open(&bytes);
        assert!(source.editability.is_unrestricted(), "{paragraph}");
        for operation in 0..3 {
            let mut model = source.document.clone();
            match operation {
                0 => split(&mut model, 0, 1),
                1 => merge(&mut model, 0),
                _ => remove(&mut model, 0),
            }
            let reason = refused(export(&source, &model));
            if expected == "properties" {
                assert!(
                    reason.contains("default paragraph and character properties"),
                    "{paragraph} {operation}: {reason}"
                );
            } else {
                assert_eq!(reason, expected, "{paragraph} {operation}");
            }
        }
        assert_eq!(source.backing.original_bytes(), bytes);
    }
}

#[test]
fn moves_duplicates_and_forged_blocks_are_refused() {
    let bytes = package(&format!("{}{}{}", p("一"), p("二"), p("三")));
    let source = open(&bytes);
    let body = source.document.body_id();
    let blocks: Vec<Block> = source.document.body().blocks.iter().cloned().collect();
    let replace = |list: Vec<Block>| {
        let mut model = source.document.clone();
        model.story_mut(body).unwrap().blocks = joyword_model::BlockSeq::from_vec(list);
        model
    };
    let order = "body structure or order changed";
    for list in [
        vec![blocks[1].clone(), blocks[0].clone(), blocks[2].clone()],
        vec![blocks[0].clone(), blocks[0].clone(), blocks[2].clone()],
        vec![blocks[0].clone(), blocks[2].clone(), blocks[1].clone()],
    ] {
        assert_eq!(refused(export(&source, &replace(list))), order);
    }
    // A new paragraph cannot claim an original source element.
    let mut model = source.document.clone();
    let id = model.ids().node();
    let mut forged = Paragraph::new(id);
    forged.source = blocks[1].as_paragraph().unwrap().source.clone();
    forged.push_text("伪", None, Arc::default()).unwrap();
    model
        .story_mut(body)
        .unwrap()
        .blocks
        .insert(1, forged.into());
    assert_eq!(
        refused(export(&source, &model)),
        "new paragraph claims an original source"
    );
    // Nor can an unknown block appear.
    let mut model = source.document.clone();
    let id = model.ids().node();
    let handle = model.ids().preserved();
    let opaque = OpaqueBlock {
        id,
        preserved: Preserved {
            element: "w:tbl".into(),
            handle,
        },
        source: None,
    };
    model
        .story_mut(body)
        .unwrap()
        .blocks
        .insert(1, opaque.into());
    assert_eq!(refused(export(&source, &model)), "new non-paragraph block");
    // A new paragraph must be plain too.
    let mut model = source.document.clone();
    split(&mut model, 0, 1);
    edit(&mut model, 1, |paragraph| {
        paragraph.style = Some(joyword_model::StyleId::new("Heading1"));
    });
    assert_eq!(
        refused(export(&source, &model)),
        "new paragraphs require default paragraph and character properties"
    );
}

#[test]
fn restricted_signed_and_locked_documents_refuse_structure_edits() {
    let protected = package_with(
        &format!("{}{}", p("一"), p("二")),
        Some(r#"<w:documentProtection w:edit="readOnly" w:enforcement="1"/>"#),
        &[],
    );
    let bookmark = package(&format!(
        r#"{}<w:p><w:bookmarkStart w:id="0" w:name="mark"/><w:r><w:t>二</w:t></w:r><w:bookmarkEnd w:id="0"/></w:p>"#,
        p("一")
    ));
    let signed = package_with(
        &format!("{}{}", p("一"), p("二")),
        None,
        &[("_xmlsignatures/sig1.xml", b"<signature/>")],
    );
    for (bytes, expected) in [
        (protected, ExportError::Restricted),
        (bookmark, ExportError::Restricted),
        (signed, ExportError::SignedPackage),
    ] {
        let source = open(&bytes);
        for operation in 0..2 {
            let mut model = source.document.clone();
            if operation == 0 {
                split(&mut model, 0, 1);
            } else {
                merge(&mut model, 0);
            }
            assert_eq!(export(&source, &model).unwrap_err(), expected);
        }
        // The untouched document is still copied exactly.
        assert_eq!(
            export(&source, &source.document).unwrap().bytes(),
            bytes.as_slice()
        );
    }
}

#[test]
fn budgets_bound_new_paragraph_markup_and_reimport() {
    let bytes = package(&p("预算"));
    let parsed = Package::read(&bytes, &Budget::default()).unwrap();
    let main = parsed.part(MAIN).unwrap().data.len() as u64;
    let source = open(&bytes);
    let mut model = source.document.clone();
    for _ in 0..64 {
        split(&mut model, 0, 0);
    }
    // Same text size, but 64 new empty paragraphs of markup.
    let tight = Budget {
        max_part_bytes: main + 200,
        ..Budget::default()
    };
    let source_tight = import(&bytes, &tight).unwrap();
    assert!(matches!(
        plan_export(&source_tight.backing, &model.snapshot(), &tight),
        Err(ExportError::Budget { .. })
    ));
    let nodes = Budget {
        max_xml_nodes: 60,
        ..Budget::default()
    };
    let source_nodes = import(&bytes, &nodes).unwrap();
    assert!(matches!(
        plan_export(&source_nodes.backing, &model.snapshot(), &nodes),
        Err(ExportError::Import(_))
    ));
    assert!(export(&source, &model).is_ok());
}

#[test]
fn text_xml_cannot_represent_is_refused_before_writing() {
    let bytes = package(&p("字"));
    let source = open(&bytes);
    for text in ["\u{FFFF}", "\u{FFFE}", "\u{1}", "\u{1B}"] {
        // Both a new paragraph and an existing text leaf.
        let mut model = source.document.clone();
        split(&mut model, 0, 1);
        insert(&mut model, 1, 0, text);
        assert!(
            refused(export(&source, &model)).contains("XML cannot represent"),
            "{text:?}"
        );
        let mut model = source.document.clone();
        insert(&mut model, 0, 0, text);
        assert!(refused(export(&source, &model)).contains("XML cannot represent"));
    }
}

#[test]
fn deterministic_split_merge_sequences_round_trip() {
    let bytes = package(&format!("{}{}{}", p("春夏"), p("秋冬"), p("ABC")));
    let source = open(&bytes);
    let mut model = source.document.clone();
    let mut seed = 0x0005_2026_1015u64;
    let pool = ["中", "🙂", "e\u{301}", "<&>", " ", "\t", "👨‍👩‍👧"];
    let mut next = |bound: usize| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % bound
    };
    for step in 0..300 {
        let count = model.body().blocks.len();
        let index = next(count);
        let len = model_len(&model, index);
        match next(5) {
            0 | 1 => split(&mut model, index, next(len + 1)),
            2 if index + 1 < count => merge(&mut model, index),
            3 if count > 1 && step % 7 == 0 => remove(&mut model, index),
            _ => insert(&mut model, index, next(len + 1), pool[next(pool.len())]),
        }
        if step % 10 == 0 || step == 299 {
            let plan = export(&source, &model).unwrap_or_else(|e| panic!("step {step}: {e}"));
            verify(&source, &model, &plan);
        }
    }
}

#[test]
fn new_markup_names_wordprocessingml_as_the_document_does() {
    let cases = [
        (
            format!(r#"<ns0:document xmlns:ns0="{W}"><ns0:body>"#),
            "</ns0:body></ns0:document>",
            "ns0:",
        ),
        (
            format!(r#"<document xmlns="{W}"><body>"#),
            "</body></document>",
            "",
        ),
        // `w` is rebound inside the body; only `x` still names W there.
        (
            format!(r#"<w:document xmlns:w="{W}"><x:body xmlns:x="{W}" xmlns:w="urn:w05:other">"#),
            "</x:body></w:document>",
            "x:",
        ),
    ];
    for (open_tags, close, prefix) in cases {
        let para = |text: &str| {
            format!(
                r#"<{prefix}p><{prefix}r><{prefix}t>{text}</{prefix}t></{prefix}r></{prefix}p>"#
            )
        };
        let document = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>{open_tags}<{prefix}p/>{}{}{close}"#,
            para("甲乙"),
            para("丙")
        );
        let bytes = package_document(&document, None, &[]);
        let source = open(&bytes);
        assert!(source.editability.is_unrestricted(), "{prefix}");
        let mut model = source.document.clone();
        // Merge into the empty first paragraph, then split and add one.
        merge(&mut model, 0);
        split(&mut model, 0, 1);
        split(&mut model, 2, 1);
        insert(&mut model, 3, 0, "\t新");
        assert_eq!(texts(&model), ["甲", "乙", "丙", "\t新"]);
        let plan = export(&source, &model).unwrap();
        let saved = open(plan.bytes());
        assert_eq!(texts(&saved.document), texts(&model), "{prefix}");
        assert_eq!(
            content(&saved.document, &saved.backing),
            content(&model, &source.backing)
        );
        let xml = main_xml(plan.bytes());
        assert!(
            xml.contains(&format!(
                r#"<{prefix}p><{prefix}r><{prefix}t xml:space="preserve">甲</{prefix}t></{prefix}r></{prefix}p>"#
            )),
            "{xml}"
        );
        assert!(xml.contains(&format!(
            r#"<{prefix}p><{prefix}r><{prefix}tab/><{prefix}t xml:space="preserve">新</{prefix}t></{prefix}r></{prefix}p>"#
        )));
        assert!(!xml.contains("jwe:p") && !xml.contains("jwe:r"), "{xml}");
    }
}
