//! JoyWord AI foundation (T400/T401, AI-001, AI-002, AI-004, AI-005,
//! QUALITY-007).
//!
//! The crate is optional. Document editing crates do not depend on it, and it
//! does not depend on them. Nothing here opens a socket, downloads a model, or
//! applies a [`ChangeProposal`](tools::ChangeProposal). The first-launch state
//! is disabled; a generation counter is the kill-switch linearization point.
//!
//! T401 adds scoped authorization ([`scope`]), an abstract read-only document
//! port with an in-memory fake ([`document`]), and allowlist-enforced
//! restricted tools over the minimal context ([`scoped`]). The port is a seam,
//! not a dependency: the gate script keeps document and engine crates out of
//! this crate, so a host implements [`DocumentPort`](document::DocumentPort).
//!
//! T402 adds an opaque [`ReplaceProposal`] for one complete, nonempty
//! authorized selection, a side-effect-free [`ReplacePreview`], and
//! [`ToolSession::accept_replace`](scoped::ToolSession::accept_replace). Accept
//! re-validates the grant and commits through [`DocumentCommit`]; this crate
//! still does not link the editing engine or a provider.
//!
//! AIGC labeling is not implemented. [`aigc`] records that the legal assessment
//! is still pending and refuses to write a label.

#![forbid(unsafe_code)]

mod aigc;
mod config;
mod credential;
mod document;
mod fake;
mod gate;
mod http;
mod policy;
mod provider;
mod replace;
mod runtime;
mod scope;
mod scoped;
mod telemetry;
mod tools;

pub use aigc::{AigcStatus, labeling_status, write_label};
pub use config::{AiConfig, ConfigError, ProviderSelection, SCHEMA};
pub use credential::{
    CredentialError, CredentialStore, MemoryCredentialStore, Secret, UnavailableSystemStore, redact,
};
pub use document::{
    DocumentPort, DocumentSnapshot, MAX_DOCUMENT_TEXT_BYTES, MAX_PARAGRAPH_BYTES, MAX_PARAGRAPHS,
    MemoryDocument, MemoryParagraph, OutlineNode, PortError, SnapshotParagraph,
};
pub use fake::{FakeProvider, RecordingProvider, Script};
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
pub use replace::{
    AI_REVISION_AUTHOR, CommitError, DocumentCommit, MAX_REPLACE_EXPLANATION_BYTES,
    MAX_REPLACE_ID_BYTES, MAX_REPLACE_IDS, MAX_REPLACE_TEXT_BYTES, ReplaceCommit, ReplaceError,
    ReplacePreview, ReplaceProposal, ReplaceReceipt, ReplaceRequest,
};
pub use runtime::{AgentError, AgentRuntime, BootReport};
pub use scope::{
    ContentFingerprint, DocumentId, Grant, MAX_SCOPE_CHARS, MAX_TTL_TICKS, ScopeError, ScopeRange,
    TickClock, ToolAllowlist,
};
pub use scoped::{
    Citation, Context, MAX_CONTEXT_CONTENT_BYTES, MAX_HITS, MAX_QUERY_CHARS, MAX_QUESTION_BYTES,
    MAX_STRUCTURE_NODES, SearchHits, SelectionText, StructureNode, StructureProjection, ToolCall,
    ToolError, ToolOutput, ToolSession,
};
pub use telemetry::Telemetry;
pub use tools::{ApplyBlocked, ChangeProposal, ToolId, ToolUnavailable};
