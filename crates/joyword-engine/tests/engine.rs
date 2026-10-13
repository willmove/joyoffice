//! T101 property and scenario tests.
//!
//! The reference model stores each paragraph as a list of characters and
//! position markers. Edits are plain list operations, so where a marker ends
//! up follows from the list itself rather than from the engine's mapping
//! rules; the engine's PositionMap must agree with it after every edit.

use std::sync::Arc;

use joyword_engine::{
    ChangeSet, Command, CommandError, EditPolicy, Mapped, Origin, Outcome, ParagraphChange,
    Precondition, RunChange, Selection, Session, Transaction, TxError, UndoError,
};
use joyword_model::{
    Anchor, AnnotationKind, Bias, Block, BlockSeq, BreakKind, Cell, Document, DumpOptions,
    InlineKind, Justification, Measure, NodeId, Paragraph, Position, Row, ScalarOffset, Sdt,
    SdtLock, SdtProps, StoryKind, Table, VerticalMerge,
};

const DUMP: DumpOptions = DumpOptions { node_ids: true };

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

fn paragraph_ids(session: &Session) -> Vec<NodeId> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .map(Block::id)
        .collect()
}

fn texts(session: &Session) -> Vec<String> {
    session
        .document()
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| "□".into()))
        .collect()
}

fn pos(session: &Session, paragraph: NodeId, offset: usize, bias: Bias) -> Position {
    Position {
        story: session.document().body_id(),
        paragraph,
        offset: ScalarOffset(offset),
        bias,
    }
}

/// Method form so command arguments may read the session (two-phase borrow).
trait Exec {
    fn exec(&mut self, commands: Vec<Command>) -> Result<Outcome, TxError>;
}

impl Exec for Session {
    fn exec(&mut self, commands: Vec<Command>) -> Result<Outcome, TxError> {
        self.apply(Transaction::new(self.version(), commands))
    }
}

fn anchor_of(session: &Session, id: joyword_model::AnnotationId) -> Anchor {
    session.document().annotation(id).unwrap().anchor.clone()
}

fn applied(outcome: Outcome) -> joyword_engine::ApplyResult {
    match outcome {
        Outcome::Applied(result) => *result,
        Outcome::NoOp(_) => panic!("expected a change"),
    }
}

// ---------------------------------------------------------------------------
// Reference model

#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Char(char),
    Mark(usize, Bias),
}

#[derive(Clone, Debug)]
struct Reference {
    paragraphs: Vec<Vec<Item>>,
    collapsed: Vec<bool>,
}

impl Reference {
    fn chars(paragraph: &[Item]) -> usize {
        paragraph
            .iter()
            .filter(|item| matches!(item, Item::Char(_)))
            .count()
    }

    /// List index of the gap before character `offset` (after any markers
    /// that precede that character).
    fn gap(paragraph: &[Item], offset: usize) -> usize {
        let mut seen = 0;
        for (index, item) in paragraph.iter().enumerate() {
            if let Item::Char(_) = item {
                if seen == offset {
                    return index;
                }
                seen += 1;
            }
        }
        paragraph.len()
    }

    /// Markers sitting exactly at `offset`, and the list range they occupy.
    fn marks_at(paragraph: &[Item], offset: usize) -> (usize, usize) {
        let end = Self::gap(paragraph, offset);
        let mut start = end;
        while start > 0 && matches!(paragraph[start - 1], Item::Mark(..)) {
            start -= 1;
        }
        (start, end)
    }

    fn insert(&mut self, paragraph: usize, offset: usize, text: &[char]) {
        let items = &mut self.paragraphs[paragraph];
        let (start, end) = Self::marks_at(items, offset);
        let marks: Vec<Item> = items.drain(start..end).collect();
        let before = marks
            .iter()
            .filter(|m| matches!(m, Item::Mark(_, Bias::Before)));
        let after = marks
            .iter()
            .filter(|m| matches!(m, Item::Mark(_, Bias::After)));
        let replacement: Vec<Item> = before
            .copied()
            .chain(text.iter().map(|&ch| Item::Char(ch)))
            .chain(after.copied())
            .collect();
        items.splice(start..start, replacement);
    }

