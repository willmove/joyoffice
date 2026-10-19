//! T300: paired moveFrom/moveTo accept, reject, undo, and the typing seal.

use std::sync::Arc;

use joyword_engine::{Command, CommandError, Selection, Session, Transaction, TxError, UndoGroup};
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
