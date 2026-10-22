//! Integration slice: features developed on separate branches, exercised in
//! ONE export plan.
//!
//! `w:cols/@w:sep` is ECMA-376 Part 1 §17.6.23, numbering writeback is §17.9
//! and format revisions are §17.13.5.29/30. No spec couples them, so this file
//! exists to prove the implementation does not either: the section envelope
//! comparison (which now includes the separator flag), the append-only
//! numbering diff, and the "an unresolved `*Change` element is never
//! synthesized away" refusal policy all run over one document without
//! disturbing each other's bytes.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ExportError, ExportMode, ImportResult, import, plan_export};
use joyword_model::{Block, Justification, NumberingRef, Paragraph, list_marks};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PACKAGE_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const NUMBERING_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const SETTINGS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const NUMBERING_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";

/// Two equal columns with a separator rule between them (§17.6.23).
const COLUMNS: &str = r#"<w:cols w:num="2" w:sep="true"/>"#;
const PAGE: &str = r#"<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/>"#;

/// The one decimal definition the source package carries, surrounded by
/// children JoyWord does not model, so preservation is observable.
const EXISTING_ABSTRACT: &str = r#"<w:abstractNum w:abstractNumId="0"><w:nsid w:val="0A1B2C3D"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/></w:lvl></w:abstractNum>"#;

/// What `ensure_bullet` appends: a JoyWord preset, not a Word measurement.
const APPENDED_ABSTRACT: &str = r#"<w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:pPr><w:ind w:start="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum>"#;
const APPENDED_NUM: &str = r#"<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#;

fn archive(items: &[(&str, &str)]) -> Vec<u8> {
    let items: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Stored))
        .collect();
    write_archive(&items).unwrap()
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

fn part_text(package: &[u8], name: &str) -> String {
    String::from_utf8(part_bytes(package, name)).unwrap()
}

/// A numbered first paragraph, a plain second one, and a two-column section
/// with a separator rule.
fn column_and_numbering_package() -> Vec<u8> {
    let numbering = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="{W}">{EXISTING_ABSTRACT}<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:numIdMacAtCleanup w:val="5"/></w:numbering>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="{SETTINGS_REL}" Target="settings.xml"/><Relationship Id="rId2" Type="{NUMBERING_REL}" Target="numbering.xml"/></Relationships>"#
    );
    let types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="{NUMBERING_TYPE}"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#
    );
    let body = format!(
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>一</w:t></w:r></w:p><w:p><w:r><w:t>二</w:t></w:r></w:p><w:sectPr>{PAGE}{COLUMNS}</w:sectPr>"#
    );
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="{OFFICE_REL}" Target="word/document.xml"/></Relationships>"#
            ),
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/numbering.xml", &numbering),
        (
            "word/settings.xml",
            &format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}"/>"#),
        ),
    ])
}

const OFFICE_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";

fn open(bytes: &[u8]) -> ImportResult {
    import(bytes, &Budget::default()).unwrap()
}

/// Points paragraph `index` at `num_id` level 0, like `list.apply` does.
fn assign_numbering(model: &mut joyword_model::Document, index: usize, num_id: u32) {
    let body = model.body_id();
    model
        .story_mut(body)
        .unwrap()
        .blocks
        .update(index, |block| {
            let Block::Paragraph(paragraph) = block else {
                panic!("paragraph");
            };
            let paragraph = Arc::make_mut(paragraph);
            let mut props = (*paragraph.props).clone();
            props.numbering = Some(NumberingRef { num_id, level: 0 });
            paragraph.props = Arc::new(props);
        });
}

