//! First bounded T402 slice (AI.md §2, §4): one full-selection plain-text
//! replacement and a local preview, not an editing transaction. The host must
//! issue the grant from a user action; model data cannot supply its bindings.
//! No engine mapping, apply, undo, authorship, or provider call lives here.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::document::DocumentPort;
use crate::runtime::AgentRuntime;
use crate::scope::{Grant, ScopeError, TickClock};
use crate::scoped::{ToolError, fresh_snapshot};

/// Independent byte budgets for the complete before and replacement texts.
pub const MAX_REPLACE_TEXT_BYTES: usize = 24 * 1024;
pub const MAX_REPLACE_EXPLANATION_BYTES: usize = 4 * 1024;
pub const MAX_REPLACE_ID_BYTES: usize = 128;
/// Runtime-lifetime reservations. Full means refusal, never eviction.
pub const MAX_REPLACE_IDS: usize = 256;

/// Untrusted, borrowed payload. Exactly one replacement of the entire grant
/// range; empty replacement means deletion. Text and explanation are inert
/// UTF-8, with no parsing, normalization, or truncation. IDs use nonempty ASCII
/// letters/digits, `-`, and `_`. All lengths are bytes, not scalar counts.
#[derive(Clone, Copy)]
pub struct ReplaceRequest<'a> {
    pub id: &'a str,
    pub replacement: &'a str,
    pub explanation: &'a str,
}

impl fmt::Debug for ReplaceRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplaceRequest")
            .field("id_bytes", &self.id.len())
            .field("replacement_bytes", &self.replacement.len())
            .field("explanation_bytes", &self.explanation.len())
            .finish()
    }
}

impl ReplaceRequest<'_> {
    fn validate(&self) -> Result<(), ReplaceError> {
        if self.id.is_empty()
            || self.id.len() > MAX_REPLACE_ID_BYTES
            || !self
                .id
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-' || ch == b'_')
        {
            return Err(ReplaceError::InvalidId);
        }
        if self.replacement.len() > MAX_REPLACE_TEXT_BYTES {
            return Err(ReplaceError::ReplacementTooLarge);
        }
        if self.explanation.len() > MAX_REPLACE_EXPLANATION_BYTES {
            return Err(ReplaceError::ExplanationTooLarge);
        }
        Ok(())
    }
}

/// Opaque validated proposal, usable only through `ToolSession::preview_replace`.
/// Bindings and payload cannot be edited or constructed by a caller. No body
/// getter bypasses the fresh authorization check, and there is no apply API.
///
/// ```compile_fail
/// use joyword_agent::{Grant, ReplaceProposal};
/// fn rebind(proposal: &mut ReplaceProposal, grant: Grant) {
///     proposal.grant = grant;
/// }
/// ```
pub struct ReplaceProposal {
    id: String,
    grant: Grant,
    origin: Arc<()>,
    replacement: String,
    explanation: String,
}

impl fmt::Debug for ReplaceProposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Do not format the Grant: document IDs are untrusted too.
        f.debug_struct("ReplaceProposal")
            .field("id_bytes", &self.id.len())
            .field("replacement_bytes", &self.replacement.len())
            .field("explanation_bytes", &self.explanation.len())
            .field("base_version", &self.grant.base_version())
            .field("range_chars", &self.grant.range().len_chars())
            .finish()
    }
}

/// An owned, point-in-time view of authorized text only. This is not a commit
/// token. A host must discard displayed previews when its authorization changes
/// and call `preview_replace` again before showing a refreshed view.
#[derive(Clone, PartialEq, Eq)]
pub struct ReplacePreview {
    before: String,
    after: String,
    explanation: String,
}

impl ReplacePreview {
    pub fn before(&self) -> &str {
        &self.before
    }

    pub fn after(&self) -> &str {
        &self.after
    }

    pub fn explanation(&self) -> &str {
        &self.explanation
    }
}

impl fmt::Debug for ReplacePreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplacePreview")
            .field("before_bytes", &self.before.len())
            .field("after_bytes", &self.after.len())
            .field("explanation_bytes", &self.explanation.len())
            .finish()
    }
}

/// Static diagnostics: never carry IDs, document content, or explanation text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplaceError {
    InvalidId,
    ReplacementTooLarge,
    ExplanationTooLarge,
    BeforeTooLarge,
    EmptySelection,
    BindingMismatch,
    DuplicateId,
    IdCapacityReached,
    RegistryPoisoned,
}