    fn collapse_marks(&mut self, items: &[Item]) {
        for item in items {
            if let Item::Mark(id, _) = item {
                self.collapsed[*id] = true;
            }
        }
    }

    /// Deletes characters `start..end` of one paragraph. Markers strictly
    /// inside stay where the characters were and are flagged collapsed.
    fn delete(&mut self, paragraph: usize, start: usize, end: usize) {
        let items = self.paragraphs[paragraph].clone();
        let mut kept = Vec::new();
        let mut seen = 0;
        let mut inside = Vec::new();
        for item in items {
            match item {
                Item::Char(_) => {
                    if !(start..end).contains(&seen) {
                        kept.push(item);
                    }
                    seen += 1;
                }
                Item::Mark(..) => {
                    if seen > start && seen < end {
                        inside.push(item);
                    }
                    kept.push(item);
                }
            }
        }
        self.collapse_marks(&inside);
        self.paragraphs[paragraph] = kept;
    }

    /// Deletes from `start` in `first` to `end` in `last`, including the
    /// paragraph marks between, then joins what remains.
    fn delete_across(&mut self, first: usize, start: usize, last: usize, end: usize) {
        let mut head = Vec::new();
        let mut collapsed = Vec::new();
        let mut seen = 0;
        for item in self.paragraphs[first].clone() {
            match item {
                Item::Char(_) => {
                    if seen < start {
                        head.push(item);
                    }
                    seen += 1;
                }
                Item::Mark(..) if seen <= start => head.push(item),
                Item::Mark(..) => collapsed.push(item),
            }
        }
        for middle in &self.paragraphs[first + 1..last] {
            collapsed.extend(middle.iter().filter(|item| matches!(item, Item::Mark(..))));
        }
        let mut tail = Vec::new();
        let mut seen = 0;
        for item in self.paragraphs[last].clone() {
            match item {
                Item::Char(_) => {
                    if seen >= end {
                        tail.push(item);
                    }
                    seen += 1;
                }
                Item::Mark(..) if seen < end => collapsed.push(item),
                Item::Mark(..) => tail.push(item),
            }
        }
        self.collapse_marks(&collapsed);
        head.extend(collapsed);
        head.extend(tail);
        self.paragraphs[first] = head;
        self.paragraphs.drain(first + 1..=last);
    }

    fn split(&mut self, paragraph: usize, offset: usize) {
        let items = self.paragraphs[paragraph].clone();
        let (marks_start, marks_end) = Self::marks_at(&items, offset);
        let mut left: Vec<Item> = items[..marks_start].to_vec();
        let mut right = Vec::new();
        for item in &items[marks_start..marks_end] {
            match item {
                Item::Mark(_, Bias::Before) => left.push(*item),
                _ => right.push(*item),
            }
        }
        right.extend_from_slice(&items[marks_end..]);
        self.paragraphs[paragraph] = left;
        self.paragraphs.insert(paragraph + 1, right);
    }

    fn text(&self, paragraph: usize) -> String {
        self.paragraphs[paragraph]
            .iter()
            .filter_map(|item| match item {
                Item::Char(ch) => Some(*ch),
                Item::Mark(..) => None,
            })
            .collect()
    }

    /// (paragraph index, offset) of marker `id`.
    fn find(&self, id: usize) -> (usize, usize) {
        for (index, items) in self.paragraphs.iter().enumerate() {
            let mut seen = 0;
            for item in items {
                match item {
                    Item::Char(_) => seen += 1,
                    Item::Mark(mark, _) if *mark == id => return (index, seen),
                    Item::Mark(..) => {}
                }
            }
        }
        panic!("marker {id} vanished from the reference")
    }
}

