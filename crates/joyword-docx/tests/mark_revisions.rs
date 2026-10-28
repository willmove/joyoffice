//! T402: deleted paragraph marks (`w:pPr/w:rPr/w:del`) import, export and
//! round-trip, including the AI-authored case.

use std::path::PathBuf;
use std::sync::Arc;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{ExportMode, import, plan_export};
use joyword_model::{Block, Document, RevisionKind, TextRevision};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn body(body: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    let items: Vec<(&str, &[u8], Method)> = vec![
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("_rels/.rels", PACKAGE_RELS.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ]
    .into_iter()
    .map(|(name, data)| (name, data, Method::Deflated))
    .collect();
    write_archive(&items).expect("package")
}

fn fixture(id: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/docx")
        .join(format!("{id}.docx"));
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A clean three-paragraph document without revisions.
fn clean_bytes() -> Vec<u8> {
    body(concat!(
        r#"<w:p><w:r><w:t>甲乙</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>丙丁</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>戊己</w:t></w:r></w:p>"#,
    ))
}

fn paragraphs(document: &Document) -> Vec<&joyword_model::Paragraph> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .collect()
}

fn set_mark(document: &mut Document, index: usize, revision: Option<TextRevision>) {
    if let Some(revision) = &revision {
        document.note_revision_id(revision.id);
    }
    let id = document.body_id();
    let blocks = &mut document.story_mut(id).expect("body").blocks;
    let Some(block) = blocks.get_mut(index) else {
        panic!("missing paragraph {index}");
    };
    let Block::Paragraph(paragraph) = block else {
        panic!("not a paragraph");
    };
    Arc::make_mut(paragraph).mark_revision = revision;
}

#[test]
fn a_deleted_paragraph_mark_imports_typed_and_keeps_the_document_editable() {
    let bytes = body(concat!(
        r#"<w:p><w:pPr><w:rPr><w:del w:id="7" w:author="审阅人" w:date="2020-01-02T03:04:05Z"/></w:rPr></w:pPr><w:r><w:t>一</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>二</w:t></w:r></w:p>"#,
    ));
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(!result.editability.is_read_only());
    let mark = paragraphs(&result.document)[0]
        .mark_revision
        .as_ref()
        .expect("typed mark revision");
    assert_eq!(mark.id, 7);
    assert_eq!(mark.kind, RevisionKind::Delete);
    assert_eq!(mark.author.as_ref(), "审阅人");
    assert_eq!(mark.date.as_deref(), Some("2020-01-02T03:04:05Z"));
    assert!(paragraphs(&result.document)[1].mark_revision.is_none());
}

#[test]
fn a_malformed_or_rich_paragraph_mark_stays_a_preserved_gap() {
    for rpr in [
        r#"<w:del w:id="7" w:author="A"><w:b/></w:del>"#,
        r#"<w:del w:author="A"/>"#,
        r#"<w:del w:id="7" w:author="A"/><w:del w:id="8" w:author="A"/>"#,
        r#"<w:ins w:id="7" w:author="A"><w:b/></w:ins>"#,
        r#"<w:ins w:author="A"/>"#,
    ] {
        let bytes = body(&format!(
            r#"<w:p><w:pPr><w:rPr>{rpr}</w:rPr></w:pPr><w:r><w:t>一</w:t></w:r></w:p><w:p><w:r><w:t>二</w:t></w:r></w:p>"#
        ));
        let result = import(&bytes, &Budget::default()).unwrap();
        assert!(
            paragraphs(&result.document)[0].mark_revision.is_none(),
            "{rpr} must not type"
        );
        assert!(
            paragraphs(&result.document)[0]
                .mark_props
                .preserved
                .is_some(),
            "{rpr} must stay preserved"
        );
    }
}

#[test]
fn a_typed_imported_mark_round_trips_and_survives_a_rewrite() {
    let bytes = body(concat!(
        r#"<w:p><w:pPr><w:jc w:val="center"/><w:rPr><w:del w:id="7" w:author="审阅人" w:date="2020-01-02T03:04:05Z"/></w:rPr></w:pPr><w:r><w:t>一</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>二</w:t></w:r></w:p>"#,
    ));
    let source = import(&bytes, &Budget::default()).unwrap();
    // Unchanged export keeps the original bytes.
    let untouched = plan_export(
        &source.backing,
        &source.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(untouched.mode(), ExportMode::OriginalPackage);
    assert_eq!(untouched.bytes(), bytes.as_slice());

    // A tracked text change on the same paragraph rewrites it; the mark
    // revision must survive the rewrite byte-for-byte in the re-import.
    let mut model = source.document.clone();
    let id = model.body_id();
    let story = model.story_mut(id).expect("body");
    let Some(block) = story.blocks.get_mut(0) else {
        panic!("paragraph");
    };
    let Block::Paragraph(paragraph) = block else {
        panic!("paragraph");
    };
    let paragraph = Arc::make_mut(paragraph);
    paragraph
        .push_text_marked(
            "新",
            None,
            Arc::default(),
            TextRevision {
                id: 9,
                kind: RevisionKind::Insert,
                author: Arc::from("JoyWord AI"),
                date: Some(Arc::from("2026-10-23T00:00:00Z")),
            },
        )
        .unwrap();
    model.note_revision_id(9);
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        model.semantic_difference(&saved.document).is_none(),
        "model:\n{}\n---\nsaved:\n{}",
        model.canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        saved
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
    );
    let mark = paragraphs(&saved.document)[0]
        .mark_revision
        .as_ref()
        .expect("mark survives the rewrite");
    assert_eq!(
        (mark.id, mark.kind, mark.author.as_ref()),
        (7, RevisionKind::Delete, "审阅人")
    );
}

#[test]
fn an_engine_created_mark_is_injected_and_a_resolved_one_is_stripped() {
    let bytes = clean_bytes();
    let source = import(&bytes, &Budget::default()).unwrap();

    // One AI-authored deleted mark on the first paragraph: export must
    // synthesize w:pPr/w:rPr/w:del even though the original pPr is absent.
    let mut model = source.document.clone();
    let revision = TextRevision {
        id: 4,
        kind: RevisionKind::Delete,
        author: Arc::from("JoyWord AI"),
        date: Some(Arc::from("2026-10-23T00:00:00Z")),
    };
    set_mark(&mut model, 0, Some(revision));
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        model.semantic_difference(&saved.document).is_none(),
        "model:\n{}\n---\nsaved:\n{}",
        model.canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        saved
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
    );
    let mark = paragraphs(&saved.document)[0]
        .mark_revision
        .as_ref()
        .expect("injected mark re-imports");
    assert_eq!((mark.id, mark.author.as_ref()), (4, "JoyWord AI"));

    // Resolving the mark (reject) strips the injected bytes again.
    set_mark(&mut model, 0, None);
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(model.semantic_difference(&saved.document).is_none());
    assert!(paragraphs(&saved.document)[0].mark_revision.is_none());
}

