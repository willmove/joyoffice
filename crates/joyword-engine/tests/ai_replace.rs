//! T402 engine slice: one Agent transaction, AI revision author, one undo step.

use std::sync::Arc;

use joyword_engine::{
    AI_REVISION_AUTHOR, Command, EditPolicy, Origin, Session, Transaction, TxError, UndoGroup,
    apply_ai_text_replace, body_final_paragraphs,
};
use joyword_model::{
    Bias, Block, BlockSeq, Document, Paragraph, Position, RevisionKind, ScalarOffset,
};

fn document(lines: &[&str]) -> Document {
    let mut document = Document::new_blank();
    let mut blocks = Vec::new();
    for line in lines {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(line, None, Arc::default()).unwrap();
        blocks.push(Block::from(paragraph));
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    document
}

fn session(lines: &[&str]) -> Session {
    let mut session = Session::new(document(lines));
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: true,
    });
    session.set_revision_author("用户");
    session
}

fn final_text(session: &Session) -> Vec<String> {
    body_final_paragraphs(session.document()).unwrap()
}

fn authors(session: &Session) -> Vec<(RevisionKind, String)> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .flat_map(|paragraph| paragraph.runs().iter())
        .filter_map(|run| {
            run.revision
                .as_ref()
                .map(|revision| (revision.kind, revision.author.to_string()))
        })
        .collect()
}

fn assert_unchanged(session: &Session, text: &[String], version: u64, depth: usize) {
    assert_eq!(final_text(session), text);
    assert_eq!(session.version().get(), version);
    assert_eq!(session.undo_depth(), depth);
    assert_eq!(session.redo_depth(), 0);
}

#[test]
fn one_transaction_stamps_the_ai_author_and_undoes_in_one_step() {
    let mut session = session(&["付款期限为十个工作日"]);
    assert_eq!(session.document().settings().track_revisions, None);
    let base = session.version();
    let depth = session.undo_depth();
    apply_ai_text_replace(&mut session, base, 5, 10, "五个工作日").unwrap();
    assert_eq!(final_text(&session), vec!["付款期限为五个工作日"]);
    assert_eq!(session.undo_depth(), depth + 1);
    assert_eq!(session.version().get(), base.get() + 1);
    assert_eq!(session.revision_author(), "用户");
    assert!(
        authors(&session)
            .iter()
            .all(|(_, author)| author == AI_REVISION_AUTHOR)
    );
    assert!(
        authors(&session)
            .iter()
            .any(|(kind, _)| *kind == RevisionKind::Delete)
    );
    assert!(
        authors(&session)
            .iter()
            .any(|(kind, _)| *kind == RevisionKind::Insert)
    );
    assert_eq!(session.document().settings().track_revisions, None);
    let marked = session.version();
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec!["付款期限为十个工作日"]);
    assert!(authors(&session).is_empty());
    assert_eq!(session.undo_depth(), depth);
    assert!(session.version().get() > marked.get());
    session.redo().unwrap();
    assert_eq!(final_text(&session), vec!["付款期限为五个工作日"]);
    assert_eq!(session.undo_depth(), depth + 1);
}

#[test]
fn deletion_is_one_tracked_undo_step() {
    let mut session = session(&["保留", "删掉这段"]);
    let base = session.version();
    apply_ai_text_replace(&mut session, base, 3, 7, "").unwrap();
    assert_eq!(final_text(&session), vec!["保留".to_owned(), String::new()]);
    assert_eq!(session.undo_depth(), 1);
    assert!(
        authors(&session)
            .iter()
            .all(|(_, author)| author == AI_REVISION_AUTHOR)
    );
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec!["保留", "删掉这段"]);
    assert_eq!(session.undo_depth(), 0);
}

#[test]
fn an_ai_replace_does_not_coalesce_with_typing() {
    let mut session = session(&[""]);
    let id = session.document().body().blocks.iter().next().unwrap().id();
    let at = Position {
        story: session.document().body_id(),
        paragraph: id,
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    let mut typing = Transaction::new(
        session.version(),
        vec![Command::InsertText {
            at,
            text: "ab".into(),
        }],
    );
    typing.undo_group = UndoGroup::Typing;
    typing.selection = Some(joyword_engine::Selection::caret(at));
    session.apply(typing).unwrap();
    assert_eq!(session.undo_depth(), 1);
    let base = session.version();
    apply_ai_text_replace(&mut session, base, 0, 2, "AB").unwrap();
    assert_eq!(final_text(&session), vec!["AB"]);
    assert_eq!(session.undo_depth(), 2);
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec!["ab"]);
    assert_eq!(session.undo_depth(), 1);
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec![""]);
}

