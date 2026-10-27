//! T300: paired moveFrom/moveTo accept, reject, undo, and the typing seal.

use std::sync::Arc;

use joyword_engine::{
    Command, CommandError, Selection, Session, Transaction, TxError, UndoGroup,
    body_final_paragraphs,
};
use joyword_model::{
    Bias, Block, BlockSeq, Document, MoveMarker, MoveSide, NodeId, Paragraph, Position,
    RevisionKind, RevisionView, ScalarOffset, StoryId, TextRevision,
};

fn mark(kind: RevisionKind, id: u32) -> TextRevision {
    TextRevision {
        id,
        kind,
        author: Arc::from("A"),
        date: Some(Arc::from("2020-01-02T03:04:05Z")),
    }
}

fn marker(
    story: StoryId,
    paragraph: NodeId,
    offset: usize,
    at_end: bool,
    side: MoveSide,
    id: u32,
) -> MoveMarker {
    MoveMarker {
        id,
        name: Arc::from("m"),
        author: Arc::from("A"),
        date: Some(Arc::from("2020-01-02T03:04:05Z")),
        side,
        at_end,
        position: MoveMarker::position_for(story, paragraph, offset, at_end),
    }
}

/// `P` + moveFrom `src`, then moveTo `dst` + insert `I` + `S`, then insert `X`.
///
/// Source markers cover the plain `P` as well as `src`. Destination markers
/// cover `dst` and `I`. `S` and `X` are outside the pair.
fn paired_move() -> Session {
    let mut document = Document::new_blank();
    let story = document.body_id();
    let mut source = Paragraph::new(document.ids().node());
    let mut destination = Paragraph::new(document.ids().node());
    let mut extra = Paragraph::new(document.ids().node());
    source.push_text("P", None, Arc::default()).unwrap();
    source
        .push_text_marked("src", None, Arc::default(), mark(RevisionKind::MoveFrom, 2))
        .unwrap();
    destination
        .push_text_marked("dst", None, Arc::default(), mark(RevisionKind::MoveTo, 5))
        .unwrap();
    destination
        .push_text_marked("I", None, Arc::default(), mark(RevisionKind::Insert, 7))
        .unwrap();
    destination.push_text("S", None, Arc::default()).unwrap();
    extra
        .push_text_marked("X", None, Arc::default(), mark(RevisionKind::Insert, 9))
        .unwrap();
    let source_id = source.id();
    let destination_id = destination.id();
    document.note_revision_id(9);
    document.story_mut(story).unwrap().blocks =
        BlockSeq::from_vec(vec![source.into(), destination.into(), extra.into()]);
    document.move_markers_mut().extend([
        marker(story, source_id, 0, false, MoveSide::From, 1),
        marker(story, source_id, 4, true, MoveSide::From, 3),
        marker(story, destination_id, 0, false, MoveSide::To, 4),
        marker(story, destination_id, 4, true, MoveSide::To, 6),
    ]);
    Session::new(document)
}

fn texts(session: &Session) -> Vec<String> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| String::new()))
        .collect()
}

fn projected(session: &Session, view: RevisionView) -> Vec<String> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| {
            let plain = paragraph.plain_text(|_| String::new());
            let mut chars = plain.chars();
            let mut out = String::new();
            for run in paragraph.runs() {
                let piece: String = chars.by_ref().take(run.len).collect();
                let show = run
                    .revision
                    .as_ref()
                    .map(|revision| revision.kind.visible_in(view))
                    .unwrap_or(true);
                if show {
                    out.push_str(&piece);
                }
            }
            out
        })
        .collect()
}

fn revision_ids(session: &Session) -> Vec<u32> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| run.revision.as_ref().map(|mark| mark.id))
        .collect()
}

fn marker_count(session: &Session) -> usize {
    session.document().move_markers().len()
}

fn exec(session: &mut Session, commands: Vec<Command>) -> Result<(), TxError> {
    session
        .apply(Transaction::new(session.version(), commands))
        .map(|_| ())
}

