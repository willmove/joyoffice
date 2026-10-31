use super::{close_runtime, runtime};
use std::sync::Arc;

#[test]
fn poisoned_show_is_rejected_without_resuming_partial_state() {
    let deck = Arc::new(joydeck::authoring_demo::deck());
    let shared = runtime(deck.clone(), 0).unwrap();
    let _ = std::panic::catch_unwind(|| {
        let _guard = shared.lock().unwrap();
        panic!("inject interrupted show update");
    });
    assert!(close_runtime(&shared).is_err());
    assert!(shared.is_poisoned());
    let guard = match shared.lock() {
        Err(error) => error.into_inner(),
        Ok(_) => panic!("expected poisoned state"),
    };
    assert_eq!(guard.snapshot.deck, deck);
}
