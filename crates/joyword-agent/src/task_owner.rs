//! A single-request host task owner, independent of any UI or wire protocol.
//!
//! Polling never joins an unfinished thread. A stop deadline only changes the
//! reported status: it does not detach or terminate the worker. Hosts must keep
//! this owner alive until Disabled before closing it. Dropping a live owner
//! requests cancellation but cannot guarantee termination of a Rust thread.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{AgentError, AgentRuntime, AiPhase, CompletionResponse};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskError {
    Busy,
    Runtime(AgentError),
    SpawnFailed,
    WorkerPanicked,
}

/// Completion text is returned only while the owner is not stopping.
pub enum TaskPoll {
    Idle,
    Running,
    Stopping { timed_out: bool },
    Disabled,
    Completed(Result<CompletionResponse, TaskError>),
}

#[must_use = "retain the task owner until shutdown completes"]
pub struct AgentTaskOwner {
    runtime: Arc<AgentRuntime>,
    worker: Option<JoinHandle<Result<CompletionResponse, AgentError>>>,
    stopping: Option<(Instant, Duration)>,
}

impl AgentTaskOwner {
    pub fn new(runtime: AgentRuntime) -> Result<Self, TaskError> {
        match runtime.phase().map_err(TaskError::Runtime)? {
            AiPhase::Disabled | AiPhase::EnabledIdle => Ok(Self {
                runtime: Arc::new(runtime),
                worker: None,
                stopping: None,
            }),
            _ => Err(TaskError::Busy),
        }
    }

    /// The caller supplies already-authorized input, not a UI document handle.
    /// This primitive does not establish a selection grant or apply a result.
    pub fn start(&mut self, prompt: String) -> Result<(), TaskError> {
        if self.worker.is_some() || self.stopping.is_some() {
            return Err(TaskError::Busy);
        }
        if self.runtime.phase().map_err(TaskError::Runtime)? != AiPhase::EnabledIdle {
            return Err(TaskError::Busy);
        }
        let runtime = Arc::clone(&self.runtime);
        self.worker = Some(
            thread::Builder::new()
                .name("joyword-agent-request".into())
                .spawn(move || runtime.complete(&prompt))
                .map_err(|_| TaskError::SpawnFailed)?,
        );
        Ok(())
    }

    /// Repeated stop requests preserve the original deadline.
    pub fn request_stop(&mut self, now: Instant, timeout: Duration) -> Result<(), TaskError> {
        if self.stopping.is_some() {
            return Ok(());
        }
        if self.runtime.phase().map_err(TaskError::Runtime)? != AiPhase::Disabled {
            self.runtime.begin_stop().map_err(TaskError::Runtime)?;
        }
        self.stopping = Some((now, timeout));
        Ok(())
    }

    /// Call from the host event loop; no waiting, sleeping or unfinished join.
    pub fn poll(&mut self, now: Instant) -> Result<TaskPoll, TaskError> {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.is_finished())
        {
            return Ok(match self.stopping {
                Some((started, timeout)) => TaskPoll::Stopping {
                    timed_out: now.saturating_duration_since(started) >= timeout,
                },
                None => TaskPoll::Running,
            });
        }
        let completed = self.worker.take().map(|worker| {
            worker
                .join()
                .map_err(|_| TaskError::WorkerPanicked)
                .and_then(|result| result.map_err(TaskError::Runtime))
        });
        if self.stopping.is_some() {
            // Joining has released the worker's Arc. No runtime clones escape.
            Arc::get_mut(&mut self.runtime)
                .ok_or(TaskError::Busy)?
                .commit_disabled()
                .map_err(TaskError::Runtime)?;
            self.stopping = None;
            // Drop both successful late text and failures after stopping.
            return Ok(TaskPoll::Disabled);
        }
        if let Some(result) = completed {
            return Ok(TaskPoll::Completed(result));
        }
        Ok(
            if self.runtime.phase().map_err(TaskError::Runtime)? == AiPhase::Disabled {
                TaskPoll::Disabled
            } else {
                TaskPoll::Idle
            },
        )
    }

    pub fn enable(&mut self) -> Result<(), TaskError> {
        if self.worker.is_some() || self.stopping.is_some() {
            return Err(TaskError::Busy);
        }
        Arc::get_mut(&mut self.runtime)
            .ok_or(TaskError::Busy)?
            .set_user_enabled(true)
            .map_err(TaskError::Runtime)
    }

    /// Close only after shutdown completes; otherwise return the intact owner.
    /// Hosts should keep the returned owner and continue polling it.
    pub fn try_close(self) -> Result<(), Self> {
        if self.ready_to_close() {
            Ok(())
        } else {
            Err(self)
        }
    }

    /// False during Running/Stopping, including after a stop timeout.
    pub fn ready_to_close(&self) -> bool {
        self.worker.is_none()
            && self.stopping.is_none()
            && matches!(self.runtime.phase(), Ok(AiPhase::Disabled))
    }
}

impl Drop for AgentTaskOwner {
    fn drop(&mut self) {
        // Best effort only. The host must not use Drop as a worker-reaping API.
        let _ = self.runtime.begin_stop();
    }
}