fn command_error(error: TxError) -> CommandError {
    match error {
        TxError::Command { error, .. } => error,
        other => panic!("expected a command error, got {other:?}"),
    }
}

fn pos(session: &Session, paragraph: NodeId, offset: usize) -> Position {
    Position {
        story: session.document().body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::After,
    }
}

#[test]
fn accept_all_and_reject_all_restore_the_move_projections() {
    let mut session = paired_move();
    let final_text = projected(&session, RevisionView::Final);
    let original_text = projected(&session, RevisionView::Original);
    assert_eq!(
        final_text,
        vec!["P".to_string(), "dstIS".to_string(), "X".to_string()]
    );
    assert_eq!(
        original_text,
        vec!["Psrc".to_string(), "S".to_string(), String::new()]
    );

    exec(&mut session, vec![Command::AcceptAllRevisions]).unwrap();
    assert_eq!(texts(&session), final_text);
    assert!(revision_ids(&session).is_empty());
    assert_eq!(marker_count(&session), 0);
    session.undo().unwrap();
    assert_eq!(texts(&session), vec!["Psrc", "dstIS", "X"]);
    assert_eq!(marker_count(&session), 4);
    assert_eq!(revision_ids(&session), vec![2, 5, 7, 9]);

    exec(&mut session, vec![Command::RejectAllRevisions]).unwrap();
    assert_eq!(texts(&session), original_text);
    assert!(revision_ids(&session).is_empty());
    assert_eq!(marker_count(&session), 0);
    session.undo().unwrap();
    assert_eq!(marker_count(&session), 4);
    assert_eq!(texts(&session), vec!["Psrc", "dstIS", "X"]);
}

#[test]
fn accepting_either_move_end_resolves_the_pair_and_leaves_other_revisions() {
    let anchor = paired_move();
    let final_chars = projected(&anchor, RevisionView::Final);
    for id in [2u32, 5] {
        let mut session = paired_move();
        exec(&mut session, vec![Command::AcceptRevision { id }]).unwrap();
        assert_eq!(texts(&session), final_chars);
        assert_eq!(revision_ids(&session), vec![7, 9]);
        assert_eq!(marker_count(&session), 0);
        session.undo().unwrap();
        assert_eq!(texts(&session), vec!["Psrc", "dstIS", "X"]);
        assert_eq!(revision_ids(&session), vec![2, 5, 7, 9]);
        assert_eq!(marker_count(&session), 4);
        assert!(
            session
                .document()
                .move_markers()
                .iter()
                .all(|marker| marker.name.as_ref() == "m")
        );
    }

    let mut session = paired_move();
    exec(&mut session, vec![Command::RejectRevision { id: 5 }]).unwrap();
    assert_eq!(texts(&session), vec!["Psrc", "IS", "X"]);
    assert_eq!(revision_ids(&session), vec![7, 9]);
    assert_eq!(marker_count(&session), 0);
    session.undo().unwrap();
    assert_eq!(marker_count(&session), 4);
    assert_eq!(texts(&session), vec!["Psrc", "dstIS", "X"]);

    let mut session = paired_move();
    exec(&mut session, vec![Command::AcceptRevision { id: 9 }]).unwrap();
    assert_eq!(texts(&session), vec!["Psrc", "dstIS", "X"]);
    assert_eq!(revision_ids(&session), vec![2, 5, 7]);
    assert_eq!(marker_count(&session), 4);
}

