//! T209 input-source undo groups at the shell boundary. Keyboard typing and
//! keyboard deletion coalesce; Enter, paste, IME commits, commands and caret
//! moves are boundaries (SPEC §6, behavior registry `EDIT-UNDO-*`).

use joyoffice_opc::Budget;
use joyword_engine::{MAX_UNDO_GROUP_STEPS, Session};
use joyword_model::{BlockSeq, Document, Paragraph};
use joyword_shell::docx::DocxSession;
use joyword_shell::input::{Direction, InputBackend, InputLimits, InputSource, TextInput};
use std::sync::Arc;

fn input(text: &str) -> TextInput {
    let mut doc = Document::new_blank();
    let mut blocks = Vec::new();
    for line in text.split('\n') {
        let mut p = Paragraph::new(doc.ids().node());
        p.push_text(line, None, Arc::default()).unwrap();
        blocks.push(p.into());
    }
    let body = doc.body_id();
    doc.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    TextInput::new(Session::new(doc), InputLimits::default()).unwrap()
}

fn body<B: InputBackend>(input: &TextInput<B>) -> String {
    input.view().index().text().to_owned()
}

fn keys<B: InputBackend>(input: &mut TextInput<B>, text: &str) {
    for ch in text.chars() {
        input.replace(None, &ch.to_string()).unwrap();
    }
}

fn depth<B: InputBackend>(input: &TextInput<B>) -> usize {
    input.backend().session().undo_depth()
}

/// Undo everything, returning the body after each step.
fn unwind<B: InputBackend>(input: &mut TextInput<B>) -> Vec<String> {
    let mut seen = Vec::new();
    while input.can_undo() {
        input.undo().unwrap();
        seen.push(body(input));
    }
    seen
}

#[test]
fn keystrokes_are_one_undo_step_restoring_the_starting_caret() {
    let mut i = input("前");
    i.select(1..1, false).unwrap();
    let start = i.view().selection();
    keys(&mut i, "abc中文");
    assert_eq!(body(&i), "前abc中文");
    assert_eq!(depth(&i), 1);
    let end = i.view().selection();
    i.undo().unwrap();
    assert_eq!(body(&i), "前");
    assert_eq!(i.view().selection(), start);
    assert!(!i.can_undo());
    i.redo().unwrap();
    assert_eq!(body(&i), "前abc中文");
    assert_eq!(i.view().selection(), end);
}

#[test]
fn enter_paste_commands_and_ime_commits_are_separate_steps() {
    let mut i = input("");
    keys(&mut i, "ab");
    i.replace(None, "\n").unwrap();
    keys(&mut i, "cd");
    i.replace_from(InputSource::Paste, None, "x\ny").unwrap();
    keys(&mut i, "e");
    i.replace_from(InputSource::Paste, None, "P").unwrap();
    keys(&mut i, "f");
    // AppKit unmark commits the marked text.
    i.mark(None, "ni", None).unwrap();
    i.mark(None, "你", None).unwrap();
    i.unmark().unwrap();
    keys(&mut i, "g");
    // insertText while composing also commits the composition.
    i.mark(None, "hao", None).unwrap();
    i.replace(None, "好").unwrap();
    keys(&mut i, "h");
    i.replace_from(InputSource::Command, None, "C").unwrap();
    keys(&mut i, "j");
    assert_eq!(body(&i), "ab\ncdx\nyePf你g好hCj");
    assert_eq!(depth(&i), 13);
    assert_eq!(
        unwind(&mut i),
        [
            "ab\ncdx\nyePf你g好hC",
            "ab\ncdx\nyePf你g好h",
            "ab\ncdx\nyePf你g好",
            "ab\ncdx\nyePf你g",
            "ab\ncdx\nyePf你",
            "ab\ncdx\nyePf",
            "ab\ncdx\nyeP",
            "ab\ncdx\nye",
            "ab\ncdx\ny",
            "ab\ncd",
            "ab\n",
            "ab",
            "",
        ]
    );
}