#[test]
fn stale_disabled_cross_paragraph_and_bad_text_write_nothing() {
    let mut session = session(&["甲乙", "丙丁"]);
    let stale = session.version();
    let mut user = Transaction::new(
        session.version(),
        vec![Command::InsertText {
            at: Position {
                story: session.document().body_id(),
                paragraph: session.document().body().blocks.iter().next().unwrap().id(),
                offset: ScalarOffset(2),
                bias: Bias::After,
            },
            text: "戊".into(),
        }],
    );
    user.origin = Origin::User;
    session.apply(user).unwrap();
    let edited = final_text(&session);
    let version = session.version().get();
    let depth = session.undo_depth();
    assert!(matches!(
        apply_ai_text_replace(&mut session, stale, 0, 2, "己"),
        Err(TxError::StaleVersion { .. })
    ));
    assert_unchanged(&session, &edited, version, depth);

    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: false,
    });
    let current = session.version();
    assert_eq!(
        apply_ai_text_replace(&mut session, current, 0, 1, "庚"),
        Err(TxError::AgentWritesDisabled)
    );
    session.set_policy(EditPolicy {
        read_only: true,
        agent_writes: true,
    });
    assert_eq!(
        apply_ai_text_replace(&mut session, current, 0, 1, "庚"),
        Err(TxError::ReadOnly)
    );
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: true,
    });
    assert_eq!(
        apply_ai_text_replace(&mut session, current, 0, 99, "跨段"),
        Err(TxError::AiRangeRefused)
    );
    assert_eq!(
        apply_ai_text_replace(&mut session, current, 0, 1, "坏\n了"),
        Err(TxError::AiReplacementRefused)
    );
    assert_eq!(
        apply_ai_text_replace(&mut session, current, 0, 1, "坏\u{FDD0}"),
        Err(TxError::AiReplacementRefused)
    );
    assert_eq!(final_text(&session), edited);
    assert_eq!(session.version().get(), version);
    assert_eq!(session.undo_depth(), depth);
    let display = format!(
        "{}",
        apply_ai_text_replace(&mut session, current, 0, 99, "跨段").unwrap_err()
    );
    assert!(!display.contains("甲乙"));
    assert!(!display.contains("跨段"));
}

#[test]
fn a_gap_in_final_text_is_refused_and_a_contiguous_span_is_not() {
    let mut session = session(&["abcd"]);
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::SetTrackRevisions { enabled: true }],
        ))
        .unwrap();
    let id = session.document().body().blocks.iter().next().unwrap().id();
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::DeleteRange {
                start: Position {
                    story: session.document().body_id(),
                    paragraph: id,
                    offset: ScalarOffset(1),
                    bias: Bias::After,
                },
                end: Position {
                    story: session.document().body_id(),
                    paragraph: id,
                    offset: ScalarOffset(2),
                    bias: Bias::Before,
                },
            }],
        ))
        .unwrap();
    assert_eq!(final_text(&session), vec!["acd"]);
    let version = session.version();
    let depth = session.undo_depth();
    assert_eq!(
        apply_ai_text_replace(&mut session, version, 0, 3, "xyz"),
        Err(TxError::AiRangeRefused)
    );
    assert_eq!(session.version(), version);
    assert_eq!(session.undo_depth(), depth);
    apply_ai_text_replace(&mut session, version, 1, 3, "CD").unwrap();
    assert_eq!(final_text(&session), vec!["aCD"]);
    assert_eq!(session.undo_depth(), depth + 1);
    let ai: Vec<_> = authors(&session)
        .into_iter()
        .filter(|(_, author)| author == AI_REVISION_AUTHOR)
        .collect();
    assert!(!ai.is_empty());
    let user_delete = authors(&session)
        .into_iter()
        .any(|(kind, author)| kind == RevisionKind::Delete && author == "用户");
    assert!(user_delete);
}

#[test]
fn tab_replacement_is_ordinary_text() {
    let mut session = session(&["ab"]);
    let base = session.version();
    apply_ai_text_replace(&mut session, base, 1, 2, "\t").unwrap();
    assert_eq!(final_text(&session), vec!["a\t"]);
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec!["ab"]);
}

fn paragraph(session: &Session, index: usize) -> &joyword_model::Paragraph {
    session
        .document()
        .body()
        .blocks
        .iter()
        .nth(index)
        .and_then(Block::as_paragraph)
        .unwrap()
}

fn mark_authors(session: &Session) -> Vec<(usize, String)> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let paragraph = block.as_paragraph()?;
            paragraph
                .mark_revision
                .as_ref()
                .map(|revision| (index, revision.author.to_string()))
        })
        .collect()
}

#[test]
fn a_cross_paragraph_replace_deletes_the_marks_and_joins_in_the_final_view() {
    let mut session = session(&["甲乙", "丙丁", "戊己"]);
    let base = session.version();
    // "乙\n丙" — the tail of the first paragraph, the mark, and the head of
    // the second one.
    apply_ai_text_replace(&mut session, base, 1, 4, "X").unwrap();
    assert_eq!(final_text(&session), vec!["甲X丁", "戊己"]);
    assert_eq!(
        mark_authors(&session),
        vec![(0, AI_REVISION_AUTHOR.to_string())]
    );
    assert_eq!(session.undo_depth(), 1);
    let kinds: Vec<_> = paragraph(&session, 0)
        .runs()
        .iter()
        .filter_map(|run| run.revision.as_ref().map(|r| r.kind))
        .collect();
    assert!(kinds.contains(&RevisionKind::Delete));
    assert!(kinds.contains(&RevisionKind::Insert));
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec!["甲乙", "丙丁", "戊己"]);
    assert!(mark_authors(&session).is_empty());
}

