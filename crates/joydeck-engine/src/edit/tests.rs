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
