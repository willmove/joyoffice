//! T104 undo/redo property tests (SPEC EDIT-005, ARCHITECTURE §3–§4).
//!
//! A hand-rolled xorshift64* generator drives seeded random editing
//! transactions through the public session API. The checked invariants:
//! undoing the whole history restores the pristine snapshot exactly; undoing
//! k entries then redoing k restores the pre-undo state; a fresh edit after
//! undo truncates redo permanently; rejected and failed transactions leave
//! the document and both history stacks untouched; undo/redo preserve
//! paragraph identity and only ever publish new, strictly increasing
//! versions. Every assertion carries the seed, step and command log so a
//! failure reproduces from the seed alone.

use std::collections::HashSet;
use std::sync::Arc;

use joyword_engine::{
    Command, Origin, Outcome, ParagraphChange, Precondition, RunChange, Session, Transaction,
    TxError, UndoError,
};
use joyword_model::{
    Bias, Block, BlockSeq, BreakKind, Document, DocumentVersion, DumpOptions, InlineKind,
    Justification, NodeId, Paragraph, Position, ScalarOffset,
};

const DUMP: DumpOptions = DumpOptions { node_ids: true };
const SEEDS: [u64; 4] = [2026_1026, 0x5EED_C0FF, 7, 0xC0DE_5EED];
const STEPS: usize = 140;

/// xorshift64* (Marsaglia): small, dependency-free and deterministic.
struct Xorshift64Star(u64);

impl Xorshift64Star {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}

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

/// Random pristine content keeps the initial state from being special.
fn seeded_document(rng: &mut Xorshift64Star) -> Document {
    let words = [
        "甲方付款",
        "Party B",
        "第三条",
        "违约责任",
        "附件一",
        "notes",
    ];
    let lines: Vec<String> = (0..2 + rng.below(3))
        .map(|_| {
            let word = words[rng.below(words.len())];
            word.repeat(1 + rng.below(2))
        })
        .collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    document(&refs)
}

fn paragraph_ids(session: &Session) -> Vec<NodeId> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .map(Block::id)
        .collect()
}

fn lengths(session: &Session) -> Vec<usize> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .map(|block| block.as_paragraph().map_or(0, Paragraph::len))
        .collect()
}

fn dump(session: &Session) -> String {
    session.document().canonical_dump(DUMP)
}

fn pos(session: &Session, paragraph: NodeId, offset: usize, bias: Bias) -> Position {
    Position {
        story: session.document().body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias,
    }
}

/// One recorded point of the committed timeline.
#[derive(Clone)]
struct State {
    dump: String,
    ids: Vec<NodeId>,
    version: u64,
}

fn state(session: &Session) -> State {
    State {
        dump: dump(session),
        ids: paragraph_ids(session),
        version: session.version().get(),
    }
}

struct Driver {
    seed: u64,
    step: usize,
    log: Vec<String>,
    /// Paragraph ids removed by edits; stale references for hostile inputs.
    retired: Vec<NodeId>,
}

impl Driver {
    fn note(&mut self, entry: String) {
        self.log.push(format!("{}: {}", self.step, entry));
    }

    /// Printed on every failure: replay the logged transactions, or rerun
    /// with the same seed.
    fn ctx(&self) -> String {
        format!(
            "seed {} step {}:\n{}",
            self.seed,
            self.step,
            self.log.join("\n")
        )
    }
}

