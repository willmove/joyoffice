//! Import tests over the five original minimal DOCX cases (T004 registry) and
//! hand-built hostile or edge packages.

use std::path::PathBuf;

use joyoffice_opc::{Budget, BudgetError, ExternalKind, Method, OpcError, XmlError, write_archive};
use joyword_docx::{
    DiagnosticKind, ExportError, ExportMode, ImportError, ImportResult, PreservedReason,
    ReadOnlyReason, Severity, import, plan_export,
};
use joyword_model::{
    Block, BreakKind, BuiltinStyle, DEFAULT_COMPATIBILITY_MODE, FontRef, InlineKind, Measure,
    Orientation, Paragraph, Segment, Style, StyleId, StyleKind,
};
use std::sync::Arc;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn fixture(id: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/docx")
        .join(format!("{id}.docx"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn open(bytes: &[u8]) -> ImportResult {
    import(bytes, &Budget::default()).unwrap()
}

fn paragraphs(result: &ImportResult) -> Vec<&Paragraph> {
    result
        .document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect()
}

fn texts(result: &ImportResult) -> Vec<String> {
    paragraphs(result)
        .iter()
        .map(|paragraph| paragraph.plain_text(|_| "\u{FFFC}".into()))
        .collect()
}

fn assert_model_valid(result: &ImportResult) {
    let validation = result.document.validate();
    assert!(
        !validation.has_errors(),
        "{:?}",
        validation.iter().collect::<Vec<_>>()
    );
}

fn has(result: &ImportResult, predicate: impl Fn(&DiagnosticKind) -> bool) -> bool {
    result.diagnostics.iter().any(|item| predicate(&item.kind))
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn archive(items: &[(&str, &str)]) -> Vec<u8> {
    let items: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&items).unwrap()
}

fn document_xml(namespaces: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"{namespaces}><w:body>{body}</w:body></w:document>"#
    )
}

/// A package whose main document is `document` (and optionally settings).
fn package(document: &str, settings: Option<&str>) -> Vec<u8> {
    let mut items = vec![
        ("[Content_Types].xml", CONTENT_TYPES.to_owned()),
        ("_rels/.rels", PACKAGE_RELS.to_owned()),
        ("word/document.xml", document.to_owned()),
    ];
    if let Some(settings) = settings {
        items.push((
            "word/_rels/document.xml.rels",
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#.to_owned(),
        ));
        items.push((
            "word/settings.xml",
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}">{settings}</w:settings>"#),
        ));
    }
    let borrowed: Vec<(&str, &str)> = items.iter().map(|(n, d)| (*n, d.as_str())).collect();
    archive(&borrowed)
}

fn body(body: &str) -> Vec<u8> {
    package(&document_xml("", body), None)
}

#[test]
fn rejects_unbound_markup_compatibility_prefixes() {
    for attribute in ["MustUnderstand", "Ignorable"] {
        let namespaces = format!(
            r#" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:{attribute}="missing""#
        );
        let bytes = package(&document_xml(&namespaces, "<w:p/>"), None);
        assert!(
            matches!(
                import(&bytes, &Budget::default()),
                Err(ImportError::Xml { .. })
            ),
            "undeclared prefixes in mc:{attribute} must not be silently ignored"
        );
    }
}

#[test]
fn rejects_literal_xml_control_characters() {
    for content in [
        "<w:p><w:r><w:t>before\u{1}after</w:t></w:r></w:p>",
        "<w:p><w:r><w:t><![CDATA[before\u{1}after]]></w:t></w:r></w:p>",
        "<w:p><w:pPr><w:pStyle w:val=\"bad\u{1}style\"/></w:pPr></w:p>",
    ] {
        assert!(
            matches!(
                import(&body(content), &Budget::default()),
                Err(ImportError::Xml { .. })
            ),
            "invalid XML characters must be rejected in text, CDATA and attributes"
        );
    }
}

#[test]
fn min_001_reads_body_text_and_defaults_compatibility_mode_to_12() {
    let bytes = fixture("DOCX-MIN-001");
    let result = open(&bytes);
    assert_eq!(texts(&result), vec!["合作协议：甲方与乙方于 2026 年签署。"]);
    let settings = result.document.settings();
    assert_eq!(settings.compatibility_mode, None);
    assert_eq!(
        settings.effective_compatibility_mode(),
        DEFAULT_COMPATIBILITY_MODE
    );
    assert_eq!(DEFAULT_COMPATIBILITY_MODE, 12);
    let section = &result.document.sections()[0];
    assert_eq!(result.document.sections().len(), 1);
    assert_eq!(section.page.width, Measure::Twips(11_906));
    assert_eq!(section.page.left, Measure::Twips(1_800));
    assert_eq!(section.page.orientation, Orientation::Portrait);
    assert!(
        result.editability.is_unrestricted(),
        "{:?}",
        result.editability
    );
    assert_eq!(result.backing.original_bytes(), bytes.as_slice());
    assert_eq!(result.backing.main_part().as_str(), "/word/document.xml");
    assert_eq!(result.backing.interpreted_parts().len(), 1);
    assert_eq!(
        result.diagnostics.max_severity(),
        None,
        "{:?}",
        result.diagnostics
    );
    let source = paragraphs(&result)[0].source.as_ref().unwrap();
    assert_eq!(&*source.part, "/word/document.xml");
    assert_model_valid(&result);
}

#[test]
fn min_002_recognises_builtin_styles_by_name_and_keeps_mode_15() {
    let result = open(&fixture("DOCX-MIN-002"));
    let styles = result.document.styles();
    assert_eq!(
        styles.builtin(BuiltinStyle::Normal).unwrap().id.as_str(),
        "a"
    );
    let heading = styles.builtin(BuiltinStyle::Heading(1)).unwrap();
    assert_eq!(heading.id.as_str(), "1");
    assert_eq!(heading.based_on, Some(StyleId::new("a")));
    assert_eq!(heading.run.bold, Some(true));
    assert_eq!(heading.run.size, Some(Measure::HalfPoints(32)));
    assert_eq!(heading.para.outline_level, Some(0));
    assert!(heading.is_default.eq(&false));
    assert!(styles.builtin(BuiltinStyle::Normal).unwrap().is_default);
    assert_eq!(
        styles
            .builtin(BuiltinStyle::DefaultParagraphFont)
            .unwrap()
            .id
            .as_str(),
        "a0"
    );
    // No style is looked up by a guessed id.
    assert!(styles.get(&StyleId::new("Heading1")).is_none());

    let defaults = &styles.defaults.run;
    assert_eq!(
        defaults.fonts.ascii,
        Some(FontRef::Named("Times New Roman".into()))
    );
    assert_eq!(
        defaults.fonts.h_ansi,
        Some(FontRef::Named("Times New Roman".into()))
    );
    assert_eq!(
        defaults.fonts.east_asia,
        Some(FontRef::Named("宋体".into()))
    );
    assert_eq!(
        defaults.fonts.complex_script,
        Some(FontRef::Named("Times New Roman".into()))
    );
    assert_eq!(defaults.size, Some(Measure::HalfPoints(21)));
    assert_eq!(defaults.bold, None, "unset stays distinct from off");

    let body = paragraphs(&result);
    assert_eq!(body[0].style, Some(StyleId::new("1")));
    let used = styles.get(body[0].style.as_ref().unwrap()).unwrap();
    assert_eq!(used.builtin(), Some(BuiltinStyle::Heading(1)));
    assert_eq!(
        texts(&result),
        vec!["第一条 合作内容", "甲方委托乙方完成设计服务。"]
    );

    assert_eq!(result.document.settings().compatibility_mode, Some(15));
    // qFormat has no model field: the style is preserved, not locked.
    assert!(heading.preserved.is_some());
    assert!(has(&result, |kind| matches!(
        kind,
        DiagnosticKind::PropertiesPreserved { .. }
    )));
    assert!(result.editability.is_unrestricted());
    assert_eq!(result.backing.interpreted_parts().len(), 3);
    assert_model_valid(&result);

    let heading_resolved = styles.resolve_paragraph(Some(&StyleId::new("1")), &body[0].props);
    assert!(heading_resolved.problems.is_empty());
    assert_eq!(heading_resolved.run.bold, Some(true));
    assert_eq!(heading_resolved.run.size, Some(Measure::HalfPoints(32)));
    assert_eq!(heading_resolved.props.outline_level, Some(0));
    assert_eq!(
        heading_resolved.run.fonts.east_asia,
        Some(FontRef::Named("宋体".into()))
    );
    assert_eq!(
        heading_resolved.run.fonts.ascii,
        Some(FontRef::Named("Times New Roman".into()))
    );
    let body_resolved = styles.resolve_run(body[1].style.as_ref(), None, &body[1].runs()[0].props);
    assert!(body_resolved.problems.is_empty());
    assert_eq!(body_resolved.props.bold, None);
    assert_eq!(body_resolved.props.size, Some(Measure::HalfPoints(21)));
    assert_eq!(
        body_resolved.props.fonts.east_asia,
        Some(FontRef::Named("宋体".into()))
    );
    assert_eq!(
        body_resolved.props.fonts.ascii,
        Some(FontRef::Named("Times New Roman".into()))
    );
}

#[test]
fn min_003_preserves_ignorable_unknown_content_and_locks_it() {
    let bytes = fixture("DOCX-MIN-003");
    let result = open(&bytes);
    let blocks: Vec<&Block> = result.document.body().blocks.iter().collect();
    assert_eq!(blocks.len(), 3);
    let Block::Opaque(opaque) = blocks[1] else {
        panic!("expected an opaque block, got {:?}", blocks[1]);
    };
    assert_eq!(&*opaque.preserved.element, "jw:annotation");
    let fragment = result.backing.fragment(opaque.preserved.handle).unwrap();
    assert_eq!(fragment.reason, PreservedReason::Ignorable);
    assert_eq!(
        fragment.namespace.as_deref(),
        Some("urn:joyoffice:test:unknown")
    );
    assert!(
        fragment
            .namespaces
            .iter()
            .any(|(prefix, uri)| &**prefix == "jw" && &**uri == "urn:joyoffice:test:unknown")
    );
    let kept = result
        .backing
        .fragment_bytes(opaque.preserved.handle)
        .unwrap();
    assert_eq!(
        std::str::from_utf8(kept).unwrap(),
        r#"<jw:annotation jw:kind="原创测试">未知命名空间中的内容</jw:annotation>"#
    );
    assert!(!result.editability.is_read_only());
    assert!(!result.editability.can_edit(opaque.id));
    let neighbours: Vec<_> = paragraphs(&result).iter().map(|p| p.id()).collect();
    assert!(neighbours.iter().all(|&id| result.editability.can_edit(id)));
    assert_eq!(
        texts(&result),
        vec!["第一段：未知内容之前。", "第二段：未知内容之后。"]
    );

    // The customXml part is kept verbatim and reported as not imported.
    let custom = result
        .backing
        .package()
        .part("/customXml/item1.xml")
        .unwrap();
    assert!(
        std::str::from_utf8(&custom.data)
            .unwrap()
            .contains("<party>甲方</party>")
    );
    assert!(has(
        &result,
        |kind| matches!(kind, DiagnosticKind::NotImported { part, .. } if part == "/customXml/item1.xml")
    ));
    assert!(has(&result, |kind| matches!(
        kind,
        DiagnosticKind::Preserved {
            reason: PreservedReason::Ignorable,
            ..
        }
    )));
    assert_model_valid(&result);
}

#[test]
fn min_004_records_external_targets_without_accessing_them() {
    let result = open(&fixture("DOCX-MIN-004"));
    let externals: Vec<(String, ExternalKind)> = result
        .diagnostics
        .iter()
        .filter_map(|item| match &item.kind {
            DiagnosticKind::ExternalTarget { target, kind, .. } => Some((target.clone(), *kind)),
            _ => None,
        })
        .collect();
    assert!(externals.contains(&("https://example.invalid/joyword".into(), ExternalKind::Web)));
    assert!(externals.contains(&(
        "file://fileserver.invalid/templates/contract.dotm".into(),
        ExternalKind::NetworkShare
    )));
    assert!(result.diagnostics.iter().any(|item| matches!(
        item.kind,
        DiagnosticKind::ExternalTarget {
            kind: ExternalKind::NetworkShare,
            ..
        }
    ) && item.severity == Severity::Warning));
    // The hyperlink text is visible; the hyperlink itself is preserved and
    // its paragraph locked.
    assert_eq!(
        texts(&result),
        vec!["项目说明", "模板与链接都指向本包之外。"]
    );
    let body = paragraphs(&result);
    assert!(!result.editability.can_edit(body[0].id()));
    assert!(result.editability.can_edit(body[1].id()));
    assert!(
        result
            .backing
            .fragments()
            .any(|(_, fragment)| &*fragment.element == "w:hyperlink")
    );
    assert_eq!(result.document.settings().compatibility_mode, Some(15));
    assert_model_valid(&result);
}

#[test]
fn min_005_protection_and_forced_tracking_open_read_only() {
    let result = open(&fixture("DOCX-MIN-005"));
    let reasons: Vec<_> = result.editability.read_only_reasons().cloned().collect();
    assert!(reasons.contains(&ReadOnlyReason::TrackRevisions));
    assert!(reasons.contains(&ReadOnlyReason::DocumentProtection {
        edit: Some("readOnly".into())
    }));
    assert!(result.editability.is_read_only());
    let id = paragraphs(&result)[0].id();
    assert!(!result.editability.can_edit(id));
    assert_eq!(result.document.settings().track_revisions, Some(true));
    assert_eq!(result.diagnostics.max_severity(), Some(Severity::Error));
    assert_eq!(texts(&result), vec!["本文档要求修订跟踪并限制编辑。"]);
    assert_model_valid(&result);
}

#[test]
fn every_fixture_keeps_original_bytes_and_exact_fragments() {
    for id in [
        "DOCX-MIN-001",
        "DOCX-MIN-002",
        "DOCX-MIN-003",
        "DOCX-MIN-004",
        "DOCX-MIN-005",
    ] {
        let bytes = fixture(id);
        let result = open(&bytes);
        assert_eq!(result.backing.original_bytes(), bytes.as_slice(), "{id}");
        for (handle, fragment) in result.backing.fragments() {
            let kept = result.backing.fragment_bytes(handle).unwrap();
            assert!(
                kept.starts_with(format!("<{}", fragment.element).as_bytes()),
                "{id}"
            );
            assert!(kept.ends_with(b">"), "{id}");
        }
        assert_model_valid(&result);
        // Reading twice gives the same document.
        let again = open(&bytes);
        assert_eq!(
            result.document.semantic_difference(&again.document),
            None,
            "{id}"
        );
    }
}

#[test]
fn text_is_kept_verbatim_with_tabs_and_breaks() {
    let result = open(&body(concat!(
        r#"<w:p><w:r><w:t>  trimmed  </w:t><w:t xml:space="preserve">  kept  </w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>甲</w:t><w:tab/><w:t>乙</w:t><w:br/><w:t>丙</w:t><w:br w:type="page"/><w:cr/></w:r></w:p>"#,
        r#"<w:p/>"#,
    )));
    let body = paragraphs(&result);
    // Characters are kept verbatim either way; edge white space without
    // xml:space="preserve" is reported for reference verification.
    assert_eq!(body[0].plain_text(|_| String::new()), "  trimmed    kept  ");
    let flagged: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|item| item.kind == DiagnosticKind::UnpreservedWhitespace)
        .collect();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0].severity, Severity::Info);
    let kinds: Vec<_> = body[1]
        .segments()
        .into_iter()
        .map(|segment| match segment {
            Segment::Text(text) => text,
            Segment::Inline(object) => match object.kind {
                InlineKind::Break(BreakKind::Line) => "<br>".into(),
                InlineKind::Break(BreakKind::Page) => "<page>".into(),
                _ => "?".into(),
            },
        })
        .collect();
    assert_eq!(kinds, vec!["甲\t乙", "<br>", "丙", "<page>", "<br>"]);
    assert!(body[2].is_empty());
    // No sectPr: defaults are used and reported.
    assert!(has(&result, |kind| matches!(
        kind,
        DiagnosticKind::SectionDefaults {
            missing: "w:sectPr"
        }
    )));
    assert!(result.editability.is_unrestricted());
    assert_model_valid(&result);
}

