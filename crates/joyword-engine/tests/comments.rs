//! T302: comment commands. Resolve is not deletion (REVIEW-006).

use std::sync::Arc;

use joyword_engine::{Command, CommandError, Session, Transaction, TxError};
use joyword_model::{
    Anchor, AnnotationKind, Bias, Block, Comment, Document, DumpOptions, NodeId, Paragraph,
    Position, ScalarOffset, StoryId, StoryKind,
};

fn session(text: &str) -> (Session, NodeId) {
    let mut session = Session::new(Document::new_blank());
    let paragraph = session
        .document()
        .body()
        .blocks
        .iter()
        .next()
        .expect("paragraph")
        .id();
    let at = Position {
        story: session.document().body_id(),
        paragraph,
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::InsertText {
                at,
                text: text.to_owned(),
            }],
        ))
        .expect("insert");
    (session, paragraph)
}

fn at(story: StoryId, paragraph: NodeId, offset: usize) -> Position {
    Position {
        story,
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::After,
    }
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

fn dump(session: &Session) -> String {
    session
        .document()
        .canonical_dump(DumpOptions { node_ids: false })
}

fn comments(session: &Session) -> Vec<(u32, bool, Anchor, Option<String>)> {
    session
        .document()
        .annotations()
        .filter_map(|annotation| {
            let AnnotationKind::Comment(comment) = &annotation.kind else {
                return None;
            };
            Some((
                comment.source_id,
                comment.resolved,
                annotation.anchor.clone(),
                comment.parent_para_id.as_deref().map(str::to_owned),
            ))
        })
        .collect()
}

#[test]
fn add_reply_resolve_and_reopen_are_undoable() {
    let (mut session, paragraph) = session("支付期限");
    let story = session.document().body_id();
    exec(
        &mut session,
        vec![Command::AddComment {
            start: at(story, paragraph, 0),
            end: at(story, paragraph, 2),
            author: "Ada".into(),
            date: Some("2020-01-02T03:04:05Z".into()),
            initials: Some("AL".into()),
            text: "请确认".into(),
        }],
    )
    .unwrap();
    let parent = session.document().annotations().next().expect("comment").id;
    let anchor = session
        .document()
        .annotation(parent)
        .unwrap()
        .anchor
        .clone();
    exec(
        &mut session,
        vec![Command::ReplyComment {
            parent,
            author: "Bea".into(),
            date: None,
            initials: None,
            text: "已改".into(),
        }],
    )
    .unwrap();
    let reply = session.document().annotations().nth(1).expect("reply").id;
    assert_eq!(session.document().annotation(reply).unwrap().anchor, anchor);
    let AnnotationKind::Comment(comment) = &session.document().annotation(reply).unwrap().kind
    else {
        panic!("reply");
    };
    assert!(comment.parent_para_id.is_some());
    assert!(!comment.resolved);
    exec(&mut session, vec![Command::ResolveComment { id: parent }]).unwrap();
    let AnnotationKind::Comment(comment) = &session.document().annotation(parent).unwrap().kind
    else {
        panic!("root");
    };
    assert!(comment.resolved);
    assert_eq!(
        session.document().annotation(parent).unwrap().anchor,
        anchor
    );
    exec(&mut session, vec![Command::ReopenComment { id: parent }]).unwrap();
    let AnnotationKind::Comment(comment) = &session.document().annotation(parent).unwrap().kind
    else {
        panic!("root");
    };
    assert!(!comment.resolved);
    session.undo().unwrap();
    session.undo().unwrap();
    session.undo().unwrap();
    session.undo().unwrap();
    assert!(comments(&session).is_empty());
}

#[test]
fn deleting_the_range_detaches_and_does_not_resolve() {
    let (mut session, paragraph) = session("支付期限");
    let story = session.document().body_id();
    exec(
        &mut session,
        vec![Command::AddComment {
            start: at(story, paragraph, 0),
            end: at(story, paragraph, 2),
            author: "Ada".into(),
            date: None,
            initials: None,
            text: "注".into(),
        }],
    )
    .unwrap();
    exec(
        &mut session,
        vec![Command::DeleteRange {
            start: at(story, paragraph, 0),
            end: at(story, paragraph, 2),
        }],
    )
    .unwrap();
    let found = comments(&session);
    let (id, resolved, anchor, _) = &found[0];
    assert_eq!(*id, 0);
    assert!(!*resolved);
    assert_eq!(*anchor, Anchor::Detached);
    let kept = session.document().annotations().next().unwrap().id;
    assert!(session.document().annotation(kept).is_some());
}

#[test]
fn resolving_does_not_detach() {
    let (mut session, paragraph) = session("支付期限");
    let story = session.document().body_id();
    exec(
        &mut session,
        vec![Command::AddComment {
            start: at(story, paragraph, 1),
            end: at(story, paragraph, 3),
            author: "Ada".into(),
            date: Some("2024-05-06T07:08:09Z".into()),
            initials: None,
            text: String::new(),
        }],
    )
    .unwrap();
    let id = session.document().annotations().next().unwrap().id;
    let before = session.document().annotation(id).unwrap().anchor.clone();
    exec(&mut session, vec![Command::ResolveComment { id }]).unwrap();
    assert_eq!(session.document().annotation(id).unwrap().anchor, before);
    assert!(matches!(before, Anchor::Range { .. }));
}

#[test]
fn orphan_reply_missing_author_and_bad_reopen_leave_the_document_unchanged() {
    let (mut session, paragraph) = session("正文");
    let before = dump(&session);
    let orphan = {
        let mut document = session.document().clone();
        document.ids().annotation()
    };
    let error = exec(
        &mut session,
        vec![Command::ReplyComment {
            parent: orphan,
            author: "Ada".into(),
            date: None,
            initials: None,
            text: "no".into(),
        }],
    )
    .unwrap_err();
    assert_eq!(command_error(error), CommandError::OrphanReply(orphan));
    assert_eq!(dump(&session), before);

    let story = session.document().body_id();
    let error = exec(
        &mut session,
        vec![Command::AddComment {
            start: at(story, paragraph, 0),
            end: at(story, paragraph, 1),
            author: String::new(),
            date: None,
            initials: None,
            text: "x".into(),
        }],
    )
    .unwrap_err();
    assert_eq!(command_error(error), CommandError::CommentAuthorMissing);
    assert_eq!(dump(&session), before);

    exec(
        &mut session,
        vec![Command::AddComment {
            start: at(story, paragraph, 0),
            end: at(story, paragraph, 1),
            author: "Ada".into(),
            date: None,
            initials: None,
            text: "x".into(),
        }],
    )
    .unwrap();
    let id = session.document().annotations().next().expect("comment").id;
    let error = exec(&mut session, vec![Command::ReopenComment { id }]).unwrap_err();
    assert_eq!(command_error(error), CommandError::CommentNotResolved(id));
    exec(&mut session, vec![Command::ResolveComment { id }]).unwrap();
    let error = exec(&mut session, vec![Command::ResolveComment { id }]).unwrap_err();
    assert_eq!(
        command_error(error),
        CommandError::CommentAlreadyResolved(id)
    );

    let mut document = session.document().clone();
    let body = {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text("无编号", None, Arc::default()).unwrap();
        document.insert_story(StoryKind::Comment, vec![Block::from(paragraph)])
    };
    let bare = document.insert_annotation(
        AnnotationKind::Comment(Comment::new(body)),
        Anchor::Detached,
    );
    let mut bare_session = Session::new(document);
    let before_bare = bare_session
        .document()
        .canonical_dump(DumpOptions { node_ids: true });
    let error = exec(
        &mut bare_session,
        vec![Command::ReplyComment {
            parent: bare,
            author: "Ada".into(),
            date: None,
            initials: None,
            text: "no".into(),
        }],
    )
    .unwrap_err();
    assert_eq!(command_error(error), CommandError::CommentThreadUnavailable);
    assert_eq!(
        bare_session
            .document()
            .canonical_dump(DumpOptions { node_ids: true }),
        before_bare
    );
}
