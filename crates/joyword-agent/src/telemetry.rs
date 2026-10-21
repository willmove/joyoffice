//! Diagnostics stay on the machine. The upload counter has no incrementing
//! API: QUALITY-007 forbids a default upload of body text, credentials,
//! revisions, or comments, and T400 does not add an uploader.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub struct Telemetry {
    enabled: bool,
    uploads: AtomicU64,
}

impl Telemetry {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            uploads: AtomicU64::new(0),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Local note only. `None` when telemetry is off. The returned kind is a
    /// static label, never document text.
    pub fn note_local(&self, kind: &'static str) -> Option<&'static str> {
        if self.enabled { Some(kind) } else { None }
    }

    pub fn upload_attempts(&self) -> u64 {
        self.uploads.load(Ordering::SeqCst)
    }
}