#[test]
fn run_and_paragraph_properties_map_and_unknown_children_are_preserved() {
    let result = open(&body(concat!(
        r#"<w:p><w:pPr><w:jc w:val="center"/><w:ind w:firstLineChars="200"/><w:rPr><w:b/></w:rPr></w:pPr>"#,
        r#"<w:r><w:rPr><w:i/><w:szCs w:val="28"/></w:rPr><w:t>标题</w:t></w:r></w:p>"#,
    )));
    let paragraph = paragraphs(&result)[0];
    assert_eq!(
        paragraph.props.indent.first_line,
        Some(Measure::CharHundredths(200))
    );
    assert_eq!(paragraph.mark_props.bold, Some(true));
    let run = &paragraph.runs()[0];
    assert_eq!(run.props.italic, Some(true));
    let handle = run.props.preserved.expect("szCs is kept");
    let kept = std::str::from_utf8(result.backing.fragment_bytes(handle).unwrap()).unwrap();
    assert_eq!(kept, r#"<w:rPr><w:i/><w:szCs w:val="28"/></w:rPr>"#);
    // Preserved properties do not lock text editing.
    assert!(result.editability.can_edit(paragraph.id()));
}

#[test]
fn unsupported_content_is_preserved_and_locks_its_container() {
    let result = open(&body(concat!(
        r#"<w:p><w:bookmarkStart w:id="0" w:name="_GoBack"/><w:r><w:t>前</w:t></w:r><w:bookmarkEnd w:id="0"/>"#,
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:proofErr w:type="spellStart"/><w:r><w:t>后</w:t></w:r></w:p>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>表格</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:p><w:r><w:t>自由</w:t></w:r></w:p>"#,
    )));
    let blocks: Vec<&Block> = result.document.body().blocks.iter().collect();
    assert_eq!(blocks.len(), 3);
    assert!(matches!(blocks[1], Block::Opaque(opaque) if &*opaque.preserved.element == "w:tbl"));
    let first = blocks[0].as_paragraph().unwrap();
    // Bookmarks take no position; the field character is an inline object.
    assert_eq!(first.plain_text(|_| "◆".into()), "前◆后");
    assert!(!result.editability.can_edit(first.id()));
    assert!(!result.editability.can_edit(blocks[1].id()));
    assert!(result.editability.can_edit(blocks[2].id()));
    assert!(!result.editability.is_read_only());
    assert!(has(
        &result,
        |kind| matches!(kind, DiagnosticKind::HintsSkipped { element, count: 1 } if element == "w:proofErr")
    ));
    let elements: Vec<&str> = result
        .backing
        .fragments()
        .map(|(_, f)| &*f.element)
        .collect();
    for expected in ["w:bookmarkStart", "w:bookmarkEnd", "w:fldChar", "w:tbl"] {
        assert!(elements.contains(&expected), "{expected} in {elements:?}");
    }
    assert_model_valid(&result);
}

#[test]
fn tracked_changes_make_the_document_read_only() {
    let result = open(&body(concat!(
        r#"<w:p><w:ins w:id="1" w:author="A"><w:r><w:t>新增</w:t></w:r></w:ins>"#,
        r#"<w:del w:id="2" w:author="A"><w:r><w:delText>删除</w:delText></w:r></w:del></w:p>"#,
    )));
    assert!(
        result
            .editability
            .read_only_reasons()
            .any(|reason| *reason == ReadOnlyReason::Revisions)
    );
    // Inserted text shows; deleted text is preserved, not shown.
    assert_eq!(texts(&result), vec!["新增\u{FFFC}"]);
    // Revisions inside otherwise preserved content are found too.
    let nested = open(&body(
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:rPr><w:rPrChange w:id="3" w:author="A"/></w:rPr></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    ));
    assert!(nested.editability.is_read_only());
}

#[test]
fn markup_compatibility_rules() {
    let unknown = open(&package(
        &document_xml(
            r#" xmlns:x="urn:example:other""#,
            r#"<w:p><w:r><w:t>a</w:t></w:r><x:thing/></w:p>"#,
        ),
        None,
    ));
    assert!(
        unknown
            .editability
            .read_only_reasons()
            .any(|reason| matches!(
                reason,
                ReadOnlyReason::UnknownNamespace { namespace } if namespace == "urn:example:other"
            ))
    );

    let must = open(&package(
        &document_xml(
            r#" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="urn:example:other" mc:MustUnderstand="x""#,
            r#"<w:p/>"#,
        ),
        None,
    ));
    assert!(must.editability.read_only_reasons().any(|reason| matches!(
        reason,
        ReadOnlyReason::MustUnderstand { namespace } if namespace == "urn:example:other"
    )));

    let alternate = open(&package(
        &document_xml(
            r#" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape""#,
            r#"<w:p><w:r><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing/></mc:Choice><mc:Fallback><w:pict/></mc:Fallback></mc:AlternateContent></w:r></w:p>"#,
        ),
        None,
    ));
    assert!(!alternate.editability.is_read_only());
    let id = paragraphs(&alternate)[0].id();
    assert!(!alternate.editability.can_edit(id));
    assert!(
        alternate
            .backing
            .fragments()
            .any(|(_, fragment)| fragment.reason == PreservedReason::AlternateContent)
    );
}

#[test]
fn settings_without_compatibility_mode_and_with_write_protection() {
    let plain = open(&package(
        &document_xml("", "<w:p/>"),
        Some("<w:zoom w:percent=\"100\"/>"),
    ));
    assert_eq!(plain.document.settings().compatibility_mode, None);
    assert_eq!(plain.document.settings().effective_compatibility_mode(), 12);
    assert!(!plain.editability.is_read_only());
    assert!(plain.document.settings().preserved.is_some());

    let protected = open(&package(
        &document_xml("", "<w:p/>"),
        Some("<w:writeProtection w:recommended=\"1\"/>"),
    ));
    assert!(
        protected
            .editability
            .read_only_reasons()
            .any(|reason| *reason == ReadOnlyReason::WriteProtection)
    );

    // Protection that is not enforced, or restricts nothing, stays editable.
    let unenforced = open(&package(
        &document_xml("", "<w:p/>"),
        Some(r#"<w:documentProtection w:edit="readOnly" w:enforcement="0"/>"#),
    ));
    assert!(!unenforced.editability.is_read_only());
}

fn assert_original_plan(bytes: &[u8], result: &ImportResult) {
    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .expect("unchanged document");
    assert_eq!(plan.mode(), ExportMode::OriginalPackage);
    assert_eq!(plan.bytes(), bytes);
}

#[test]
fn settings_are_preserved_only_when_a_child_has_no_model_field() {
    let modeled_xml = r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat><w:trackRevisions w:val="0"/><w:evenAndOddHeaders w:val="0"/><w:defaultTabStop w:val="720"/><w:documentProtection w:edit="none" w:enforcement="0"/><w:writeProtection/>"#;
    let modeled_bytes = package(
        &document_xml("", "<w:p><w:r><w:t>a</w:t></w:r></w:p>"),
        Some(modeled_xml),
    );
    let modeled = open(&modeled_bytes);
    assert!(modeled.document.settings().preserved.is_none());
    assert_eq!(modeled.document.settings().compatibility_mode, Some(15));
    assert_eq!(modeled.document.settings().track_revisions, Some(false));
    assert_eq!(
        modeled.document.settings().even_and_odd_headers,
        Some(false)
    );
    assert_eq!(
        modeled.document.settings().default_tab_stop,
        Some(Measure::Twips(720))
    );
    assert!(modeled.document.settings().attached_template.is_none());
    assert!(
        modeled
            .editability
            .read_only_reasons()
            .any(|reason| *reason == ReadOnlyReason::WriteProtection)
    );
    assert_original_plan(&modeled_bytes, &modeled);

    let empty_bytes = package(&document_xml("", "<w:p/>"), Some(""));
    let empty = open(&empty_bytes);
    assert!(empty.document.settings().preserved.is_none());
    assert_eq!(empty.document.settings().compatibility_mode, None);
    assert_original_plan(&empty_bytes, &empty);

    let unknown_bytes = package(
        &document_xml("", "<w:p/>"),
        Some(r#"<w:zoom w:percent="100"/>"#),
    );
    let unknown = open(&unknown_bytes);
    assert!(unknown.document.settings().preserved.is_some());
    assert_original_plan(&unknown_bytes, &unknown);

    let foreign = open(&package(
        &document_xml("", "<w:p/>"),
        Some(r#"<x:extra xmlns:x="urn:example:settings"/>"#),
    ));
    assert!(foreign.document.settings().preserved.is_some());

    let other_setting = open(&package(
        &document_xml("", "<w:p/>"),
        Some(
            r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/><w:compatSetting w:name="doNotAutofitConstrainedTables" w:uri="http://schemas.microsoft.com/office/word" w:val="1"/></w:compat>"#,
        ),
    ));
    assert!(other_setting.document.settings().preserved.is_some());
    assert_eq!(
        other_setting.document.settings().compatibility_mode,
        Some(15)
    );

    let wrong_uri = open(&package(
        &document_xml("", "<w:p/>"),
        Some(
            r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://example.invalid" w:val="15"/></w:compat>"#,
        ),
    ));
    assert!(wrong_uri.document.settings().preserved.is_some());
    assert_eq!(wrong_uri.document.settings().compatibility_mode, None);

    let unparsed_bytes = package(
        &document_xml("", "<w:p/>"),
        Some(
            r#"<w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="nope"/></w:compat>"#,
        ),
    );
    let unparsed = open(&unparsed_bytes);
    assert!(unparsed.document.settings().preserved.is_some());
    assert_eq!(unparsed.document.settings().compatibility_mode, None);
    assert!(has(&unparsed, |kind| matches!(
        kind,
        DiagnosticKind::InvalidValue { value, .. } if value == "nope"
    )));
    assert_original_plan(&unparsed_bytes, &unparsed);

    let template_bytes = attached_template_package();
    let template = open(&template_bytes);
    assert!(template.document.settings().preserved.is_none());
    assert_eq!(template.document.settings().compatibility_mode, Some(15));
    assert_eq!(
        template.document.settings().attached_template.as_deref(),
        Some("rId1")
    );
    assert!(template.diagnostics.iter().any(|item| matches!(
        &item.kind,
        DiagnosticKind::ExternalTarget { target, kind, .. }
            if target == "file://fileserver.invalid/templates/contract.dotm"
                && *kind == ExternalKind::NetworkShare
    )));
    assert_original_plan(&template_bytes, &template);

    for (id, mode, template_id) in [
        ("DOCX-MIN-001", None, None),
        ("DOCX-MIN-002", Some(15), None),
        ("DOCX-MIN-004", Some(15), Some("rId1")),
        ("DOCX-MIN-005", Some(15), None),
    ] {
        let result = open(&fixture(id));
        assert!(
            result.document.settings().preserved.is_none(),
            "{id} preserved settings"
        );
        assert_eq!(
            result.document.settings().compatibility_mode,
            mode,
            "{id} mode"
        );
        assert_eq!(
            result.document.settings().attached_template.as_deref(),
            template_id,
            "{id} template"
        );
        assert_original_plan(&fixture(id), &result);
    }
}

fn attached_template_package() -> Vec<u8> {
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}" xmlns:r="{R}"><w:attachedTemplate r:id="rId1"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#
    );
    let document_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;
    let settings_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate" Target="file://fileserver.invalid/templates/contract.dotm" TargetMode="External"/></Relationships>"#;
    archive(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", PACKAGE_RELS),
        (
            "word/document.xml",
            &document_xml("", "<w:p><w:r><w:t>a</w:t></w:r></w:p>"),
        ),
        ("word/_rels/document.xml.rels", document_rels),
        ("word/settings.xml", &settings),
        ("word/_rels/settings.xml.rels", settings_rels),
    ])
}

#[test]
fn reserved_object_marker_in_text_is_preserved_not_imported_as_text() {
    let result = open(&body("<w:p><w:r><w:t>a\u{FDD0}b</w:t></w:r></w:p>"));
    let paragraph = paragraphs(&result)[0];
    assert_eq!(paragraph.inlines().len(), 1);
    assert!(!result.editability.can_edit(paragraph.id()));
    assert!(
        result
            .backing
            .fragments()
            .any(|(_, fragment)| fragment.reason == PreservedReason::ReservedText)
    );
    assert_model_valid(&result);
}

#[test]
fn paragraph_section_breaks_split_sections() {
    let sect = r#"<w:sectPr><w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:titlePg/></w:sectPr>"#;
    let result = open(&body(&format!(
        r#"<w:p><w:pPr>{sect}</w:pPr><w:r><w:t>一</w:t></w:r></w:p><w:p><w:r><w:t>二</w:t></w:r></w:p>{sect}"#
    )));
    let sections = result.document.sections();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].last_block, Some(paragraphs(&result)[0].id()));
    assert_eq!(sections[0].page.orientation, Orientation::Landscape);
    assert_eq!(sections[0].title_page, Some(true));
    assert_eq!(sections[1].last_block, None);
    assert_model_valid(&result);
}