#[test]
fn a_whole_middle_paragraph_is_cleared_and_its_mark_is_deleted() {
    let mut session = session(&["头", "中", "尾"]);
    let base = session.version();
    // Covers "头\n中\n" from the start of the first paragraph through the
    // second mark.
    apply_ai_text_replace(&mut session, base, 0, 4, "").unwrap();
    assert_eq!(final_text(&session), vec!["尾"]);
    assert_eq!(
        mark_authors(&session),
        vec![
            (0, AI_REVISION_AUTHOR.to_string()),
            (1, AI_REVISION_AUTHOR.to_string()),
        ]
    );
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec!["头", "中", "尾"]);
}

#[test]
fn accepting_the_cross_paragraph_revision_merges_and_rejecting_restores() {
    let mut session = session(&["甲乙", "丙丁"]);
    let base = session.version();
    apply_ai_text_replace(&mut session, base, 1, 4, "").unwrap();
    assert_eq!(final_text(&session), vec!["甲丁"]);
    let marks = mark_authors(&session);
    assert_eq!(marks.len(), 1);
    // Reject the deletion: text and paragraph break come back.
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::RejectAllRevisions],
        ))
        .unwrap();
    assert_eq!(final_text(&session), vec!["甲乙", "丙丁"]);
    assert!(mark_authors(&session).is_empty());

    let again = session.version();
    apply_ai_text_replace(&mut session, again, 1, 4, "").unwrap();
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::AcceptAllRevisions],
        ))
        .unwrap();
    assert_eq!(final_text(&session), vec!["甲丁"]);
    assert_eq!(session.document().body().blocks.len(), 1);
    assert!(mark_authors(&session).is_empty());
}

#[test]
fn a_pure_separator_replace_merges_two_paragraphs() {
    let mut session = session(&["左", "右"]);
    let base = session.version();
    // Only the separator between the two paragraphs.
    apply_ai_text_replace(&mut session, base, 1, 2, "").unwrap();
    assert_eq!(final_text(&session), vec!["左右"]);
    assert_eq!(
        mark_authors(&session),
        vec![(0, AI_REVISION_AUTHOR.to_string())]
    );
    session.undo().unwrap();
    assert_eq!(final_text(&session), vec!["左", "右"]);
}

#[test]
fn a_replacement_cannot_contain_newlines_or_cross_hidden_runs() {
    let mut session = session(&["abcd", "efgh"]);
    let base = session.version();
    assert_eq!(
        apply_ai_text_replace(&mut session, base, 0, 6, "两\n行"),
        Err(TxError::AiReplacementRefused)
    );
    // A user-tracked deletion hides a run in the first paragraph: the
    // covered suffix no longer reaches the paragraph end contiguously.
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::SetTrackRevisions { enabled: true }],
        ))
        .unwrap();
    let id = session.document().body().blocks.iter().next().unwrap().id();
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::DeleteRange {
                start: Position {
                    story: session.document().body_id(),
                    paragraph: id,
                    offset: ScalarOffset(3),
                    bias: Bias::After,
                },
                end: Position {
                    story: session.document().body_id(),
                    paragraph: id,
                    offset: ScalarOffset(4),
                    bias: Bias::Before,
                },
            }],
        ))
        .unwrap();
    let version = session.version();
    // The final view is "abc\nefgh"; a span covering the first paragraph's
    // visible suffix and the mark also covers the hidden "d" structurally.
    // The user's deletion survives untouched and the replace succeeds.
    apply_ai_text_replace(&mut session, version, 1, 5, "X").unwrap();
    assert_eq!(final_text(&session), vec!["aXfgh"]);
    let user_delete = authors(&session)
        .into_iter()
        .any(|(kind, author)| kind == RevisionKind::Delete && author == "用户");
    assert!(
        user_delete,
        "another author's deletion survives the replace"
    );
    assert_eq!(
        mark_authors(&session),
        vec![(0, AI_REVISION_AUTHOR.to_string())]
    );
}

#[test]
fn a_second_replace_uses_the_merged_projection() {
    let mut session = session(&["一", "二", "三"]);
    let base = session.version();
    apply_ai_text_replace(&mut session, base, 1, 3, "").unwrap();
    // Final view is now "一\n三" (one separator): the mark deletion joined
    // the groups. Replace across the remaining separator.
    let second = session.version();
    apply_ai_text_replace(&mut session, second, 0, 3, "全").unwrap();
    assert_eq!(final_text(&session), vec!["全"]);
}
