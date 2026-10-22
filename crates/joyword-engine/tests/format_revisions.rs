//! T301: format revision (`w:rPrChange` / `w:pPrChange`) accept, reject,
//! batch resolution, the shared id case, and undo. Each command is one
//! transaction, so one undo restores the whole prior state.

use std::sync::Arc;

use joyword_engine::{
    Command, CommandError, Selection, Session, Transaction, TxError, current_revision,
};
use joyword_model::{
    Bias, Block, BlockSeq, Document, FormatRevision, Justification, NodeId, ParaProps, Paragraph,
    Position, RevisionKind, RunProps, ScalarOffset, TextRevision,
};

const DATE: &str = "2020-01-02T03:04:05Z";

fn run_change(id: u32, author: &str, previous: RunProps) -> FormatRevision {
    FormatRevision {
        id,
        author: Arc::from(author),
        date: Some(Arc::from(DATE)),
        previous_run: Some(Arc::new(previous)),
        previous_para: None,
    }
}

fn para_change(id: u32, author: &str, previous: ParaProps) -> FormatRevision {
    FormatRevision {
        id,
        author: Arc::from(author),
        date: Some(Arc::from(DATE)),
        previous_run: None,
        previous_para: Some(Arc::new(previous)),
    }
}

fn bold() -> Arc<RunProps> {
    Arc::new(RunProps {
        bold: Some(true),
        ..RunProps::default()
    })
}

fn italic() -> RunProps {
    RunProps {
        italic: Some(true),
        ..RunProps::default()
    }
}

fn center() -> ParaProps {
    ParaProps {
        justification: Some(Justification::Center),
        ..ParaProps::default()
    }
}

fn strike() -> RunProps {
    RunProps {
        strike: Some(true),
        ..RunProps::default()
    }
}

/// Paragraph 0: justification Both (current) with `w:pPrChange` id 11
/// (previous center) and a bold run with `w:rPrChange` id 12 (previous
/// italic). Paragraph 1: a plain run with `w:rPrChange` id 13 (previous
/// strike).
fn format_session() -> (Session, NodeId, NodeId) {
    let mut document = Document::new_blank();
    let story = document.body_id();
    let mut first = Paragraph::new(document.ids().node());
    first.push_text("bold", None, bold()).unwrap();
    first.stamp_format_from(0, Arc::new(run_change(12, "B", italic())));
    first.props = Arc::new(ParaProps {
        justification: Some(Justification::Both),
        ..ParaProps::default()
    });
    first.props_revision = Some(Arc::new(para_change(11, "A", center())));
    let mut second = Paragraph::new(document.ids().node());
    second.push_text("plain", None, Arc::default()).unwrap();
    second.stamp_format_from(0, Arc::new(run_change(13, "C", strike())));
    let first_id = first.id();
    let second_id = second.id();
    document.note_revision_id(11);
    document.note_revision_id(12);
    document.note_revision_id(13);
    document.story_mut(story).unwrap().blocks =
        BlockSeq::from_vec(vec![first.into(), second.into()]);
    (Session::new(document), first_id, second_id)
}

fn paragraph_of(session: &Session, index: usize) -> &Paragraph {
    session
        .document()
        .body()
        .blocks
        .get(index)
        .unwrap()
        .as_paragraph()
        .unwrap()
}

fn run_ids(session: &Session) -> Vec<u32> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| run.format_revision.as_ref().map(|change| change.id))
        .collect()
}

fn para_ids(session: &Session) -> Vec<u32> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .filter_map(|paragraph| paragraph.props_revision.as_ref().map(|change| change.id))
        .collect()
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
fn accepting_one_format_revision_is_one_undo_step() {
    let (mut session, first_id, _) = format_session();
    exec(&mut session, vec![Command::AcceptRevision { id: 12 }]).unwrap();
    let first = paragraph_of(&session, 0);
    // Accept keeps the current (post-change) formatting and drops the record.
    assert_eq!(first.runs()[0].props.bold, Some(true));
    assert_eq!(first.runs()[0].props.italic, None);
    assert_eq!(run_ids(&session), vec![13]);
    // The paragraph-level change and the other paragraph are untouched.
    assert_eq!(para_ids(&session), vec![11]);
    assert_eq!(first.props.justification, Some(Justification::Both));

    // One transaction, so one undo restores everything.
    session.undo().unwrap();
    let first = paragraph_of(&session, 0);
    assert_eq!(run_ids(&session), vec![12, 13]);
    let change = first.runs()[0].format_revision.as_ref().unwrap();
    assert_eq!(change.id, 12);
    assert_eq!(&*change.author, "B");
    assert_eq!(change.date.as_deref(), Some(DATE));
    assert_eq!(
        change.previous_run.as_ref().map(|p| p.italic),
        Some(Some(true))
    );

    // An unknown id is refused and changes nothing.
    let error = exec(&mut session, vec![Command::AcceptRevision { id: 999 }]).unwrap_err();
    assert!(matches!(
        command_error(error),
        CommandError::UnknownRevision(999)
    ));
    assert_eq!(run_ids(&session), vec![12, 13]);
    assert_eq!(para_ids(&session), vec![11]);
    assert!(
        current_revision(
            session.document(),
            &Selection::caret(pos(&session, first_id, 0))
        ) == Some(11)
    );
}