/// A transaction whose inputs are all inside the current document. Only a
/// NoOp or an engine defect is an acceptable non-commit outcome.
fn good_transaction(session: &Session, rng: &mut Xorshift64Star) -> Transaction {
    let ids = paragraph_ids(session);
    let lens = lengths(session);
    let pick = |rng: &mut Xorshift64Star| {
        let p = rng.below(ids.len());
        (ids[p], lens[p])
    };
    let text = ["新增", "ab", "🙂", "e\u{301}", "，甲"];
    // One shared target paragraph; structural arms that need neighbours pick
    // their own below.
    let (id, len) = pick(rng);
    let commands = match rng.below(11) {
        0 | 1 => {
            vec![Command::InsertText {
                at: pos(session, id, rng.below(len + 1), Bias::After),
                text: text[rng.below(text.len())].into(),
            }]
        }
        2 => {
            let start = rng.below(len + 1);
            let end = start + rng.below(len - start + 1);
            vec![Command::DeleteRange {
                start: pos(session, id, start, Bias::After),
                end: pos(session, id, end, Bias::Before),
            }]
        }
        3 if ids.len() > 1 => {
            let first = rng.below(ids.len() - 1);
            let last = first + 1;
            vec![Command::DeleteRange {
                start: pos(session, ids[first], rng.below(lens[first] + 1), Bias::After),
                end: pos(session, ids[last], rng.below(lens[last] + 1), Bias::Before),
            }]
        }
        4 => {
            vec![Command::SplitParagraph {
                at: pos(session, id, rng.below(len + 1), Bias::After),
            }]
        }
        5 if len > 0 => {
            // Bold over a non-empty sub-range; an empty paragraph has no
            // representable span, so it falls through to the replacement.
            let start = rng.below(len);
            vec![Command::FormatText {
                start: pos(session, id, start, Bias::After),
                end: pos(
                    session,
                    id,
                    start + 1 + rng.below(len - start),
                    Bias::Before,
                ),
                change: RunChange::Bold(Some(rng.below(2) == 0)),
            }]
        }
        6 => {
            // Pick a justification the paragraph does not already carry, so
            // the command changes something instead of NoOp-ing.
            let current = session
                .document()
                .body()
                .blocks
                .iter()
                .find(|block| block.id() == id)
                .and_then(Block::as_paragraph)
                .and_then(|paragraph| paragraph.props.justification);
            let next = [
                Justification::Start,
                Justification::Center,
                Justification::Both,
            ]
            .into_iter()
            .find(|value| Some(*value) != current)
            .unwrap_or(Justification::Start);
            vec![Command::FormatParagraph {
                story: session.document().body_id(),
                paragraph: id,
                change: ParagraphChange::Justification(Some(next)),
            }]
        }
        7 => {
            vec![Command::InsertInline {
                at: pos(session, id, rng.below(len + 1), Bias::After),
                kind: InlineKind::Break(BreakKind::Line),
            }]
        }
        8 if ids.len() > 1 => {
            let from = rng.below(ids.len());
            let before = (rng.below(2) == 0).then(|| ids[rng.below(ids.len())]);
            vec![Command::MoveBlock {
                story: session.document().body_id(),
                block: ids[from],
                before,
            }]
        }
        9 if ids.len() > 1 => {
            let victim = rng.below(ids.len());
            vec![Command::RemoveBlock {
                story: session.document().body_id(),
                block: ids[victim],
            }]
        }
        _ => {
            // A multi-command replacement: delete a span, insert at the hole.
            let start = rng.below(len + 1);
            let end = start + rng.below(len - start + 1);
            vec![
                Command::DeleteRange {
                    start: pos(session, id, start, Bias::After),
                    end: pos(session, id, end, Bias::Before),
                },
                Command::InsertText {
                    at: pos(session, id, start, Bias::After),
                    text: text[rng.below(text.len())].into(),
                },
            ]
        }
    };
    Transaction::new(session.version(), commands)
}

