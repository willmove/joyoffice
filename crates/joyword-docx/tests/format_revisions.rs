//! T301: typed paragraph and run format revisions (`w:pPrChange` §17.13.5.29,
//! `w:rPrChange` §17.13.5.30): import, preservation, resolution, the export
//! refusal policy, and the deferred table structure revisions.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{
    DiagnosticKind, ExportError, ExportMode, ImportResult, ReadOnlyReason, import, plan_export,
};
use joyword_model::{
    Block, DumpOptions, Justification, Paragraph, RunProps, ScalarOffset, StyleId,
};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;

fn package_with_settings(body: &str, settings: &str) -> Vec<u8> {
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

fn package(body: &str) -> Vec<u8> {
    package_with_settings(body, "<w:trackRevisions/>")
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

fn texts(document: &joyword_model::Document) -> Vec<String> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

fn document_xml(package_bytes: &[u8]) -> String {
    String::from_utf8(
        Package::read(package_bytes, &Budget::default())
            .unwrap()
            .part("/word/document.xml")
            .unwrap()
            .data
            .to_vec(),
    )
    .unwrap()
}

fn has(result: &ImportResult, predicate: impl Fn(&DiagnosticKind) -> bool) -> bool {
    result.diagnostics.iter().any(|item| predicate(&item.kind))
}

/// Paragraph 0 carries a typed `w:pPrChange` (id 11, previous center) and a
/// run-level `w:rPrChange` (id 12, previous italic); paragraph 1 is plain.
fn format_body() -> String {
    concat!(
        r#"<w:p>"#,
        r#"<w:pPr><w:jc w:val="both"/>"#,
        r#"<w:pPrChange w:id="11" w:author="A" w:date="2020-01-02T03:04:05Z">"#,
        r#"<w:pPr><w:jc w:val="center"/></w:pPr>"#,
        r#"</w:pPrChange></w:pPr>"#,
        r#"<w:r><w:rPr><w:b/>"#,
        r#"<w:rPrChange w:id="12" w:author="B" w:date="2020-01-03T04:05:06Z">"#,
        r#"<w:rPr><w:i/></w:rPr>"#,
        r#"</w:rPrChange></w:rPr><w:t>bold</w:t></w:r>"#,
        r#"</w:p>"#,
        r#"<w:p><w:r><w:t>plain</w:t></w:r></w:p>"#,
    )
    .to_string()
}

const PPR_CHANGE_11: &str = r#"<w:pPrChange w:id="11" w:author="A" w:date="2020-01-02T03:04:05Z"><w:pPr><w:jc w:val="center"/></w:pPr></w:pPrChange>"#;
const RPR_CHANGE_12: &str = r#"<w:rPrChange w:id="12" w:author="B" w:date="2020-01-03T04:05:06Z"><w:rPr><w:i/></w:rPr></w:rPrChange>"#;

/// A single paragraph whose only run has a bold `w:rPr` and a typed
/// `w:rPrChange` (id 12) recording previous italic.
fn run_change_body() -> String {
    format!(r#"<w:p><w:r><w:rPr><w:b/>{RPR_CHANGE_12}</w:rPr><w:t>bold</w:t></w:r></w:p>"#)
}

fn resolve(model: &mut joyword_model::Document, index: usize, id: u32, accept: bool) -> bool {
    let body_id = model.body_id();
    let mut resolved = false;
    model
        .story_mut(body_id)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            resolved = Arc::make_mut(paragraph).resolve_format_revision(id, accept);
        });
    resolved
}

fn resolve_all(model: &mut joyword_model::Document, index: usize, accept: bool) -> bool {
    let body_id = model.body_id();
    let mut resolved = false;
    model
        .story_mut(body_id)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            resolved = Arc::make_mut(paragraph).resolve_all_format(accept);
        });
    resolved
}