#[test]
fn position_mapping_matches_reference_model_under_random_edits() {
    for seed in 1..=40 {
        let mut rng = Lcg(seed);
        let lines = [
            "甲方应在十日内付款",
            "Party B shall deliver",
            "第三条 违约责任",
            "附件",
        ];
        let mut session = Session::new(document(&lines));
        let mut reference = Reference {
            paragraphs: lines
                .iter()
                .map(|line| line.chars().map(Item::Char).collect())
                .collect(),
            collapsed: Vec::new(),
        };
        // Scatter markers with both biases.
        let mut markers: Vec<Position> = Vec::new();
        for id in 0..24 {
            let paragraph = rng.next(lines.len());
            let len = Reference::chars(&reference.paragraphs[paragraph]);
            let offset = rng.next(len + 1);
            let bias = if rng.next(2) == 0 {
                Bias::Before
            } else {
                Bias::After
            };
            let gap = Reference::gap(&reference.paragraphs[paragraph], offset);
            reference.paragraphs[paragraph].insert(gap, Item::Mark(id, bias));
            reference.collapsed.push(false);
            let ids = paragraph_ids(&session);
            markers.push(pos(&session, ids[paragraph], offset, bias));
        }
        let mut collapsed = vec![false; markers.len()];

        for step in 0..120 {
            let ids = paragraph_ids(&session);
            let count = ids.len();
            let len_of =
                |reference: &Reference, p: usize| Reference::chars(&reference.paragraphs[p]);
            let command = match rng.next(4) {
                0 => {
                    let p = rng.next(count);
                    let offset = rng.next(len_of(&reference, p) + 1);
                    let text: Vec<char> = ["改", "ab", "🙂", "e\u{301}"][rng.next(4)]
                        .chars()
                        .collect();
                    reference.insert(p, offset, &text);
                    Command::InsertText {
                        at: pos(&session, ids[p], offset, Bias::After),
                        text: text.iter().collect(),
                    }
                }
                1 => {
                    let p = rng.next(count);
                    let len = len_of(&reference, p);
                    let start = rng.next(len + 1);
                    let end = start + rng.next(len - start + 1);
                    reference.delete(p, start, end);
                    Command::DeleteRange {
                        start: pos(&session, ids[p], start, Bias::After),
                        end: pos(&session, ids[p], end, Bias::Before),
                    }
                }
                2 if count > 1 => {
                    let first = rng.next(count - 1);
                    let last = first + 1 + rng.next((count - first - 1).min(2));
                    let start = rng.next(len_of(&reference, first) + 1);
                    let end = rng.next(len_of(&reference, last) + 1);
                    reference.delete_across(first, start, last, end);
                    Command::DeleteRange {
                        start: pos(&session, ids[first], start, Bias::After),
                        end: pos(&session, ids[last], end, Bias::Before),
                    }
                }
                _ => {
                    let p = rng.next(count);
                    let offset = rng.next(len_of(&reference, p) + 1);
                    reference.split(p, offset);
                    Command::SplitParagraph {
                        at: pos(&session, ids[p], offset, Bias::After),
                    }
                }
            };
            let outcome = session
                .exec(vec![command.clone()])
                .unwrap_or_else(|error| panic!("seed {seed} step {step}: {command:?}: {error}"));
            if let Outcome::Applied(result) = outcome {
                for (marker, flag) in markers.iter_mut().zip(collapsed.iter_mut()) {
                    match result.map.map(*marker) {
                        Mapped::Kept(next) => *marker = next,
                        Mapped::Collapsed(next) => {
                            *marker = next;
                            *flag = true;
                        }
                        Mapped::Lost => panic!("seed {seed}: a marker was lost"),
                    }
                }
            }
            let ids = paragraph_ids(&session);
            let reference_texts: Vec<String> = (0..reference.paragraphs.len())
                .map(|p| reference.text(p))
                .collect();
            assert_eq!(
                texts(&session),
                reference_texts,
                "seed {seed} step {step}: {command:?}"
            );
            for (id, marker) in markers.iter().enumerate() {
                let (paragraph, offset) = reference.find(id);
                assert_eq!(
                    (marker.paragraph, marker.offset.0),
                    (ids[paragraph], offset),
                    "seed {seed} step {step} marker {id}: {command:?}"
                );
                assert_eq!(
                    collapsed[id], reference.collapsed[id],
                    "seed {seed} marker {id} collapse"
                );
            }
        }
        assert!(!session.document().validate().has_errors(), "seed {seed}");
    }
}

