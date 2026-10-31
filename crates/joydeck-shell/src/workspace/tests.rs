use super::*;
#[test]
fn poisoned_session_keeps_snapshot_dirty_and_rejects_writes() {
    let mut session = SharedSession::new();
    let before = session.deck.clone();
    let core = session.core.clone();
    let _ = std::thread::spawn(move || {
        let _guard = core.lock().unwrap();
        panic!("injected session failure")
    })
    .join();
    assert!(session.failure().is_some());
    assert!(session.dirty());
    assert!(!session.begin_save());
    assert!(!session.can_undo());
    assert!(session.execute(session.id, Command::AddPage).is_err());
    session.mark_saved(before.clone());
    session.undo();
    session.redo();
    assert!(Arc::ptr_eq(&before, &session.deck));
    assert!(session.dirty());
}