#[test]
fn format_revisions_import_typed_with_ids_authors_dates_and_previous_properties() {
    let result = open(&package(&format_body()));
    assert!(!result.editability.is_read_only());
    let locked: Vec<_> = result.editability.locked().iter().collect();
    assert!(locked.is_empty(), "typed changes must not lock: {locked:?}");
    assert_eq!(texts(&result.document), vec!["bold", "plain"]);

    let paragraphs = paragraphs(&result);
    let first = paragraphs[0];
    let change = first.props_revision.as_ref().expect("typed pPrChange");
    assert_eq!(change.id, 11);
    assert_eq!(&*change.author, "A");
    assert_eq!(change.date.as_deref(), Some("2020-01-02T03:04:05Z"));
    assert!(change.previous_run.is_none());
    let previous_para = change.previous_para.as_ref().expect("previous pPr");
    assert_eq!(previous_para.justification, Some(Justification::Center));
    assert_eq!(first.props.justification, Some(Justification::Both));

    let run = &first.runs()[0];
    let change = run.format_revision.as_ref().expect("typed rPrChange");
    assert_eq!(change.id, 12);
    assert_eq!(&*change.author, "B");
    assert_eq!(change.date.as_deref(), Some("2020-01-03T04:05:06Z"));
    assert!(change.previous_para.is_none());
    let previous_run = change.previous_run.as_ref().expect("previous rPr");
    assert_eq!(previous_run.italic, Some(true));
    assert_eq!(previous_run.bold, None);
    assert_eq!(run.props.bold, Some(true));
    assert_eq!(run.props.italic, None);
    // The typed subset produces no preservation fallback.
    assert!(run.props.preserved.is_none());
    assert!(first.props.preserved.is_none());
    assert!(paragraphs[1].props_revision.is_none());
    assert!(
        !has(&result, |kind| matches!(
            kind,
            DiagnosticKind::RevisionSubset { .. }
        )),
        "typed changes must not diagnose a subset fallback"
    );
}