#[test]
fn a_fixture_export_with_an_ai_mark_round_trips() {
    let bytes = fixture("DOCX-MIN-001");
    let source = import(&bytes, &Budget::default()).unwrap();
    let mut model = source.document.clone();
    let count = paragraphs(&model).len();
    let revision = TextRevision {
        id: 100,
        kind: RevisionKind::Delete,
        author: Arc::from("JoyWord AI"),
        date: Some(Arc::from("2026-10-23T08:00:00Z")),
    };
    set_mark(&mut model, 0, Some(revision));
    let _ = count;
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        model.semantic_difference(&saved.document).is_none(),
        "model:\n{}\n---\nsaved:\n{}",
        model.canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        saved
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
    );
}

#[test]
fn the_comparator_cannot_canonicalize_a_mark_revision_away() {
    // Two documents that differ only in the mark revision must not compare
    // equal through the preserved-handle normalization path.
    let bytes = clean_bytes();
    let source = import(&bytes, &Budget::default()).unwrap();
    let mut with_mark = source.document.clone();
    let revision = TextRevision {
        id: 4,
        kind: RevisionKind::Delete,
        author: Arc::from("JoyWord AI"),
        date: None,
    };
    set_mark(&mut with_mark, 0, Some(revision));
    assert!(
        source.document.semantic_difference(&with_mark).is_some(),
        "the canonical dump must see the mark"
    );
}

