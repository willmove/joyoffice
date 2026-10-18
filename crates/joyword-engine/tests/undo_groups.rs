//! T209 undo-group tests (SPEC §6, ARCHITECTURE §4).
//!
//! Typing and deleting coalesce only with the immediately preceding entry of
//! the same kind and user origin when the selection continues where that entry
//! ended. Undo, redo, explicit seals, standalone commits and the step cap are
//! boundaries. Every committed version stays monotonic.

use std::collections::HashSet;
use std::sync::Arc;

use joyword_engine::{
    Command, EditPolicy, MAX_UNDO_GROUP_STEPS, Origin, Outcome, RunChange, Selection, Session,
    Transaction, TxError, UndoError, UndoGroup,
};
use joyword_model::{
    Bias, Block, BlockSeq, Document, DumpOptions, NodeId, Paragraph, Position, ScalarOffset,
};

const DUMP: DumpOptions = DumpOptions { node_ids: true };

fn session(text: &str) -> Session {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![Block::from(paragraph)]);
    Session::new(document)
}

fn first(session: &Session) -> NodeId {
    session.document().body().blocks.get(0).unwrap().id()
}

fn caret(session: &Session, offset: usize) -> Selection {
    Selection::caret(Position {
        story: session.document().body_id(),
        paragraph: first(session),
        offset: ScalarOffset(offset),
        bias: Bias::After,
    })
}

fn text(session: &Session) -> String {
    session
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new())
}

fn dump(session: &Session) -> String {
    session.document().canonical_dump(DUMP)
}

/// Insert `text` at the caret the way the input layer does: the undo
/// selection is the caret before, the redo selection the mapped caret after.
fn type_at(
    session: &mut Session,
    at: Selection,
    text: &str,
    group: UndoGroup,
    origin: Origin,
) -> Selection {
    let mut transaction = Transaction::new(
        session.version(),
        vec![Command::InsertText {
            at: at.head,
            text: text.into(),
        }],
    );
    transaction.selection = Some(at);
    transaction.undo_group = group;
    transaction.origin = origin;
    match session
        .apply_with_mapped_selection(transaction, Selection::caret(at.head))
        .unwrap()
    {
        Outcome::Applied(result) => result.selection.unwrap(),
        Outcome::NoOp(_) => panic!("expected a change"),
    }
}

fn typed(session: &mut Session, at: Selection, text: &str) -> Selection {
    type_at(session, at, text, UndoGroup::Typing, Origin::User)
}

/// Backspace one scalar before the caret.
fn backspace(session: &mut Session, at: Selection) -> Selection {
    let offset = at.head.offset.0;
    let start = Position {
        offset: ScalarOffset(offset - 1),
        bias: Bias::After,
        ..at.head
    };
    let end = Position {
        bias: Bias::Before,
        ..at.head
    };
    let mut transaction =
        Transaction::new(session.version(), vec![Command::DeleteRange { start, end }]);
    transaction.selection = Some(at);
    transaction.undo_group = UndoGroup::Deleting;
    match session
        .apply_with_mapped_selection(transaction, Selection::caret(start))
        .unwrap()
    {
        Outcome::Applied(result) => result.selection.unwrap(),
        Outcome::NoOp(_) => panic!("expected a change"),
    }
}

