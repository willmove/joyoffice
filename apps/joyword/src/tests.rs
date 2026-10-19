//! Width-fold and regex checks for the find bar.
//!
//! The no-panic gate scans `main.rs` in full, including `#[cfg(test)]`.
//! A path ending in `tests.rs` is outside that scan.

use super::input_ui::edit_request_from_args;
use super::{
    ArgValue, CommandId, FIND_NEXT, FIND_PREVIOUS, FIND_REPLACE, FIND_REPLACE_ALL, FindBarFlags,
    supplement_find_flags,
};

fn mapped(args: &[(&'static str, ArgValue)]) -> std::collections::BTreeMap<&'static str, ArgValue> {
    args.iter().cloned().collect()
}

fn flag(args: &std::collections::BTreeMap<&'static str, ArgValue>, name: &str) -> bool {
    matches!(args.get(name), Some(ArgValue::Bool(true)))
}

fn request_width_regex(request: &joyword_shell::input::EditRequest) -> (bool, bool) {
    match request {
        joyword_shell::input::EditRequest::Find {
            fold_width, regex, ..
        }
        | joyword_shell::input::EditRequest::Replace {
            fold_width, regex, ..
        } => (*fold_width, *regex),
        _ => panic!("not a find request"),
    }
}

#[test]
fn find_bar_checkboxes_set_width_and_regex_on_every_find_command() {
    let off = FindBarFlags::default();
    assert_eq!(
        off,
        FindBarFlags {
            case_sensitive: false,
            whole_word: false,
            fold_width: false,
            regex: false,
        }
    );
    for replacement in [None, Some("x")] {
        let args = off.args("ABC", replacement);
        let map = mapped(&args);
        assert!(!flag(&map, "case"));
        assert!(!flag(&map, "word"));
        assert!(!flag(&map, "width"));
        assert!(!flag(&map, "regex"));
        assert_eq!(map.contains_key("replacement"), replacement.is_some());
    }

    let cases = [
        (
            FindBarFlags {
                fold_width: true,
                ..off
            },
            true,
            false,
        ),
        (FindBarFlags { regex: true, ..off }, false, true),
        (
            FindBarFlags {
                fold_width: true,
                regex: true,
                ..off
            },
            true,
            true,
        ),
    ];
    for (flags, width_on, regex_on) in cases {
        for (id, replacement) in [
            (FIND_NEXT, None),
            (FIND_PREVIOUS, None),
            (FIND_REPLACE, Some("x")),
            (FIND_REPLACE_ALL, Some("x")),
        ] {
            let map = mapped(&flags.args("ABC", replacement));
            assert_eq!(flag(&map, "width"), width_on, "{id}");
            assert_eq!(flag(&map, "regex"), regex_on, "{id}");
            let request = edit_request_from_args(id, &map).unwrap();
            assert_eq!(request_width_regex(&request), (width_on, regex_on), "{id}");
        }
    }

    // A caller that only sends the older keys (the ribbon binder) picks up
    // the checkboxes. An explicit false stays false.
    let older: Vec<_> = off
        .args("ABC", None)
        .into_iter()
        .filter(|(name, _)| *name != "width" && *name != "regex")
        .collect();
    let filled = supplement_find_flags(&older, true, true);
    let map = mapped(&filled);
    assert!(flag(&map, "width") && flag(&map, "regex"));
    let explicit = supplement_find_flags(&off.args("ABC", None), true, true);
    let map = mapped(&explicit);
    assert!(!flag(&map, "width") && !flag(&map, "regex"));
}

fn wide_editor() -> joyword_shell::input::TextInput<joyword_engine::Session> {
    use joyword_engine::{EditPolicy, Session};
    use joyword_model::{BlockSeq, Document, PageSetup, Paragraph};
    use std::sync::Arc;

    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph
        .push_text("ＡＢＣ 12 dog 34", None, Arc::default())
        .unwrap();
    let story = document.body_id();
    document.story_mut(story).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let mut session = Session::new(document);
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: false,
    });
    joyword_shell::input::TextInput::new(session, joyword_shell::input::InputLimits::default())
        .unwrap()
}

fn body_text(input: &joyword_shell::input::TextInput<joyword_engine::Session>) -> String {
    input
        .backend()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .plain_text(|_| String::new())
}

fn invoke_find(
    input: &mut joyword_shell::input::TextInput<joyword_engine::Session>,
    id: CommandId,
    flags: FindBarFlags,
    query: &str,
    replacement: Option<&str>,
) -> Result<joyword_shell::input::EditReport, joyword_shell::input::InputError> {
    let map = mapped(&flags.args(query, replacement));
    let request = edit_request_from_args(id, &map).expect("find bar args");
    input.editing(request)
}

#[test]
fn find_bar_width_hits_fullwidth_and_regex_rejects_invalid_patterns() {
    let original = "ＡＢＣ 12 dog 34";
    let mut input = wide_editor();
    assert_eq!(body_text(&input), original);

    let miss = invoke_find(&mut input, FIND_NEXT, FindBarFlags::default(), "ABC", None).unwrap();
    assert_eq!(miss.matches, 0);
    assert_eq!(input.selected_text().unwrap(), "");
    assert_eq!(body_text(&input), original);

    let literal = invoke_find(
        &mut input,
        FIND_NEXT,
        FindBarFlags::default(),
        "ＡＢＣ",
        None,
    )
    .unwrap();
    assert_eq!(literal.matches, 1);
    assert_eq!(input.selected_text().unwrap(), "ＡＢＣ");

    let mut folded = wide_editor();
    let next = invoke_find(
        &mut folded,
        FIND_NEXT,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        None,
    )
    .unwrap();
    assert_eq!(next.matches, 1);
    assert_eq!(folded.selected_text().unwrap(), "ＡＢＣ");
    let mut backward = wide_editor();
    let previous = invoke_find(
        &mut backward,
        FIND_PREVIOUS,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        None,
    )
    .unwrap();
    assert_eq!(previous.matches, 1);
    assert_eq!(backward.selected_text().unwrap(), "ＡＢＣ");

    let replaced = invoke_find(
        &mut folded,
        FIND_REPLACE,
        FindBarFlags {
            fold_width: true,
            ..FindBarFlags::default()
        },
        "ABC",
        Some("Z"),
    )
    .unwrap();
    assert_eq!(replaced.replaced, 1);
    assert_eq!(body_text(&folded), "Z 12 dog 34");
    folded.undo().unwrap();
    assert_eq!(body_text(&folded), original);

    let literal_digits = invoke_find(
        &mut input,
        FIND_REPLACE_ALL,
        FindBarFlags::default(),
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(literal_digits.replaced, 0);
    assert_eq!(body_text(&input), original);

    let once = invoke_find(
        &mut input,
        FIND_REPLACE,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(once.replaced, 1);
    assert_eq!(body_text(&input), "ＡＢＣ # dog 34");
    input.undo().unwrap();
    assert_eq!(body_text(&input), original);

    let all = invoke_find(
        &mut input,
        FIND_REPLACE_ALL,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        r"\d+",
        Some("#"),
    )
    .unwrap();
    assert_eq!(all.replaced, 2);
    assert_eq!(body_text(&input), "ＡＢＣ # dog #");
    input.undo().unwrap();
    assert_eq!(body_text(&input), original);

    let invalid = invoke_find(
        &mut input,
        FIND_NEXT,
        FindBarFlags {
            regex: true,
            ..FindBarFlags::default()
        },
        "(",
        None,
    )
    .unwrap_err();
    assert_eq!(
        invalid,
        joyword_shell::input::InputError::Rejected("query is not a valid pattern")
    );
    assert_eq!(body_text(&input), original);
}