// ---------------------------------------------------------------------------
// Atomicity, versions and undo

fn random_command(session: &Session, rng: &mut Lcg) -> Command {
    let ids = paragraph_ids(session);
    let lens: Vec<usize> = session
        .document()
        .body()
        .blocks
        .iter()
        .map(|block| block.as_paragraph().map_or(0, Paragraph::len))
        .collect();
    let p = rng.next(ids.len());
    let offset = rng.next(lens[p] + 1);
    match rng.next(6) {
        0 => Command::InsertText {
            at: pos(session, ids[p], offset, Bias::After),
            text: "新增".into(),
        },
        1 => Command::SplitParagraph {
            at: pos(session, ids[p], offset, Bias::After),
        },
        2 => Command::InsertInline {
            at: pos(session, ids[p], offset, Bias::After),
            kind: InlineKind::Break(BreakKind::Line),
        },
        3 if ids.len() > 1 && p + 1 < ids.len() => Command::DeleteRange {
            start: pos(session, ids[p], offset, Bias::After),
            end: pos(session, ids[p + 1], rng.next(lens[p + 1] + 1), Bias::Before),
        },
        4 => Command::FormatText {
            start: pos(session, ids[p], 0, Bias::After),
            end: pos(session, ids[p], offset, Bias::Before),
            change: RunChange::Bold(Some(rng.next(2) == 0)),
        },
        _ => Command::FormatParagraph {
            story: session.document().body_id(),
            paragraph: ids[p],
            change: ParagraphChange::Justification(Some(Justification::Center)),
        },
    }
}

#[test]
fn undo_and_redo_restore_exact_states_with_monotonic_versions() {
    for seed in 1..=10 {
        let mut rng = Lcg(seed);
        let mut session = Session::new(document(&["第一段", "第二段 second", "第三段"]));
        let mut states = vec![session.document().canonical_dump(DUMP)];
        let mut versions = vec![session.version()];
        for _ in 0..30 {
            let command = random_command(&session, &mut rng);
            if let Outcome::Applied(result) = session.exec(vec![command]).unwrap() {
                assert!(result.version > *versions.last().unwrap());
                versions.push(result.version);
                states.push(session.document().canonical_dump(DUMP));
            }
        }
        let mut last_version = session.version();
        // Undo everything, checking each intermediate state.
        for expected in states.iter().rev().skip(1) {
            let undone = session.undo().unwrap();
            assert!(undone.version > last_version, "versions never go back");
            last_version = undone.version;
            assert_eq!(
                &session.document().canonical_dump(DUMP),
                expected,
                "seed {seed}"
            );
        }
        assert_eq!(session.undo(), Err(UndoError::Nothing));
        for expected in states.iter().skip(1) {
            let redone = session.redo().unwrap();
            assert!(redone.version > last_version);
            last_version = redone.version;
            assert_eq!(
                &session.document().canonical_dump(DUMP),
                expected,
                "seed {seed}"
            );
        }
        assert!(!session.can_redo());
        // A new identifier is never one that undo discarded.
        session.undo().unwrap();
        let ids_before: Vec<NodeId> = paragraph_ids(&session);
        let first = ids_before[0];
        let result = applied(
            session
                .exec(vec![Command::SplitParagraph {
                    at: pos(&session, first, 0, Bias::After),
                }])
                .unwrap(),
        );
        let new_id = *result.changes.inserted.iter().next().unwrap();
        assert!(
            !states
                .iter()
                .any(|dump| dump.contains(&format!("paragraph {new_id} ")))
        );
        assert!(!session.can_redo(), "a new edit clears redo");
    }
}