impl ReplaceError {
    pub const fn kind(self) -> &'static str {
        match self {
            Self::InvalidId => "replace-invalid-id",
            Self::ReplacementTooLarge => "replace-text-too-large",
            Self::ExplanationTooLarge => "replace-explanation-too-large",
            Self::BeforeTooLarge => "replace-before-too-large",
            Self::EmptySelection => "replace-empty-selection",
            Self::BindingMismatch => "replace-binding-mismatch",
            Self::DuplicateId => "replace-duplicate-id",
            Self::IdCapacityReached => "replace-id-capacity-reached",
            Self::RegistryPoisoned => "replace-registry-poisoned",
        }
    }
}

impl fmt::Display for ReplaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind())
    }
}

impl std::error::Error for ReplaceError {}

impl From<ReplaceError> for ToolError {
    fn from(error: ReplaceError) -> Self {
        Self::Replace(error)
    }
}

/// Separate from commit admission: reservations neither increment commit
/// counters nor call any gate commit hook. Identity survives runtime moves,
/// but another runtime (even with the same generation and IDs) cannot reuse it.
pub(crate) struct ReplaceRegistry {
    ids: Mutex<BTreeSet<String>>,
    origin: Arc<()>,
}

impl ReplaceRegistry {
    pub(crate) fn new() -> Self {
        Self {
            ids: Mutex::new(BTreeSet::new()),
            origin: Arc::new(()),
        }
    }

    fn reserve(&self, id: &str) -> Result<Arc<()>, ReplaceError> {
        let mut ids = self
            .ids
            .lock()
            .map_err(|_| ReplaceError::RegistryPoisoned)?;
        if ids.contains(id) {
            return Err(ReplaceError::DuplicateId);
        }
        if ids.len() >= MAX_REPLACE_IDS {
            return Err(ReplaceError::IdCapacityReached);
        }
        ids.insert(id.to_owned());
        Ok(Arc::clone(&self.origin))
    }
}

// Both helpers are reachable from allowlist-checked ToolSession methods only.
pub(crate) fn propose(
    runtime: &AgentRuntime,
    port: &dyn DocumentPort,
    grant: &Grant,
    clock: &TickClock,
    request: ReplaceRequest<'_>,
) -> Result<ReplaceProposal, ToolError> {
    request.validate()?;
    let snapshot = fresh_snapshot(runtime, port, grant, clock)?;
    if grant.range().is_empty() {
        return Err(ReplaceError::EmptySelection.into());
    }
    let before = snapshot
        .slice_chars(grant.range())
        .ok_or(ToolError::Scope(ScopeError::RangeOutOfBounds))?;
    if before.len() > MAX_REPLACE_TEXT_BYTES {
        return Err(ReplaceError::BeforeTooLarge.into());
    }
    // Reserve only after all validation succeeds. Duplicate detection and
    // insertion share one lock across every ToolSession on this runtime.
    let origin = runtime.replace_registry().reserve(request.id)?;
    Ok(ReplaceProposal {
        id: request.id.to_owned(),
        grant: grant.clone(),
        origin,
        replacement: request.replacement.to_owned(),
        explanation: request.explanation.to_owned(),
    })
}

pub(crate) fn preview(
    runtime: &AgentRuntime,
    port: &dyn DocumentPort,
    grant: &Grant,
    clock: &TickClock,
    proposal: &ReplaceProposal,
) -> Result<ReplacePreview, ToolError> {
    // Full equality also prevents a wider/narrower scope, a renewed TTL, or a
    // different allowlist from laundering the original authorization.
    if proposal.grant != *grant
        || !Arc::ptr_eq(&proposal.origin, &runtime.replace_registry().origin)
    {
        return Err(ReplaceError::BindingMismatch.into());
    }
    let snapshot = fresh_snapshot(runtime, port, grant, clock)?;
    let before = snapshot
        .slice_chars(grant.range())
        .ok_or(ToolError::Scope(ScopeError::RangeOutOfBounds))?;
    if before.len() > MAX_REPLACE_TEXT_BYTES {
        return Err(ReplaceError::BeforeTooLarge.into());
    }
    // No registry mutation, provider call, or document mutation in a preview.
    Ok(ReplacePreview {
        before: before.to_owned(),
        after: proposal.replacement.clone(),
        explanation: proposal.explanation.clone(),
    })
}