#[test]
fn consecutive_typing_is_one_undo_step_that_restores_the_first_selection() {
    let mut session = session("甲");
    let start = caret(&session, 1);
    let initial = dump(&session);
    let mut at = start;
    let mut versions = vec![session.version()];
    for ch in ["乙", "丙", "a", "🙂"] {
        at = typed(&mut session, at, ch);
        versions.push(session.version());
    }
    assert_eq!(text(&session), "甲乙丙a🙂");
    assert_eq!(session.undo_depth(), 1);
    let typed_state = dump(&session);

    let undone = session.undo().unwrap();
    assert_eq!(dump(&session), initial);
    assert_eq!(undone.selection, Some(start));
    assert!(undone.changes.paragraphs.contains(&first(&session)));
    versions.push(session.version());
    assert!(!session.can_undo());

    let redone = session.redo().unwrap();
    assert_eq!(dump(&session), typed_state);
    assert_eq!(redone.selection, Some(at));
    versions.push(session.version());
    assert!(versions.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn a_moved_caret_starts_a_new_group() {
    let mut session = session("甲乙");
    let c2 = caret(&session, 2);
    let at = typed(&mut session, c2, "丙");
    typed(&mut session, at, "丁");
    // Typing continues elsewhere: the starting selection does not equal the
    // previous entry's resulting selection.
    let c0 = caret(&session, 0);
    typed(&mut session, c0, "戊");
    assert_eq!(text(&session), "戊甲乙丙丁");
    assert_eq!(session.undo_depth(), 2);
    session.undo().unwrap();
    assert_eq!(text(&session), "甲乙丙丁");
    session.undo().unwrap();
    assert_eq!(text(&session), "甲乙");
}

#[test]
fn standalone_commits_seals_and_kind_changes_are_boundaries() {
    let mut session = session("");
    let c0 = caret(&session, 0);
    let mut at = typed(&mut session, c0, "a");
    at = typed(&mut session, at, "b");
    // A pasted/IME/command insertion is standalone even at the same caret.
    at = type_at(
        &mut session,
        at,
        "PASTE",
        UndoGroup::Standalone,
        Origin::User,
    );
    // Typing after it does not fold into the standalone entry.
    at = typed(&mut session, at, "c");
    session.seal_undo_group();
    at = typed(&mut session, at, "d");
    at = typed(&mut session, at, "e");
    // Deleting does not fold into typing.
    at = backspace(&mut session, at);
    backspace(&mut session, at);
    assert_eq!(text(&session), "abPASTEc");
    assert_eq!(session.undo_depth(), 5);

    let expected = ["abPASTEcde", "abPASTEc", "abPASTE", "ab", ""];
    for want in expected {
        session.undo().unwrap();
        assert_eq!(text(&session), want);
    }
    assert_eq!(session.undo(), Err(UndoError::Nothing));
}

#[test]
fn formatting_between_keystrokes_separates_the_typing() {
    let mut session = session("甲");
    let c1 = caret(&session, 1);
    let at = typed(&mut session, c1, "乙");
    let start = caret(&session, 0).head;
    let end = Position {
        offset: ScalarOffset(1),
        bias: Bias::Before,
        ..start
    };
    session
        .apply(Transaction::new(
            session.version(),
            vec![Command::FormatText {
                start,
                end,
                change: RunChange::Bold(Some(true)),
            }],
        ))
        .unwrap();
    typed(&mut session, at, "丙");
    assert_eq!(session.undo_depth(), 3);
}

#[test]
fn typing_after_undo_never_folds_into_the_restored_entry() {
    let mut session = session("");
    let c0 = caret(&session, 0);
    let mut at = typed(&mut session, c0, "a");
    at = typed(&mut session, at, "b");
    session.seal_undo_group();
    typed(&mut session, at, "c");
    session.undo().unwrap();
    assert_eq!(text(&session), "ab");
    // The selection after this undo is the caret before "c", which equals the
    // previous group's end; the undo still closed that group.
    typed(&mut session, at, "x");
    assert_eq!(text(&session), "abx");
    assert_eq!(session.undo_depth(), 2);
    assert!(!session.can_redo());
    session.undo().unwrap();
    assert_eq!(text(&session), "ab");
    // Redo then typing at the redone caret also starts a new step.
    session.redo().unwrap();
    let c3 = caret(&session, 3);
    typed(&mut session, c3, "y");
    assert_eq!(session.undo_depth(), 3);
}

#[test]
fn agent_and_system_changes_never_coalesce() {
    let mut session = session("");
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: true,
    });
    let c0 = caret(&session, 0);
    let mut at = type_at(&mut session, c0, "A", UndoGroup::Typing, Origin::Agent);
    at = type_at(&mut session, at, "B", UndoGroup::Typing, Origin::Agent);
    at = type_at(&mut session, at, "S", UndoGroup::Typing, Origin::System);
    at = type_at(&mut session, at, "T", UndoGroup::Typing, Origin::System);
    // User typing after an agent edit at the same caret starts its own step.
    at = typed(&mut session, at, "u");
    typed(&mut session, at, "v");
    assert_eq!(text(&session), "ABSTuv");
    assert_eq!(session.undo_depth(), 5);
}

#[test]
fn a_long_burst_is_split_at_the_step_cap() {
    let mut session = session("");
    let mut at = caret(&session, 0);
    for _ in 0..MAX_UNDO_GROUP_STEPS + 1 {
        at = typed(&mut session, at, "字");
    }
    assert_eq!(session.undo_depth(), 2);
    session.undo().unwrap();
    assert_eq!(text(&session).chars().count(), MAX_UNDO_GROUP_STEPS);
    session.undo().unwrap();
    assert_eq!(text(&session), "");
}