#[test]
fn failing_transaction_leaves_no_trace() {
    let mut session = Session::new(document(&["甲方", "乙方"]));
    let before = session.document().canonical_dump(DUMP);
    let version = session.version();
    let ids = paragraph_ids(&session);
    let error = session
        .exec(vec![
            Command::InsertText {
                at: pos(&session, ids[0], 2, Bias::After),
                text: "已签字".into(),
            },
            Command::SplitParagraph {
                at: pos(&session, ids[1], 0, Bias::After),
            },
            Command::InsertText {
                at: pos(&session, ids[1], 99, Bias::After),
                text: "越界".into(),
            },
        ])
        .unwrap_err();
    assert!(
        matches!(error, TxError::Command { index: 2, .. }),
        "{error:?}"
    );
    assert_eq!(session.document().canonical_dump(DUMP), before);
    assert_eq!(session.version(), version);
    assert!(!session.can_undo());

    // Stale base version.
    applied(
        session
            .exec(vec![Command::InsertText {
                at: pos(&session, ids[0], 0, Bias::After),
                text: "（".into(),
            }])
            .unwrap(),
    );
    let stale = session.apply(Transaction::new(
        version,
        vec![Command::InsertText {
            at: pos(&session, ids[0], 0, Bias::After),
            text: "x".into(),
        }],
    ));
    assert!(matches!(stale, Err(TxError::StaleVersion { .. })));

    // Invalid text and reversed ranges.
    for (command, expected) in [
        (
            Command::InsertText {
                at: pos(&session, ids[0], 0, Bias::After),
                text: "a\nb".into(),
            },
            CommandError::InvalidText('\n'),
        ),
        (
            Command::DeleteRange {
                start: pos(&session, ids[1], 1, Bias::After),
                end: pos(&session, ids[0], 0, Bias::Before),
            },
            CommandError::ReversedRange,
        ),
    ] {
        let error = session.exec(vec![command]).unwrap_err();
        assert_eq!(
            error,
            TxError::Command {
                index: 0,
                error: expected
            }
        );
    }
}

#[test]
fn no_op_transactions_do_not_advance_the_version() {
    let mut session = Session::new(document(&["正文"]));
    let ids = paragraph_ids(&session);
    let version = session.version();
    for command in [
        Command::InsertText {
            at: pos(&session, ids[0], 1, Bias::After),
            text: String::new(),
        },
        Command::DeleteRange {
            start: pos(&session, ids[0], 1, Bias::After),
            end: pos(&session, ids[0], 1, Bias::Before),
        },
        Command::FormatText {
            start: pos(&session, ids[0], 0, Bias::After),
            end: pos(&session, ids[0], 2, Bias::Before),
            change: RunChange::Bold(None),
        },
        Command::MoveBlock {
            story: session.document().body_id(),
            block: ids[0],
            before: None,
        },
    ] {
        assert_eq!(session.exec(vec![command]).unwrap(), Outcome::NoOp(version));
    }
    assert!(!session.can_undo());
}

#[test]
fn policy_blocks_read_only_documents_and_disabled_agent_writes() {
    let mut session = Session::new(document(&["受保护文档"]));
    let ids = paragraph_ids(&session);
    let insert = Command::InsertText {
        at: pos(&session, ids[0], 0, Bias::After),
        text: "x".into(),
    };
    let mut agent = Transaction::new(session.version(), vec![insert.clone()]);
    agent.origin = Origin::Agent;
    assert_eq!(
        session.apply(agent.clone()),
        Err(TxError::AgentWritesDisabled)
    );
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: true,
    });
    assert!(session.apply(agent).is_ok());
    session.set_policy(EditPolicy {
        read_only: true,
        agent_writes: true,
    });
    assert_eq!(session.exec(vec![insert]), Err(TxError::ReadOnly));
    assert_eq!(session.undo(), Err(UndoError::ReadOnly));
}

