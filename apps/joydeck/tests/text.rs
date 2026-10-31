use joydeck::{
    edit::{self, Command, Session},
    text::{self, TextEdit},
};
#[test]
fn utf16_surrogates_reversed_and_out_of_bounds_are_rejected() {
    let s = "A😀中e\u{301}";
    assert_eq!(text::utf16_range(s, 1..3), Some(1..5));
    for r in [2..3, 1..2, 0..99, std::ops::Range { start: 4, end: 3 }] {
        assert!(text::utf16_range(s, r).is_none());
    }
    assert_eq!(text::to_utf16(s, 1..5), 1..3);
}
#[test]
fn graphemes_are_atomic_for_zwj_flags_and_combining_marks() {
    let s = "中👩‍👩‍👦🇨🇳e\u{301}";
    let mut n = 0;
    for part in ["中", "👩‍👩‍👦", "🇨🇳", "e\u{301}"] {
        assert_eq!(text::next(s, n), n + part.len());
        assert_eq!(text::previous(s, n + part.len()), n);
        n += part.len();
    }
}
#[test]
fn composition_commit_cancel_and_failed_replacement_are_atomic() {
    let mut session = Session::new();
    session
        .execute(session.id, Command::AddObject(0, edit::object("text")))
        .unwrap();
    let before = session.deck.clone();
    let mut t = TextEdit::new(before.slides[0].objects[0].clone());
    t.replace(None, "ni", true, Some(2..2)).unwrap();
    t.replace(None, "你好😀", true, Some(2..4)).unwrap();
    assert_eq!(t.selection, 6..10);
    let draft = t.object.clone();
    assert!(t.replace(Some(3..4), "x", false, None).is_err());
    assert_eq!(draft, t.object);
    t.cancel();
    assert_eq!(t.object, before.slides[0].objects[0]);
    t.replace(None, "zhong", true, None).unwrap();
    t.replace(None, "中文", false, None).unwrap();
    session
        .execute(session.id, Command::ReplaceObject(0, 0, t.object))
        .unwrap();
    session.undo();
    assert_eq!(session.deck, before);
    session.redo();
    assert_eq!(session.deck.slides[0].objects[0].text, "中文");
}
#[test]
fn rich_splice_preserves_adjacent_styles_and_roundtrips() {
    let mut o = edit::object("text");
    o.text = "中文 ABC".into();
    let mut style = text::Style::for_object(&o);
    style.bold = true;
    style.color = "#ff0000".into();
    text::format(&mut o, 0..6, style.clone()).unwrap();
    assert_eq!(o.runs.len(), 2);
    text::replace(&mut o, 6..7, "😀", &style).unwrap();
    assert_eq!(o.text, "中文😀ABC");
    assert_eq!(o.runs.len(), 2);
    text::validate(&o.text, &o.runs).unwrap();
    let mut session = Session::new();
    session
        .execute(session.id, Command::AddObject(0, o))
        .unwrap();
    let path =
        std::env::temp_dir().join(format!("joydeck-rich-test-{}.joydeck", std::process::id()));
    edit::save_new(&path, &session.deck).unwrap();
    assert_eq!(edit::open_editable(&path).unwrap(), *session.deck);
    let raster = joydeck::preview::render(
        &session.deck,
        0,
        960,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    assert!(
        raster
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[2] > 150 && p[1] < 100)
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn invalid_run_gaps_style_and_boundaries_are_rejected() {
    let mut o = edit::object("text");
    let len = o.text.len();
    text::format(
        &mut o,
        0..len,
        text::Style::for_object(&edit::object("text")),
    )
    .unwrap();
    let valid = o.runs.clone();
    o.runs[0].start = 1;
    assert!(text::validate(&o.text, &o.runs).is_err());
    o.runs = valid.clone();
    o.runs[0].style.size = f32::NAN;
    assert!(text::validate(&o.text, &o.runs).is_err());
    o.runs = valid;
    o.runs[0].end = 1;
    assert!(text::validate(&o.text, &o.runs).is_err());
}

#[test]
fn reversed_selection_extends_and_shrinks_across_unicode() {
    let mut o = edit::object("text");
    o.text = "A😀中文".into();
    let mut t = TextEdit::new(o);
    t.move_to(5, false);
    t.move_to(1, true);
    assert_eq!(t.selection, 1..5);
    assert!(t.reversed);
    assert_eq!(t.cursor(), 1);
    t.move_to(0, true);
    assert_eq!(t.selection, 0..5);
    t.move_to(8, true);
    assert_eq!(t.selection, 5..8);
    assert!(!t.reversed);
}

#[test]
fn empty_mark_cancels_replacement_of_existing_selection() {
    let o = edit::object("text");
    let mut t = TextEdit::new(o.clone());
    t.replace(None, "pinyin", true, Some(6..6)).unwrap();
    t.replace(None, "", true, Some(0..0)).unwrap();
    assert_eq!(t.object, o);
    assert!(t.marked.is_none());
    assert_eq!(t.selection, 0..o.text.len());
}

#[test]
fn unavailable_font_is_located_and_preserved() {
    let mut s = Session::new();
    let mut o = edit::object("text");
    let mut style = text::Style::for_object(&o);
    style.family = "JoyDeck Synthetic Missing Font 000000".into();
    let len = o.text.len();
    text::format(&mut o, 0..len, style).unwrap();
    s.execute(s.id, Command::AddObject(0, o)).unwrap();
    let mut deck = (*s.deck).clone();
    joydeck::preview::diagnose_fonts(&mut deck);
    assert!(
        deck.warnings
            .iter()
            .any(|w| w.code == "missing-font" && w.page == Some(1) && w.part.contains("object/"))
    );
    assert_eq!(
        deck.slides[0].objects[0].runs,
        s.deck.slides[0].objects[0].runs
    );
}
