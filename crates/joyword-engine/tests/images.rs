//! Inline image insertion (T203 first slice).

use std::sync::Arc;

use joyword_engine::{Command, Outcome, Selection, Session, Transaction, TxError};
use joyword_model::{
    Bias, BlockSeq, Document, InlineKind, Paragraph, Position, ScalarOffset, sample_png_1x1,
};

fn session() -> Session {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("ab", None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    Session::new(document)
}

fn at(session: &Session, offset: usize) -> Position {
    let paragraph = session.document().body().blocks.get(0).unwrap().id();
    Position {
        story: session.document().body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::After,
    }
}

fn apply(
    session: &mut Session,
    caret: Position,
    commands: Vec<Command>,
) -> Result<Outcome, TxError> {
    session.apply_with_mapped_selection(
        Transaction::new(session.version(), commands),
        Selection::caret(caret),
    )
}

#[test]
fn inserting_an_image_is_one_undo() {
    let mut session = session();
    let before = session.document().clone();
    let png = Arc::<[u8]>::from(sample_png_1x1());
    let caret = at(&session, 1);
    apply(
        &mut session,
        caret,
        vec![Command::InsertImage {
            at: caret,
            data: png,
        }],
    )
    .unwrap();
    assert_eq!(session.document().resources().len(), 1);
    let paragraph = session
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(
        paragraph
            .inlines()
            .iter()
            .any(|inline| matches!(inline.kind, InlineKind::Image { .. }))
    );
    session.undo().unwrap();
    assert_eq!(
        session
            .document()
            .canonical_dump(joyword_model::DumpOptions { node_ids: false }),
        before.canonical_dump(joyword_model::DumpOptions { node_ids: false })
    );
    session.redo().unwrap();
    assert_eq!(session.document().resources().len(), 1);
}

#[test]
fn two_equal_inserts_share_one_resource() {
    let mut session = session();
    let png = Arc::<[u8]>::from(sample_png_1x1());
    let first = at(&session, 0);
    apply(
        &mut session,
        first,
        vec![Command::InsertImage {
            at: first,
            data: Arc::clone(&png),
        }],
    )
    .unwrap();
    let second = at(&session, 2);
    apply(
        &mut session,
        second,
        vec![Command::InsertImage {
            at: second,
            data: png,
        }],
    )
    .unwrap();
    assert_eq!(session.document().resources().len(), 1);
    let paragraph = session
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    let ids: Vec<_> = paragraph
        .inlines()
        .iter()
        .filter_map(|inline| match inline.kind {
            InlineKind::Image { resource, .. } => Some(resource),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], ids[1]);
}

#[test]
fn a_refused_image_inside_a_transaction_discards_the_delete() {
    let mut session = session();
    let before = session.document().clone();
    let version = session.version();
    let start = at(&session, 0);
    let end = at(&session, 1);
    let error = apply(
        &mut session,
        start,
        vec![
            Command::DeleteRange { start, end },
            Command::InsertImage {
                at: start,
                data: Arc::from(b"not an image".as_slice()),
            },
        ],
    )
    .unwrap_err();
    assert!(matches!(error, TxError::Command { index: 1, .. }));
    assert_eq!(session.version(), version);
    let text = |document: &joyword_model::Document| {
        document
            .body()
            .blocks
            .get(0)
            .unwrap()
            .as_paragraph()
            .unwrap()
            .plain_text(|_| "□".into())
    };
    assert_eq!(text(session.document()), text(&before));
}
