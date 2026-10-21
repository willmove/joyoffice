//! Deterministic pause for an in-flight open or save.
//!
//! Production passes [`None`] and does not block. A test moves the [`StepGate`]
//! onto the worker, waits until the worker has entered, checks the editor,
//! then drops the [`StepController`] to release the worker. The wait is a
//! channel, not a sleep.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

/// Blocks inside an open or save until the matching [`StepController`] is dropped.
pub struct StepGate {
    entered: Sender<()>,
    release: Receiver<()>,
}

/// Test side of a [`StepGate`]. Dropping it releases the worker.
pub struct StepController {
    entered: Receiver<()>,
    release: Sender<()>,
}

impl StepGate {
    pub fn pair() -> (StepController, Self) {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        (
            StepController {
                entered: entered_rx,
                release: release_tx,
            },
            Self {
                entered: entered_tx,
                release: release_rx,
            },
        )
    }

    /// No-op when `gate` is `None`. A disconnected controller lets the call continue.
    pub fn wait(gate: Option<&Self>) {
        let Some(gate) = gate else {
            return;
        };
        if gate.entered.send(()).is_err() {
            return;
        }
        let _ = gate.release.recv();
    }
}

impl StepController {
    /// Blocks until the worker enters the gate, or until `limit` elapses.
    /// The success path returns as soon as the worker enters.
    pub fn wait_entered(&self, limit: Duration) -> bool {
        match self.entered.recv_timeout(limit) {
            Ok(()) => true,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => false,
        }
    }
}

impl Drop for StepController {
    fn drop(&mut self) {
        let _ = self.release.send(());
    }
}