fn bullet_marks(document: &joyword_model::Document) -> Vec<String> {
    list_marks(document)
        .values()
        .map(|mark| mark.as_ref().map(|label| label.text.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|problem| panic!("unrenderable label: {problem}"))
}

#[test]
fn an_appended_bullet_and_a_column_separator_share_one_export_plan() {
    let bytes = column_and_numbering_package();
    let result = open(&bytes);
    assert!(!result.document.validate().has_errors());
    assert!(result.editability.is_unrestricted());

    // Both features are typed on import, so neither is carried as opaque
    // preserved markup.
    let section = &result.document.sections()[0];
    assert!(section.preserved.is_none());
    assert!(section.column_separator);
    assert_eq!(section.columns.unwrap().count, 2);
    assert!(!result.document.numbering().is_empty());
    let original_numbering = part_bytes(&bytes, "/word/numbering.xml");

    // One edit that touches the numbering store only.
    let mut edited = result.document.clone();
    let num_id = edited.numbering_mut().ensure_bullet();
    assert_eq!(num_id, 2, "instance ids continue after the highest");
    assign_numbering(&mut edited, 1, num_id);

    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let exported = plan.bytes().to_vec();

    // The numbering part gained exactly the appended definition, and the
    // column separator survived in the main part even though the numbering
    // write path also ran.
    let written = part_text(&exported, "/word/numbering.xml");
    assert!(written.contains(APPENDED_ABSTRACT), "{written}");
    assert!(written.contains(APPENDED_NUM), "{written}");
    assert!(
        written.find(APPENDED_ABSTRACT).unwrap() < written.find(r#"<w:num w:numId="1""#).unwrap(),
        "abstract definitions stay grouped before the instances"
    );
    assert!(written.contains(r#"<w:nsid w:val="0A1B2C3D"/>"#));
    assert!(written.contains(r#"<w:lvlJc w:val="left"/>"#));
    assert!(written.contains(r#"<w:numIdMacAtCleanup w:val="5"/>"#));
    let stripped = written
        .replacen(APPENDED_ABSTRACT, "", 1)
        .replacen(APPENDED_NUM, "", 1);
    assert_eq!(
        stripped.as_bytes(),
        original_numbering.as_slice(),
        "nothing outside the two insertion points moved"
    );

    let document = part_text(&exported, "/word/document.xml");
    assert!(document.contains(r#"w:sep="true""#), "{document}");
    assert!(document.contains(r#"<w:numId w:val="2"/>"#), "{document}");

    assert!(
        plan.changed_parts()
            .iter()
            .any(|name| name.as_str() == "/word/numbering.xml")
    );
    assert!(
        !plan
            .preserved_parts()
            .iter()
            .any(|name| name.as_str() == "/word/numbering.xml")
    );

    // Re-importing the export reproduces both features.
    let verified = open(&exported);
    assert!(!verified.document.validate().has_errors());
    let verified_section = &verified.document.sections()[0];
    assert!(verified_section.preserved.is_none());
    assert!(verified_section.column_separator);
    assert_eq!(verified_section.columns.unwrap().count, 2);
    assert_eq!(verified.document.numbering(), edited.numbering());
    assert_eq!(bullet_marks(&verified.document), ["1.", "•"]);

    // The plan is byte-deterministic for the same model.
    let again = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    assert_eq!(again.bytes(), plan.bytes());
}

#[test]
fn restoring_the_model_snapshot_preserves_the_separator_and_numbering_part() {
    let bytes = column_and_numbering_package();
    let result = open(&bytes);
    let original_numbering = part_bytes(&bytes, "/word/numbering.xml");

    // This checks export after restoring a model snapshot, not engine undo.
    let mut edited = result.document.clone();
    edited.numbering_mut().ensure_bullet();
    assert_ne!(
        edited.numbering(),
        result.document.numbering(),
        "the command did append a definition"
    );
    let restored = result.document.clone();
    assert_eq!(restored.numbering(), result.document.numbering());

    let plan = plan_export(&result.backing, &restored.snapshot(), &Budget::default()).unwrap();
    let exported = plan.bytes().to_vec();
    assert_eq!(
        part_bytes(&exported, "/word/numbering.xml"),
        original_numbering,
        "an unchanged store keeps the original part bytes"
    );
    assert!(
        plan.preserved_parts()
            .iter()
            .any(|name| name.as_str() == "/word/numbering.xml")
    );
    assert!(part_text(&exported, "/word/document.xml").contains(r#"w:sep="true""#));
}

/// Paragraph 0 carries a typed `w:pPrChange` (id 11, previous center) and a
/// run-level `w:rPrChange` (id 12, previous italic); paragraph 1 is plain.
const PPR_CHANGE_11: &str = r#"<w:pPrChange w:id="11" w:author="A" w:date="2020-01-02T03:04:05Z"><w:pPr><w:jc w:val="center"/></w:pPr></w:pPrChange>"#;
const RPR_CHANGE_12: &str = r#"<w:rPrChange w:id="12" w:author="B" w:date="2020-01-03T04:05:06Z"><w:rPr><w:i/></w:rPr></w:rPrChange>"#;

/// All three features at once: a numbered first paragraph is not needed, so
/// paragraph 0 keeps its unresolved format revisions while paragraph 1 is the
/// one the bullet command touches, inside a two-column section with a
/// separator rule.
fn revision_column_and_numbering_package() -> Vec<u8> {
    let numbering = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="{W}">{EXISTING_ABSTRACT}<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:numIdMacAtCleanup w:val="5"/></w:numbering>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="{SETTINGS_REL}" Target="settings.xml"/><Relationship Id="rId2" Type="{NUMBERING_REL}" Target="numbering.xml"/></Relationships>"#
    );
    let types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="{NUMBERING_TYPE}"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#
    );
    let body = format!(
        r#"<w:p><w:pPr><w:jc w:val="both"/>{PPR_CHANGE_11}</w:pPr><w:r><w:rPr><w:b/>{RPR_CHANGE_12}</w:rPr><w:t>粗体</w:t></w:r></w:p><w:p><w:r><w:t>正文</w:t></w:r></w:p><w:sectPr>{PAGE}{COLUMNS}</w:sectPr>"#
    );
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="{OFFICE_REL}" Target="word/document.xml"/></Relationships>"#
            ),
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/numbering.xml", &numbering),
        (
            "word/settings.xml",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}"><w:trackRevisions/></w:settings>"#
            ),
        ),
    ])
}

fn paragraph_blocks(document: &joyword_model::Document) -> Vec<&Paragraph> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect()
}

/// What the review commands do: accept keeps the current formatting and drops
/// the record, reject restores the recorded previous formatting
/// (§17.13.5.29/30).
fn resolve_all(model: &mut joyword_model::Document, index: usize, accept: bool) -> bool {
    let body = model.body_id();
    let mut resolved = false;
    model
        .story_mut(body)
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
fn an_unresolved_format_revision_survives_a_bullet_append_beside_a_column_separator() {
    let bytes = revision_column_and_numbering_package();
    let result = open(&bytes);
    assert!(!result.document.validate().has_errors());
    assert!(!result.editability.is_read_only());
    assert!(
        result.editability.locked().iter().next().is_none(),
        "typed format revisions must not lock the paragraph"
    );

    let section = &result.document.sections()[0];
    assert!(section.preserved.is_none());
    assert!(section.column_separator);
    assert_eq!(section.columns.unwrap().count, 2);

    let paragraphs = paragraph_blocks(&result.document);
    assert_eq!(paragraphs[0].props_revision.as_ref().unwrap().id, 11);
    assert_eq!(
        paragraphs[0].runs()[0].format_revision.as_ref().unwrap().id,
        12
    );
    assert!(paragraphs[1].props_revision.is_none());
    let original_numbering = part_bytes(&bytes, "/word/numbering.xml");

    // The bullet command touches paragraph 1 only, so paragraph 0 must keep its
    // revision bytes verbatim while the numbering part is rewritten.
    let mut edited = result.document.clone();
    let num_id = edited.numbering_mut().ensure_bullet();
    assign_numbering(&mut edited, 1, num_id);

    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let exported = plan.bytes().to_vec();

    let document = part_text(&exported, "/word/document.xml");
    assert!(document.contains(PPR_CHANGE_11), "{document}");
    assert!(document.contains(RPR_CHANGE_12), "{document}");
    assert!(document.contains(r#"w:sep="true""#), "{document}");
    assert!(document.contains(r#"<w:numId w:val="2"/>"#), "{document}");

    let written = part_text(&exported, "/word/numbering.xml");
    assert!(written.contains(APPENDED_ABSTRACT), "{written}");
    assert!(written.contains(APPENDED_NUM), "{written}");
    let stripped = written
        .replacen(APPENDED_ABSTRACT, "", 1)
        .replacen(APPENDED_NUM, "", 1);
    assert_eq!(
        stripped.as_bytes(),
        original_numbering.as_slice(),
        "nothing outside the two insertion points moved"
    );

    // All three features survive the round trip.
    let verified = open(&exported);
    assert!(!verified.document.validate().has_errors());
    let verified_section = &verified.document.sections()[0];
    assert!(verified_section.column_separator);
    assert_eq!(verified_section.columns.unwrap().count, 2);
    assert_eq!(verified.document.numbering(), edited.numbering());
    assert!(
        bullet_marks(&verified.document)
            .iter()
            .any(|mark| mark == "•"),
        "the appended bullet still renders"
    );
    let verified_paragraphs = paragraph_blocks(&verified.document);
    assert_eq!(
        verified_paragraphs[0].props_revision.as_ref().unwrap().id,
        11
    );
    assert_eq!(
        &*verified_paragraphs[0]
            .props_revision
            .as_ref()
            .unwrap()
            .author,
        "A"
    );
    assert_eq!(
        verified_paragraphs[0].runs()[0]
            .format_revision
            .as_ref()
            .unwrap()
            .id,
        12
    );

    let again = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    assert_eq!(again.bytes(), plan.bytes());
}

#[test]
fn numbering_the_paragraph_that_carries_an_unresolved_revision_is_refused() {
    let bytes = revision_column_and_numbering_package();
    let result = open(&bytes);

    // Assigning a number rewrites `w:pPr`, which would synthesize the paragraph
    // and drop the unresolved `w:pPrChange`. Refusing beats losing review
    // history, and the refusal must be locatable.
    let mut edited = result.document.clone();
    let num_id = edited.numbering_mut().ensure_bullet();
    assign_numbering(&mut edited, 0, num_id);

    match plan_export(&result.backing, &edited.snapshot(), &Budget::default()) {
        Err(ExportError::Unsupported { node, reason }) => {
            assert_eq!(node, Some(paragraph_blocks(&edited)[0].id()));
            assert_eq!(
                reason,
                "unresolved format revision is kept verbatim and cannot be restyled"
            );
        }
        other => panic!("expected an unsupported-edit refusal, got {other:?}"),
    }
}

#[test]
fn accepting_a_revision_writes_clean_formatting_beside_the_separator_and_the_bullet() {
    let bytes = revision_column_and_numbering_package();
    let result = open(&bytes);
    let original_numbering = part_bytes(&bytes, "/word/numbering.xml");

    let mut edited = result.document.clone();
    assert!(resolve_all(&mut edited, 0, true), "both records resolve");
    let num_id = edited.numbering_mut().ensure_bullet();
    assign_numbering(&mut edited, 1, num_id);

    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let exported = plan.bytes().to_vec();
    let document = part_text(&exported, "/word/document.xml");

    // Accept keeps the current formatting and drops the record, so the exported
    // paragraph is clean: bold and justified, with no trace of the previous
    // italic or center, and no `*Change` element left to synthesize away.
    assert!(!document.contains("pPrChange"), "{document}");
    assert!(!document.contains("rPrChange"), "{document}");
    assert!(document.contains(r#"w:jc w:val="both""#), "{document}");
    assert!(!document.contains(r#"w:jc w:val="center""#), "{document}");
    assert!(document.contains("<w:b/>"), "{document}");
    assert!(!document.contains("<w:i/>"), "{document}");
    // The other two features are unaffected by the resolution.
    assert!(document.contains(r#"w:sep="true""#), "{document}");
    assert!(document.contains(r#"<w:numId w:val="2"/>"#), "{document}");
    let written = part_text(&exported, "/word/numbering.xml");
    assert!(written.contains(APPENDED_ABSTRACT), "{written}");
    assert!(written.contains(APPENDED_NUM), "{written}");
    let stripped = written
        .replacen(APPENDED_ABSTRACT, "", 1)
        .replacen(APPENDED_NUM, "", 1);
    assert_eq!(stripped.as_bytes(), original_numbering.as_slice());

    let verified = open(&exported);
    let verified_paragraphs = paragraph_blocks(&verified.document);
    assert!(verified_paragraphs[0].props_revision.is_none());
    assert!(verified_paragraphs[0].runs()[0].format_revision.is_none());
    assert_eq!(
        verified_paragraphs[0].props.justification,
        Some(Justification::Both)
    );
    assert_eq!(verified_paragraphs[0].runs()[0].props.bold, Some(true));
    assert_eq!(verified_paragraphs[0].runs()[0].props.italic, None);
    assert_eq!(verified_paragraphs[0].props.numbering, None);
    assert_eq!(
        verified_paragraphs[1].props.numbering,
        Some(NumberingRef { num_id, level: 0 })
    );
    assert!(verified.document.sections()[0].column_separator);
    assert_eq!(verified.document.numbering(), edited.numbering());
    assert!(edited.semantic_difference(&verified.document).is_none());
}

#[test]
fn rejecting_a_revision_restores_previous_formatting_beside_the_separator_and_bullet() {
    let bytes = revision_column_and_numbering_package();
    let result = open(&bytes);
    let original_numbering = part_bytes(&bytes, "/word/numbering.xml");

    let mut edited = result.document.clone();
    assert!(resolve_all(&mut edited, 0, false), "both records resolve");
    let num_id = edited.numbering_mut().ensure_bullet();
    assign_numbering(&mut edited, 1, num_id);

    let plan = plan_export(&result.backing, &edited.snapshot(), &Budget::default()).unwrap();
    let exported = plan.bytes();
    let document = part_text(exported, "/word/document.xml");
    assert!(!document.contains("pPrChange"), "{document}");
    assert!(!document.contains("rPrChange"), "{document}");
    assert!(document.contains(r#"w:sep="true""#), "{document}");

    let written = part_text(exported, "/word/numbering.xml");
    assert!(written.contains(APPENDED_ABSTRACT), "{written}");
    assert!(written.contains(APPENDED_NUM), "{written}");
    let stripped = written
        .replacen(APPENDED_ABSTRACT, "", 1)
        .replacen(APPENDED_NUM, "", 1);
    assert_eq!(stripped.as_bytes(), original_numbering.as_slice());

    let verified = open(exported);
    assert!(!verified.document.validate().has_errors());
    let paragraphs = paragraph_blocks(&verified.document);
    assert!(paragraphs[0].props_revision.is_none());
    assert!(paragraphs[0].runs()[0].format_revision.is_none());
    assert_eq!(
        paragraphs[0].props.justification,
        Some(Justification::Center)
    );
    assert_eq!(paragraphs[0].runs()[0].props.italic, Some(true));
    assert_eq!(paragraphs[0].runs()[0].props.bold, None);
    assert_eq!(paragraphs[0].props.numbering, None);
    assert_eq!(
        paragraphs[1].props.numbering,
        Some(NumberingRef { num_id, level: 0 })
    );
    let section = &verified.document.sections()[0];
    assert!(section.column_separator);
    assert_eq!(section.columns.unwrap().count, 2);
    assert_eq!(verified.document.numbering(), edited.numbering());
    assert_eq!(bullet_marks(&verified.document), ["•"]);
    assert!(edited.semantic_difference(&verified.document).is_none());
}
