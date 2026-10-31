use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A bounded unit of background work. Cancellation is checked before and after
/// each chunk, including when the owning window has disappeared.
pub fn chunk(cancel: &AtomicBool) -> bool {
    if cancel.load(Ordering::Relaxed) {
        return false;
    }
    std::thread::sleep(Duration::from_millis(40));
    !cancel.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_job_never_publishes_a_completed_chunk() {
        assert!(!chunk(&AtomicBool::new(true)));
    }

    #[test]
    fn restarting_uses_an_independent_cancellation_token() {
        let previous = AtomicBool::new(true);
        let next = AtomicBool::new(false);
        assert!(!chunk(&previous));
        assert!(chunk(&next));
        assert!(previous.load(Ordering::Relaxed));
    }
}