#[test]
fn a_full_ai_replace_round_trips_through_docx() {
    use joyword_engine::{AI_REVISION_AUTHOR, EditPolicy, Session, apply_ai_text_replace};
    use joyword_model::DocumentVersion;

    let bytes = clean_bytes();
    let source = import(&bytes, &Budget::default()).unwrap();
    let mut session = Session::new(source.document.clone());
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: true,
    });
    session.set_revision_author("用户");
    session.set_revision_date(Some(Arc::from("2026-10-23T08:00:00Z")));
    let base: DocumentVersion = session.version();
    // "乙\n丙" — the tail of the first paragraph, its mark, and the head of
    // the second one, replaced in one Agent transaction.
    apply_ai_text_replace(&mut session, base, 1, 4, "X").unwrap();
    assert_eq!(
        joyword_engine::body_final_paragraphs(session.document()).unwrap(),
        vec!["甲X丁", "戊己"]
    );

    let plan = plan_export(
        &source.backing,
        &session.document().snapshot(),
        &Budget::default(),
    )
    .unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        session
            .document()
            .semantic_difference(&saved.document)
            .is_none(),
        "revised:\n{}\n---\nre-imported:\n{}",
        session
            .document()
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        saved
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
    );
    let first = paragraphs(&saved.document)[0];
    let mark = first.mark_revision.as_ref().expect("mark revision");
    assert_eq!(mark.kind, RevisionKind::Delete);
    assert_eq!(mark.author.as_ref(), AI_REVISION_AUTHOR);
    assert_eq!(mark.date.as_deref(), Some("2026-10-23T08:00:00Z"));
    let run_authors: Vec<(RevisionKind, &str)> = first
        .runs()
        .iter()
        .filter_map(|run| {
            run.revision
                .as_ref()
                .map(|revision| (revision.kind, revision.author.as_ref()))
        })
        .collect();
    assert!(!run_authors.is_empty());
    assert!(
        run_authors
            .iter()
            .all(|(_, author)| *author == AI_REVISION_AUTHOR)
    );
    // The final-view projection agrees after the round trip.
    assert_eq!(
        joyword_engine::body_final_paragraphs(&saved.document).unwrap(),
        vec!["甲X丁", "戊己"]
    );
}

#[test]
fn an_inserted_paragraph_mark_imports_typed_and_round_trips() {
    let bytes = body(concat!(
        r#"<w:p><w:pPr><w:jc w:val="center"/><w:rPr><w:ins w:id="7" w:author="审阅人" w:date="2020-01-02T03:04:05Z"/></w:rPr></w:pPr><w:r><w:t>新段</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>旧段</w:t></w:r></w:p>"#,
    ));
    let result = import(&bytes, &Budget::default()).unwrap();
    assert!(!result.editability.is_read_only());
    let mark = paragraphs(&result.document)[0]
        .mark_revision
        .as_ref()
        .expect("typed inserted mark");
    assert_eq!(
        (mark.id, mark.kind, mark.author.as_ref()),
        (7, RevisionKind::Insert, "审阅人")
    );

    // An untouched export keeps the original bytes; a tracked text change on
    // the same paragraph rewrites it and the inserted mark survives.
    let source = import(&bytes, &Budget::default()).unwrap();
    let untouched = plan_export(
        &source.backing,
        &source.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(untouched.mode(), ExportMode::OriginalPackage);
    assert_eq!(untouched.bytes(), bytes.as_slice());

    let mut model = source.document.clone();
    let id = model.body_id();
    let story = model.story_mut(id).expect("body");
    let Some(Block::Paragraph(paragraph)) = story.blocks.get_mut(0) else {
        panic!("paragraph");
    };
    Arc::make_mut(paragraph)
        .push_text_marked(
            "补",
            None,
            Arc::default(),
            TextRevision {
                id: 9,
                kind: RevisionKind::Insert,
                author: Arc::from("比较者"),
                date: Some(Arc::from("2026-10-28T00:00:00Z")),
            },
        )
        .unwrap();
    model.note_revision_id(9);
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        model.semantic_difference(&saved.document).is_none(),
        "model:
{}
---
saved:
{}",
        model.canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        saved
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
    );
    let mark = paragraphs(&saved.document)[0]
        .mark_revision
        .as_ref()
        .expect("inserted mark survives the rewrite");
    assert_eq!(
        (mark.id, mark.kind, mark.author.as_ref()),
        (7, RevisionKind::Insert, "审阅人")
    );
}