#[test]
fn refuses_packages_it_cannot_read_safely() {
    let missing_rel = archive(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("word/document.xml", &document_xml("", "<w:p/>")),
    ]);
    assert_eq!(
        import(&missing_rel, &Budget::default()).unwrap_err(),
        ImportError::MissingMainDocument
    );

    let missing_part = archive(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", PACKAGE_RELS),
    ]);
    assert!(matches!(
        import(&missing_part, &Budget::default()).unwrap_err(),
        ImportError::MissingPart { .. }
    ));

    let strict = package(
        r#"<w:document xmlns:w="http://purl.oclc.org/ooxml/wordprocessingml/main"><w:body/></w:document>"#,
        None,
    );
    assert_eq!(
        import(&strict, &Budget::default()).unwrap_err(),
        ImportError::StrictUnsupported
    );

    let doctype = package(
        &format!(
            r#"<!DOCTYPE w:document [<!ENTITY x SYSTEM "file:///etc/passwd">]><w:document xmlns:w="{W}"><w:body/></w:document>"#
        ),
        None,
    );
    assert!(matches!(
        import(&doctype, &Budget::default()).unwrap_err(),
        ImportError::Xml {
            error: XmlError::DocumentType,
            ..
        }
    ));

    let undefined_entity = body("<w:p><w:r><w:t>&x;</w:t></w:r></w:p>");
    assert!(matches!(
        import(&undefined_entity, &Budget::default()).unwrap_err(),
        ImportError::Xml {
            error: XmlError::Malformed(_),
            ..
        }
    ));

    let unclosed = package(&format!(r#"<w:document xmlns:w="{W}"><w:body><w:p>"#), None);
    assert!(matches!(
        import(&unclosed, &Budget::default()).unwrap_err(),
        ImportError::Xml {
            error: XmlError::Malformed(_),
            ..
        }
    ));

    let no_body = package(&format!(r#"<w:document xmlns:w="{W}"/>"#), None);
    assert!(matches!(
        import(&no_body, &Budget::default()).unwrap_err(),
        ImportError::UnexpectedElement { .. }
    ));

    let not_word = package(
        r#"<x:document xmlns:x="urn:x"><x:body/></x:document>"#,
        None,
    );
    assert!(matches!(
        import(&not_word, &Budget::default()).unwrap_err(),
        ImportError::UnexpectedElement { .. }
    ));
}

#[test]
fn damaged_and_truncated_packages_are_refused() {
    let bytes = fixture("DOCX-MIN-002");
    for cut in [0, 1, 30, bytes.len() / 2, bytes.len() - 22, bytes.len() - 1] {
        assert!(
            matches!(
                import(&bytes[..cut], &Budget::default()),
                Err(ImportError::Package(_))
            ),
            "cut at {cut}"
        );
    }
    // Truncated XML inside a valid container.
    let document = document_xml("", "<w:p><w:r><w:t>甲方</w:t></w:r></w:p>");
    for cut in (1..document.len()).step_by(7) {
        let Some(prefix) = document.get(..cut) else {
            continue;
        };
        let result = import(&package(prefix, None), &Budget::default());
        assert!(
            matches!(result, Err(ImportError::Xml { .. })),
            "cut at {cut}: {result:?}"
        );
    }
}

#[test]
fn xml_budgets_apply_to_document_parts() {
    let deep = body(&format!(
        "<w:p>{}{}</w:p>",
        "<w:hyperlink>".repeat(40),
        "</w:hyperlink>".repeat(40)
    ));
    let budget = Budget {
        max_xml_depth: 32,
        ..Budget::default()
    };
    assert!(matches!(
        import(&deep, &budget).unwrap_err(),
        ImportError::Xml {
            error: XmlError::Budget(BudgetError::XmlDepth { .. }),
            ..
        }
    ));
    let wide = body(&"<w:p/>".repeat(2_000));
    let budget = Budget {
        max_xml_nodes: 1_000,
        ..Budget::default()
    };
    assert!(matches!(
        import(&wide, &budget).unwrap_err(),
        ImportError::Xml {
            error: XmlError::Budget(BudgetError::XmlNodes { .. }),
            ..
        }
    ));
    // Container budgets come from the OPC layer.
    let budget = Budget {
        max_entries: 2,
        ..Budget::default()
    };
    assert!(matches!(
        import(&fixture("DOCX-MIN-001"), &budget).unwrap_err(),
        ImportError::Package(OpcError::Zip(_))
    ));
}

/// Deterministic mutation smoke run: flipped bytes anywhere in each fixture
/// either refuse the package or produce a valid model; never a panic.
#[test]
fn mutation_smoke() {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for id in [
        "DOCX-MIN-001",
        "DOCX-MIN-002",
        "DOCX-MIN-003",
        "DOCX-MIN-004",
        "DOCX-MIN-005",
    ] {
        let original = fixture(id);
        for _ in 0..200 {
            let mut bytes = original.clone();
            for _ in 0..(next() % 4 + 1) {
                let at = (next() % bytes.len() as u64) as usize;
                bytes[at] ^= (next() % 255 + 1) as u8;
            }
            if let Ok(result) = import(&bytes, &Budget::default()) {
                assert_model_valid(&result);
            }
        }
    }
    // Mutations of the XML text itself, re-archived so the container is valid.
    let document = document_xml(
        r#" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006""#,
        r#"<w:p><w:pPr><w:jc w:val="both"/></w:pPr><w:hyperlink r:id="rId9"><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve"> 甲 </w:t></w:r></w:hyperlink><w:r><w:br w:type="page"/></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#,
    );
    for _ in 0..400 {
        let mut bytes = document.clone().into_bytes();
        let at = (next() % bytes.len() as u64) as usize;
        bytes[at] = b"<>/=\"' :wx"[(next() % 10) as usize];
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if let Ok(result) = import(&package(&text, None), &Budget::default()) {
            assert_model_valid(&result);
        }
    }
}

fn set_paragraph_style(model: &mut joyword_model::Document, index: usize, style: Option<StyleId>) {
    let body = model.body_id();
    model
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            Arc::make_mut(paragraph).style = style;
        });
}

#[test]
fn style_assignments_export_and_unknown_styles_are_rejected() {
    let source = open(&fixture("DOCX-MIN-002"));
    let mut model = source.document.clone();
    set_paragraph_style(&mut model, 1, Some(StyleId::new("1")));
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = open(plan.bytes());
    assert_eq!(
        paragraphs(&saved)[1].style.as_ref().map(StyleId::as_str),
        Some("1")
    );
    let package = joyoffice_opc::Package::read(plan.bytes(), &Budget::default()).unwrap();
    let xml = String::from_utf8(package.part("/word/document.xml").unwrap().data.to_vec()).unwrap();
    assert!(xml.contains(r#"w:val="1""#), "{xml}");
    assert_eq!(
        package.part("/word/styles.xml").unwrap().data,
        source
            .backing
            .package()
            .part("/word/styles.xml")
            .unwrap()
            .data,
        "the styles part stays untouched"
    );

    let mut model = source.document.clone();
    set_paragraph_style(&mut model, 1, Some(StyleId::new("missing")));
    assert!(matches!(
        plan_export(&source.backing, &model.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported { .. })
    ));

    let mut model = source.document.clone();
    model
        .styles_mut()
        .insert(Style::new(
            "TableGrid",
            StyleKind::Table,
            Some("Table Grid"),
        ))
        .unwrap();
    set_paragraph_style(&mut model, 1, Some(StyleId::new("TableGrid")));
    assert!(matches!(
        plan_export(&source.backing, &model.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported { .. })
    ));
}

#[test]
fn a_theme_part_supplies_font_and_colour_schemes() {
    let theme = r#"<?xml version="1.0" encoding="UTF-8"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:accent1><a:srgbClr val="2E75B6"/></a:accent1></a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Calibri"/><a:ea typeface="宋体"/><a:cs typeface=""/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#;
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#;
    let document = document_xml("", "<w:p><w:r><w:t>甲</w:t></w:r></w:p>");
    let bytes = archive(&[
        ("[Content_Types].xml", types),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", rels),
        ("word/theme/theme1.xml", theme),
    ]);
    let result = open(&bytes);
    let fonts = &result.document.styles().theme().fonts;
    assert_eq!(fonts.minor_east_asia.as_deref(), Some("宋体"));
    assert_eq!(fonts.minor_latin.as_deref(), Some("Calibri"));
    assert_eq!(fonts.major_latin.as_deref(), Some("Calibri Light"));
    assert!(fonts.major_east_asia.is_none());
    assert_eq!(
        result
            .document
            .styles()
            .theme()
            .colors
            .get(&joyword_model::ThemeColorSlot::Accent1),
        Some(&(0x2E, 0x75, 0xB6))
    );
}

#[test]
fn numbering_and_tab_stops_import_and_export_keeps_the_numbering_part() {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;
    let numbering = r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="10"/><w:numFmt w:val="chineseCounting"/><w:lvlText w:val="%1"/><w:suff w:val="nothing"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#;
    let settings = r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="360"/></w:settings>"#;
    let document = document_xml(
        "",
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>甲</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr><w:r><w:t>乙</w:t></w:r></w:p><w:p><w:pPr><w:tabs><w:tab w:val="left" w:leader="dot" w:pos="720"/></w:tabs></w:pPr><w:r><w:t>丙</w:t></w:r></w:p>"#,
    );
    let bytes = archive(&[
        ("[Content_Types].xml", types),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", rels),
        ("word/numbering.xml", numbering),
        ("word/settings.xml", settings),
    ]);
    let result = open(&bytes);
    assert_eq!(
        result.document.settings().default_tab_stop,
        Some(Measure::Twips(360))
    );
    let marks = joyword_model::list_marks(&result.document);
    let paragraphs = paragraphs(&result);
    let chinese = marks.get(&paragraphs[0].id()).unwrap().as_ref().unwrap();
    assert_eq!(chinese.text, "十");
    assert!(matches!(
        marks.get(&paragraphs[1].id()).unwrap(),
        Err(joyword_model::NumberingProblem::UnsupportedFormat { .. })
    ));
    let tabs = paragraphs[2].props.tabs.as_ref().unwrap();
    assert_eq!(
        tabs[0],
        joyword_model::TabStop {
            position: Measure::Twips(720),
            align: joyword_model::TabAlign::Left,
            leader: joyword_model::TabLeader::Dot,
        }
    );

    let mut model = result.document.clone();
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        let paragraph = Arc::make_mut(paragraph);
        let mut props = (*paragraph.props).clone();
        props.numbering = Some(joyword_model::NumberingRef {
            num_id: 0,
            level: 0,
        });
        paragraph.props = Arc::new(props);
    });
    let plan = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap();
    let package = joyoffice_opc::Package::read(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(
        package.part("/word/numbering.xml").unwrap().data,
        result
            .backing
            .package()
            .part("/word/numbering.xml")
            .unwrap()
            .data
    );
    let xml = String::from_utf8(package.part("/word/document.xml").unwrap().data.to_vec()).unwrap();
    assert!(xml.contains(r#"<w:numId w:val="0"/>"#), "{xml}");

    let mut model = result.document.clone();
    model.numbering_mut().abstracts_mut()[0].levels[0]
        .as_mut()
        .unwrap()
        .start = 3;
    assert!(plan_export(&result.backing, &model.snapshot(), &Budget::default()).is_err());

    let mut model = result.document.clone();
    model.settings_mut().default_tab_stop = Some(Measure::Twips(720));
    assert!(plan_export(&result.backing, &model.snapshot(), &Budget::default()).is_err());

    let mut model = result.document.clone();
    model.story_mut(body).unwrap().blocks.update(2, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        let paragraph = Arc::make_mut(paragraph);
        let mut props = (*paragraph.props).clone();
        props.numbering = Some(joyword_model::NumberingRef {
            num_id: 1,
            level: 0,
        });
        paragraph.props = Arc::new(props);
    });
    let plan = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap();
    let package = joyoffice_opc::Package::read(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(
        package.part("/word/numbering.xml").unwrap().data,
        result
            .backing
            .package()
            .part("/word/numbering.xml")
            .unwrap()
            .data,
        "assigning numPr does not rewrite numbering.xml"
    );
    let xml = String::from_utf8(package.part("/word/document.xml").unwrap().data.to_vec()).unwrap();
    assert!(xml.contains(r#"<w:numId w:val="1"/>"#), "{xml}");
    assert!(xml.contains(r#"w:pos="720""#), "{xml}");
    assert!(xml.contains(r#"w:leader="dot""#), "{xml}");

    let mut model = result.document.clone();
    model.story_mut(body).unwrap().blocks.update(2, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        let paragraph = Arc::make_mut(paragraph);
        let mut props = (*paragraph.props).clone();
        props.tabs = Some(vec![joyword_model::TabStop {
            position: Measure::Twips(720),
            align: joyword_model::TabAlign::Bar,
            leader: joyword_model::TabLeader::None,
        }]);
        paragraph.props = Arc::new(props);
    });
    assert!(matches!(
        plan_export(&result.backing, &model.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported { .. })
    ));
}

#[test]
fn default_header_and_footer_import_page_fields_without_cached_results() {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/header-orphan.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header-orphan.xml"/></Relationships>"#;
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}"><w:p><w:r><w:t>H</w:t></w:r><w:fldSimple w:instr=" PAGE \* MERGEFORMAT "><w:r><w:t>3</w:t></w:r></w:fldSimple><w:fldSimple w:instr=" DATE "><w:r><w:t>9</w:t></w:r></w:fldSimple></w:p></w:hdr>"#
    );
    let footer = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:ftr xmlns:w="{W}"><w:p><w:fldSimple w:instr=" NUMPAGES "><w:r><w:t>8</w:t></w:r></w:fldSimple></w:p></w:ftr>"#
    );
    let orphan = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}"><w:p><w:r><w:t>ORPHAN</w:t></w:r></w:p></w:hdr>"#
    );
    let sect = r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:headerReference w:type="default" r:id="rId2"/><w:footerReference w:type="default" r:id="rId3"/></w:sectPr>"#;
    let next = r#"<w:sectPr><w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/><w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720" w:header="360" w:footer="360" w:gutter="0"/></w:sectPr>"#;
    let document = document_xml(
        "",
        &format!(
            r#"<w:p><w:pPr>{sect}</w:pPr><w:r><w:t>One</w:t></w:r></w:p><w:p><w:r><w:t>Two</w:t></w:r></w:p>{next}"#
        ),
    );
    let bytes = archive(&[
        ("[Content_Types].xml", types),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", rels),
        ("word/header1.xml", &header),
        ("word/footer1.xml", &footer),
        ("word/header-orphan.xml", &orphan),
    ]);
    let result = open(&bytes);
    let sections = result.document.sections();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].page.width, Measure::Twips(12_240));
    assert_eq!(
        sections[0].headers,
        vec![(
            joyword_model::HeaderFooterKind::Default,
            sections[0].headers[0].1
        )]
    );
    assert_eq!(sections[0].footers.len(), 1);
    assert!(sections[1].headers.is_empty());
    assert!(sections[1].footers.is_empty());
    assert_eq!(sections[1].page.orientation, Orientation::Landscape);

    let header_story = result
        .document
        .stories()
        .find(|story| story.id == sections[0].headers[0].1)
        .unwrap();
    let header_paragraph = header_story.blocks.get(0).unwrap().as_paragraph().unwrap();
    let kinds: Vec<_> = header_paragraph
        .inlines()
        .iter()
        .map(|inline| &inline.kind)
        .collect();
    assert!(
        kinds
            .iter()
            .any(|kind| is_page_field(kind, joyword_model::PageFieldKind::Page))
    );
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, InlineKind::Preserved(_)))
    );
    assert!(
        !kinds
            .iter()
            .any(|kind| is_page_field(kind, joyword_model::PageFieldKind::NumPages))
    );
    let shown = header_paragraph.plain_text(|_| String::new());
    assert_eq!(shown, "H");
    assert!(!shown.contains('3'));
    assert!(!shown.contains('9'));

    let footer_story = result
        .document
        .stories()
        .find(|story| story.id == sections[0].footers[0].1)
        .unwrap();
    let footer_paragraph = footer_story.blocks.get(0).unwrap().as_paragraph().unwrap();
    assert!(
        footer_paragraph
            .inlines()
            .iter()
            .any(|inline| { is_page_field(&inline.kind, joyword_model::PageFieldKind::NumPages) })
    );
    assert!(!footer_paragraph.plain_text(|_| String::new()).contains('8'));

    assert!(has(&result, |kind| matches!(
        kind,
        DiagnosticKind::NotImported { part, .. } if part.contains("header-orphan")
    )));
    assert!(
        !result
            .document
            .stories()
            .any(|story| story
                .blocks
                .iter()
                .any(
                    |block| block.as_paragraph().is_some_and(|paragraph| paragraph
                        .plain_text(|_| String::new())
                        .contains("ORPHAN"))
                ))
    );

    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    let package = joyoffice_opc::Package::read(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(
        package.part("/word/header1.xml").unwrap().data.as_ref(),
        header.as_bytes()
    );

    let mut changed = result.document.clone();
    changed.sections_mut()[0].page.width = Measure::Twips(10_000);
    assert!(plan_export(&result.backing, &changed.snapshot(), &Budget::default()).is_err());
}

