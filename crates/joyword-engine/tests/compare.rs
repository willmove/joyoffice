//! T303 first slice: the headless compare engine. Accepting every revision of
//! a comparison reproduces the revised document's body text, rejecting every
//! revision restores the original's, and every refusal is explicit.

use std::sync::Arc;

use joyword_engine::{
    Command, CompareError, Outcome, Session, Transaction, body_final_paragraphs, compare_documents,
};
use joyword_model::{Block, Document, Paragraph, RevisionKind, RunProps};

fn document_with(lines: &[&str]) -> Document {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in lines {
        let mut paragraph = Paragraph::new(document.ids().node());
        if !line.is_empty() {
            paragraph.push_text(line, None, Arc::default()).unwrap();
        }
        blocks.push(Block::from(paragraph));
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = joyword_model::BlockSeq::from_vec(blocks);
    document
}

fn texts(document: &Document) -> Vec<String> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

fn resolved_texts(document: &Document, accept: bool) -> Vec<String> {
    let mut session = Session::new(document.clone());
    let command = if accept {
        Command::AcceptAllRevisions
    } else {
        Command::RejectAllRevisions
    };
    match session.apply(Transaction::new(session.version(), vec![command])) {
        Ok(Outcome::Applied(_)) | Ok(Outcome::NoOp(_)) => {}
        Err(error) => panic!("resolve failed: {error:?}"),
    }
    texts(session.document())
}

fn compare(original: &Document, revised: &Document) -> Result<Document, CompareError> {
    compare_documents(original, revised, "比较者", None)
}

fn revision_runs(document: &Document) -> Vec<RevisionKind> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| run.revision.as_ref().map(|revision| revision.kind))
        .collect()
}

fn mark_kinds(document: &Document) -> Vec<RevisionKind> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .filter_map(|paragraph| paragraph.mark_revision.as_ref().map(|mark| mark.kind))
        .collect()
}

#[test]
fn identical_documents_compare_without_revisions() {
    let original = document_with(&["甲乙", "丙丁"]);
    let result = compare(&original, &original).unwrap();
    assert!(revision_runs(&result).is_empty());
    assert!(mark_kinds(&result).is_empty());
    assert_eq!(texts(&result), vec!["甲乙", "丙丁"]);
}

#[test]
fn all_accept_reproduces_revised_and_all_reject_restores_original() {
    let original = document_with(&["甲", "乙", "丙", "丁"]);
    let revised = document_with(&["甲", "乙补", "丙", "丁改", "戊"]);
    let result = compare(&original, &revised).unwrap();
    // "甲" kept; "乙"→"乙补" and "丁"→"丁改" are in-paragraph
    // insertions; "戊" is appended after the last paragraph.
    assert_eq!(
        resolved_texts(&result, true),
        vec!["甲", "乙补", "丙", "丁改", "戊"]
    );
    assert_eq!(resolved_texts(&result, false), vec!["甲", "乙", "丙", "丁"]);
    let kinds = revision_runs(&result);
    assert!(!kinds.is_empty());
    assert!(
        kinds
            .iter()
            .all(|kind| matches!(kind, RevisionKind::Insert | RevisionKind::Delete))
    );
}

#[test]
fn a_deleted_paragraph_between_kept_ones_vanishes_on_accept() {
    let original = document_with(&["一", "二", "三", "四"]);
    let revised = document_with(&["一", "三改", "四"]);
    let result = compare(&original, &revised).unwrap();
    // "二" is deleted outright; "三"→"三改" is an in-paragraph
    // insertion; both sides resolve to the revised and the original text.
    assert_eq!(resolved_texts(&result, true), vec!["一", "三改", "四"]);
    assert_eq!(resolved_texts(&result, false), vec!["一", "二", "三", "四"]);
}