#[test]
fn an_engine_created_inserted_mark_is_injected_and_a_resolved_one_is_stripped() {
    let bytes = clean_bytes();
    let source = import(&bytes, &Budget::default()).unwrap();

    // A compare-created inserted mark on the first paragraph: export must
    // synthesize w:pPr/w:rPr/w:ins even though the original pPr is absent.
    let mut model = source.document.clone();
    let revision = TextRevision {
        id: 4,
        kind: RevisionKind::Insert,
        author: Arc::from("比较者"),
        date: None,
    };
    set_mark(&mut model, 0, Some(revision));
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        model.semantic_difference(&saved.document).is_none(),
        "model:
{}
---
saved:
{}",
        model.canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        saved
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
    );
    let mark = paragraphs(&saved.document)[0]
        .mark_revision
        .as_ref()
        .expect("injected inserted mark re-imports");
    assert_eq!((mark.id, mark.kind), (4, RevisionKind::Insert));

    // Resolving the mark strips the injected bytes again.
    set_mark(&mut model, 0, None);
    let plan = plan_export(&source.backing, &model.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(model.semantic_difference(&saved.document).is_none());
    assert!(paragraphs(&saved.document)[0].mark_revision.is_none());
}

#[test]
fn an_in_paragraph_compare_result_round_trips_through_docx() {
    let source = import(&clean_bytes(), &Budget::default()).unwrap();
    // The compare inputs share the package envelope: the original is the
    // imported document, the revised one applies the same text change
    // untracked. The compare result is exported against the ORIGINAL package
    // backing, so the pending revisions are injected into the copied
    // paragraphs. A comparison with structural paragraph changes (added or
    // removed paragraphs) is not exportable yet: the export plan patches
    // existing paragraphs only, and that boundary is recorded for the next
    // slice.
    let original = source.document.clone();
    let mut session = joyword_engine::Session::new(original.clone());
    let paragraph = original
        .body()
        .blocks
        .iter()
        .nth(1)
        .and_then(|block| block.as_paragraph())
        .map(joyword_model::Paragraph::id)
        .unwrap();
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::InsertText {
                at: joyword_model::Position {
                    story: original.body_id(),
                    paragraph,
                    offset: joyword_model::ScalarOffset(2),
                    bias: joyword_model::Bias::Before,
                },
                text: "补".into(),
            }],
        ))
        .unwrap();
    let revised = session.document().clone();
    let result = joyword_engine::compare_documents(
        &original,
        &revised,
        "比较者",
        Some(Arc::from("2026-10-28T00:00:00Z")),
    )
    .unwrap();
    let plan = plan_export(&source.backing, &result.snapshot(), &Budget::default()).unwrap();
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert!(
        result.semantic_difference(&saved.document).is_none(),
        "model:
{}
---
saved:
{}",
        result.canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        saved
            .document
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
    );
    // The re-imported comparison still resolves to both projections.
    let texts = |document: &joyword_model::Document| {
        paragraphs(document)
            .iter()
            .map(|paragraph| paragraph.plain_text(|_| String::new()))
            .collect::<Vec<_>>()
    };
    let mut session = joyword_engine::Session::new(saved.document.clone());
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::AcceptAllRevisions],
        ))
        .unwrap();
    assert_eq!(
        texts(session.document()),
        vec!["甲乙".to_owned(), "丙丁补".to_owned(), "戊己".to_owned()]
    );
    let mut session = joyword_engine::Session::new(saved.document);
    session
        .apply(joyword_engine::Transaction::new(
            session.version(),
            vec![joyword_engine::Command::RejectAllRevisions],
        ))
        .unwrap();
    assert_eq!(
        texts(session.document()),
        vec!["甲乙".to_owned(), "丙丁".to_owned(), "戊己".to_owned()]
    );
}