/// Every hostile input class the public API rejects. `stale_base` is the
/// pristine version, stale once anything has committed; when the envelope is
/// refused (stale base, disabled agent writes, failed precondition) the
/// command itself is valid, otherwise the command fails.
fn hostile_transaction(
    session: &Session,
    rng: &mut Xorshift64Star,
    retired: &[NodeId],
    stale_base: Option<DocumentVersion>,
) -> Transaction {
    let ids = paragraph_ids(session);
    let lens = lengths(session);
    let p = rng.below(ids.len());
    let section = session.document().sections()[0].id;
    let insert_valid = || {
        vec![Command::InsertText {
            at: pos(session, ids[p], 0, Bias::After),
            text: "通过".into(),
        }]
    };
    // Half the hostile inputs fail inside a command, half are refused by the
    // transaction envelope before any command runs. Without a captured stale
    // version every kind falls back to command-level failures.
    let kind = rng.below(4);
    let has_stale = stale_base.is_some();
    let stale = kind == 0 && has_stale;
    let agent = kind == 1 && has_stale;
    let precondition = kind == 2 && has_stale;
    let envelope = stale || agent || precondition;
    let commands = if envelope {
        insert_valid()
    } else {
        match rng.below(6) {
            0 => vec![Command::InsertText {
                at: pos(session, ids[p], 0, Bias::After),
                text: "a\nb".into(),
            }],
            1 => vec![Command::InsertText {
                at: pos(session, ids[p], lens[p] + 997, Bias::After),
                text: "越界".into(),
            }],
            2 if lens[p] > 0 => vec![Command::DeleteRange {
                start: pos(session, ids[p], lens[p], Bias::After),
                end: pos(session, ids[p], 0, Bias::Before),
            }],
            3 => vec![Command::InsertText {
                at: pos(session, section, 0, Bias::After),
                text: "甲".into(),
            }],
            4 => {
                // First command is valid; the second fails, so nothing may
                // apply (atomicity of multi-command transactions). The offset
                // is out of bounds even after the first command's growth.
                let text = "应回滚";
                let grown = lens[p] + text.chars().count();
                vec![
                    Command::InsertText {
                        at: pos(session, ids[p], rng.below(lens[p] + 1), Bias::After),
                        text: text.into(),
                    },
                    Command::SplitParagraph {
                        at: pos(session, ids[p], grown + 1, Bias::After),
                    },
                ]
            }
            _ if let Some(&gone) = retired.first() => vec![Command::SplitParagraph {
                at: pos(session, gone, 0, Bias::After),
            }],
            _ => vec![Command::InsertText {
                at: pos(session, ids[p], lens[p] + 1, Bias::After),
                text: "长度".into(),
            }],
        }
    };
    let base = if stale {
        stale_base.unwrap()
    } else {
        session.version()
    };
    let mut transaction = Transaction::new(base, commands);
    if agent {
        transaction.origin = Origin::Agent;
    }
    if precondition {
        transaction.preconditions = vec![Precondition::Text {
            story: session.document().body_id(),
            paragraph: ids[p],
            start: ScalarOffset(0),
            end: ScalarOffset(0),
            expected: "不会成立的期望文本".into(),
        }];
    }
    transaction
}

fn expect_hostile_error(error: &TxError, ctx: &str) {
    let expected = matches!(
        error,
        TxError::StaleVersion { .. }
            | TxError::AgentWritesDisabled
            | TxError::PreconditionFailed { .. }
            | TxError::Command { .. }
    );
    assert!(expected, "{ctx}: unexpected rejection class {error:?}");
}

/// Record paragraph ids that left the document so hostile inputs can address
/// them; identifiers are never reused, so they stay unknown forever.
fn retire_removed(driver: &mut Driver, before: &[NodeId], session: &Session) {
    let after = paragraph_ids(session);
    for id in before {
        if !after.contains(id) {
            driver.retired.push(*id);
        }
    }
}

/// Drive `steps` transactions; roughly one in five is hostile. Returns the
/// timeline of committed states, starting with the pristine state.
fn drive_random_history(session: &mut Session, seed: u64, rng: &mut Xorshift64Star) -> Vec<State> {
    let mut driver = Driver {
        seed,
        step: 0,
        log: Vec::new(),
        retired: Vec::new(),
    };
    let mut states = vec![state(session)];
    let mut commits = 0usize;
    // The pristine version only becomes a usable stale base after the first
    // commit; before that it equals the current version.
    let mut stale_base: Option<DocumentVersion> = None;
    for step in 0..STEPS {
        driver.step = step;
        let hostile = rng.below(5) == 0;
        let before_ids = paragraph_ids(session);
        let before = state(session);
        let before_version = session.version();
        let undo_depth = session.undo_depth();
        if hostile {
            let transaction = hostile_transaction(session, rng, &driver.retired, stale_base);
            driver.note(format!("{transaction:?}"));
            match session.apply(transaction) {
                Err(error) => expect_hostile_error(&error, &driver.ctx()),
                Ok(Outcome::NoOp(_)) => panic!("{}", driver.ctx()),
                Ok(Outcome::Applied(_)) => panic!("{}: hostile input committed", driver.ctx()),
            }
            assert_eq!(dump(session), before.dump, "{}", driver.ctx());
            assert_eq!(session.version().get(), before.version, "{}", driver.ctx());
            assert_eq!(session.undo_depth(), undo_depth, "{}", driver.ctx());
            assert_eq!(session.redo_depth(), 0, "{}", driver.ctx());
        } else {
            let transaction = good_transaction(session, rng);
            driver.note(format!("{transaction:?}"));
            match session.apply(transaction) {
                Ok(Outcome::NoOp(current)) => {
                    assert_eq!(current.get(), before.version, "{}", driver.ctx());
                    assert_eq!(session.undo_depth(), undo_depth, "{}", driver.ctx());
                }
                Ok(Outcome::Applied(result)) => {
                    assert!(result.version.get() > before.version, "{}", driver.ctx());
                    assert_eq!(session.version(), result.version, "{}", driver.ctx());
                    assert_eq!(session.undo_depth(), undo_depth + 1, "{}", driver.ctx());
                    assert!(!session.can_redo(), "{}", driver.ctx());
                    commits += 1;
                    stale_base.get_or_insert(before_version);
                    states.push(state(session));
                }
                Err(error) => panic!("{}: good input rejected: {error}", driver.ctx()),
            }
        }
        retire_removed(&mut driver, &before_ids, session);
        assert!(
            !session.document().validate().has_errors(),
            "{}",
            driver.ctx()
        );
    }
    assert!(
        commits > STEPS / 4,
        "seed {seed}: only {commits} commits, too few to exercise undo"
    );
    states
}