#[test]
fn a_move_run_that_crosses_its_marker_is_refused() {
    let mut document = Document::new_blank();
    let story = document.body_id();
    let mut source = Paragraph::new(document.ids().node());
    let mut destination = Paragraph::new(document.ids().node());
    source
        .push_text_marked("src", None, Arc::default(), mark(RevisionKind::MoveFrom, 2))
        .unwrap();
    destination
        .push_text_marked("dstI", None, Arc::default(), mark(RevisionKind::MoveTo, 5))
        .unwrap();
    let source_id = source.id();
    let destination_id = destination.id();
    document.story_mut(story).unwrap().blocks =
        BlockSeq::from_vec(vec![source.into(), destination.into()]);
    document.move_markers_mut().extend([
        marker(story, source_id, 0, false, MoveSide::From, 1),
        marker(story, source_id, 3, true, MoveSide::From, 3),
        marker(story, destination_id, 0, false, MoveSide::To, 4),
        marker(story, destination_id, 2, true, MoveSide::To, 6),
    ]);
    let mut session = Session::new(document);
    let before = texts(&session);
    let error = exec(&mut session, vec![Command::AcceptRevision { id: 2 }]).unwrap_err();
    assert!(matches!(
        command_error(error),
        CommandError::RevisionConflict
    ));
    assert_eq!(texts(&session), before);
    assert_eq!(marker_count(&session), 4);
    let error = exec(&mut session, vec![Command::AcceptAllRevisions]).unwrap_err();
    assert!(matches!(
        command_error(error),
        CommandError::RevisionConflict
    ));
    assert_eq!(texts(&session), before);
    assert_eq!(marker_count(&session), 4);
}

#[test]
fn incomplete_and_crossed_moves_leave_the_document_unchanged() {
    let mut document = Document::new_blank();
    let story = document.body_id();
    let mut source = Paragraph::new(document.ids().node());
    let mut destination = Paragraph::new(document.ids().node());
    source
        .push_text_marked("src", None, Arc::default(), mark(RevisionKind::MoveFrom, 2))
        .unwrap();
    destination
        .push_text_marked("dst", None, Arc::default(), mark(RevisionKind::MoveTo, 5))
        .unwrap();
    let source_id = source.id();
    let destination_id = destination.id();
    document.story_mut(story).unwrap().blocks =
        BlockSeq::from_vec(vec![source.into(), destination.into()]);
    document.move_markers_mut().extend([
        marker(story, source_id, 0, false, MoveSide::From, 1),
        marker(story, source_id, 3, true, MoveSide::From, 3),
        marker(story, destination_id, 0, false, MoveSide::To, 4),
    ]);
    let mut session = Session::new(document);
    let before = texts(&session);
    let error = exec(&mut session, vec![Command::AcceptAllRevisions]).unwrap_err();
    assert!(matches!(
        command_error(error),
        CommandError::IncompleteMove(_)
    ));
    assert_eq!(texts(&session), before);
    assert_eq!(marker_count(&session), 3);

    let mut document = Document::new_blank();
    let story = document.body_id();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text_marked("src", None, Arc::default(), mark(RevisionKind::MoveFrom, 2))
        .unwrap();
    paragraph
        .push_text_marked("dst", None, Arc::default(), mark(RevisionKind::MoveTo, 5))
        .unwrap();
    let id = paragraph.id();
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    document.move_markers_mut().extend([
        marker(story, id, 0, false, MoveSide::From, 1),
        marker(story, id, 3, true, MoveSide::From, 3),
        marker(story, id, 1, false, MoveSide::To, 4),
        marker(story, id, 4, true, MoveSide::To, 6),
    ]);
    let mut session = Session::new(document);
    let before = texts(&session);
    let error = exec(&mut session, vec![Command::RejectRevision { id: 2 }]).unwrap_err();
    assert!(matches!(command_error(error), CommandError::CrossedMove(_)));
    assert_eq!(texts(&session), before);
    assert_eq!(marker_count(&session), 4);
}

