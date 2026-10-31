use super::*;
#[test]
fn history_cap_and_new_branch_clear_redo() {
    let mut s = Session::new();
    for _ in 0..120 {
        s.execute(s.id, Command::AddObject(0, object("rect")))
            .unwrap();
    }
    assert!(s.undo.len() <= 100);
    s.undo();
    assert!(s.can_redo());
    s.execute(s.id, Command::AddPage).unwrap();
    assert!(!s.can_redo());
    let mut empty = Session::new();
    assert!(empty.execute(empty.id, Command::DeletePage(0)).is_err());
    assert!(!empty.dirty());
}
#[test]
fn history_budget_includes_redo_and_large_to_small_edits() {
    let mut s = Session::new();
    s.execute(s.id, Command::AddObject(0, object("text")))
        .unwrap();
    for i in 0..24 {
        let mut o = s.deck.slides[0].objects[0].clone();
        o.text = if i % 2 == 0 {
            "x".repeat(3 * 1024 * 1024)
        } else {
            "small".into()
        };
        s.execute(s.id, Command::ReplaceObject(0, 0, o)).unwrap();
        assert!(
            s.undo
                .iter()
                .chain(s.redo.iter())
                .map(|v| v.bytes)
                .sum::<usize>()
                <= 32 * 1024 * 1024
        );
    }
    for _ in 0..24 {
        s.undo();
        assert!(
            s.undo
                .iter()
                .chain(s.redo.iter())
                .map(|v| v.bytes)
                .sum::<usize>()
                <= 32 * 1024 * 1024
        );
    }
    for _ in 0..24 {
        s.redo();
        assert!(
            s.undo
                .iter()
                .chain(s.redo.iter())
                .map(|v| v.bytes)
                .sum::<usize>()
                <= 32 * 1024 * 1024
        );
    }
}
#[test]
fn id_overflow_is_checked() {
    let mut s = Session::new();
    s.next_id = u64::MAX;
    let old = s.deck.clone();
    assert!(s.execute(s.id, Command::AddPage).is_err());
    assert!(Arc::ptr_eq(&old, &s.deck));
}

#[test]
fn enabled_enforces_the_same_model_budget_as_commit() {
    let mut d = crate::identity::blank();
    let mut slide = d.slides[0].clone();
    slide.notes = "x".repeat(100000);
    d.slides = (0..83)
        .map(|i| {
            let mut s = slide.clone();
            s.id = format!("page-{i}");
            s
        })
        .collect();
    let mut session = Session::from_deck(d, true);
    let before = session.deck.clone();
    let command = Command::DuplicatePage(0);
    assert!(session.enabled(&command).unwrap_err().contains("8 MiB"));
    assert!(
        session
            .execute(session.id, command)
            .unwrap_err()
            .contains("8 MiB")
    );
    assert_eq!(session.deck, before);
    assert!(!session.can_undo());
}

#[test]
fn validation_keeps_id_cursor_and_independent_sessions_allocate_distinct_objects() {
    let mut a = Session::new();
    let mut b = Session::new();
    let cursor = a.next_id;
    for _ in 0..4 {
        a.enabled(&Command::AddObject(0, object("rect"))).unwrap();
    }
    assert_eq!(a.next_id, cursor);
    assert!(a.deck.slides[0].objects.is_empty());
    a.execute(a.id, Command::AddObject(0, object("rect")))
        .unwrap();
    b.execute(b.id, Command::AddObject(0, object("rect")))
        .unwrap();
    assert_ne!(
        a.deck.slides[0].objects[0].id,
        b.deck.slides[0].objects[0].id
    );
    assert_eq!(
        a.deck.slides[0].objects[0].id,
        format!("object-{}-{}", a.id, cursor + 1)
    );
}
