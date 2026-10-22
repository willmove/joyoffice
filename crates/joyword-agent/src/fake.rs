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

/// Contract-test provider that captures the exact prompt bytes it was asked to
/// send, so tests can assert the minimal-context guarantee (T401). Prompt
/// capture is a test affordance that lives in process memory only; runtime
/// diagnostics and telemetry never store body text (QUALITY-007), and no
/// default runtime wiring uses this type.
pub struct RecordingProvider {
    id: ProviderId,
    script: Mutex<VecDeque<Script>>,
    prompts: Mutex<Vec<String>>,
    calls: CallCounter,
}

/// Most prompts one [`RecordingProvider`] keeps.
pub const MAX_RECORDED_PROMPTS: usize = 64;

impl RecordingProvider {
    pub fn scripted(id: &str, steps: Vec<Script>) -> Result<Self, crate::provider::IdError> {
        Ok(Self {
            id: ProviderId::parse(id)?,
            script: Mutex::new(steps.into()),
            prompts: Mutex::new(Vec::new()),
            calls: CallCounter::default(),
        })
    }

    pub fn id(&self) -> &ProviderId {
        &self.id
    }

    /// Every prompt actually handed to `complete`, in order.
    pub fn prompts(&self) -> Vec<String> {
        self.prompts
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl Provider for RecordingProvider {
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
        if request.cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        if let Ok(mut prompts) = self.prompts.lock()
            && prompts.len() < MAX_RECORDED_PROMPTS
        {
            prompts.push(request.prompt.to_owned());
        }
        let mut script = self.script.lock().map_err(|_| ProviderError::Offline)?;
        match script.pop_front() {
            Some(Script::Reply(text)) => Ok(CompletionResponse { text }),
            Some(Script::Fail(error)) => Err(error),
            None => Err(ProviderError::Offline),
        }
    }
}