#[test]
fn switching_tracking_seals_a_typing_group_beside_a_move() {
    let mut session = paired_move();
    session.set_revision_date(Some(Arc::from("2026-01-01T00:00:00Z")));
    exec(
        &mut session,
        vec![Command::SetTrackRevisions { enabled: true }],
    )
    .unwrap();
    let id = session.document().body().blocks.get(0).unwrap().id();
    let mut typing = Transaction::new(
        session.version(),
        vec![Command::InsertText {
            at: pos(&session, id, 0),
            text: "c".into(),
        }],
    );
    typing.undo_group = UndoGroup::Typing;
    typing.selection = Some(Selection::caret(pos(&session, id, 0)));
    session.apply(typing).unwrap();
    let after_c = Selection::caret(pos(&session, id, 1));
    let mut more = Transaction::new(
        session.version(),
        vec![Command::InsertText {
            at: pos(&session, id, 1),
            text: "d".into(),
        }],
    );
    more.undo_group = UndoGroup::Typing;
    more.selection = Some(after_c);
    session.apply(more).unwrap();
    assert_eq!(texts(&session)[0], "cdPsrc");
    assert_eq!(marker_count(&session), 4);

    exec(
        &mut session,
        vec![Command::SetTrackRevisions { enabled: false }],
    )
    .unwrap();
    session.undo().unwrap();
    assert_eq!(session.document().settings().track_revisions, Some(true));
    assert_eq!(texts(&session)[0], "cdPsrc");
    session.undo().unwrap();
    assert_eq!(texts(&session), vec!["Psrc", "dstIS", "X"]);
    assert_eq!(marker_count(&session), 4);
    let final_text = projected(&session, RevisionView::Final);
    exec(&mut session, vec![Command::AcceptAllRevisions]).unwrap();
    assert_eq!(texts(&session), final_text);
    assert_eq!(marker_count(&session), 0);
}

// --- T402/T301: tracked cross-paragraph deletion with deleted marks ---

fn tracked_session(lines: &[&str]) -> Session {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in lines {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(line, None, Arc::default()).unwrap();
        blocks.push(Block::from(paragraph));
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    let mut session = Session::new(document);
    session.set_revision_author("用户");
    exec(
        &mut session,
        vec![Command::SetTrackRevisions { enabled: true }],
    )
    .unwrap();
    session
}

fn paragraph_ids(session: &Session) -> Vec<NodeId> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(Paragraph::id)
        .collect()
}

fn mark_ids(session: &Session) -> Vec<(NodeId, u32)> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .filter_map(|paragraph| {
            paragraph
                .mark_revision
                .as_ref()
                .map(|revision| (paragraph.id(), revision.id))
        })
        .collect()
}

fn delete_between(session: &mut Session, first: NodeId, at: usize, last: NodeId, to: usize) {
    let start = pos(session, first, at);
    let end = pos(session, last, to);
    exec(session, vec![Command::DeleteRange { start, end }]).unwrap();
}

#[test]
fn a_user_tracked_delete_across_paragraphs_marks_the_marks() {
    let mut session = tracked_session(&["甲乙", "丙丁", "戊己"]);
    let ids = paragraph_ids(&session);
    delete_between(&mut session, ids[0], 1, ids[2], 1);
    // Original text is untouched; the final view joins the leftovers.
    assert_eq!(texts(&session), vec!["甲乙", "丙丁", "戊己"]);
    assert_eq!(
        body_final_paragraphs(session.document()).unwrap(),
        vec!["甲己"]
    );
    assert_eq!(
        mark_ids(&session).len(),
        2,
        "both covered marks are deleted"
    );
    let kinds: Vec<_> = session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| run.revision.as_ref().map(|r| r.kind))
        .collect();
    assert!(kinds.iter().all(|kind| *kind == RevisionKind::Delete));
    // One undo returns the whole tracked delete.
    session.undo().unwrap();
    assert_eq!(texts(&session), vec!["甲乙", "丙丁", "戊己"]);
    assert!(mark_ids(&session).is_empty());
}

#[test]
fn accepting_one_mark_revision_merges_the_two_paragraphs() {
    let mut session = tracked_session(&["一", "二", "三"]);
    let ids = paragraph_ids(&session);
    delete_between(&mut session, ids[0], 1, ids[1], 0);
    let [(owner, id)] = mark_ids(&session).try_into().unwrap();
    exec(&mut session, vec![Command::AcceptRevision { id }]).unwrap();
    assert_eq!(texts(&session), vec!["一二", "三"]);
    assert!(mark_ids(&session).is_empty());
    let after = paragraph_ids(&session);
    assert_eq!(after[0], owner, "the merged paragraph keeps the first id");
    assert_eq!(after.len(), 2);
}

