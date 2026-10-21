//! In-process provider for contract tests. It never opens a socket.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::provider::{
    CallCounter, Capabilities, CompletionRequest, CompletionResponse, Provider, ProviderError,
    ProviderId,
};

/// One scripted step. The prompt is not stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Script {
    Reply(String),
    Fail(ProviderError),
}

/// Deterministic provider. [`FakeProvider::bomb`] fails if `complete` is called,
/// which is how startup proves it made no request.
pub struct FakeProvider {
    id: ProviderId,
    bomb: bool,
    script: Mutex<VecDeque<Script>>,
    calls: CallCounter,
}

impl FakeProvider {
    pub fn bomb() -> Self {
        Self {
            id: ProviderId::from_validated("bomb"),
            bomb: true,
            script: Mutex::new(VecDeque::new()),
            calls: CallCounter::default(),
        }
    }

    pub fn scripted(id: &str, steps: Vec<Script>) -> Result<Self, crate::provider::IdError> {
        Ok(Self {
            id: ProviderId::parse(id)?,
            bomb: false,
            script: Mutex::new(steps.into()),
            calls: CallCounter::default(),
        })
    }

    pub fn id(&self) -> &ProviderId {
        &self.id
    }
}

impl Provider for FakeProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: false,
            tools: false,
        }
    }

    fn calls(&self) -> u64 {
        self.calls.get()
    }

    fn complete(
        &self,
        request: &CompletionRequest<'_>,
    ) -> Result<CompletionResponse, ProviderError> {
        self.calls.add();
        if self.bomb {
            return Err(ProviderError::UnexpectedCall);
        }
        if request.cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        let mut script = self.script.lock().map_err(|_| ProviderError::Offline)?;
        match script.pop_front() {
            Some(Script::Reply(text)) => Ok(CompletionResponse { text }),
            Some(Script::Fail(error)) => Err(error),
            None => Err(ProviderError::Offline),
        }
    }
}