#[test]
fn preconditions_reject_proposals_built_on_changed_text() {
    let mut session = Session::new(document(&["付款期限为十个工作日"]));
    let ids = paragraph_ids(&session);
    let story = session.document().body_id();
    let precondition = Precondition::Text {
        story,
        paragraph: ids[0],
        start: ScalarOffset(5),
        end: ScalarOffset(10),
        expected: "十个工作日".into(),
    };
    let replace = vec![
        Command::DeleteRange {
            start: pos(&session, ids[0], 5, Bias::After),
            end: pos(&session, ids[0], 10, Bias::Before),
        },
        Command::InsertText {
            at: pos(&session, ids[0], 5, Bias::After),
            text: "五个工作日".into(),
        },
    ];
    let mut transaction = Transaction::new(session.version(), replace.clone());
    transaction.preconditions = vec![precondition.clone()];
    applied(session.apply(transaction).unwrap());
    assert_eq!(texts(&session), vec!["付款期限为五个工作日"]);
    let mut again = Transaction::new(session.version(), replace);
    again.preconditions = vec![precondition];
    assert_eq!(
        session.apply(again),
        Err(TxError::PreconditionFailed { index: 0 })
    );
}

// ---------------------------------------------------------------------------
// Annotations, selection, structure

#[test]
fn bookmarks_and_comments_follow_cross_paragraph_edits_and_undo() {
    let session = Session::new(document(&[
        "原稿：甲方应在十个工作日内支付",
        "全部款项。",
        "附则",
    ]));
    let ids = paragraph_ids(&session);
    let story = session.document().body_id();
    let mut doc = session.document().clone();
    let comment_body = {
        let mut paragraph = Paragraph::new(doc.ids().node());
        paragraph
            .push_text("请确认期限", None, Arc::default())
            .unwrap();
        doc.insert_story(StoryKind::Comment, vec![paragraph.into()])
    };
    let at = |paragraph, offset, bias| Position {
        story,
        paragraph,
        offset: ScalarOffset(offset),
        bias,
    };
    let comment = doc.insert_annotation(
        AnnotationKind::Comment { body: comment_body },
        Anchor::Range {
            start: at(ids[0], 7, Bias::After),
            end: at(ids[0], 12, Bias::Before),
        },
    );
    let bookmark = doc.insert_annotation(
        AnnotationKind::Bookmark {
            name: "款项".into(),
        },
        Anchor::Range {
            start: at(ids[1], 0, Bias::After),
            end: at(ids[1], 4, Bias::Before),
        },
    );
    let field = doc.insert_annotation(
        AnnotationKind::Field {
            separator: Some(at(ids[1], 2, Bias::After)),
        },
        Anchor::Range {
            start: at(ids[0], 0, Bias::After),
            end: at(ids[2], 2, Bias::Before),
        },
    );
    let mut session = Session::new(doc);
    let before = session.document().canonical_dump(DUMP);

    // Join paragraphs 0 and 1 by deleting "支付" + paragraph mark + "全部".
    let result = applied(
        session
            .exec(vec![Command::DeleteRange {
                start: at(ids[0], 13, Bias::After),
                end: at(ids[1], 2, Bias::Before),
            }])
            .unwrap(),
    );
    assert_eq!(
        texts(&session),
        vec!["原稿：甲方应在十个工作日内款项。", "附则"]
    );
    assert!(result.changes.annotations.contains(&bookmark));
    // The comment anchor is untouched; the bookmark moved into paragraph 0.
    assert_eq!(
        anchor_of(&session, comment),
        Anchor::Range {
            start: at(ids[0], 7, Bias::After),
            end: at(ids[0], 12, Bias::Before)
        }
    );
    assert_eq!(
        anchor_of(&session, bookmark),
        Anchor::Range {
            start: at(ids[0], 13, Bias::After),
            end: at(ids[0], 15, Bias::Before)
        }
    );
    let Some(AnnotationKind::Field {
        separator: Some(separator),
    }) = session.document().annotation(field).map(|a| a.kind.clone())
    else {
        panic!("field keeps its separator")
    };
    assert_eq!((separator.paragraph, separator.offset.0), (ids[0], 13));

    // Deleting the comment's whole anchor detaches it instead of deleting it.
    applied(
        session
            .exec(vec![Command::DeleteRange {
                start: at(ids[0], 7, Bias::After),
                end: at(ids[0], 12, Bias::Before),
            }])
            .unwrap(),
    );
    assert_eq!(anchor_of(&session, comment), Anchor::Detached);
    assert!(session.document().annotation(comment).is_some());
    assert!(!session.document().validate().has_errors());

    session.undo().unwrap();
    session.undo().unwrap();
    assert_eq!(session.document().canonical_dump(DUMP), before);
}