#[test]
fn unchanged_and_settings_only_exports_keep_format_change_bytes_verbatim() {
    let bytes = package(&format_body());
    let source = open(&bytes);

    // No edit: the original package is handed back byte for byte.
    let copy = plan_export(
        &source.backing,
        &source.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(copy.mode(), ExportMode::OriginalPackage);
    assert_eq!(copy.bytes(), &bytes[..]);

    // A settings-only edit patches settings.xml and never touches document.xml.
    let mut turned_off = source.document.clone();
    turned_off.settings_mut().track_revisions = Some(false);
    let patched = plan_export(&source.backing, &turned_off.snapshot(), &Budget::default()).unwrap();
    assert_eq!(patched.mode(), ExportMode::TextPatch);
    assert!(
        patched
            .changed_parts()
            .iter()
            .all(|part| !part.as_str().ends_with("document.xml"))
    );
    assert_eq!(document_xml(patched.bytes()), document_xml(&bytes));
    let again = open(patched.bytes());
    assert_eq!(again.document.settings().track_revisions, Some(false));
    let first = paragraphs(&again)[0];
    let change = first.props_revision.as_ref().expect("pPrChange survived");
    assert_eq!((change.id, &*change.author), (11, "A"));
    assert_eq!(
        change.previous_para.as_ref().map(|p| p.justification),
        Some(Some(Justification::Center))
    );
    let run_change = first.runs()[0].format_revision.as_ref().expect("rPrChange");
    assert_eq!((run_change.id, &*run_change.author), (12, "B"));
    assert!(turned_off.semantic_difference(&again.document).is_none());
}

#[test]
fn editing_a_neighbour_keeps_the_revision_paragraph_bytes_and_reimport_matches() {
    let bytes = package(&format_body());
    let source = open(&bytes);
    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(1, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(5), "X")
            .unwrap();
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let xml = document_xml(plan.bytes());
    // The untouched paragraph keeps its change markup byte for byte.
    assert!(xml.contains(PPR_CHANGE_11), "pPrChange bytes lost:\n{xml}");
    assert!(xml.contains(RPR_CHANGE_12), "rPrChange bytes lost:\n{xml}");
    let saved = open(plan.bytes());
    assert_eq!(texts(&saved.document), vec!["bold", "plainX"]);
    let first = paragraphs(&saved)[0];
    assert_eq!(
        first.props_revision.as_ref().map(|c| c.id),
        Some(11),
        "pPrChange must still be typed after the patch"
    );
    assert_eq!(
        first.runs()[0].format_revision.as_ref().map(|c| c.id),
        Some(12),
        "rPrChange must still be typed after the patch"
    );
    assert!(model.semantic_difference(&saved.document).is_none());
}

#[test]
fn accepting_a_run_format_revision_exports_the_current_formatting() {
    let source = open(&package(&run_change_body()));
    let mut model = source.document.clone();
    assert!(resolve(&mut model, 0, 12, true));
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(0, |block| {
        let paragraph = block.as_paragraph().expect("paragraph");
        let run = &paragraph.runs()[0];
        // Accept keeps the post-change formatting and drops the record.
        assert_eq!(run.props.bold, Some(true));
        assert!(run.format_revision.is_none());
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let xml = document_xml(plan.bytes());
    assert!(
        !xml.contains("rPrChange"),
        "resolved change must not be re-emitted:\n{xml}"
    );
    let saved = open(plan.bytes());
    let run = &paragraphs(&saved)[0].runs()[0];
    assert_eq!(run.props.bold, Some(true));
    assert_eq!(run.props.italic, None);
    assert!(run.format_revision.is_none());
    assert!(model.semantic_difference(&saved.document).is_none());
}

#[test]
fn rejecting_a_run_format_revision_restores_the_previous_formatting() {
    let source = open(&package(&run_change_body()));
    let mut model = source.document.clone();
    assert!(resolve(&mut model, 0, 12, false));
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(0, |block| {
        let paragraph = block.as_paragraph().expect("paragraph");
        let run = &paragraph.runs()[0];
        // Reject restores the pre-change formatting recorded in the change.
        assert_eq!(run.props.italic, Some(true));
        assert_eq!(run.props.bold, None);
        assert!(run.format_revision.is_none());
    });
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let xml = document_xml(plan.bytes());
    assert!(
        !xml.contains("rPrChange"),
        "resolved change must not be re-emitted:\n{xml}"
    );
    let saved = open(plan.bytes());
    let run = &paragraphs(&saved)[0].runs()[0];
    assert_eq!(run.props.italic, Some(true));
    assert_eq!(run.props.bold, None);
    assert!(run.format_revision.is_none());
    assert!(model.semantic_difference(&saved.document).is_none());
}

#[test]
fn accept_all_and_reject_all_resolve_paragraph_and_run_format_changes() {
    // Batch accept keeps the current (post-change) formatting everywhere.
    let source = open(&package(&format_body()));
    let mut model = source.document.clone();
    assert!(resolve_all(&mut model, 0, true));
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let xml = document_xml(plan.bytes());
    assert!(
        !xml.contains("PrChange"),
        "resolved changes must not be re-emitted:\n{xml}"
    );
    let saved = open(plan.bytes());
    let first = paragraphs(&saved)[0];
    assert!(first.props_revision.is_none());
    assert_eq!(first.props.justification, Some(Justification::Both));
    assert_eq!(first.runs()[0].props.bold, Some(true));
    assert!(first.runs()[0].format_revision.is_none());
    assert!(model.semantic_difference(&saved.document).is_none());

    // Batch reject restores every recorded previous value.
    let source = open(&package(&format_body()));
    let mut model = source.document.clone();
    assert!(resolve_all(&mut model, 0, false));
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let xml = document_xml(plan.bytes());
    assert!(
        !xml.contains("PrChange"),
        "resolved changes must not be re-emitted:\n{xml}"
    );
    let saved = open(plan.bytes());
    let first = paragraphs(&saved)[0];
    assert!(first.props_revision.is_none());
    assert_eq!(first.props.justification, Some(Justification::Center));
    assert_eq!(first.runs()[0].props.italic, Some(true));
    assert_eq!(first.runs()[0].props.bold, None);
    assert!(model.semantic_difference(&saved.document).is_none());
}

fn refusal_reason(body: &str, edit: impl Fn(&mut Paragraph)) -> &'static str {
    let source = open(&package(body));
    let mut model = source.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        edit(Arc::make_mut(paragraph));
    });
    match plan_export(&source.backing, &model.snapshot(), &Budget::default()) {
        Err(ExportError::Unsupported { reason, .. }) => reason,
        other => panic!("expected an unsupported-edit refusal, got {other:?}"),
    }
}

