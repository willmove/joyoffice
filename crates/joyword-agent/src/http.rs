//! HTTP provider shape. The transport is injected. This module does not open
//! a socket; CI uses [`RecordingTransport`].

use std::fmt;
use std::sync::Mutex;

use crate::credential::Secret;
use crate::provider::{
    CallCounter, Capabilities, CompletionRequest, CompletionResponse, LogCredential, Provider,
    ProviderError, ProviderId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointError {
    Invalid,
}

/// Explicit endpoint. Userinfo (`user:pass@host`) is rejected so a URL cannot
/// smuggle a secret.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Endpoint(String);

impl Endpoint {
    pub fn parse(raw: &str) -> Result<Self, EndpointError> {
        if raw.is_empty()
            || raw.len() > 2048
            || raw.chars().any(char::is_whitespace)
            || raw.contains('@')
            || raw.contains('\\')
        {
            return Err(EndpointError::Invalid);
        }
        let rest = raw
            .strip_prefix("https://")
            .or_else(|| raw.strip_prefix("http://"));
        let Some(rest) = rest else {
            return Err(EndpointError::Invalid);
        };
        if rest.is_empty() || rest.starts_with('/') {
            return Err(EndpointError::Invalid);
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Outbound<'a> {
    pub url: &'a str,
    pub body: &'a str,
    pub authorization: Option<&'a Secret>,
}

pub trait Transport: Send + Sync {
    fn exchange(&self, call: &Outbound<'_>) -> Result<Vec<u8>, ProviderError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedExchange {
    pub url: String,
    pub body_bytes: usize,
    pub authorization: LogCredential,
}

/// Records the exchange and returns a fixed body. The authorization value is
/// not copied into the record.
#[derive(Debug, Default)]
pub struct RecordingTransport {
    records: Mutex<Vec<RecordedExchange>>,
}

impl RecordingTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn exchanges(&self) -> Vec<RecordedExchange> {
        self.records
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.records
            .lock()
            .map(|guard| guard.len())
            .unwrap_or(usize::MAX)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Transport for RecordingTransport {
    fn exchange(&self, call: &Outbound<'_>) -> Result<Vec<u8>, ProviderError> {
        let authorization = if call.authorization.is_some() {
            LogCredential::Redacted
        } else {
            LogCredential::Absent
        };
        let mut guard = self.records.lock().map_err(|_| ProviderError::Offline)?;
        if guard.len() >= 1024 {
            return Err(ProviderError::RateLimited);
        }
        guard.push(RecordedExchange {
            url: call.url.to_owned(),
            body_bytes: call.body.len(),
            authorization,
        });
        Ok(b"recorded".to_vec())
    }
}

pub struct HttpProvider<T> {
    id: ProviderId,
    endpoint: Endpoint,
    transport: T,
    calls: CallCounter,
}

impl<T> HttpProvider<T> {
    pub fn new(id: &str, endpoint: &str, transport: T) -> Result<Self, BuildError> {
        Ok(Self {
            id: ProviderId::parse(id)?,
            endpoint: Endpoint::parse(endpoint)?,
            transport,
            calls: CallCounter::default(),
        })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn id(&self) -> &ProviderId {
        &self.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildError {
    Id,
    Endpoint,
}

impl<T: Transport> Provider for HttpProvider<T> {
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
        if request.cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        self.calls.add();
        let call = Outbound {
            url: self.endpoint.as_str(),
            body: request.prompt,
            authorization: request.credential,
        };
        let bytes = self.transport.exchange(&call)?;
        let text = String::from_utf8(bytes).map_err(|_| ProviderError::InvalidToolParams)?;
        Ok(CompletionResponse { text })
    }
}

impl From<crate::provider::IdError> for BuildError {
    fn from(_: crate::provider::IdError) -> Self {
        BuildError::Id
    }
}

impl From<EndpointError> for BuildError {
    fn from(_: EndpointError) -> Self {
        BuildError::Endpoint
    }
}
