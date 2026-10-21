//! Word-processor caret units (Ctrl+arrows, Ctrl+Home/End), word deletion
//! (Ctrl+Backspace/Delete) and the Ctrl+B/I/U toggles at the shell boundary.

use joyword_engine::{EditPolicy, Session};
use joyword_model::{BlockSeq, Document, Paragraph, Underline};
use joyword_shell::input::{CaretUnit, Direction, InputBackend, InputLimits, RunToggle, TextInput};
use std::sync::Arc;

fn input(text: &str) -> TextInput {
    TextInput::new(session(text), InputLimits::default()).unwrap()
}

fn read_only(text: &str) -> TextInput {
    let mut session = session(text);
    session.set_policy(EditPolicy {
        read_only: true,
        agent_writes: false,
    });
    TextInput::new(session, InputLimits::default()).unwrap()
}

fn session(text: &str) -> Session {
    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();
    for line in text.split('\n') {
        let mut p = Paragraph::new(doc.ids().node());
        p.push_text(line, None, Arc::default()).unwrap();
        blocks.push(p.into());
    }
    let body = doc.body_id();
    doc.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    Session::new(doc)
}

fn body<B: InputBackend>(input: &TextInput<B>) -> String {
    input.view().index().text().to_owned()
}

fn caret<B: InputBackend>(input: &TextInput<B>) -> usize {
    let (range, _) = input.view().selected_range();
    assert!(range.is_empty(), "expected a caret, got {range:?}");
    range.start
}

/// Every stop `unit` makes from `start` in `direction` until it stops moving.
fn stops(text: &str, start: usize, unit: CaretUnit, direction: Direction) -> Vec<usize> {
    let mut i = input(text);
    i.select(start..start, false).unwrap();
    let mut seen = Vec::new();
    loop {
        i.move_by(unit, direction, false).unwrap();
        let here = caret(&i);
        if seen.last() == Some(&here) || (seen.is_empty() && here == start) {
            return seen;
        }
        seen.push(here);
    }
}

#[test]
fn word_moves_stop_at_word_starts_punctuation_and_paragraph_ends() {
    // h0 ,5 ␠6 w7 ␠␠12 n14 ¶18 s19
    let text = "hello, world  next\nsecond";
    assert_eq!(
        stops(text, 0, CaretUnit::Word, Direction::Forward),
        [5, 7, 14, 18, 19, 25]
    );
    assert_eq!(
        stops(text, 25, CaretUnit::Word, Direction::Backward),
        [19, 18, 14, 7, 5, 0]
    );
    // From inside a word, backward goes to that word's start.
    assert_eq!(stops(text, 9, CaretUnit::Word, Direction::Backward)[0], 7);
    // From inside a run of spaces, forward goes to the next word.
    assert_eq!(stops(text, 13, CaretUnit::Word, Direction::Forward)[0], 14);
}

#[test]
fn word_moves_treat_leading_spaces_and_empty_paragraphs_as_stops() {
    let text = "  lead\n\ntail";
    assert_eq!(
        stops(text, 0, CaretUnit::Word, Direction::Forward),
        [2, 6, 7, 8, 12]
    );
    assert_eq!(
        stops(text, 12, CaretUnit::Word, Direction::Backward),
        [8, 7, 6, 2, 0]
    );
}