#[test]
fn random_histories_undo_to_the_pristine_snapshot() {
    for seed in SEEDS {
        let mut rng = Xorshift64Star::new(seed);
        let mut session = Session::new(seeded_document(&mut rng));
        let pristine = state(&session);
        let states = drive_random_history(&mut session, seed, &mut rng);

        let mut max_version = session.version().get();
        let mut walked = Vec::new();
        while session.can_undo() {
            let undone = session.undo().unwrap();
            assert!(undone.version.get() > max_version, "seed {seed}");
            max_version = undone.version.get();
            assert_eq!(session.version().get(), max_version, "seed {seed}");
            walked.push(state(&session));
        }
        assert_eq!(session.undo(), Err(UndoError::Nothing), "seed {seed}");
        let mut expected: Vec<&State> = states[..states.len() - 1].iter().rev().collect();
        assert_eq!(walked.len(), expected.len(), "seed {seed}");
        for (index, (seen, want)) in walked.iter().zip(expected.drain(..)).enumerate() {
            assert_eq!(seen.dump, want.dump, "seed {seed}: state {index}");
            assert_eq!(seen.ids, want.ids, "seed {seed}: identity {index}");
        }
        assert_eq!(dump(&session), pristine.dump, "seed {seed}");
        assert_eq!(paragraph_ids(&session), pristine.ids, "seed {seed}");
        assert!(!session.document().validate().has_errors(), "seed {seed}");
    }
}

#[test]
fn undoing_k_then_redoing_k_restores_the_pre_undo_state() {
    for seed in SEEDS {
        let mut rng = Xorshift64Star::new(seed ^ 0xA5A5_A5A5);
        let mut session = Session::new(seeded_document(&mut rng));
        let states = drive_random_history(&mut session, seed, &mut rng);
        let depth = session.undo_depth();
        let pre_undo = states.last().unwrap();

        let mut max_version = session.version().get();
        for trial in 0..6 {
            let k = 1 + rng.below(depth);
            let target = states[states.len() - 1 - k].clone();
            for _ in 0..k {
                let undone = session.undo().unwrap();
                assert!(undone.version.get() > max_version, "seed {seed}");
                max_version = undone.version.get();
            }
            assert_eq!(dump(&session), target.dump, "seed {seed} trial {trial}");
            assert_eq!(paragraph_ids(&session), target.ids, "seed {seed}");
            for _ in 0..k {
                let redone = session.redo().unwrap();
                assert!(redone.version.get() > max_version, "seed {seed}");
                max_version = redone.version.get();
            }
            assert_eq!(dump(&session), pre_undo.dump, "seed {seed} trial {trial}");
            assert_eq!(paragraph_ids(&session), pre_undo.ids, "seed {seed}");
            assert_eq!(session.redo(), Err(UndoError::Nothing), "seed {seed}");
            assert_eq!(session.undo_depth(), depth, "seed {seed}");
        }
    }
}

