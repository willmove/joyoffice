//! JoyWord AI foundation (T400, AI-001, AI-005, QUALITY-007).
//!
//! The crate is optional. Document editing crates do not depend on it, and it
//! does not depend on them. Nothing here opens a socket, downloads a model, or
//! applies a [`ChangeProposal`](tools::ChangeProposal). The first-launch state
//! is disabled; a generation counter is the kill-switch linearization point.
//!
//! AIGC labeling is not implemented. [`aigc`] records that the legal assessment
//! is still pending and refuses to write a label.

#![forbid(unsafe_code)]

mod aigc;
mod config;
mod credential;
mod fake;
mod gate;
mod http;
mod policy;
mod provider;
mod runtime;
mod telemetry;
mod tools;

pub use aigc::{AigcStatus, labeling_status, write_label};
pub use config::{AiConfig, ConfigError, ProviderSelection, SCHEMA};
pub use credential::{
    CredentialError, CredentialStore, MemoryCredentialStore, Secret, UnavailableSystemStore, redact,
};
pub use fake::{FakeProvider, Script};
pub use gate::{Admission, AiGate, AiPhase, CommitDecision, GateError, Generation};
pub use http::{
    BuildError, Endpoint, EndpointError, HttpProvider, Outbound, RecordedExchange,
    RecordingTransport, Transport,
};
pub use policy::{
    AdminPolicy, ManagedReason, OsPolicyBackend, PolicySource, SettingsView, StaticPolicy,
    UnmanagedPolicy,
};
pub use provider::{
    CancelFlag, Capabilities, CompletionRequest, CompletionResponse, IdError, LogCredential,
    LoggedRequest, Provider, ProviderError, ProviderId, RequestLog,
};
pub use runtime::{AgentError, AgentRuntime, BootReport};
pub use telemetry::Telemetry;
pub use tools::{ApplyBlocked, ChangeProposal, ToolId, ToolUnavailable};
