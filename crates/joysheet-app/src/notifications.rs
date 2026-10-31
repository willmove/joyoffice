//! Completion wakeups coalesce; snapshots and replies remain the source of truth.
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Notifications(Arc<Mutex<Vec<async_channel::Sender<()>>>>);
impl Notifications {
    pub fn subscribe(&self) -> async_channel::Receiver<()> {
        let (sender, receiver) = async_channel::bounded(1);
        // Cover publication concurrent with subscription. The receiver rereads latest.
        let _ = sender.try_send(());
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(sender);
        receiver
    }
    pub fn publish(&self) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|s| !matches!(s.try_send(()), Err(async_channel::TrySendError::Closed(_))));
    }
}

#[cfg(test)]
mod tests {
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
}
