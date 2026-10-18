use joyoffice_opc::Budget;
use joyword_engine::{Command, EditPolicy, Session, Transaction};
use joyword_model::{Bias, BlockSeq, Document, DumpOptions, Paragraph, Position, ScalarOffset};
use joyword_shell::docx::DocxSession;
use joyword_shell::input::{Direction, InputError, InputLimits, InputSource, TextInput};
use std::sync::Arc;
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
fn input(text: &str) -> TextInput {
    TextInput::new(session(text), InputLimits::default()).unwrap()
}
fn body(input: &TextInput) -> String {
    input
        .backend()
        .document()
        .body()
        .blocks
        .iter()
        .map(|b| b.as_paragraph().unwrap().plain_text(|_| unreachable!()))
        .collect::<Vec<_>>()
        .join("\n")
}
fn assert_index(input: &TextInput, expected: &str) {
    let index = input.view().index();
    assert_eq!(index.text(), expected);
    assert!(!index.snapshot().validate().has_errors());
    let mut offset = 0;
    for c in expected.chars() {
        assert_eq!(
            index
                .offset(index.position(offset, Bias::After).unwrap())
                .unwrap(),
            offset
        );
        if c.len_utf16() == 2 {
            assert_eq!(
                index.position(offset + 1, Bias::After),
                Err(InputError::InsideSurrogate)
            );
        }
        offset += c.len_utf16();
    }
    assert_eq!(
        index
            .offset(index.position(offset, Bias::After).unwrap())
            .unwrap(),
        offset
    );
}
#[test]
fn utf16_ranges_and_model_positions_round_trip_through_paragraph_marks() {
    let input = input("甲🙂e\u{301}\n\n👩‍👩‍👧‍👦\n尾");
    assert_index(&input, input.view().index().text());
    assert_eq!(input.view().index().text_for_range(1..3).unwrap(), "🙂");
    assert!(input.view().index().text_for_range(2..3).is_err());
    assert!(
        input
            .view()
            .index()
            .text_for_range(std::ops::Range { start: 9, end: 2 })
            .is_err()
    );
    let start = input.view().index().position(0, Bias::After).unwrap();
    assert!(
        input
            .view()
            .index()
            .offset(Position {
                offset: ScalarOffset(usize::MAX),
                ..start
            })
            .is_err()
    );
}
#[test]
fn committed_multiline_replacement_uses_one_undo_and_restores_selections() {
    let mut i = input("abc\ndef\nghi");
    i.select(1..9, true).unwrap();
    let before = i.view().selection();
    i.replace(None, "甲\r\n\r乙\n🙂").unwrap();
    assert_index(&i, "a甲\n\n乙\n🙂hi");
    assert_eq!(body(&i), i.view().index().text());
    let after = i.view().selection();
    assert_eq!(i.view().selected_range().0, 8..8);
    i.undo().unwrap();
    assert_eq!(body(&i), "abc\ndef\nghi");
    assert_eq!(i.view().selection(), before);
    assert!(!i.can_undo());
    i.redo().unwrap();
    assert_eq!(i.view().selection(), after);
    assert_eq!(body(&i), "a甲\n\n乙\n🙂hi");
}
#[test]
fn repeated_composition_updates_never_commit_until_confirmed() {
    let mut i = input("甲乙");
    i.select(1..2, false).unwrap();
    let before = i
        .backend()
        .document()
        .canonical_dump(DumpOptions { node_ids: true });
    let version = i.backend().version();
    for text in ["n", "ni", "你"] {
        i.mark(None, text, None).unwrap();
        assert_eq!(i.view().index().text(), format!("甲{text}"));
        assert_eq!(i.backend().version(), version);
        assert_eq!(
            i.backend()
                .document()
                .canonical_dump(DumpOptions { node_ids: true }),
            before
        );
        assert!(!i.backend().can_undo());
        assert!(!i.has_unsaved_changes());
        assert!(i.view().marked_range().is_some());
    }
    i.replace(None, "你好").unwrap();
    assert_eq!(body(&i), "甲你好");
    assert!(i.has_unsaved_changes());
    assert!(!i.is_composing());
    i.undo().unwrap();
    assert_eq!(body(&i), "甲乙");
    assert_eq!(i.view().selected_range().0, 1..2);
    assert!(!i.backend().can_undo());
    i.redo().unwrap();
    assert_eq!(body(&i), "甲你好");
    assert_eq!(i.view().selected_range().0, 3..3);
}
#[test]
fn cancellation_and_empty_mark_restore_original_range_without_dirtying() {
    let mut i = input("abc");
    i.select(0..2, true).unwrap();
    let original = i.view().selection();
    let version = i.backend().version();
    i.mark(None, "汉", Some(0..1)).unwrap();
    assert_eq!(i.view().selected_range().0, 0..1);
    i.cancel_composition().unwrap();
    assert_eq!(i.view().selection(), original);
    assert_eq!(body(&i), "abc");
    assert_eq!(i.backend().version(), version);
    assert!(!i.can_undo());
    i.mark(None, "preedit", None).unwrap();
    i.mark(None, "", None).unwrap();
    assert!(!i.is_composing());
    assert_eq!(i.view().selection(), original);
    assert_eq!(i.view().index().text(), "abc");
    assert!(!i.unmark().unwrap());
    assert_eq!(i.backend().version(), version);
}
#[test]
fn unmark_preserves_preview_text_with_one_real_commit() {
    let mut i = input("abc");
    i.select(1..1, false).unwrap();
    i.mark(None, "拼音", None).unwrap();
    assert!(i.unmark().unwrap());
    assert_eq!(body(&i), "a拼音bc");
    assert!(!i.unmark().unwrap());
    i.undo().unwrap();
    assert_eq!(body(&i), "abc");
    assert!(!i.can_undo());
}
#[test]
fn partial_preedit_and_surrounding_range_updates_fold_into_one_transaction() {
    let mut i = input("abc");
    i.select(1..2, false).unwrap();
    i.mark(None, "XYZ", None).unwrap();
    i.mark(Some(2..3), "中", Some(1..1)).unwrap();
    assert_eq!(i.view().index().text(), "aX中Zc");
    assert_eq!(i.view().marked_range(), Some(2..3));
    i.replace(None, "文").unwrap();
    assert_eq!(body(&i), "aX文Zc");
    i.undo().unwrap();
    assert_eq!(body(&i), "abc");
    i.mark(Some(1..2), "Q", None).unwrap();
    i.mark(Some(0..3), "R", None).unwrap();
    assert_eq!(i.view().index().text(), "R");
    i.unmark().unwrap();
    assert_eq!(body(&i), "R");
    i.undo().unwrap();
    assert_eq!(body(&i), "abc");
}
#[test]
fn invalid_composition_ranges_and_controls_leave_every_state_unchanged() {
    let mut i = input("A🙂B");
    i.select(1..3, false).unwrap();
    i.mark(None, "XY", None).unwrap();
    let text = i.view().index().text().to_owned();
    let revision = i.view().revision();
    let version = i.backend().version();
    let selected = i.view().selection();
    for range in [
        4..100,
        std::ops::Range { start: 3, end: 1 },
        usize::MAX..usize::MAX,
    ] {
        assert!(i.mark(Some(range), "new", None).is_err());
    }
    assert!(i.mark(None, "🙂", Some(1..1)).is_err());
    assert!(i.replace(None, "\t").is_err());
    assert!(i.replace(None, "\0").is_err());
    assert_eq!(i.view().index().text(), text);
    assert_eq!(i.view().revision(), revision);
    assert_eq!(i.view().selection(), selected);
    assert_eq!(i.backend().version(), version);
    i.cancel_composition().unwrap();
    assert!(i.replace(Some(2..3), "x").is_err());
    assert_eq!(body(&i), "A🙂B");
}
#[test]
fn grapheme_navigation_and_deletion_keep_emoji_combining_and_flags_whole() {
    for cluster in ["e\u{301}", "👩‍👩‍👧‍👦", "🇨🇳", "👍🏽", "क्‍ष"]
    {
        let mut i = input(&format!("a{cluster}b"));
        let end = 1 + cluster.encode_utf16().count();
        i.select(end..end, false).unwrap();
        i.move_grapheme(Direction::Backward, false).unwrap();
        assert_eq!(i.view().selected_range().0, 1..1);
        i.move_grapheme(Direction::Forward, true).unwrap();
        assert_eq!(i.view().selected_range().0, 1..end);
        i.delete(Direction::Backward).unwrap();
        assert_eq!(body(&i), "ab");
        i.undo().unwrap();
        assert_eq!(body(&i), format!("a{cluster}b"));
        i.select(end..end, false).unwrap();
        i.delete(Direction::Backward).unwrap();
        assert_eq!(body(&i), "ab");
    }
}
#[test]
fn deletion_inside_a_scalar_valid_grapheme_expands_to_the_whole_cluster() {
    for direction in [Direction::Backward, Direction::Forward] {
        let mut i = input("ae\u{301}b");
        i.select(2..2, false).unwrap();
        i.delete(direction).unwrap();
        assert_eq!(body(&i), "ab");
        let mut i = input("ae\u{301}b");
        i.select(2..3, false).unwrap();
        i.delete(direction).unwrap();
        assert_eq!(body(&i), "ab");
    }
}
#[test]
fn paragraph_mark_deletion_merges_and_empty_edges_are_noops() {
    let mut i = input("a\n\nb");
    i.select(2..2, false).unwrap();
    i.delete(Direction::Backward).unwrap();
    assert_eq!(body(&i), "a\nb");
    i.select(1..1, false).unwrap();
    i.delete(Direction::Forward).unwrap();
    assert_eq!(body(&i), "ab");
    let mut i = input("");
    assert!(!i.delete(Direction::Backward).unwrap());
    assert!(!i.delete(Direction::Forward).unwrap());
    assert!(!i.has_unsaved_changes());
    i.replace(None, "\n\n").unwrap();
    assert_eq!(body(&i), "\n\n");
    i.undo().unwrap();
    assert_eq!(body(&i), "");
}
#[test]
fn read_only_engine_and_docx_backing_cannot_be_bypassed_by_preview() {
    let mut engine = session("protected");
    engine.set_policy(EditPolicy {
        read_only: true,
        agent_writes: false,
    });
    let mut i = TextInput::new(engine, InputLimits::default()).unwrap();
    assert_eq!(i.mark(None, "x", None), Err(InputError::ReadOnly));
    assert_eq!(i.replace(None, "x"), Err(InputError::ReadOnly));
    assert_eq!(body(&i), "protected");
    let bytes = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-005.docx");
    let docx = DocxSession::open(bytes, &Budget::default()).unwrap();
    let mut i = TextInput::new(docx, InputLimits::default()).unwrap();
    assert_eq!(i.mark(None, "x", None), Err(InputError::ReadOnly));
    assert_eq!(i.replace(None, "x"), Err(InputError::ReadOnly));
    assert_eq!(i.backend().unchanged_package_bytes().unwrap(), bytes);
}
#[test]
fn editable_docx_uses_the_same_input_and_keeps_the_export_contract() {
    let bytes = include_bytes!("../../../tests/fixtures/docx/DOCX-MIN-001.docx");
    let docx = DocxSession::open(bytes, &Budget::default()).unwrap();
    let mut i = TextInput::new(docx, InputLimits::default()).unwrap();
    let original = i.view().index().text().to_owned();
    i.mark(None, "xin", None).unwrap();
    assert_eq!(i.backend().unchanged_package_bytes().unwrap(), bytes);
    i.replace(None, "新").unwrap();
    assert!(i.backend().unchanged_package_bytes().is_err());
    let plan = i.backend().export_plan().unwrap();
    assert!(!plan.bytes().is_empty());
    i.undo().unwrap();
    assert_eq!(i.view().index().text(), original);
    assert_eq!(i.backend().export_plan().unwrap().bytes(), bytes);
}
#[test]
fn budgets_and_failed_edits_preserve_committed_and_composing_states() {
    let limits = InputLimits {
        max_insert_bytes: 4,
        max_composition_bytes: 3,
        max_commands: 2,
        max_text_bytes: 6,
        ..InputLimits::default()
    };
    let mut i = TextInput::new(session("abc"), limits).unwrap();
    i.mark(None, "x", None).unwrap();
    let revision = i.view().revision();
    assert!(i.mark(None, "long", None).is_err());
    assert!(i.replace(None, "toolong").is_err());
    assert!(i.replace(None, "a\nb\nc").is_err());
    assert_eq!(body(&i), "abc");
    assert_eq!(i.view().index().text(), "xabc");
    assert_eq!(i.view().revision(), revision);
    assert!(!i.backend().can_undo());
    i.cancel_composition().unwrap();
    assert!(i.replace(None, "1234").is_err());
    assert_eq!(body(&i), "abc");
}
#[test]
fn history_outside_the_input_budget_is_rejected_before_undo_mutates_it() {
    let mut s = session("abcdef");
    let doc = s.document();
    let at = Position {
        story: doc.body_id(),
        paragraph: doc.body().blocks.get(0).unwrap().id(),
        offset: ScalarOffset(0),
        bias: Bias::After,
    };
    s.apply(Transaction::new(
        s.version(),
        vec![Command::DeleteRange {
            start: at,
            end: Position {
                offset: ScalarOffset(5),
                ..at
            },
        }],
    ))
    .unwrap();
    let mut i = TextInput::new(
        s,
        InputLimits {
            max_text_bytes: 2,
            ..InputLimits::default()
        },
    )
    .unwrap();
    let version = i.backend().version();
    assert!(i.undo().is_err());
    assert_eq!(body(&i), "f");
    assert_eq!(i.backend().version(), version);
    assert!(i.can_undo());
}
#[test]
fn deterministic_unicode_replacements_match_an_independent_string_model() {
    let mut i = input("甲🙂\n\nabc");
    let mut reference = String::from("甲🙂\n\nabc");
    let mut seed = 0x0c07_2026_u64;
    let pieces = ["中", "🙂", "e\u{301}", "👩‍👩‍👧‍👦", "\n", "\n\n", "AV", "", "🇨🇳"];
    for step in 0..120 {
        let boundaries: Vec<_> = reference
            .char_indices()
            .map(|(b, _)| b)
            .chain(std::iter::once(reference.len()))
            .collect();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = (seed as usize) % boundaries.len();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let b = (seed as usize) % boundaries.len();
        let (start, end) = (boundaries[a.min(b)], boundaries[a.max(b)]);
        let range =
            reference[..start].encode_utf16().count()..reference[..end].encode_utf16().count();
        let before = reference.clone();
        let piece = pieces[step % pieces.len()];
        i.select(range.clone(), step % 2 == 0).unwrap();
        // Command replacements are standalone undo steps; keyboard typing at
        // the same caret would coalesce (T209, tests/undo_groups.rs).
        let changed = i.replace_from(InputSource::Command, None, piece).unwrap();
        reference.replace_range(start..end, piece);
        assert_eq!(body(&i), reference, "step {step}");
        assert_index(&i, &reference);
        if changed && step % 3 == 0 {
            i.undo().unwrap();
            assert_eq!(body(&i), before);
            i.redo().unwrap();
            assert_eq!(body(&i), reference);
        }
    }
}