#[test]
fn unresolved_format_revisions_refuse_the_five_rewrite_paths() {
    // 1. A text revision rewrite would synthesize the paragraph and drop the
    //    unresolved `w:pPrChange`.
    let ins_with_change = concat!(
        r#"<w:p><w:pPr><w:pPrChange w:id="21" w:author="A">"#,
        r#"<w:pPr><w:jc w:val="center"/></w:pPr></w:pPrChange></w:pPr>"#,
        r#"<w:ins w:id="20" w:author="A" w:date="2020-01-02T03:04:05Z">"#,
        r#"<w:r><w:t>ins</w:t></w:r></w:ins></w:p>"#,
    );
    assert_eq!(
        refusal_reason(ins_with_change, |paragraph| {
            paragraph.push_text("x", None, Arc::default()).unwrap();
        }),
        "unresolved format revision cannot be combined with a text revision rewrite"
    );

    // 2. Restyling would rewrite `w:pPr` and drop the `w:pPrChange`.
    let change_only = concat!(
        r#"<w:p><w:pPr><w:pPrChange w:id="22" w:author="A">"#,
        r#"<w:pPr><w:jc w:val="center"/></w:pPr></w:pPrChange></w:pPr>"#,
        r#"<w:r><w:t>t</w:t></w:r></w:p>"#,
    );
    assert_eq!(
        refusal_reason(change_only, |paragraph| {
            paragraph.style = Some(StyleId::new("Heading1"));
        }),
        "unresolved format revision is kept verbatim and cannot be restyled"
    );

    // 3. A paragraph rewrite (here: paragraph mark formatting) would drop it.
    assert_eq!(
        refusal_reason(change_only, |paragraph| {
            paragraph.mark_props = Arc::new(RunProps {
                bold: Some(true),
                ..RunProps::default()
            });
        }),
        "unresolved format revision is kept verbatim and its paragraph cannot be rewritten"
    );

    // 4. A run boundary change would re-synthesize every `w:rPr`.
    let run_change = concat!(
        r#"<w:p><w:r><w:rPr><w:b/>"#,
        r#"<w:rPrChange w:id="24" w:author="A"><w:rPr><w:i/></w:rPr></w:rPrChange>"#,
        r#"</w:rPr><w:t>AB</w:t></w:r></w:p>"#,
    );
    assert_eq!(
        refusal_reason(run_change, |paragraph| {
            paragraph
                .format_runs(ScalarOffset(0)..ScalarOffset(1), |props| {
                    props.italic = Some(true);
                })
                .unwrap();
        }),
        "unresolved format revision is kept verbatim and its run boundaries cannot change"
    );

    // 5. New text in an empty paragraph would synthesize a run into a
    //    paragraph whose `w:pPrChange` must stay verbatim.
    let empty_with_change = r#"<w:p><w:pPr><w:pPrChange w:id="25" w:author="A"/></w:pPr></w:p>"#;
    assert_eq!(
        refusal_reason(empty_with_change, |paragraph| {
            paragraph.push_text("new", None, Arc::default()).unwrap();
        }),
        "unresolved format revision is kept verbatim and new text cannot be added"
    );
}

