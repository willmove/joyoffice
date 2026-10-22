//! Numbering part writeback (ECMA-376 Part 1 §17.9).
//!
//! An unchanged store keeps the original part bytes; definitions appended to
//! the store are inserted into the part (or a fresh part is created with its
//! relationship and content-type Override); modified existing entries and
//! unrepresentable new entries are refused.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ExportError, ExportMode, ImportResult, import, plan_export};
use joyword_model::{
    AbstractNum, Block, FontRef, Justification, Measure, NumberFormat, NumberingLevel,
    NumberingRef, TabAlign, TabLeader, TabStop, list_marks,
};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PACKAGE_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const NUMBERING_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const SETTINGS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const NUMBERING_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";

fn archive(items: &[(&str, &str)]) -> Vec<u8> {
    let items: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Stored))
        .collect();
    write_archive(&items).unwrap()
}

fn content_types(extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>{extra}</Types>"#
    )
}

fn document_xml(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    )
}

fn settings_xml() -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:settings xmlns:w="{W}"/>"#)
}

fn open(bytes: &[u8]) -> ImportResult {
    import(bytes, &Budget::default()).unwrap()
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

/// A package with one decimal definition, unmodeled children around it and
/// two paragraphs, the second plain.
fn existing_part_package() -> Vec<u8> {
    let numbering = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="0"><w:nsid w:val="0A1B2C3D"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num><w:numIdMacAtCleanup w:val="5"/></w:numbering>"#
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="{SETTINGS_REL}" Target="settings.xml"/><Relationship Id="rId2" Type="{NUMBERING_REL}" Target="numbering.xml"/></Relationships>"#
    );
    let types = content_types(&format!(
        r#"<Override PartName="/word/numbering.xml" ContentType="{NUMBERING_TYPE}"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>"#
    ));
    let document = document_xml(
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>one</w:t></w:r></w:p><w:p><w:r><w:t>two</w:t></w:r></w:p>"#,
    );
    archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
            ),
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/numbering.xml", &numbering),
        ("word/settings.xml", &settings_xml()),
    ])
}