#[test]
fn direct_indent_spacing_and_tabs_round_trip_and_survive_a_font_change() {
    let bytes = package(
        &document_xml("", "<w:p><w:r><w:t>甲</w:t></w:r></w:p>"),
        None,
    );
    let result = open(&bytes);
    let mut model = result.document.clone();
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        let paragraph = Arc::make_mut(paragraph);
        let mut props = (*paragraph.props).clone();
        props.indent = joyword_model::Indent {
            start: Some(Measure::Twips(720)),
            first_line: Some(Measure::Twips(360)),
            ..joyword_model::Indent::default()
        };
        props.spacing = joyword_model::Spacing {
            before: Some(Measure::Twips(120)),
            after: Some(Measure::Twips(80)),
            line: Some(joyword_model::LineSpacing::Auto(360)),
            ..joyword_model::Spacing::default()
        };
        props.tabs = Some(vec![joyword_model::TabStop {
            position: Measure::Twips(1440),
            align: joyword_model::TabAlign::Left,
            leader: joyword_model::TabLeader::Dot,
        }]);
        paragraph.props = Arc::new(props);
    });
    let plan = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap();
    let again = open(plan.bytes());
    let paragraph = paragraphs(&again)[0];
    assert_eq!(paragraph.props.indent.start, Some(Measure::Twips(720)));
    assert_eq!(paragraph.props.indent.first_line, Some(Measure::Twips(360)));
    assert!(paragraph.props.indent.hanging.is_none());
    assert_eq!(paragraph.props.spacing.before, Some(Measure::Twips(120)));
    assert_eq!(paragraph.props.spacing.after, Some(Measure::Twips(80)));
    assert_eq!(
        paragraph.props.spacing.line,
        Some(joyword_model::LineSpacing::Auto(360))
    );
    assert_eq!(
        paragraph.props.tabs.as_ref().unwrap()[0].position,
        Measure::Twips(1440)
    );

    let mut bolded = again.document.clone();
    let body = bolded.body_id();
    bolded.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        let paragraph = Arc::make_mut(paragraph);
        let end = joyword_model::ScalarOffset(paragraph.len());
        paragraph
            .format_runs(joyword_model::ScalarOffset(0)..end, |props| {
                props.bold = Some(true);
            })
            .unwrap();
    });
    let plan = plan_export(&again.backing, &bolded.snapshot(), &Budget::default()).unwrap();
    let kept = open(plan.bytes());
    let paragraph = paragraphs(&kept)[0];
    assert_eq!(paragraph.props.indent.start, Some(Measure::Twips(720)));
    assert_eq!(paragraph.runs()[0].props.bold, Some(true));
}

