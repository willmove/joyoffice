//! Provider contract. Implementations in this crate are in-process. A socket
//! client is not part of the contract and is not a dependency.

use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::credential::Secret;
use crate::gate::Generation;

/// Stable provider name. Not a URL and not a secret.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    /// Caller guarantees `value` already satisfies [`Self::parse`].
    pub(crate) fn from_validated(value: &str) -> Self {
        Self(value.to_owned())
    }

    pub fn parse(value: &str) -> Result<Self, IdError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
        {
            return Err(IdError::Invalid);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdError {
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub streaming: bool,
    pub tools: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderError {
    MissingCredential,
    Offline,
    RateLimited,
    TimedOut,
    ContentFiltered,
    InvalidToolParams,
    Cancelled,
    UnexpectedCall,
    EndpointNotAllowed,
    ProviderNotAllowed,
    NoProvider,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ProviderError::MissingCredential => "provider credential is missing",
            ProviderError::Offline => "provider is offline",
            ProviderError::RateLimited => "provider rate limited the request",
            ProviderError::TimedOut => "provider timed out",
            ProviderError::ContentFiltered => "provider filtered the request",
            ProviderError::InvalidToolParams => "provider rejected the tool parameters",
            ProviderError::Cancelled => "provider request was cancelled",
            ProviderError::UnexpectedCall => "provider was called while ai is off",
            ProviderError::EndpointNotAllowed => "provider endpoint is not allowed",
            ProviderError::ProviderNotAllowed => "provider is not allowed by policy",
            ProviderError::NoProvider => "no provider is configured",
        })
    }
}

impl std::error::Error for ProviderError {}

/// Cooperative cancel flag. The provider must observe it; the gate does not
/// depend on the provider honoring it.
#[derive(Debug, Default)]
pub struct CancelFlag {
    cancelled: AtomicBool,
}

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub struct CompletionRequest<'a> {
    pub prompt: &'a str,
    pub generation: Generation,
    pub cancel: &'a CancelFlag,
    pub credential: Option<&'a Secret>,
}

impl fmt::Debug for CompletionRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionRequest")
            .field("prompt_bytes", &self.prompt.len())
            .field("generation", &self.generation)
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionResponse {
    pub text: String,
}

pub trait Provider: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    fn calls(&self) -> u64;
    fn complete(
        &self,
        request: &CompletionRequest<'_>,
    ) -> Result<CompletionResponse, ProviderError>;
}

/// Count of attempts that actually entered a provider. Prompt text is not stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggedRequest {
    pub provider_id: String,
    pub prompt_bytes: usize,
    pub credential: LogCredential,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogCredential {
    Absent,
    Redacted,
}

#[derive(Debug, Default)]
pub struct RequestLog {
    entries: Mutex<Vec<LoggedRequest>>,
}

impl RequestLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, entry: LoggedRequest) -> bool {
        let Ok(mut guard) = self.entries.lock() else {
            return false;
        };
        if guard.len() >= 1024 {
            return false;
        }
        guard.push(entry);
        true
    }

    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .map(|guard| guard.len())
            .unwrap_or(usize::MAX)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> Vec<LoggedRequest> {
        self.entries
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

/// Shared call counter used by the in-process providers.
#[derive(Debug, Default)]
pub(crate) struct CallCounter(AtomicU64);

impl CallCounter {
    pub(crate) fn add(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn get(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