#[test]
fn rejecting_one_paragraph_format_revision_restores_the_previous_props() {
    let (mut session, _, _) = format_session();
    exec(&mut session, vec![Command::RejectRevision { id: 11 }]).unwrap();
    let first = paragraph_of(&session, 0);
    // Reject restores the pre-change formatting recorded in the change.
    assert_eq!(first.props.justification, Some(Justification::Center));
    assert!(first.props_revision.is_none());
    // The run-level change of the same paragraph is untouched.
    assert_eq!(run_ids(&session), vec![12, 13]);

    session.undo().unwrap();
    let first = paragraph_of(&session, 0);
    assert_eq!(first.props.justification, Some(Justification::Both));
    assert_eq!(para_ids(&session), vec![11]);
    assert_eq!(run_ids(&session), vec![12, 13]);
}

#[test]
fn batch_accept_and_reject_resolve_every_format_revision_in_one_undo_step() {
    let (mut session, _, _) = format_session();
    exec(&mut session, vec![Command::AcceptAllRevisions]).unwrap();
    assert!(run_ids(&session).is_empty());
    assert!(para_ids(&session).is_empty());
    let first = paragraph_of(&session, 0);
    assert_eq!(first.props.justification, Some(Justification::Both));
    assert_eq!(first.runs()[0].props.bold, Some(true));
    session.undo().unwrap();
    assert_eq!(run_ids(&session), vec![12, 13]);
    assert_eq!(para_ids(&session), vec![11]);
    assert_eq!(
        paragraph_of(&session, 0).props.justification,
        Some(Justification::Both)
    );

    let (mut session, _, _) = format_session();
    exec(&mut session, vec![Command::RejectAllRevisions]).unwrap();
    assert!(run_ids(&session).is_empty());
    assert!(para_ids(&session).is_empty());
    let first = paragraph_of(&session, 0);
    // Every recorded previous value is restored.
    assert_eq!(first.props.justification, Some(Justification::Center));
    assert_eq!(first.runs()[0].props.italic, Some(true));
    assert_eq!(first.runs()[0].props.bold, None);
    assert_eq!(paragraph_of(&session, 1).runs()[0].props.strike, Some(true));
    session.undo().unwrap();
    assert_eq!(run_ids(&session), vec![12, 13]);
    assert_eq!(para_ids(&session), vec![11]);
    let first = paragraph_of(&session, 0);
    assert_eq!(first.props.justification, Some(Justification::Both));
    assert_eq!(first.runs()[0].props.bold, Some(true));
    assert_eq!(paragraph_of(&session, 1).runs()[0].props.strike, None);
}

#[test]
fn one_id_carrying_a_text_and_a_format_revision_resolves_both_in_one_command() {
    // A run inserted under id 7 whose run formatting is also revised under
    // id 7, plus a paragraph-level change sharing the same id: one accept
    // settles the whole change, and one undo restores all of it.
    let mut document = Document::new_blank();
    let story = document.body_id();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text_marked(
            "new",
            None,
            bold(),
            TextRevision {
                id: 7,
                kind: RevisionKind::Insert,
                author: Arc::from("A"),
                date: Some(Arc::from(DATE)),
            },
        )
        .unwrap();
    paragraph.stamp_format_from(0, Arc::new(run_change(7, "A", italic())));
    paragraph.props_revision = Some(Arc::new(para_change(7, "A", center())));
    document.note_revision_id(7);
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let mut session = Session::new(document);

    exec(&mut session, vec![Command::AcceptRevision { id: 7 }]).unwrap();
    let first = paragraph_of(&session, 0);
    assert_eq!(first.plain_text(|_| String::new()), "new");
    assert!(first.runs()[0].revision.is_none());
    assert!(first.runs()[0].format_revision.is_none());
    assert!(first.props_revision.is_none());
    assert_eq!(first.runs()[0].props.bold, Some(true));

    session.undo().unwrap();
    let first = paragraph_of(&session, 0);
    assert_eq!(
        first.runs()[0].revision.as_ref().map(|mark| mark.id),
        Some(7)
    );
    assert_eq!(
        first.runs()[0].format_revision.as_ref().map(|c| c.id),
        Some(7)
    );
    assert_eq!(para_ids(&session), vec![7]);
}

#[test]
fn current_revision_finds_paragraph_and_run_format_revisions() {
    let (mut session, first_id, second_id) = format_session();
    // A paragraph-level change applies wherever the caret sits.
    for offset in [0usize, 2, 4] {
        assert_eq!(
            current_revision(
                session.document(),
                &Selection::caret(pos(&session, first_id, offset))
            ),
            Some(11)
        );
    }
    // A run-level change applies at its run when the paragraph has none.
    assert_eq!(
        current_revision(
            session.document(),
            &Selection::caret(pos(&session, second_id, 1))
        ),
        Some(13)
    );
    // Resolving the paragraph change uncovers the run change.
    exec(&mut session, vec![Command::AcceptRevision { id: 11 }]).unwrap();
    assert_eq!(
        current_revision(
            session.document(),
            &Selection::caret(pos(&session, first_id, 1))
        ),
        Some(12)
    );
}