/// A package without any numbering part or relationship.
fn bare_package() -> Vec<u8> {
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="{SETTINGS_REL}" Target="settings.xml"/></Relationships>"#
    );
    let types = content_types(
        r#"<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>"#,
    );
    let document = document_xml(r#"<w:p><w:r><w:t>text</w:t></w:r></w:p>"#);
    archive(&[
        ("[Content_Types].xml", &types),
        (
            "_rels/.rels",
            &format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PACKAGE_REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
            ),
        ),
        ("word/document.xml", &document),
        ("word/_rels/document.xml.rels", &rels),
        ("word/settings.xml", &settings_xml()),
    ])
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
fn appended_bullet_definitions_are_inserted_without_touching_other_numbering_bytes() {
    let bytes = existing_part_package();
    let result = open(&bytes);
    let original_numbering = part_bytes(&bytes, "/word/numbering.xml");

    let mut with_bullet = result.document.clone();
    let num_id = with_bullet.numbering_mut().ensure_bullet();
    assert_eq!(num_id, 2, "instance ids continue after the highest");
    assign_numbering(&mut with_bullet, 1, num_id);

    let plan = plan_export(&result.backing, &with_bullet.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let exported = plan.bytes().to_vec();
    let written = part_text(&exported, "/word/numbering.xml");

    // The new abstract definition is inserted after the existing one and
    // before the first instance; the new instance after the existing one and
    // before the unmodeled cleanup element. Every original child survives
    // verbatim, including the unmodeled `nsid` and `numIdMacAtCleanup`.
    let expected_level = r#"<w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:pPr><w:ind w:start="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum>"#;
    let expected_num = r#"<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#;
    assert!(written.contains(expected_level), "{written}");
    assert!(written.contains(expected_num), "{written}");
    let new_abstract_at = written.find(expected_level).unwrap();
    let new_num_at = written.find(expected_num).unwrap();
    assert!(new_abstract_at < written.find(r#"<w:num w:numId="1""#).unwrap());
    assert!(new_num_at > written.find(r#"<w:num w:numId="1""#).unwrap());
    assert!(new_num_at < written.find("numIdMacAtCleanup").unwrap());
    assert!(written.contains(r#"<w:nsid w:val="0A1B2C3D"/>"#));
    assert!(written.contains(r#"<w:lvlJc w:val="left"/>"#));
    assert!(written.contains(r#"<w:numIdMacAtCleanup w:val="5"/>"#));
    // Nothing outside the two insertion points moved: removing the inserted
    // markup restores the original bytes exactly.
    let stripped = written
        .replacen(expected_level, "", 1)
        .replacen(expected_num, "", 1);
    assert_eq!(stripped.as_bytes(), original_numbering.as_slice());

    // The changed part is reported; the re-import reproduces store and marks.
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
    let verified = open(&exported);
    assert_eq!(verified.document.numbering(), with_bullet.numbering());
    assert_eq!(bullet_marks(&verified.document), ["1.", "•"]);
    let document = part_text(&exported, "/word/document.xml");
    assert!(document.contains(r#"<w:numId w:val="2"/>"#), "{document}");

    // Two plans from the same model are byte-identical.
    let again = plan_export(&result.backing, &with_bullet.snapshot(), &Budget::default()).unwrap();
    assert_eq!(again.bytes(), plan.bytes());
}

#[test]
fn a_new_numbering_part_gets_a_relationship_and_a_content_type_override() {
    let bytes = bare_package();
    let result = open(&bytes);
    assert!(result.document.numbering().is_empty());
    let original_rels = part_bytes(&bytes, "/word/_rels/document.xml.rels");
    let original_types = Package::read(&bytes, &Budget::default())
        .unwrap()
        .content_types_xml()
        .to_vec();

    let mut model = result.document.clone();
    let num_id = model.numbering_mut().ensure_bullet();
    assert_eq!(num_id, 1, "ids start at 1 in an empty store");
    assign_numbering(&mut model, 0, num_id);

    let plan = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let exported = plan.bytes().to_vec();

    let written = part_text(&exported, "/word/numbering.xml");
    assert_eq!(
        written,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:pPr><w:ind w:start="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
        )
    );

    // The relationship is appended after the existing one; earlier bytes
    // are untouched and no other part was added.
    let rels = part_bytes(&exported, "/word/_rels/document.xml.rels");
    let prefix_len = original_rels.len() - "</Relationships>".len();
    assert!(rels.starts_with(&original_rels[..prefix_len]));
    let rels_text = String::from_utf8(rels).unwrap();
    assert!(rels_text.contains(&format!(
        r#"<Relationship Id="rId2" Type="{NUMBERING_REL}" Target="numbering.xml"/>"#
    )));
    let types = Package::read(&exported, &Budget::default())
        .unwrap()
        .content_types_xml()
        .to_vec();
    let prefix_len = original_types.len() - "</Types>".len();
    assert!(types.starts_with(&original_types[..prefix_len]));
    let types_text = String::from_utf8(types).unwrap();
    assert!(types_text.contains(&format!(
        r#"<Override PartName="/word/numbering.xml" ContentType="{NUMBERING_TYPE}"/>"#
    )));
    let package = Package::read(&exported, &Budget::default()).unwrap();
    assert_eq!(package.parts().count(), 5, "only numbering.xml was added");
    assert!(
        plan.changed_parts()
            .iter()
            .any(|name| name.as_str() == "/word/numbering.xml")
    );

    let verified = open(&exported);
    assert_eq!(verified.document.numbering(), model.numbering());
    assert_eq!(bullet_marks(&verified.document), ["•"]);
}

#[test]
fn an_unchanged_numbering_store_keeps_the_part_verbatim_and_preserved() {
    let bytes = existing_part_package();
    let result = open(&bytes);
    // Only a paragraph numPr rewrite: the store itself does not change.
    let mut model = result.document.clone();
    assign_numbering(&mut model, 1, 1);
    let plan = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    assert_eq!(
        part_bytes(plan.bytes(), "/word/numbering.xml"),
        part_bytes(&bytes, "/word/numbering.xml")
    );
    assert!(
        plan.preserved_parts()
            .iter()
            .any(|name| name.as_str() == "/word/numbering.xml")
    );
    assert!(
        !plan
            .changed_parts()
            .iter()
            .any(|name| name.as_str() == "/word/numbering.xml")
    );
}

#[test]
fn restoring_the_previous_store_keeps_the_package_unchanged() {
    let bytes = existing_part_package();
    let result = open(&bytes);
    let mut model = result.document.clone();
    model.numbering_mut().ensure_bullet();
    assert!(plan_export(&result.backing, &model.snapshot(), &Budget::default()).is_ok());
    // One undo restores the store; the export copies the package exactly.
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
fn modified_or_unrepresentable_numbering_changes_are_refused() {
    let bytes = existing_part_package();
    let result = open(&bytes);

    // A modified existing level is not rewritten.
    let mut model = result.document.clone();
    model.numbering_mut().abstracts_mut()[0].levels[0]
        .as_mut()
        .unwrap()
        .start = 3;
    let error = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap_err();
    assert!(
        matches!(&error, ExportError::Unsupported { reason, .. }
            if reason.contains("not rewritten")),
        "{error}"
    );

    // A changed store-level `unmodeled` flag is not rewritten either.
    let mut model = result.document.clone();
    model.numbering_mut().unmodeled = false;
    assert!(matches!(
        plan_export(&result.backing, &model.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported { .. })
    ));

    // A linked definition cannot be written: following the link is a guess.
    let refusals: Vec<(&str, AbstractNum)> = vec![
        ("linked", {
            let mut item = AbstractNum::new(7);
            item.num_style_link = Some("List Paragraph".into());
            item
        }),
        ("unmodeled level", {
            let mut item = AbstractNum::new(7);
            let mut level = NumberingLevel::new(NumberFormat::Bullet, "•");
            level.unmodeled = true;
            item.levels[0] = Some(level);
            item
        }),
        ("picture bullet", {
            let mut item = AbstractNum::new(7);
            let mut level = NumberingLevel::new(NumberFormat::Bullet, "•");
            level.picture = Some(3);
            item.levels[0] = Some(level);
            item
        }),
        ("relative indent", {
            let mut item = AbstractNum::new(7);
            let mut level = NumberingLevel::new(NumberFormat::Decimal, "%1");
            level.para.indent.start = Some(Measure::CharHundredths(200));
            item.levels[0] = Some(level);
            item
        }),
        ("justified level", {
            let mut item = AbstractNum::new(7);
            let mut level = NumberingLevel::new(NumberFormat::Decimal, "%1");
            level.para.justification = Some(Justification::Center);
            item.levels[0] = Some(level);
            item
        }),
        ("italic label", {
            let mut item = AbstractNum::new(7);
            let mut level = NumberingLevel::new(NumberFormat::Decimal, "%1");
            level.run.italic = Some(true);
            item.levels[0] = Some(level);
            item
        }),
        ("theme font", {
            let mut item = AbstractNum::new(7);
            let mut level = NumberingLevel::new(NumberFormat::Decimal, "%1");
            level.run.fonts.ascii = Some(FontRef::Theme(joyword_model::ThemeFont::MajorAscii));
            item.levels[0] = Some(level);
            item
        }),
        ("tab in text", {
            let mut item = AbstractNum::new(7);
            item.levels[0] = Some(NumberingLevel::new(NumberFormat::Bullet, "\t"));
            item
        }),
        ("empty format token", {
            let mut item = AbstractNum::new(7);
            item.levels[0] = Some(NumberingLevel::new(NumberFormat::Other("".into()), "%1"));
            item
        }),
    ];
    for (name, abstract_num) in refusals {
        let mut model = result.document.clone();
        assert!(model.numbering_mut().insert_abstract(abstract_num));
        assert!(
            plan_export(&result.backing, &model.snapshot(), &Budget::default()).is_err(),
            "{name} must be refused"
        );
    }

    // An instance naming a definition that is not in the store is refused.
    let mut model = result.document.clone();
    assert!(
        model
            .numbering_mut()
            .insert_instance(joyword_model::NumInstance {
                id: 9,
                abstract_id: 42,
                overrides: Vec::new(),
            })
    );
    assert!(matches!(
        plan_export(&result.backing, &model.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported { .. })
    ));
}

#[test]
fn numbering_level_tabs_write_every_align_and_leader() {
    let bytes = existing_part_package();
    let result = open(&bytes);
    let mut model = result.document.clone();
    let mut level = NumberingLevel::new(NumberFormat::Decimal, "%1)");
    level.start = 1;
    level.para.tabs = Some(vec![
        TabStop {
            position: Measure::Twips(720),
            align: TabAlign::Bar,
            leader: TabLeader::MiddleDot,
        },
        TabStop {
            position: Measure::Twips(1440),
            align: TabAlign::Clear,
            leader: TabLeader::Heavy,
        },
        TabStop {
            position: Measure::Twips(2160),
            align: TabAlign::Decimal,
            leader: TabLeader::Underscore,
        },
    ]);
    level.run.bold = Some(false);
    level.run.size = Some(Measure::HalfPoints(28));
    level.run.fonts.east_asia = Some(FontRef::Named("SimSun".into()));
    let mut abstract_num = AbstractNum::new(5);
    abstract_num.levels[0] = Some(level);
    assert!(model.numbering_mut().insert_abstract(abstract_num));
    assert!(
        model
            .numbering_mut()
            .insert_instance(joyword_model::NumInstance {
                id: 7,
                abstract_id: 5,
                overrides: vec![joyword_model::LevelOverride {
                    level: 0,
                    start_override: Some(4),
                    replacement: None,
                }],
            })
    );
    assign_numbering(&mut model, 1, 7);

    let plan = plan_export(&result.backing, &model.snapshot(), &Budget::default()).unwrap();
    let exported = plan.bytes().to_vec();
    let written = part_text(&exported, "/word/numbering.xml");
    assert!(
        written.contains(
            r#"<w:tabs><w:tab w:val="bar" w:pos="720" w:leader="middleDot"/><w:tab w:val="clear" w:pos="1440" w:leader="heavy"/><w:tab w:val="decimal" w:pos="2160" w:leader="underscore"/></w:tabs>"#
        ),
        "{written}"
    );
    assert!(
        written.contains(
            r#"<w:rPr><w:rFonts w:eastAsia="SimSun"/><w:b w:val="0"/><w:sz w:val="28"/></w:rPr>"#
        ),
        "{written}"
    );
    assert!(
        written.contains(
            r#"<w:num w:numId="7"><w:abstractNumId w:val="5"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="4"/></w:lvlOverride></w:num>"#
        ),
        "{written}"
    );
    let verified = open(&exported);
    assert_eq!(verified.document.numbering(), model.numbering());
    // startOverride 4 wins over the level start.
    assert_eq!(bullet_marks(&verified.document), ["1.", "4)"]);
}