#[test]
fn page_fields_import_without_treating_the_cache_as_body_text() {
    let sect = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#;
    let markup = format!(
        r#"<w:p><w:r><w:t>A</w:t></w:r><w:fldSimple w:instr=" PAGE "><w:r><w:t>9</w:t></w:r></w:fldSimple></w:p><w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> NUMPAGES </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>8</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:p><w:fldSimple w:instr=" SECTIONPAGES  \* MERGEFORMAT "><w:r><w:t>3</w:t></w:r></w:fldSimple></w:p>{sect}"#
    );
    let result = open(&body(&markup));
    let paragraphs = paragraphs(&result);
    assert_eq!(paragraphs[0].plain_text(|_| String::new()), "A");
    assert!(paragraphs[0].inlines().iter().any(|inline| {
        is_page_field(&inline.kind, joyword_model::PageFieldKind::Page)
            && field_cache(&inline.kind) == Some("9")
    }));
    assert_eq!(paragraphs[1].plain_text(|_| String::new()), "");
    assert!(paragraphs[1].inlines().iter().any(|inline| {
        is_page_field(&inline.kind, joyword_model::PageFieldKind::NumPages)
            && field_cache(&inline.kind) == Some("8")
    }));
    assert!(paragraphs[2].inlines().iter().any(|inline| {
        is_page_field(&inline.kind, joyword_model::PageFieldKind::SectionPages)
            && field_cache(&inline.kind) == Some("3")
    }));
    assert!(
        paragraphs
            .iter()
            .all(|paragraph| result.editability.can_edit(paragraph.id()))
    );
}