#[test]
fn moving_the_caret_away_and_back_starts_a_new_step() {
    let mut i = input("");
    keys(&mut i, "ab");
    i.move_grapheme(Direction::Backward, false).unwrap();
    i.move_grapheme(Direction::Forward, false).unwrap();
    keys(&mut i, "c");
    assert_eq!(depth(&i), 2);
    // Re-asserting the current selection is not a move.
    let here = i.view().selection();
    i.select_positions(here).unwrap();
    keys(&mut i, "d");
    assert_eq!(depth(&i), 2);
    // A click elsewhere is.
    i.select(0..0, false).unwrap();
    keys(&mut i, "z");
    assert_eq!(body(&i), "zabcd");
    assert_eq!(unwind(&mut i), ["abcd", "ab", ""]);
}

#[test]
fn deletion_coalesces_separately_from_typing() {
    let mut i = input("abcdef");
    i.select(4..4, false).unwrap();
    i.delete(Direction::Backward).unwrap();
    i.delete(Direction::Backward).unwrap();
    i.delete(Direction::Forward).unwrap();
    assert_eq!(body(&i), "abf");
    assert_eq!(depth(&i), 1);
    keys(&mut i, "xy");
    i.delete(Direction::Backward).unwrap();
    assert_eq!(body(&i), "abxf");
    assert_eq!(unwind(&mut i), ["abxyf", "abf", "abcdef"]);
}

#[test]
fn a_platform_replacement_elsewhere_is_standalone() {
    let mut i = input("");
    keys(&mut i, "ab");
    // The input client names the current selection: still typing.
    let here = i.view().selected_range().0;
    i.replace(Some(here), "c").unwrap();
    assert_eq!(depth(&i), 1);
    // Replacing another range (e.g. a correction) leaves the caret's group;
    // the caret follows the replacement, and typing there is a new step.
    i.replace(Some(0..1), "A").unwrap();
    keys(&mut i, "d");
    assert_eq!(body(&i), "Adbc");
    assert_eq!(unwind(&mut i), ["Abc", "abc", ""]);
}

#[test]
fn typing_over_a_selection_is_one_step_that_restores_the_selection() {
    let mut i = input("甲乙丙");
    i.select(0..2, true).unwrap();
    let selected = i.view().selection();
    keys(&mut i, "xyz");
    assert_eq!(body(&i), "xyz丙");
    assert_eq!(depth(&i), 1);
    i.undo().unwrap();
    assert_eq!(body(&i), "甲乙丙");
    assert_eq!(i.view().selection(), selected);
}

#[test]
fn undo_then_typing_and_long_bursts_start_new_steps() {
    let mut i = input("");
    keys(&mut i, "ab");
    i.undo().unwrap();
    keys(&mut i, "c");
    i.undo().unwrap();
    assert_eq!(body(&i), "");
    assert!(!i.can_undo());

    let burst = "字".repeat(MAX_UNDO_GROUP_STEPS + 3);
    keys(&mut i, &burst);
    assert_eq!(depth(&i), 2);
    i.undo().unwrap();
    assert_eq!(body(&i).chars().count(), MAX_UNDO_GROUP_STEPS);
}

#[test]
fn docx_backends_seal_groups_too() {
    let bytes = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
    let docx = DocxSession::open(bytes, &Budget::default()).unwrap();
    let mut i = TextInput::new(docx, InputLimits::default()).unwrap();
    let original = body(&i);
    keys(&mut i, "甲乙");
    i.select(0..0, false).unwrap();
    i.select(2..2, false).unwrap();
    keys(&mut i, "丙");
    assert_eq!(depth(&i), 2);
    i.undo().unwrap();
    i.undo().unwrap();
    assert_eq!(body(&i), original);
    assert_eq!(i.backend().export_plan().unwrap().bytes(), bytes);
}