#[test]
fn word_and_paragraph_selection_include_unicode_and_paragraph_marks() {
    let mut i = input("hello café🙂\n甲乙\n");
    i.select_word_at(7).unwrap();
    assert_eq!(i.view().selected_range(), (6..10, false));
    i.select_word_at(10).unwrap();
    assert_eq!(i.view().selected_range(), (10..12, false));
    i.select_paragraph_at(3).unwrap();
    assert_eq!(i.view().selected_range(), (0..13, false));
    i.select_paragraph_at(13).unwrap();
    assert_eq!(i.view().selected_range(), (13..16, false));
    i.select_paragraph_at(16).unwrap();
    assert_eq!(i.view().selected_range(), (16..16, false));
    let before = i.view().selected_range();
    assert!(i.select_word_at(11).is_err());
    assert_eq!(i.view().selected_range(), before);
    assert!(!i.has_unsaved_changes());
}

#[test]
fn exact_command_budget_allows_one_insert_and_empty_noop_but_rejects_split() {
    let mut i = TextInput::new(
        session(""),
        InputLimits {
            max_commands: 1,
            ..InputLimits::default()
        },
    )
    .unwrap();
    i.replace(None, "a").unwrap();
    assert_eq!(body(&i), "a");
    assert!(i.replace(None, "b\nc").is_err());
    assert_eq!(body(&i), "a");
    let mut i = TextInput::new(
        session(""),
        InputLimits {
            max_commands: 0,
            ..InputLimits::default()
        },
    )
    .unwrap();
    assert!(!i.replace(None, "").unwrap());
    assert!(!i.has_unsaved_changes());
    assert!(i.replace(None, "a").is_err());
}

