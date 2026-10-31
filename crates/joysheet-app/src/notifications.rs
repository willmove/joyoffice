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
#[path = "tests/notifications_tests.rs"]
mod tests;