#[test]
fn out_of_subset_format_changes_are_preserved_and_lock_the_paragraph() {
    // A previous paragraph style is out of subset: the whole `w:pPr` is
    // preserved, the paragraph locks, and a warning is diagnosed.
    let styled_previous = concat!(
        r#"<w:p><w:pPr><w:jc w:val="both"/>"#,
        r#"<w:pPrChange w:id="31" w:author="A">"#,
        r#"<w:pPr><w:pStyle w:val="Heading1"/></w:pPr>"#,
        r#"</w:pPrChange></w:pPr><w:r><w:t>styled</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>free</w:t></w:r></w:p>"#,
    );
    let result = open(&package(styled_previous));
    assert!(!result.editability.is_read_only());
    let paras = paragraphs(&result);
    assert!(
        paras[0].props.preserved.is_some(),
        "the whole w:pPr must be preserved"
    );
    assert!(!result.editability.can_edit(paras[0].id()));
    assert!(result.editability.can_edit(paras[1].id()));
    assert!(paras[0].props_revision.is_none());
    assert_eq!(texts(&result.document), vec!["styled", "free"]);
    assert!(has(&result, |kind| matches!(
        kind,
        DiagnosticKind::RevisionSubset { element } if element == "w:pPrChange"
    )));
    // A locked paragraph refuses any rewritten export; an untouched export
    // hands back the original package byte for byte, so the out-of-subset
    // change stays preserved and is never rewritten or dropped.
    let bytes = package(styled_previous);
    let mut model = result.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(1, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(4), "X")
            .unwrap();
    });
    assert!(matches!(
        plan_export(&result.backing, &model.snapshot(), &Budget::default()),
        Err(ExportError::Restricted)
    ));
    let untouched = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(untouched.mode(), ExportMode::OriginalPackage);
    assert_eq!(untouched.bytes(), &bytes[..]);
    let original = r#"<w:pPrChange w:id="31" w:author="A"><w:pPr><w:pStyle w:val="Heading1"/></w:pPr></w:pPrChange>"#;
    assert!(
        document_xml(untouched.bytes()).contains(original),
        "preserved change lost"
    );

    // Unmapped attributes and children inside `w:rPrChange`, more than one
    // previous `w:rPr`, a nested change, and a typed change on an empty run
    // all fall back to preserve-and-lock; none is typed or dropped.
    let runs_out_of_subset = concat!(
        r#"<w:p><w:r><w:rPr><w:b/>"#,
        r#"<w:rPrChange w:id="32" w:author="A" w:name="oops"><w:rPr><w:i/></w:rPr></w:rPrChange>"#,
        r#"</w:rPr><w:t>attr</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:b/>"#,
        r#"<w:rPrChange w:id="33" w:author="A"><w:pPr><w:jc w:val="center"/></w:pPr></w:rPrChange>"#,
        r#"</w:rPr><w:t>child</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:b/>"#,
        r#"<w:rPrChange w:id="34" w:author="A"><w:rPr/><w:rPr/></w:rPrChange>"#,
        r#"</w:rPr><w:t>twice</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:b/>"#,
        r#"<w:rPrChange w:id="35" w:author="A"><w:rPr><w:rPrChange w:id="36" w:author="B"/></w:rPr></w:rPrChange>"#,
        r#"</w:rPr><w:t>nested</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:rPrChange w:id="37" w:author="A"><w:rPr><w:i/></w:rPr></w:rPrChange></w:rPr></w:r>"#,
        r#"<w:r><w:t>emptyrun</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>free</w:t></w:r></w:p>"#,
    );
    let result = open(&package(runs_out_of_subset));
    assert!(!result.editability.is_read_only());
    let paras = paragraphs(&result);
    assert_eq!(paras.len(), 6);
    for paragraph in &paras[..5] {
        assert!(
            !result.editability.can_edit(paragraph.id()),
            "out-of-subset change must lock its paragraph"
        );
        assert!(
            paragraph
                .runs()
                .iter()
                .all(|run| run.format_revision.is_none()),
            "out-of-subset change must not be typed"
        );
    }
    // The empty-run case locks too: the typed record has no run to live on,
    // so it stays in the preserved original bytes instead of vanishing.
    for paragraph in &paras[..4] {
        assert!(
            paragraph.runs()[0].props.preserved.is_some(),
            "the original rPr must be preserved"
        );
    }
    assert!(has(&result, |kind| matches!(
        kind,
        DiagnosticKind::RevisionSubset { element } if element == "w:rPrChange"
    )));
    // Same contract for the run-level fallbacks: edited export refused,
    // untouched export byte-identical with every change element preserved.
    let bytes = package(runs_out_of_subset);
    let mut model = result.document.clone();
    let body_id = model.body_id();
    model.story_mut(body_id).unwrap().blocks.update(5, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(4), "X")
            .unwrap();
    });
    assert!(matches!(
        plan_export(&result.backing, &model.snapshot(), &Budget::default()),
        Err(ExportError::Restricted)
    ));
    let untouched = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(untouched.mode(), ExportMode::OriginalPackage);
    assert_eq!(untouched.bytes(), &bytes[..]);
    let xml = document_xml(untouched.bytes());
    for original in [
        r#"<w:rPrChange w:id="32" w:author="A" w:name="oops"><w:rPr><w:i/></w:rPr></w:rPrChange>"#,
        r#"<w:rPrChange w:id="33" w:author="A"><w:pPr><w:jc w:val="center"/></w:pPr></w:rPrChange>"#,
        r#"<w:rPrChange w:id="34" w:author="A"><w:rPr/><w:rPr/></w:rPrChange>"#,
        r#"<w:rPrChange w:id="35" w:author="A"><w:rPr><w:rPrChange w:id="36" w:author="B"/></w:rPr></w:rPrChange>"#,
        r#"<w:rPrChange w:id="37" w:author="A"><w:rPr><w:i/></w:rPr></w:rPrChange>"#,
    ] {
        assert!(xml.contains(original), "preserved change lost:\n{xml}");
    }
}