#[test]
fn a_fresh_edit_after_undo_truncates_redo() {
    for seed in SEEDS {
        let mut rng = Xorshift64Star::new(seed ^ 0x1F2E_3D4C);
        let mut session = Session::new(seeded_document(&mut rng));
        let states = drive_random_history(&mut session, seed, &mut rng);
        let k = 1 + rng.below(session.undo_depth());
        let after_undo_target = states[states.len() - 1 - k].clone();
        for _ in 0..k {
            session.undo().unwrap();
        }
        assert_eq!(dump(&session), after_undo_target.dump, "seed {seed}");
        assert_eq!(session.redo_depth(), k, "seed {seed}");

        // One fresh commit: the k discarded redo entries are gone for good.
        let ids = paragraph_ids(&session);
        let id = ids[rng.below(ids.len())];
        session
            .apply(Transaction::new(
                session.version(),
                vec![Command::InsertText {
                    at: pos(&session, id, 0, Bias::After),
                    text: "截断".into(),
                }],
            ))
            .unwrap();
        let fresh_state = state(&session);
        assert!(!session.can_redo(), "seed {seed}");
        assert_eq!(session.redo_depth(), 0, "seed {seed}");
        assert_eq!(session.redo(), Err(UndoError::Nothing), "seed {seed}");

        // Redo can no longer resurrect the lost states; the only redo entry
        // is the fresh edit itself, from this point on.
        session.undo().unwrap();
        assert_eq!(dump(&session), after_undo_target.dump, "seed {seed}");
        assert_eq!(session.redo_depth(), 1, "seed {seed}");
        session.redo().unwrap();
        assert_eq!(dump(&session), fresh_state.dump, "seed {seed}");
        assert_eq!(session.redo(), Err(UndoError::Nothing), "seed {seed}");

        // The surviving undo chain still reaches the pristine snapshot, and
        // the whole current timeline replays forward to the fresh state.
        while session.can_undo() {
            session.undo().unwrap();
        }
        assert_eq!(dump(&session), states[0].dump, "seed {seed}");
        assert_eq!(paragraph_ids(&session), states[0].ids, "seed {seed}");
        while session.can_redo() {
            session.redo().unwrap();
        }
        assert_eq!(dump(&session), fresh_state.dump, "seed {seed}");
    }
}

#[test]
fn rejected_transactions_leave_document_and_stacks_unchanged() {
    for seed in SEEDS {
        let mut rng = Xorshift64Star::new(seed ^ 0x7E11_7E11);
        let mut session = Session::new(seeded_document(&mut rng));
        drive_random_history(&mut session, seed, &mut rng);

        // Hostile inputs against a mid-history position with a live redo
        // side: nothing may disturb either stack.
        let stale_base = session.version();
        let k = 1 + rng.below(session.undo_depth().saturating_sub(1).max(1));
        for _ in 0..k {
            session.undo().unwrap();
        }
        let before = state(&session);
        let undo_depth = session.undo_depth();
        let redo_depth = session.redo_depth();
        for round in 0..40 {
            let transaction = hostile_transaction(&session, &mut rng, &[], Some(stale_base));
            match session.apply(transaction.clone()) {
                Err(error) => expect_hostile_error(&error, &format!("seed {seed} round {round}")),
                Ok(Outcome::NoOp(_)) => {
                    panic!("seed {seed} round {round}: hostile input was a NoOp")
                }
                Ok(Outcome::Applied(_)) => {
                    panic!("seed {seed} round {round}: hostile input committed")
                }
            }
            assert_eq!(dump(&session), before.dump, "seed {seed} round {round}");
            assert_eq!(
                session.version().get(),
                before.version,
                "seed {seed} round {round}"
            );
            assert_eq!(
                session.undo_depth(),
                undo_depth,
                "seed {seed} round {round}"
            );
            assert_eq!(
                session.redo_depth(),
                redo_depth,
                "seed {seed} round {round}"
            );
        }

        // The untouched redo side still works after all the rejections.
        for _ in 0..redo_depth {
            session.redo().unwrap();
        }
        assert_eq!(session.redo(), Err(UndoError::Nothing), "seed {seed}");
        assert!(!session.document().validate().has_errors(), "seed {seed}");
    }
}