#[test]
fn selection_is_mapped_and_restored_by_undo() {
    let mut session = Session::new(document(&["标题", "正文内容"]));
    let ids = paragraph_ids(&session);
    let caret = Selection::caret(pos(&session, ids[1], 2, Bias::After));
    let mut transaction = Transaction::new(
        session.version(),
        vec![Command::SplitParagraph {
            at: pos(&session, ids[1], 2, Bias::After),
        }],
    );
    transaction.selection = Some(caret);
    let result = applied(session.apply(transaction).unwrap());
    let new = *result.changes.inserted.iter().next().unwrap();
    let after = result.selection.unwrap();
    assert_eq!((after.head.paragraph, after.head.offset.0), (new, 0));
    assert_eq!(session.undo().unwrap().selection, Some(caret));
    assert_eq!(session.redo().unwrap().selection, Some(after));
}

#[test]
fn locked_content_controls_and_containers_are_respected() {
    let mut doc = document(&["正文"]);
    let ids_body = doc.body().blocks.iter().map(Block::id).collect::<Vec<_>>();
    let ids = doc.ids();
    let inner = |ids: &mut joyword_model::IdAllocator, text: &str| {
        let mut paragraph = Paragraph::new(ids.node());
        paragraph.push_text(text, None, Arc::default()).unwrap();
        Block::from(paragraph)
    };
    let locked_text = inner(ids, "不可编辑");
    let locked_id = locked_text.id();
    let sdt = Sdt {
        id: ids.node(),
        props: SdtProps {
            lock: SdtLock::ContentLocked,
            ..SdtProps::default()
        },
        blocks: BlockSeq::from_vec(vec![locked_text]),
        source: None,
    };
    let sdt_id = sdt.id;
    let cell_text = inner(ids, "单元格");
    let cell_id = cell_text.id();
    let table = Table {
        id: ids.node(),
        grid: vec![Measure::Twips(3_000)],
        rows: vec![Row {
            id: ids.node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![Cell {
                id: ids.node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: None,
                blocks: BlockSeq::from_vec(vec![cell_text]),
                preserved: None,
            }],
            header: false,
            preserved: None,
        }],
        style: None,
        preserved: None,
        source: None,
    };
    let body = doc.body_id();
    let story = doc.story_mut(body).unwrap();
    story.blocks.push(sdt.into());
    story.blocks.push(table.into());
    let mut session = Session::new(doc);
    let at = |paragraph, offset| Position {
        story: body,
        paragraph,
        offset: ScalarOffset(offset),
        bias: Bias::After,
    };

    let error = session.exec(vec![Command::InsertText {
        at: at(locked_id, 0),
        text: "x".into(),
    }]);
    assert_eq!(
        error,
        Err(TxError::Command {
            index: 0,
            error: CommandError::Locked(locked_id)
        })
    );
    let error = session.exec(vec![Command::RemoveBlock {
        story: body,
        block: sdt_id,
    }]);
    assert_eq!(
        error,
        Err(TxError::Command {
            index: 0,
            error: CommandError::Locked(sdt_id)
        })
    );
    let error = session.exec(vec![Command::DeleteRange {
        start: at(ids_body[0], 0),
        end: at(cell_id, 1),
    }]);
    assert_eq!(
        error,
        Err(TxError::Command {
            index: 0,
            error: CommandError::CrossContainer
        })
    );

    // Editing inside the table cell works and copies only the touched path.
    let before = session.snapshot();
    applied(
        session
            .exec(vec![Command::InsertText {
                at: at(cell_id, 3),
                text: "内容".into(),
            }])
            .unwrap(),
    );
    let cell = session.document().find_paragraph(body, cell_id).unwrap();
    assert_eq!(cell.plain_text(|_| String::new()), "单元格内容");
    assert!(
        session
            .document()
            .body()
            .blocks
            .unshared_nodes(&before.body().blocks)
            <= 2
    );
    let unchanged = before.find_paragraph(body, cell_id).unwrap();
    assert_eq!(unchanged.plain_text(|_| String::new()), "单元格");
}