#[test]
fn single_revisions_resolve_step_by_step() {
    let original = document_with(&["一", "二", "三"]);
    let revised = document_with(&["一", "二补", "三"]);
    let result = compare(&original, &revised).unwrap();
    // The diff inside "二" is one tracked insertion of "补".
    let mut session = Session::new(result);
    let mut ids = std::collections::BTreeSet::new();
    for paragraph in session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(|b| b.as_paragraph())
    {
        for run in paragraph.runs() {
            if let Some(revision) = &run.revision {
                ids.insert(revision.id);
            }
        }
    }
    assert_eq!(ids.len(), 1, "one in-paragraph insertion");
    for id in ids {
        session
            .apply(Transaction::new(
                session.version(),
                vec![Command::AcceptRevision { id }],
            ))
            .unwrap();
    }
    assert_eq!(texts(session.document()), vec!["一", "二补", "三"]);
}

#[test]
fn trailing_and_middle_deletions_absorb_the_empty_remainder() {
    let original = document_with(&["一", "二", "三", "四"]);
    let revised = document_with(&["一", "四"]);
    let result = compare(&original, &revised).unwrap();
    assert_eq!(resolved_texts(&result, true), vec!["一", "四"]);
    assert_eq!(resolved_texts(&result, false), vec!["一", "二", "三", "四"]);

    // Deleting through the end of the document leaves no empty shell either.
    let revised = document_with(&["一"]);
    let result = compare(&original, &revised).unwrap();
    assert_eq!(resolved_texts(&result, true), vec!["一"]);
    assert_eq!(resolved_texts(&result, false), vec!["一", "二", "三", "四"]);
}

#[test]
fn inserted_paragraphs_carry_inserted_marks_and_reject_joins_the_next() {
    let original = document_with(&["一", "四"]);
    let revised = document_with(&["一", "二", "三", "四"]);
    let result = compare(&original, &revised).unwrap();
    let marks = mark_kinds(&result);
    assert_eq!(
        marks,
        vec![RevisionKind::Insert, RevisionKind::Insert],
        "each new paragraph's own mark is inserted"
    );
    assert_eq!(resolved_texts(&result, true), vec!["一", "二", "三", "四"]);
    assert_eq!(resolved_texts(&result, false), vec!["一", "四"]);
}

#[test]
fn formatting_only_changes_are_a_recorded_boundary() {
    let mut original = document_with(&["一", "同样的话"]);
    if let Some(Block::Paragraph(paragraph)) = original
        .story_mut(original.body_id())
        .unwrap()
        .blocks
        .get_mut(1)
    {
        let paragraph = Arc::make_mut(paragraph);
        paragraph
            .format_runs(
                joyword_model::ScalarOffset(paragraph.len() - 2)
                    ..joyword_model::ScalarOffset(paragraph.len()),
                &mut |props: &mut RunProps| props.bold = Some(true),
            )
            .unwrap();
    }
    let revised = document_with(&["一", "同样的话"]);
    let result = compare(&original, &revised).unwrap();
    // A matched pair contributes no revision: the comparison is text-only,
    // and the original's formatting survives — a recorded boundary, not a
    // claim of Word parity for formatting diffs.
    assert!(revision_runs(&result).is_empty());
    assert_eq!(resolved_texts(&result, true), vec!["一", "同样的话"]);
    let session = Session::new(result);
    let paragraph = session
        .document()
        .body()
        .blocks
        .iter()
        .nth(1)
        .unwrap()
        .as_paragraph()
        .unwrap();
    let (_, props) = paragraph.format_at(joyword_model::ScalarOffset(paragraph.len()));
    assert_eq!(props.bold, Some(true), "the original's formatting survives");
    let _ = body_final_paragraphs;
}

#[test]
fn one_paragraph_replaced_by_several_diffs_and_inserts() {
    let original = document_with(&["一", "二", "三"]);
    let revised = document_with(&["一", "五", "六", "三"]);
    let result = compare(&original, &revised).unwrap();
    // "二" is paired with "五" (an in-paragraph replacement); "六" is
    // inserted after it. Both projections hold.
    assert_eq!(resolved_texts(&result, true), vec!["一", "五", "六", "三"]);
    assert_eq!(resolved_texts(&result, false), vec!["一", "二", "三"]);
}