#[test]
fn pointer_selection_cannot_cancel_a_preedit_on_an_invalid_or_stale_offset() {
    let mut i = input("hello");
    i.mark(None, "ni", None).unwrap();
    let revision = i.view().revision();
    let selection = i.view().selected_range();
    assert_eq!(
        i.select_word_at(usize::MAX),
        Err(InputError::StaleComposition)
    );
    assert_eq!(
        i.select_paragraph_at(usize::MAX),
        Err(InputError::StaleComposition)
    );
    assert!(i.is_composing());
    assert_eq!(i.view().revision(), revision);
    assert_eq!(i.view().selected_range(), selection);
    assert_eq!(body(&i), "hello");
}

#[test]
fn document_budgets_are_enforced_before_initialization_and_edit_commit() {
    let cases = [
        (
            InputLimits {
                max_paragraphs: 1,
                ..InputLimits::default()
            },
            "a",
            "b\nc",
            "paragraphs",
        ),
        (
            InputLimits {
                max_paragraph_bytes: 1,
                ..InputLimits::default()
            },
            "a",
            "b",
            "paragraph bytes",
        ),
        (
            InputLimits {
                max_runs: 1,
                ..InputLimits::default()
            },
            "a",
            "b\nc",
            "runs",
        ),
    ];
    for (limits, original, inserted, budget) in cases {
        let mut i = TextInput::new(session(original), limits).unwrap();
        let version = i.backend().version();
        let revision = i.view().revision();
        assert_eq!(i.replace(None, inserted), Err(InputError::Budget(budget)));
        assert_eq!(body(&i), original);
        assert_eq!(i.backend().version(), version);
        assert_eq!(i.view().revision(), revision);
        assert!(!i.can_undo());
    }
    let mut doc = session("a").document().clone();
    doc.styles_mut()
        .insert(joyword_model::Style::new(
            "s",
            joyword_model::StyleKind::Paragraph,
            Some("Custom"),
        ))
        .unwrap();
    assert!(matches!(
        TextInput::new(
            Session::new(doc),
            InputLimits {
                max_metadata_items: 0,
                ..InputLimits::default()
            }
        ),
        Err(InputError::Budget("metadata"))
    ));
    let mut i = TextInput::new(
        session(""),
        InputLimits {
            max_insert_bytes: usize::MAX,
            ..InputLimits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        i.replace(None, &"a".repeat(65_537)),
        Err(InputError::Budget(_))
    ));
    assert!(!i.has_unsaved_changes());
}

#[test]
fn an_additional_story_is_rejected_instead_of_silently_omitted() {
    let mut doc = session("body").document().clone();
    let mut header = Paragraph::new(doc.ids().node());
    header
        .push_text("页眉", None, std::sync::Arc::default())
        .unwrap();
    doc.insert_story(joyword_model::StoryKind::Header, vec![header.into()]);
    let input = TextInput::new(Session::new(doc), InputLimits::default()).unwrap();
    assert_eq!(input.view().index().text(), "body");
    assert!(!input.view().index().text().contains("页眉"));

    let mut noted = session("body").document().clone();
    let note = Paragraph::new(noted.ids().node());
    noted.insert_story(joyword_model::StoryKind::Footnote, vec![note.into()]);
    assert!(matches!(
        TextInput::new(Session::new(noted), InputLimits::default()),
        Err(InputError::UnsupportedBody)
    ));
}