#[test]
fn moving_and_removing_blocks_keep_identity_and_collapse_positions() {
    let mut session = Session::new(document(&["一", "二", "三", "四"]));
    let ids = paragraph_ids(&session);
    let story = session.document().body_id();
    applied(
        session
            .exec(vec![Command::MoveBlock {
                story,
                block: ids[3],
                before: Some(ids[0]),
            }])
            .unwrap(),
    );
    assert_eq!(
        paragraph_ids(&session),
        vec![ids[3], ids[0], ids[1], ids[2]]
    );
    let result = applied(
        session
            .exec(vec![Command::RemoveBlock {
                story,
                block: ids[1],
            }])
            .unwrap(),
    );
    let inside = pos(&session, ids[1], 1, Bias::Before);
    assert_eq!(
        result.map.map(inside),
        Mapped::Collapsed(pos(&session, ids[2], 0, Bias::Before))
    );
    let ChangeSet { removed, .. } = result.changes;
    assert!(removed.contains(&ids[1]));
    for block in &ids[..] {
        if *block == ids[1] {
            continue;
        }
        let _ = session.exec(vec![Command::RemoveBlock {
            story,
            block: *block,
        }]);
    }
    assert_eq!(
        paragraph_ids(&session).len(),
        1,
        "the last block cannot be removed"
    );
    assert!(!session.document().validate().has_errors());
}

#[test]
fn inline_objects_and_formatting_round_trip_through_undo() {
    let mut session = Session::new(document(&["签字日期"]));
    let ids = paragraph_ids(&session);
    let original = session.document().canonical_dump(DUMP);
    applied(
        session
            .exec(vec![
                Command::InsertInline {
                    at: pos(&session, ids[0], 2, Bias::After),
                    kind: InlineKind::Break(BreakKind::Line),
                },
                Command::FormatText {
                    start: pos(&session, ids[0], 0, Bias::After),
                    end: pos(&session, ids[0], 2, Bias::Before),
                    change: RunChange::Bold(Some(true)),
                },
            ])
            .unwrap(),
    );
    assert_eq!(texts(&session), vec!["签字□日期"]);
    let dump = session
        .document()
        .canonical_dump(DumpOptions { node_ids: false });
    assert!(dump.contains("run style=- props={b=1} \"签字\""), "{dump}");
    let image = session.exec(vec![Command::InsertInline {
        at: pos(&session, ids[0], 0, Bias::After),
        kind: InlineKind::Image {
            resource: session.document().clone().ids().resource(),
            width: Measure::Emu(1),
            height: Measure::Emu(1),
        },
    }]);
    assert_eq!(
        image,
        Err(TxError::Command {
            index: 0,
            error: CommandError::MissingResource
        })
    );
    session.undo().unwrap();
    assert_eq!(session.document().canonical_dump(DUMP), original);
}