#[test]
fn accepting_all_merges_a_chain_of_marks_and_clears_the_text() {
    let mut session = tracked_session(&["a", "b", "c", "d"]);
    let ids = paragraph_ids(&session);
    delete_between(&mut session, ids[0], 1, ids[3], 0);
    assert_eq!(
        body_final_paragraphs(session.document()).unwrap(),
        vec!["ad"]
    );
    assert_eq!(mark_ids(&session).len(), 3);
    exec(&mut session, vec![Command::AcceptAllRevisions]).unwrap();
    assert_eq!(texts(&session), vec!["ad"]);
    assert_eq!(paragraph_ids(&session).len(), 1);
    assert!(mark_ids(&session).is_empty());
}

#[test]
fn rejecting_all_restores_the_breaks_and_the_text() {
    let mut session = tracked_session(&["a", "b"]);
    let ids = paragraph_ids(&session);
    delete_between(&mut session, ids[0], 1, ids[1], 1);
    assert_eq!(
        body_final_paragraphs(session.document()).unwrap(),
        vec!["a"]
    );
    exec(&mut session, vec![Command::RejectAllRevisions]).unwrap();
    assert_eq!(texts(&session), vec!["a", "b"]);
    assert!(mark_ids(&session).is_empty());
    assert_eq!(paragraph_ids(&session).len(), 2);
}

#[test]
fn a_tracked_delete_refuses_to_cross_a_table_or_a_cell() {
    use joyword_model::{Cell, CellBorders, Measure, Row, Table, VerticalMerge};
    let mut document = Document::new_blank();
    let story = document.body_id();
    let mut first = Paragraph::new(document.ids().node());
    first.push_text("前", None, Arc::default()).unwrap();
    let mut cell_text = Paragraph::new(document.ids().node());
    cell_text.push_text("格", None, Arc::default()).unwrap();
    let cell_id = cell_text.id();
    let cell = Cell {
        id: document.ids().node(),
        grid_span: 1,
        vertical_merge: VerticalMerge::Restart,
        width: Some(Measure::Twips(2000)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(vec![cell_text.into()]),
        preserved: None,
        props_revision: None,
    };
    let table = Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(2000)],
        rows: vec![Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![cell],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
            props_revision: None,
        }],
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
    let mut last = Paragraph::new(document.ids().node());
    last.push_text("后", None, Arc::default()).unwrap();
    let first_id = first.id();
    let last_id = last.id();
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![
        first.into(),
        Block::Table(Arc::new(table)),
        last.into(),
    ]);
    let mut session = Session::new(document);
    session.set_revision_author("用户");
    exec(
        &mut session,
        vec![Command::SetTrackRevisions { enabled: true }],
    )
    .unwrap();
    let start = pos(&session, first_id, 0);
    let end = pos(&session, last_id, 1);
    assert!(matches!(
        exec(&mut session, vec![Command::DeleteRange { start, end }]),
        Err(TxError::Command {
            error: CommandError::RevisionBoundary,
            ..
        })
    ));
    let start = pos(&session, cell_id, 0);
    let end = pos(&session, last_id, 1);
    assert!(matches!(
        exec(&mut session, vec![Command::DeleteRange { start, end }]),
        Err(TxError::Command {
            error: CommandError::CrossContainer,
            ..
        })
    ));
}

#[test]
fn another_authors_pending_mark_refuses_a_second_tracked_delete() {
    let mut session = tracked_session(&["a", "b", "c"]);
    let ids = paragraph_ids(&session);
    delete_between(&mut session, ids[0], 1, ids[1], 0);
    session.set_revision_author("别人");
    // The range covers the pending mark of the first paragraph, which the
    // other author already deleted: refuse instead of taking it over.
    let start = pos(&session, ids[0], 1);
    let end = pos(&session, ids[1], 0);
    assert!(matches!(
        exec(&mut session, vec![Command::DeleteRange { start, end }]),
        Err(TxError::Command {
            error: CommandError::RevisionConflict,
            ..
        })
    ));
}