#[test]
fn word_moves_keep_clusters_whole_and_step_through_ideographs() {
    let text = "a👩‍👩‍👧 b";
    let family = "👩‍👩‍👧".encode_utf16().count();
    let forward = stops(text, 0, CaretUnit::Word, Direction::Forward);
    assert_eq!(forward.last(), Some(&(text.encode_utf16().count())));
    for stop in &forward {
        assert!(*stop <= 1 || *stop > family, "inside a cluster: {stop}");
    }
    let cjk = stops("中文。测试", 0, CaretUnit::Word, Direction::Forward);
    assert_eq!(cjk.last(), Some(&5));
    assert!(cjk.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn extending_by_words_keeps_the_anchor_and_a_plain_move_collapses_first() {
    let mut i = input("one two three");
    i.select(0..0, false).unwrap();
    i.move_by(CaretUnit::Word, Direction::Forward, true)
        .unwrap();
    i.move_by(CaretUnit::Word, Direction::Forward, true)
        .unwrap();
    assert_eq!(i.view().selected_range(), (0..8, false));
    i.move_by(CaretUnit::Word, Direction::Backward, true)
        .unwrap();
    assert_eq!(i.view().selected_range(), (0..4, false));
    // A plain move starts from the selection edge in its direction.
    i.select(4..6, false).unwrap();
    i.move_by(CaretUnit::Word, Direction::Forward, false)
        .unwrap();
    assert_eq!(caret(&i), 8);
    i.select(4..6, false).unwrap();
    i.move_by(CaretUnit::Word, Direction::Backward, false)
        .unwrap();
    assert_eq!(caret(&i), 0);
}

#[test]
fn paragraph_and_document_moves_follow_word_processor_keys() {
    let text = "ab\ncd\nef";
    assert_eq!(
        stops(text, 1, CaretUnit::Paragraph, Direction::Forward),
        [3, 6, 8]
    );
    assert_eq!(
        stops(text, 7, CaretUnit::Paragraph, Direction::Backward),
        [6, 3, 0]
    );
    let mut i = input(text);
    i.select(4..4, false).unwrap();
    i.move_by(CaretUnit::Document, Direction::Forward, true)
        .unwrap();
    assert_eq!(i.view().selected_range(), (4..8, false));
    i.move_by(CaretUnit::Document, Direction::Backward, true)
        .unwrap();
    assert_eq!(i.view().selected_range(), (0..4, true));
    i.move_by(CaretUnit::Document, Direction::Forward, false)
        .unwrap();
    assert_eq!(caret(&i), 8);
}

#[test]
fn word_deletion_removes_to_the_word_stop_as_one_undo_step() {
    let mut i = input("hello world");
    i.select(11..11, false).unwrap();
    assert!(i.delete_word(Direction::Backward).unwrap());
    assert_eq!(body(&i), "hello ");
    assert!(i.delete_word(Direction::Backward).unwrap());
    assert_eq!(body(&i), "");
    assert!(!i.delete_word(Direction::Backward).unwrap());
    i.undo().unwrap();
    assert_eq!(body(&i), "hello world");
    assert!(!i.can_undo());

    let mut i = input("hello world");
    i.select(0..0, false).unwrap();
    i.delete_word(Direction::Forward).unwrap();
    assert_eq!(body(&i), "world");
    // A selection is deleted as it is, not widened to words.
    i.select(1..3, false).unwrap();
    i.delete_word(Direction::Forward).unwrap();
    assert_eq!(body(&i), "wld");
}

#[test]
fn word_deletion_at_a_paragraph_edge_joins_paragraphs() {
    let mut i = input("ab\ncd");
    i.select(3..3, false).unwrap();
    i.delete_word(Direction::Backward).unwrap();
    assert_eq!(body(&i), "abcd");
    let mut i = input("ab\ncd");
    i.select(2..2, false).unwrap();
    i.delete_word(Direction::Forward).unwrap();
    assert_eq!(body(&i), "abcd");
}

#[test]
fn read_only_documents_allow_word_moves_but_refuse_word_deletion() {
    let mut i = read_only("hello world");
    i.select(0..0, false).unwrap();
    i.move_by(CaretUnit::Word, Direction::Forward, false)
        .unwrap();
    assert_eq!(caret(&i), 6);
    assert!(i.delete_word(Direction::Forward).is_err());
    assert_eq!(body(&i), "hello world");
}

type RunFormat = (usize, Option<bool>, Option<bool>, Option<Underline>);

fn run_props(input: &TextInput) -> Vec<RunFormat> {
    input
        .backend()
        .document()
        .body()
        .blocks
        .iter()
        .flat_map(|block| block.as_paragraph().unwrap().runs().to_vec())
        .map(|run| {
            (
                run.len,
                run.props.bold,
                run.props.italic,
                run.props.underline,
            )
        })
        .collect()
}

#[test]
fn format_toggles_turn_on_for_mixed_selections_and_off_when_all_show_it() {
    let mut i = input("abcd");
    i.select(0..2, false).unwrap();
    assert!(i.toggle_runs(RunToggle::Bold).unwrap());
    assert_eq!(
        run_props(&i),
        [(2, Some(true), None, None), (2, None, None, None)]
    );
    // Partly bold: Word turns the whole selection bold.
    i.select(0..4, false).unwrap();
    i.toggle_runs(RunToggle::Bold).unwrap();
    assert_eq!(run_props(&i), [(4, Some(true), None, None)]);
    // All bold: the toggle clears it.
    i.toggle_runs(RunToggle::Bold).unwrap();
    assert_eq!(run_props(&i), [(4, Some(false), None, None)]);

    i.toggle_runs(RunToggle::Italic).unwrap();
    i.toggle_runs(RunToggle::Underline).unwrap();
    assert_eq!(
        run_props(&i),
        [(4, Some(false), Some(true), Some(Underline::Single))]
    );
    i.toggle_runs(RunToggle::Underline).unwrap();
    assert_eq!(
        run_props(&i),
        [(4, Some(false), Some(true), Some(Underline::None))]
    );
    // A caret changes nothing.
    i.select(1..1, false).unwrap();
    assert!(!i.toggle_runs(RunToggle::Italic).unwrap());
}

#[test]
fn format_toggles_span_paragraphs_and_respect_read_only() {
    let mut i = input("ab\ncd");
    i.select(1..4, false).unwrap();
    i.toggle_runs(RunToggle::Italic).unwrap();
    assert_eq!(
        run_props(&i),
        [
            (1, None, None, None),
            (1, None, Some(true), None),
            (1, None, Some(true), None),
            (1, None, None, None),
        ]
    );
    let mut i = read_only("ab\ncd");
    i.select(1..4, false).unwrap();
    assert!(i.toggle_runs(RunToggle::Italic).is_err());
}