#[test]
fn table_structure_revisions_stay_preserved_and_read_only() {
    // T301 defers table structure revisions (§17.13.5.1-.6, §17.13.5.9-.11):
    // each one keeps the whole document read-only and its bytes verbatim.
    // Each case: (name, `w:tblPr` extra, `w:trPr` markup, `w:tcPr` markup,
    // the cell paragraph's run markup).
    let cases = [
        (
            "tblPrChange",
            r#"<w:tblPrChange w:id="51" w:author="A"/>"#,
            "",
            "",
            "",
        ),
        (
            "trPrChange",
            "",
            r#"<w:trPr><w:trPrChange w:id="52" w:author="A"/></w:trPr>"#,
            "",
            "",
        ),
        (
            "tcPrChange",
            "",
            "",
            r#"<w:tcPr><w:tcPrChange w:id="53" w:author="A"/></w:tcPr>"#,
            "",
        ),
        (
            "cellIns",
            "",
            "",
            r#"<w:tcPr><w:cellIns w:id="54" w:author="A"/></w:tcPr>"#,
            "",
        ),
        (
            "cellDel",
            "",
            "",
            r#"<w:tcPr><w:cellDel w:id="55" w:author="A"/></w:tcPr>"#,
            "",
        ),
        (
            "cellMerge",
            "",
            "",
            r#"<w:tcPr><w:cellMerge w:id="56" w:author="A"/></w:tcPr>"#,
            "",
        ),
        (
            "rPrChange-in-table",
            "",
            "",
            "",
            r#"<w:rPr><w:rPrChange w:id="57" w:author="A"><w:rPr><w:i/></w:rPr></w:rPrChange></w:rPr>"#,
        ),
    ];
    for (name, tbl_pr, tr_pr, tc_pr, run_pr) in cases {
        let body = format!(
            r#"<w:tbl><w:tblPr>{tbl_pr}</w:tblPr><w:tblGrid><w:gridCol w:w="1000"/></w:tblGrid><w:tr>{tr_pr}<w:tc>{tc_pr}<w:p><w:r>{run_pr}<w:t>c</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#
        );
        let bytes = package(&body);
        let result = open(&bytes);
        assert!(
            result.editability.is_read_only(),
            "{name} must keep the document read-only"
        );
        assert!(
            result
                .editability
                .read_only_reasons()
                .any(|reason| reason == &ReadOnlyReason::Revisions),
            "{name} must refuse as a revision, got {:?}",
            result.editability.read_only_reasons().collect::<Vec<_>>()
        );
        // The markup is preserved, never typed and never dropped.
        assert!(paragraphs(&result).iter().all(|paragraph| {
            paragraph.props_revision.is_none()
                && paragraph
                    .runs()
                    .iter()
                    .all(|run| run.format_revision.is_none())
        }));
        let copy = plan_export(
            &result.backing,
            &result.document.snapshot(),
            &Budget::default(),
        )
        .unwrap();
        assert_eq!(copy.mode(), ExportMode::OriginalPackage);
        assert_eq!(copy.bytes(), &bytes[..], "{name} bytes must round-trip");
    }
}

#[test]
fn the_dump_reports_typed_format_revisions() {
    let result = open(&package(&format_body()));
    let dump = result
        .document
        .canonical_dump(DumpOptions { node_ids: false });
    assert!(
        dump.contains(r#"pPrChange#11 author="A" date=2020-01-02T03:04:05Z prev=["#),
        "paragraph change missing from dump:\n{dump}"
    );
    assert!(
        dump.contains(r#"rPrChange#12 author="B" date=2020-01-03T04:05:06Z prev=["#),
        "run change missing from dump:\n{dump}"
    );
}