#[test]
fn undo_redo_preserve_document_identity_and_monotonic_versions() {
    for seed in SEEDS {
        let mut rng = Xorshift64Star::new(seed ^ 0x4242_4242);
        let mut session = Session::new(seeded_document(&mut rng));
        let states = drive_random_history(&mut session, seed, &mut rng);

        let mut max_version = session.version().get();
        let mut seen_versions = HashSet::from([max_version]);
        // Two full undo/redo cycles: the same states, paragraph ids and their
        // order intact, while every published version is new and larger.
        for cycle in 0..2 {
            let mut expect: Vec<&State> = states[..states.len() - 1].iter().rev().collect();
            for want in expect.drain(..) {
                let undone = session.undo().unwrap();
                assert!(
                    seen_versions.insert(undone.version.get()),
                    "seed {seed} cycle {cycle}: version {} reused",
                    undone.version.get()
                );
                assert!(undone.version.get() > max_version, "seed {seed}");
                max_version = undone.version.get();
                assert_eq!(dump(&session), want.dump, "seed {seed} cycle {cycle}");
                assert_eq!(
                    paragraph_ids(&session),
                    want.ids,
                    "seed {seed} cycle {cycle}"
                );
            }
            for want in states.iter().skip(1) {
                let redone = session.redo().unwrap();
                assert!(
                    seen_versions.insert(redone.version.get()),
                    "seed {seed} cycle {cycle}: version {} reused",
                    redone.version.get()
                );
                assert!(redone.version.get() > max_version, "seed {seed}");
                max_version = redone.version.get();
                assert_eq!(dump(&session), want.dump, "seed {seed} cycle {cycle}");
                assert_eq!(
                    paragraph_ids(&session),
                    want.ids,
                    "seed {seed} cycle {cycle}"
                );
            }
            assert_eq!(session.undo_depth(), states.len() - 1, "seed {seed}");
            assert_eq!(session.redo_depth(), 0, "seed {seed}");
        }
    }
}

/// EDIT-005: undo/redo is engine session state, separate from review
/// revisions and the recovery journal (the save/journal half of the contract
/// is tested in `joyword-shell`'s `storage::recovery`). Each session owns its
/// own stacks: undoing in one document never touches another document's
/// content, version or history.
#[test]
fn two_sessions_have_independent_undo_stacks() {
    for seed in SEEDS {
        let mut rng = Xorshift64Star::new(seed ^ 0x0F0F_0F0F);
        let mut left = Session::new(seeded_document(&mut rng));
        let mut right = Session::new(seeded_document(&mut rng));
        let left_pristine = state(&left);
        let right_pristine = state(&right);

        // Interleave commits in both sessions.
        for _ in 0..STEPS / 2 {
            let transaction = good_transaction(&left, &mut rng);
            left.apply(transaction).unwrap();
            let transaction = good_transaction(&right, &mut rng);
            right.apply(transaction).unwrap();
        }
        let left_depth = left.undo_depth();
        let right_depth = right.undo_depth();
        assert!(left_depth > 0 && right_depth > 0, "seed {seed}");

        // Draining the left history leaves the right session untouched.
        let right_before = state(&right);
        while left.can_undo() {
            left.undo().unwrap();
        }
        assert_eq!(dump(&left), left_pristine.dump, "seed {seed}");
        assert_eq!(paragraph_ids(&left), left_pristine.ids, "seed {seed}");
        assert_eq!(left.redo_depth(), left_depth, "seed {seed}");
        assert_eq!(dump(&right), right_before.dump, "seed {seed}");
        assert_eq!(right.version().get(), right_before.version, "seed {seed}");
        assert_eq!(right.undo_depth(), right_depth, "seed {seed}");
        assert_eq!(right.redo_depth(), 0, "seed {seed}");

        // The right session still undoes to its own pristine snapshot.
        while right.can_undo() {
            right.undo().unwrap();
        }
        assert_eq!(dump(&right), right_pristine.dump, "seed {seed}");
        assert_eq!(paragraph_ids(&right), right_pristine.ids, "seed {seed}");
        assert_eq!(left.undo_depth(), 0, "seed {seed}");
    }
}