#[test]
fn section_break_page_numbers_and_equal_columns_are_imported() {
    let first = r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr></w:pPr><w:r><w:t>一</w:t></w:r></w:p>"#;
    let second = r#"<w:p><w:r><w:t>二</w:t></w:r></w:p><w:sectPr><w:type w:val="evenPage"/><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/><w:pgNumType w:fmt="upperRoman" w:start="4"/><w:cols w:num="2" w:space="360"/></w:sectPr>"#;
    let result = open(&body(&format!("{first}{second}")));
    let sections = result.document.sections();
    assert_eq!(sections.len(), 2);
    assert_eq!(
        sections[1].break_kind,
        joyword_model::SectionBreak::EvenPage
    );
    assert_eq!(
        sections[1].numbering.format,
        joyword_model::NumberFormat::UpperRoman
    );
    assert_eq!(sections[1].numbering.start, Some(4));
    let columns = sections[1].columns.unwrap();
    assert_eq!(columns.count, 2);
    assert_eq!(columns.space, 360);
}

#[test]
fn an_unchanged_field_package_keeps_its_markup_and_an_inserted_field_is_refused() {
    let bytes = body(
        r#"<w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#,
    );
    let result = open(&bytes);
    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    let package = joyoffice_opc::Package::read(plan.bytes(), &Budget::default()).unwrap();
    let xml =
        std::str::from_utf8(package.part("/word/document.xml").unwrap().data.as_ref()).unwrap();
    assert!(xml.contains("fldSimple"), "{xml}");
    let mut changed = result.document.clone();
    let body_id = changed.body_id();
    let field_id = changed.ids().node();
    changed
        .story_mut(body_id)
        .unwrap()
        .blocks
        .update(0, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            let paragraph = Arc::make_mut(paragraph);
            paragraph.push_inline(
                joyword_model::InlineObject {
                    id: field_id,
                    kind: InlineKind::PageField(joyword_model::PageField::new(
                        joyword_model::PageFieldKind::Page,
                    )),
                    source: None,
                },
                None,
                Arc::default(),
            );
        });
    assert!(plan_export(&result.backing, &changed.snapshot(), &Budget::default()).is_err());
}

fn is_page_field(kind: &InlineKind, expected: joyword_model::PageFieldKind) -> bool {
    match kind {
        InlineKind::PageField(field) => field.kind == expected,
        _ => false,
    }
}

fn field_cache(kind: &InlineKind) -> Option<&str> {
    match kind {
        InlineKind::PageField(field) => field.cached.as_deref(),
        _ => None,
    }
}