#[test]
fn rejected_and_empty_transactions_leave_the_group_untouched() {
    let mut session = session("");
    let stale = session.version();
    let c0 = caret(&session, 0);
    let at = typed(&mut session, c0, "a");
    let mut transaction = Transaction::new(
        stale,
        vec![Command::InsertText {
            at: at.head,
            text: "z".into(),
        }],
    );
    transaction.selection = Some(at);
    transaction.undo_group = UndoGroup::Typing;
    assert!(matches!(
        session.apply(transaction),
        Err(TxError::StaleVersion { .. })
    ));
    let mut empty = Transaction::new(session.version(), Vec::new());
    empty.undo_group = UndoGroup::Typing;
    assert!(matches!(session.apply(empty), Ok(Outcome::NoOp(_))));
    typed(&mut session, at, "b");
    assert_eq!(text(&session), "ab");
    assert_eq!(session.undo_depth(), 1);
}

#[test]
fn read_only_policy_still_rejects_grouped_input() {
    let mut session = session("甲");
    session.set_policy(EditPolicy {
        read_only: true,
        agent_writes: false,
    });
    let at = caret(&session, 1);
    let mut transaction = Transaction::new(
        session.version(),
        vec![Command::InsertText {
            at: at.head,
            text: "乙".into(),
        }],
    );
    transaction.selection = Some(at);
    transaction.undo_group = UndoGroup::Typing;
    assert_eq!(session.apply(transaction), Err(TxError::ReadOnly));
    assert_eq!(session.undo_depth(), 0);
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }
}

/// Random typing / deleting / standalone / seal / caret moves. Undoing the
/// whole history must stop at every state that preceded a boundary, visit
/// only committed states, end at the initial state, and redo must return to
/// the final state with strictly increasing versions throughout.
#[test]
fn random_sessions_stop_at_every_boundary_and_round_trip() {
    for seed in 0..48u64 {
        let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut session = session("起");
        let initial = dump(&session);
        let mut committed: HashSet<String> = HashSet::from([initial.clone()]);
        let mut boundaries: HashSet<String> = HashSet::new();
        let mut at = caret(&session, 1);
        // Expected grouping model: kind of the open group, its length and
        // the selection it ended with.
        let mut open: Option<(UndoGroup, usize)> = None;
        let mut last_after: Option<Selection> = None;
        let mut expected_depth = 0usize;
        let mut last_version = session.version();

        for _ in 0..160 {
            let len = text(&session).chars().count();
            let before = dump(&session);
            let start = at;
            let (group, next) = match rng.next(10) {
                0..=4 => (
                    UndoGroup::Typing,
                    Some(typed(&mut session, at, ["a", "字", "，", "é"][rng.next(4)])),
                ),
                5 | 6 if at.head.offset.0 > 0 => {
                    (UndoGroup::Deleting, Some(backspace(&mut session, at)))
                }
                7 => (
                    UndoGroup::Standalone,
                    Some(type_at(
                        &mut session,
                        at,
                        "贴",
                        UndoGroup::Standalone,
                        Origin::User,
                    )),
                ),
                8 => {
                    session.seal_undo_group();
                    open = None;
                    continue;
                }
                _ => {
                    // A caret move breaks a group only through selection
                    // discontinuity, which the model checks below.
                    at = caret(&session, rng.next(len + 1));
                    continue;
                }
            };
            let Some(next) = next else { continue };
            assert!(session.version() > last_version);
            last_version = session.version();
            let merges = last_after == Some(start)
                && matches!(open, Some((kind, steps))
                    if kind == group && steps < MAX_UNDO_GROUP_STEPS);
            if merges {
                if let Some((_, steps)) = open.as_mut() {
                    *steps += 1;
                }
            } else {
                boundaries.insert(before);
                expected_depth += 1;
                open = (group != UndoGroup::Standalone).then_some((group, 1));
            }
            at = next;
            last_after = Some(next);
            committed.insert(dump(&session));
        }

        assert_eq!(session.undo_depth(), expected_depth, "seed {seed}");
        let last = dump(&session);
        let mut visited = HashSet::new();
        while session.can_undo() {
            session.undo().unwrap();
            assert!(session.version() > last_version);
            last_version = session.version();
            let state = dump(&session);
            assert!(committed.contains(&state), "seed {seed}: foreign state");
            visited.insert(state);
        }
        assert_eq!(dump(&session), initial, "seed {seed}");
        for boundary in &boundaries {
            assert!(visited.contains(boundary), "seed {seed}: skipped boundary");
        }
        while session.can_redo() {
            session.redo().unwrap();
            assert!(session.version() > last_version);
            last_version = session.version();
        }
        assert_eq!(dump(&session), last, "seed {seed}");
    }
}
