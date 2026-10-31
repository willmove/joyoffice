extern crate joydeck_shell as joydeck;

use joydeck::edit::*;
use std::{path::Path, sync::Arc};
#[test]
fn rollback_identity_and_history() {
    let mut s = Session::new();
    let before = s.deck.clone();
    let mut o = object("rect");
    o.width = f32::NAN;
    assert!(s.execute(s.id, Command::AddObject(0, o)).is_err());
    assert!(Arc::ptr_eq(&before, &s.deck));
    assert!(s.execute(s.id + 1, Command::AddPage).is_err());
    s.execute(s.id, Command::AddPage).unwrap();
    s.undo();
    assert!(Arc::ptr_eq(&before, &s.deck));
    s.redo();
    assert_eq!(s.deck.slides.len(), 2);
    assert!(s.execute(s.id, Command::DeletePage(10)).is_err());
}
#[test]
fn save_reopen_and_stale_save() {
    let mut s = Session::new();
    let path = std::env::temp_dir().join(format!("joydeck-m2-{}.joydeck", std::process::id()));
    save(&path, &s.deck).unwrap();
    let d = open_editable(&path).unwrap();
    assert_eq!(d.slides.len(), 1);
    let old = s.deck.clone();
    s.execute(s.id, Command::AddPage).unwrap();
    s.mark_saved(old);
    assert!(s.dirty());
    save(&path, &s.deck).unwrap();
    assert_eq!(open_editable(&path).unwrap().slides.len(), 2);
    let bytes = std::fs::read(&path).unwrap();
    let mut invalid = (*s.deck).clone();
    invalid.width = f32::NAN;
    assert!(save(&path, &invalid).is_err());
    assert_eq!(bytes, std::fs::read(&path).unwrap());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn save_new_never_replaces_existing_target() {
    let s = Session::new();
    let path =
        std::env::temp_dir().join(format!("joydeck-no-clobber-{}.joydeck", std::process::id()));
    std::fs::write(&path, b"keep existing contents").unwrap();
    assert!(save_new(&path, &s.deck).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep existing contents");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn versioned_style_and_ids_roundtrip() {
    let mut s = Session::new();
    let mut o = object("arrow");
    o.rotation = 37.;
    o.opacity = 0.4;
    o.stroke_width = 3.;
    s.execute(s.id, Command::AddObject(0, o)).unwrap();
    let id = s.deck.slides[0].objects[0].id.clone();
    s.execute(s.id, Command::DuplicatePage(0)).unwrap();
    assert_ne!(s.deck.slides[1].objects[0].id, id);
    assert!(s.enabled(&Command::DeletePages(vec![0, 1])).is_err());
    let path = std::env::temp_dir().join(format!("joydeck-style-{}.joydeck", std::process::id()));
    save(&path, &s.deck).unwrap();
    let loaded = open_editable(&path).unwrap();
    assert_eq!(&*s.deck, &loaded);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn legacy_upgrade_and_id_overflow_are_checked() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/joydeck/native-20.joydeck");
    let legacy = open_editable(&path).unwrap();
    let s = Session::from_deck(legacy, true);
    let ids = s
        .deck
        .slides
        .iter()
        .flat_map(|s| s.objects.iter().map(|o| o.id.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 120);
    assert!(!ids.contains(""));
}
