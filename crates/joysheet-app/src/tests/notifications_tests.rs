use super::*;
#[test]
fn slow_subscriber_coalesces_without_blocking_other_windows() {
    let n = Notifications::default();
    let slow = n.subscribe();
    let fast = n.subscribe();
    fast.try_recv().unwrap();
    for _ in 0..1024 {
        n.publish();
        assert!(fast.try_recv().is_ok());
    }
    assert_eq!(slow.len(), 1);
    slow.try_recv().unwrap();
    assert!(slow.try_recv().is_err());
    drop(fast);
    n.publish();
    assert_eq!(n.0.lock().unwrap().len(), 1);
}
