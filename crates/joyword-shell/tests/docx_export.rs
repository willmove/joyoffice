//! Real importer → engine → export → importer checks, including refusal.
use std::path::PathBuf;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ExportError, ExportMode, ExportTarget, W, import, plan_export};
use joyword_engine::{Command, Origin, ParagraphChange, RunChange, Transaction};
use joyword_model::{Bias, Block, EqualColumns, Justification, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;

fn fixture(id: u8) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn package(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let source = fixture(1);
    let package = Package::read(&source, &Budget::default()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0"?><w:document xmlns:w="{W}" xmlns:x="urn:kept"><w:body>{body}</w:body></w:document>"#
    );
    let ct = std::str::from_utf8(package.content_types_xml())
        .unwrap()
        .replace(
            "</Types>",
            r#"<Default Extension="bin" ContentType="application/octet-stream"/></Types>"#,
        );
    let mut items = vec![("[Content_Types].xml", ct.as_bytes(), Method::Deflated)];
    for part in package.parts() {
        items.push((
            part.name.item_name(),
            if part.name.as_str() == "/word/document.xml" {
                xml.as_bytes()
            } else {
                &part.data
            },
            part.method,
        ));
    }
    for (name, bytes) in extra {
        items.push((name, bytes, Method::Stored));
    }
    write_archive(&items).unwrap()
}

fn pos(session: &DocxSession, index: usize, offset: usize) -> Position {
    let document = session.session().document();
    Position {
        story: document.body_id(),
        paragraph: document.body().blocks.get(index).unwrap().id(),
        offset: ScalarOffset(offset),
        bias: Bias::After,
    }
}

fn apply(session: &mut DocxSession, commands: Vec<Command>) {
    session
        .apply(Transaction::new(session.session().version(), commands))
        .unwrap();
}

fn insert(session: &mut DocxSession, at: Position, text: &str) {
    apply(
        session,
        vec![Command::InsertText {
            at,
            text: text.into(),
        }],
    );
}

fn assert_preserved(before: &[u8], after: &[u8]) {
    let before = Package::read(before, &Budget::default()).unwrap();
    let after = Package::read(after, &Budget::default()).unwrap();
    assert_eq!(before.content_types_xml(), after.content_types_xml());
    assert_eq!(before.parts().count(), after.parts().count());
    assert_eq!(before.relationships(None), after.relationships(None));
    for part in before.parts() {
        let saved = after.part(part.name.as_str()).unwrap();
        assert_eq!(part.content_type, saved.content_type);
        assert_eq!(
            before.relationships(Some(&part.name)),
            after.relationships(Some(&part.name))
        );
        if part.name.as_str() != "/word/document.xml" {
            assert_eq!(part.data, saved.data);
        }
    }
}

#[test]
fn untouched_corpus_is_exact_even_when_restricted() {
    for id in 1..=5 {
        let bytes = fixture(id);
        let session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        let plan = session.export_plan().unwrap();
        assert_eq!(plan.mode(), ExportMode::OriginalPackage);
        assert_eq!(plan.target(), ExportTarget::Docx);
        assert_eq!(
            plan.target_content_type(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
        assert_eq!(plan.bytes(), bytes);
        assert!(plan.changed_parts().is_empty());
        assert_eq!(plan.version(), session.session().version());
    }
}

#[test]
fn text_round_trip_preserves_unknown_parts_properties_and_all_other_markup() {
    let body = r#"<w:p x:para="keep"><w:pPr><w:jc w:val="center"/><w:shd w:fill="AABBCC"/></w:pPr><!--paragraph comment--><w:r x:run="keep"><w:rPr><w:b/><w:highlight w:val="yellow"/></w:rPr><!--run comment--><w:t>原文🙂</w:t></w:r></w:p><w:p><w:r><w:t>第二段</w:t></w:r></w:p>"#;
    let bytes = package(
        body,
        &[("custom/data.bin", b"\x00\xfforiginal unknown payload")],
    );
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert!(session.editability().is_unrestricted());
    let at = pos(&session, 0, 2);
    insert(&mut session, at, "  &<中文>e\u{301}👨‍👩‍👧‍👦  ");
    let plan = session.export_plan().unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    assert_eq!(plan.changed_parts()[0].as_str(), "/word/document.xml");
    assert_preserved(&bytes, plan.bytes());
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        session
            .session()
            .document()
            .semantic_difference(&saved.document)
            .is_none()
    );
    let xml = std::str::from_utf8(
        &saved
            .backing
            .package()
            .part("/word/document.xml")
            .unwrap()
            .data,
    )
    .unwrap();
    for raw in [
        r#"x:para="keep""#,
        r#"x:run="keep""#,
        "<!--paragraph comment-->",
        "<!--run comment-->",
        r#"<w:shd w:fill="AABBCC"/>"#,
        r#"<w:highlight w:val="yellow"/>"#,
        "<w:p><w:r><w:t>第二段</w:t></w:r></w:p>",
    ] {
        assert!(xml.contains(raw), "missing {raw}");
    }
    assert_eq!(session.backing().original_bytes(), bytes);
    assert!(session.unchanged_package_bytes().is_err());
    assert_eq!(plan.bytes(), session.export_plan().unwrap().bytes());
}

#[test]
fn token_merging_tabs_and_cdata_keep_text_and_formatting() {
    let bytes = package(
        r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>A</w:t><w:t><![CDATA[<&🙂]]></w:t><w:tab/><w:t>B</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>C</w:t></w:r></w:p>"#,
        &[],
    );
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    let at = pos(&session, 0, 1);
    insert(&mut session, at, "新 ");
    let plan = session.export_plan().unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    let p = saved
        .document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert_eq!(p.plain_text(|_| String::new()), "A新 <&🙂\tBC");
    assert_eq!(p.runs().last().unwrap().props.italic, Some(true));
    assert_eq!(p.runs().first().unwrap().props.bold, Some(true));
}

#[test]
fn deleting_whole_paragraph_text_keeps_the_paragraph_and_properties() {
    let bytes = package(
        r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>ABC</w:t></w:r></w:p>"#,
        &[],
    );
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    let start = pos(&session, 0, 0);
    let end = pos(&session, 0, 3);
    apply(&mut session, vec![Command::DeleteRange { start, end }]);
    let plan = session.export_plan().unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    let p = saved
        .document
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(p.is_empty());
    assert_eq!(p.props.justification, Some(Justification::End));
}

#[test]
fn plans_are_bound_to_versions_and_survive_future_edits_undo_redo() {
    let bytes = fixture(1);
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    let at = pos(&session, 0, 0);
    insert(&mut session, at, "first");
    let first = session.export_plan().unwrap();
    let first_bytes = first.bytes().to_vec();
    let at = pos(&session, 0, 0);
    insert(&mut session, at, "second");
    let second = session.export_plan().unwrap();
    assert_ne!(first.version(), second.version());
    assert_ne!(first.bytes(), second.bytes());
    assert_eq!(first.bytes(), first_bytes);
    session.undo().unwrap();
    assert_eq!(session.export_plan().unwrap().bytes(), first.bytes());
    session.undo().unwrap();
    assert_eq!(
        session.export_plan().unwrap().mode(),
        ExportMode::OriginalPackage
    );
    assert!(session.unchanged_package_bytes().is_err());
    session.redo().unwrap();
    assert_eq!(session.export_plan().unwrap().bytes(), first.bytes());
}

// Plain paragraph splits and merges export since W05; their support and
// refusal cases are in w05_text_input_export.rs and the docx W05 tests.
#[test]
fn format_edits_refuse_without_changing_session() {
    for kind in 0..2 {
        let bytes = fixture(1);
        let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        let at = pos(&session, 0, 0);
        let end = pos(&session, 0, 1);
        let command = match kind {
            0 => Command::FormatText {
                start: at,
                end,
                change: RunChange::Bold(Some(true)),
            },
            _ => Command::FormatParagraph {
                story: at.story,
                paragraph: at.paragraph,
                change: ParagraphChange::Justification(Some(Justification::Center)),
            },
        };
        apply(&mut session, vec![command]);
        let dump = session
            .session()
            .document()
            .canonical_dump(joyword_model::DumpOptions { node_ids: true });
        let version = session.session().version();
        let plan = session.export_plan().expect("direct formatting exports");
        let xml = String::from_utf8_lossy(plan.bytes());
        match kind {
            0 => assert!(xml.contains("w:b"), "{xml}"),
            _ => assert!(xml.contains("center") || xml.contains("w:jc"), "{xml}"),
        }
        assert_eq!(session.session().version(), version);
        assert_eq!(
            session
                .session()
                .document()
                .canonical_dump(joyword_model::DumpOptions { node_ids: true }),
            dump
        );
        assert!(session.session().can_undo());
        assert_eq!(session.backing().original_bytes(), bytes);
    }
}

#[test]
fn unknown_text_attributes_comments_and_namespaces_are_not_dropped() {
    for leaf in [
        r#"<w:t x:unknown="must keep">A</w:t>"#,
        "<w:t>A<!--keep-->B</w:t>",
        "<w:t>A<?keep data?>B</w:t>",
        r#"<w:t xmlns:other="urn:unknown">A</w:t>"#,
    ] {
        let bytes = package(&format!("<w:p><w:r>{leaf}</w:r></w:p>"), &[]);
        let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        let at = pos(&session, 0, 0);
        insert(&mut session, at, "edit");
        assert!(matches!(
            session.export_plan(),
            Err(ExportError::Unsupported { .. })
        ));
        assert_eq!(session.backing().original_bytes(), bytes);
    }
}

#[test]
fn protected_and_unknown_models_cannot_bypass_export_restrictions() {
    for id in [3, 5] {
        let bytes = fixture(id);
        let source = import(&bytes, &Budget::default()).unwrap();
        let mut model = source.document.clone();
        let body = model.body_id();
        model.story_mut(body).unwrap().blocks.update(0, |block| {
            let Block::Paragraph(p) = block else {
                panic!("first paragraph");
            };
            std::sync::Arc::make_mut(p)
                .insert_text(ScalarOffset(0), "bypass")
                .unwrap();
        });
        assert_eq!(
            plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap_err(),
            ExportError::Restricted
        );
    }
}

#[test]
fn signed_packages_are_copied_exactly_but_edited_output_is_refused() {
    let bytes = package(
        "<w:p><w:r><w:t>A</w:t></w:r></w:p>",
        &[("_xmlsignatures/signature.bin", b"signature bytes")],
    );
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert_eq!(session.export_plan().unwrap().bytes(), bytes);
    let at = pos(&session, 0, 0);
    insert(&mut session, at, "edit");
    assert_eq!(
        session.export_plan().unwrap_err(),
        ExportError::SignedPackage
    );
}

#[test]
fn original_budget_bounds_edited_output_without_mutating_session() {
    let bytes = fixture(1);
    let parsed = Package::read(&bytes, &Budget::default()).unwrap();
    let largest = parsed
        .parts()
        .map(|p| p.data.len())
        .max()
        .unwrap()
        .max(parsed.content_types_xml().len());
    let budget = Budget {
        max_part_bytes: (largest + 100) as u64,
        ..Budget::default()
    };
    let mut session = DocxSession::open(&bytes, &budget).unwrap();
    let at = pos(&session, 0, 0);
    insert(&mut session, at, &"X".repeat(largest + 200));
    let version = session.session().version();
    assert!(matches!(
        session.export_plan(),
        Err(ExportError::Budget { .. })
    ));
    assert_eq!(session.session().version(), version);
    assert_eq!(session.backing().original_bytes(), bytes);
}

#[test]
fn failed_agent_and_noop_transactions_leave_exact_export_available() {
    let bytes = fixture(1);
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    let mut tx = Transaction::new(
        session.session().version(),
        vec![Command::InsertText {
            at: pos(&session, 0, 0),
            text: "AI".into(),
        }],
    );
    tx.origin = Origin::Agent;
    assert!(session.apply(tx).is_err());
    apply(&mut session, vec![]);
    assert_eq!(session.export_plan().unwrap().bytes(), bytes);
}

#[test]
fn deterministic_unicode_edit_sequences_round_trip_and_undo() {
    let bytes = package("<w:p><w:r><w:t>seed中🙂</w:t></w:r></w:p>", &[]);
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    let mut seed = 0xC020_2026u64;
    for index in 0..100 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let len = session
            .session()
            .document()
            .body()
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .len();
        let at = pos(&session, 0, (seed as usize) % (len + 1));
        insert(
            &mut session,
            at,
            ["中", "🙂", "e\u{301}", "<&>", " ", "👩‍💻"][index % 6],
        );
        let plan = session.export_plan().unwrap();
        let saved = import(plan.bytes(), &Budget::default()).unwrap();
        assert!(
            session
                .session()
                .document()
                .semantic_difference(&saved.document)
                .is_none(),
            "iteration {index}"
        );
        if index % 4 == 0 {
            session.undo().unwrap();
            session.export_plan().unwrap();
            session.redo().unwrap();
        }
    }
}

#[test]
fn typing_into_empty_paragraph_preserves_its_original_attributes_and_markup() {
    for body in [
        r#"<w:p x:keep="yes"/>"#,
        r#"<w:p x:keep="yes"><w:pPr><w:jc w:val="center"/></w:pPr><!--keep--><w:r><w:t/></w:r></w:p>"#,
    ] {
        let bytes = package(body, &[]);
        let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        let at = pos(&session, 0, 0);
        insert(&mut session, at, "新文字🙂 &<>");
        let plan = session.export_plan().unwrap();
        assert_eq!(
            plan.warnings(),
            &[joyword_docx::ExportWarning::ZipMetadataRegenerated]
        );
        let saved = import(plan.bytes(), &Budget::default()).unwrap();
        assert!(
            session
                .session()
                .document()
                .semantic_difference(&saved.document)
                .is_none()
        );
        let xml = std::str::from_utf8(
            &saved
                .backing
                .package()
                .part("/word/document.xml")
                .unwrap()
                .data,
        )
        .unwrap();
        assert!(xml.contains(r#"x:keep="yes""#));
        if body.contains("<!--keep-->") {
            assert!(xml.contains("<!--keep-->"));
        }
    }
}

#[test]
fn snapshot_changes_outside_text_are_never_silently_ignored() {
    for kind in 0..5 {
        let bytes = fixture(1);
        let source = import(&bytes, &Budget::default()).unwrap();
        let mut model = source.document.clone();
        match kind {
            0 => model.settings_mut().compatibility_mode = Some(11),
            1 => model.sections_mut()[0].title_page = Some(true),
            2 => {
                model.sections_mut()[0].columns = Some(EqualColumns {
                    count: 2,
                    space: 720,
                });
            }
            3 => {
                let mut style = joyword_model::Style::new(
                    "extra",
                    joyword_model::StyleKind::Character,
                    Some("extra"),
                );
                style.custom = true;
                model.styles_mut().insert(style).unwrap();
            }
            _ => {
                let body = model.body_id();
                model.story_mut(body).unwrap().blocks.update(0, |block| {
                    let Block::Paragraph(p) = block else {
                        unreachable!()
                    };
                    std::sync::Arc::make_mut(p).source = None;
                });
            }
        }
        assert!(matches!(
            plan_export(&source.backing, &model.snapshot(), &Budget::default()),
            Err(ExportError::Unsupported { .. })
        ));
    }
}

#[test]
fn edited_macros_and_templates_are_refused_and_exact_copy_reports_their_type() {
    for (main_type, target) in [
        (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml",
            ExportTarget::Dotx,
        ),
        (
            "application/vnd.ms-word.document.macroEnabled.main+xml",
            ExportTarget::Docm,
        ),
        (
            "application/vnd.ms-word.template.macroEnabledTemplate.main+xml",
            ExportTarget::Dotm,
        ),
    ] {
        let bytes = fixture(1);
        let parsed = Package::read(&bytes, &Budget::default()).unwrap();
        let ct = std::str::from_utf8(parsed.content_types_xml())
            .unwrap()
            .replace(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
                main_type,
            );
        let mut items = vec![("[Content_Types].xml", ct.as_bytes(), Method::Stored)];
        for part in parsed.parts() {
            items.push((part.name.item_name(), &part.data, part.method));
        }
        let bytes = write_archive(&items).unwrap();
        let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
        assert_eq!(session.export_plan().unwrap().target(), target);
        assert_eq!(session.export_plan().unwrap().bytes(), bytes);
        let at = pos(&session, 0, 0);
        insert(&mut session, at, "edit");
        assert!(matches!(
            session.export_plan(),
            Err(ExportError::Unsupported { .. })
        ));
    }
}

#[test]
fn edited_output_must_pass_total_byte_and_xml_event_budgets_too() {
    let bytes = package("<w:p><w:r><w:t>A</w:t></w:r></w:p>", &[]);
    let parsed = Package::read(&bytes, &Budget::default()).unwrap();
    let total =
        parsed.content_types_xml().len() + parsed.parts().map(|p| p.data.len()).sum::<usize>();
    let budget = Budget {
        max_total_bytes: total as u64 + 20,
        ..Budget::default()
    };
    let mut session = DocxSession::open(&bytes, &budget).unwrap();
    let at = pos(&session, 0, 0);
    insert(&mut session, at, &"x".repeat(100));
    assert!(matches!(
        session.export_plan(),
        Err(ExportError::Budget { .. })
    ));

    let budget = Budget {
        max_xml_nodes: 100,
        ..Budget::default()
    };
    let source = import(&bytes, &budget).unwrap();
    let mut model = source.document.clone();
    let body = model.body_id();
    model.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(p) = block else {
            unreachable!()
        };
        std::sync::Arc::make_mut(p)
            .insert_text(ScalarOffset(0), &"\t".repeat(110))
            .unwrap();
    });
    assert!(matches!(
        plan_export(&source.backing, &model.snapshot(), &budget),
        Err(ExportError::Import(_))
    ));
}

#[test]
fn external_relationships_and_untouched_breaks_remain_exact_after_text_export() {
    let original = fixture(4);
    let parsed = Package::read(&original, &Budget::default()).unwrap();
    // Keep the fixture's external relationships, including the unfetched
    // file:// template, and replace the body with an editable document.
    let xml = format!(
        r#"<w:document xmlns:w="{W}"><w:body><w:p><w:r><w:t>A</w:t></w:r></w:p><w:p><w:r><w:t>B</w:t><w:br/><w:t>C</w:t></w:r></w:p></w:body></w:document>"#
    );
    let mut items = vec![(
        "[Content_Types].xml",
        parsed.content_types_xml(),
        Method::Stored,
    )];
    for part in parsed.parts() {
        items.push((
            part.name.item_name(),
            if part.name.as_str() == "/word/document.xml" {
                xml.as_bytes()
            } else {
                &part.data
            },
            part.method,
        ));
    }
    let bytes = write_archive(&items).unwrap();
    let mut session = DocxSession::open(&bytes, &Budget::default()).unwrap();
    assert!(session.editability().is_unrestricted());
    let at = pos(&session, 0, 0);
    insert(&mut session, at, "safe ");
    let plan = session.export_plan().unwrap();
    assert_preserved(&bytes, plan.bytes());
    assert!(
        session
            .session()
            .document()
            .semantic_difference(&import(plan.bytes(), &Budget::default()).unwrap().document)
            .is_none()
    );
}