#[test]
fn a_change_at_the_very_beginning_is_refused() {
    let original = document_with(&["一", "二"]);
    let revised = document_with(&["二"]);
    let error = compare(&original, &revised).unwrap_err();
    assert_eq!(
        error,
        CompareError::Unsupported("changes at the very beginning of the body are not compared yet")
    );
    let revised = document_with(&["零", "一", "二"]);
    let error = compare(&original, &revised).unwrap_err();
    assert_eq!(
        error,
        CompareError::Unsupported("changes at the very beginning of the body are not compared yet")
    );
}

#[test]
fn pending_revisions_tables_comments_and_story_gaps_are_refused() {
    let original = document_with(&["一", "二"]);
    let revised = document_with(&["一", "二改"]);

    // A pending revision in either input refuses the comparison.
    let mut pending = original.clone();
    {
        let revision_id = pending.allocate_revision_id().expect("revision id");
        let body = pending.body_id();
        let blocks = &mut pending.story_mut(body).unwrap().blocks;
        let Some(Block::Paragraph(paragraph)) = blocks.get_mut(0) else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph).set_last_run_revision(Some(joyword_model::TextRevision {
            id: revision_id,
            kind: RevisionKind::Insert,
            author: Arc::from("作者"),
            date: None,
        }));
    }
    assert!(matches!(
        compare(&pending, &revised),
        Err(CompareError::PendingRevisions)
    ));
    assert!(matches!(
        compare(&original, &pending),
        Err(CompareError::PendingRevisions)
    ));

    // A table in either body refuses.
    let mut tabled = original.clone();
    {
        let ids = tabled.ids();
        let cell = joyword_model::Cell {
            id: ids.node(),
            grid_span: 1,
            vertical_merge: joyword_model::VerticalMerge::None,
            width: None,
            borders: joyword_model::CellBorders::default(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: joyword_model::BlockSeq::from_vec(vec![Block::from(Paragraph::new(
                ids.node(),
            ))]),
            preserved: None,
            props_revision: None,
        };
        let row = joyword_model::Row {
            id: ids.node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![cell],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
            props_revision: None,
        };
        let table = joyword_model::Table {
            id: ids.node(),
            grid: vec![joyword_model::Measure::Twips(2400)],
            rows: vec![row],
            style: None,
            width: None,
            layout: None,
            alignment: None,
            indent: None,
            look: None,
            cell_margin: None,
            preserved: None,
            props_revision: None,
            source: None,
        };
        let body = tabled.body_id();
        tabled
            .story_mut(body)
            .unwrap()
            .blocks
            .push(Block::from(table));
    }
    assert!(matches!(
        compare(&tabled, &revised),
        Err(CompareError::Unsupported("tables are not compared yet"))
    ));

    // A comment in either input refuses.
    let mut commented = original.clone();
    let story = {
        let mut paragraph = Paragraph::new(commented.ids().node());
        paragraph.push_text("批注", None, Arc::default()).unwrap();
        commented.insert_story(joyword_model::StoryKind::Comment, vec![paragraph.into()])
    };
    let comment = joyword_model::Comment::new(story);
    commented.insert_annotation(
        joyword_model::AnnotationKind::Comment(comment),
        joyword_model::Anchor::Detached,
    );
    assert!(matches!(
        compare(&commented, &revised),
        Err(CompareError::Unsupported("comments are not compared yet"))
    ));

    // Differing header stories refuse.
    let mut headed = revised.clone();
    let header = {
        let mut paragraph = Paragraph::new(headed.ids().node());
        paragraph.push_text("页眉", None, Arc::default()).unwrap();
        headed.insert_story(joyword_model::StoryKind::Header, vec![paragraph.into()])
    };
    headed.sections_mut()[0]
        .headers
        .push((joyword_model::HeaderFooterKind::Default, header));
    assert!(matches!(
        compare(&original, &headed),
        Err(CompareError::Unsupported("header or footer stories differ"))
    ));
}
