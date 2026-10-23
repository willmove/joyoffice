//! Bounded T402 replacement (AI.md §2–§4). [`propose`] reserves one id and
//! [`preview`] re-reads the authorized text without writing. [`accept`]
//! re-checks the same grant and asks a [`DocumentCommit`] host to apply the
//! whole selection as one undoable transaction. This crate does not link the
//! engine; the host's commit implementation performs that transaction and
//! stamps [`AI_REVISION_AUTHOR`]. Preview never commits.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::document::{DocumentPort, DocumentSnapshot};
use crate::gate::{AiPhase, GateError};
use crate::runtime::AgentRuntime;
use crate::scope::{Grant, ScopeError, ScopeRange, TickClock};
use crate::scoped::{ToolError, fresh_snapshot};

/// `w:author` a host must store on the revision created by [`accept`].
/// The engine constant of the same name is the stamp `apply_ai_text_replace` writes.
pub const AI_REVISION_AUTHOR: &str = "JoyWord AI";

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

/// Opaque validated proposal. Bindings and payload cannot be edited or
/// constructed by a caller. Preview and accept both go through [`crate::ToolSession`]
/// and re-check the original grant; there is no body getter.
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
    /// This proposal id was already accepted, or another accept is in flight.
    AlreadyApplied,
    /// The authorized range is empty or leaves the final-view text.
    RangeRefused,
    /// The replacement cannot be stored as ordinary paragraph text.
    InvalidReplacement,
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
            Self::AlreadyApplied => "replace-already-applied",
            Self::RangeRefused => "replace-range-refused",
            Self::InvalidReplacement => "replace-invalid-replacement",
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

/// Separate from the legacy commit-admission table. Reservations neither
/// increment that counter nor call its hook. Identity survives runtime moves,
/// but another runtime (even with the same generation and IDs) cannot reuse it.
///
/// `Proposed` becomes `Applying` for the duration of one commit and `Applied`
/// only after that commit returns success. A refusal releases the claim so a
/// later accept of the same proposal can still succeed once; a second success
/// is refused.
pub(crate) struct ReplaceRegistry {
    ids: Mutex<BTreeMap<String, Reservation>>,
    origin: Arc<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reservation {
    Proposed,
    Applying,
    Applied,
}

impl ReplaceRegistry {
    pub(crate) fn new() -> Self {
        Self {
            ids: Mutex::new(BTreeMap::new()),
            origin: Arc::new(()),
        }
    }

    fn reserve(&self, id: &str) -> Result<Arc<()>, ReplaceError> {
        let mut ids = self
            .ids
            .lock()
            .map_err(|_| ReplaceError::RegistryPoisoned)?;
        if ids.contains_key(id) {
            return Err(ReplaceError::DuplicateId);
        }
        if ids.len() >= MAX_REPLACE_IDS {
            return Err(ReplaceError::IdCapacityReached);
        }
        ids.insert(id.to_owned(), Reservation::Proposed);
        Ok(Arc::clone(&self.origin))
    }

    fn begin_apply(&self, id: &str) -> Result<ApplyGuard<'_>, ReplaceError> {
        let mut ids = self
            .ids
            .lock()
            .map_err(|_| ReplaceError::RegistryPoisoned)?;
        match ids.get(id).copied() {
            Some(Reservation::Proposed) => {
                ids.insert(id.to_owned(), Reservation::Applying);
            }
            Some(Reservation::Applying | Reservation::Applied) => {
                return Err(ReplaceError::AlreadyApplied);
            }
            None => return Err(ReplaceError::BindingMismatch),
        }
        drop(ids);
        Ok(ApplyGuard {
            registry: self,
            id: id.to_owned(),
            armed: true,
        })
    }

    fn finish_apply(&self, id: &str, committed: bool) {
        let Ok(mut ids) = self.ids.lock() else {
            return;
        };
        if ids.get(id).copied() != Some(Reservation::Applying) {
            return;
        }
        ids.insert(
            id.to_owned(),
            if committed {
                Reservation::Applied
            } else {
                Reservation::Proposed
            },
        );
    }
}

struct ApplyGuard<'a> {
    registry: &'a ReplaceRegistry,
    id: String,
    armed: bool,
}

impl ApplyGuard<'_> {
    fn commit(mut self) {
        self.registry.finish_apply(&self.id, true);
        self.armed = false;
    }
}

impl Drop for ApplyGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.registry.finish_apply(&self.id, false);
        }
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

/// What the host writes, after the agent has re-validated the grant. Fields
/// stay private so a caller cannot assemble a commit the agent did not check.
/// `expected` and `replacement` are untrusted document text: [`Debug`] reports
/// lengths only.
pub struct ReplaceCommit<'a> {
    document_id: &'a crate::scope::DocumentId,
    base_version: u64,
    range: ScopeRange,
    expected: &'a str,
    replacement: &'a str,
}

impl ReplaceCommit<'_> {
    pub fn document_id(&self) -> &crate::scope::DocumentId {
        self.document_id
    }

    pub const fn base_version(&self) -> u64 {
        self.base_version
    }

    pub const fn range(&self) -> ScopeRange {
        self.range
    }

    /// Authorized text at validation time. The host compares this to the bytes
    /// it is about to overwrite and refuses on mismatch.
    pub fn expected(&self) -> &str {
        self.expected
    }

    pub fn replacement(&self) -> &str {
        self.replacement
    }
}

impl fmt::Debug for ReplaceCommit<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplaceCommit")
            .field("base_version", &self.base_version)
            .field("range_chars", &self.range.len_chars())
            .field("expected_bytes", &self.expected.len())
            .field("replacement_bytes", &self.replacement.len())
            .finish()
    }
}

/// Result of one successful accept. `author` is the required revision author,
/// not a string read back from the document.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReplaceReceipt {
    version: u64,
    undo_depth: u64,
}

impl ReplaceReceipt {
    /// Hosts build this after a commit that actually landed. `undo_depth` is
    /// the number of undo steps available afterwards.
    pub const fn new(version: u64, undo_depth: u64) -> Self {
        Self {
            version,
            undo_depth,
        }
    }

    pub const fn version(self) -> u64 {
        self.version
    }

    pub const fn undo_depth(self) -> u64 {
        self.undo_depth
    }

    pub const fn author(self) -> &'static str {
        AI_REVISION_AUTHOR
    }
}

impl fmt::Debug for ReplaceReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplaceReceipt")
            .field("version", &self.version)
            .field("undo_depth", &self.undo_depth)
            .field("author", &self.author())
            .finish()
    }
}

/// Why a host commit wrote nothing. Static labels only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitError {
    Stale,
    Mismatch,
    RangeRefused,
    InvalidReplacement,
    Unauthorized,
    Rejected,
    NothingToUndo,
    Poisoned,
}

impl CommitError {
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Stale => "commit-stale",
            Self::Mismatch => "commit-mismatch",
            Self::RangeRefused => "commit-range-refused",
            Self::InvalidReplacement => "commit-invalid-replacement",
            Self::Unauthorized => "commit-unauthorized",
            Self::Rejected => "commit-rejected",
            Self::NothingToUndo => "commit-nothing-to-undo",
            Self::Poisoned => "commit-poisoned",
        }
    }
}

impl fmt::Display for CommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind())
    }
}

impl std::error::Error for CommitError {}

impl From<CommitError> for ToolError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error)
    }
}

/// Host seam for one authorized replacement. [`DocumentPort::snapshot`] and
/// [`Self::commit_replace`] must describe the same document. `authorize` is
/// the gate check the host calls immediately before mutating; a refusal from
/// it means write nothing. `undo_replace` reverses exactly one successful commit.
pub trait DocumentCommit: DocumentPort {
    fn commit_replace(
        &self,
        change: &ReplaceCommit<'_>,
        authorize: &mut dyn FnMut() -> Result<(), ToolError>,
    ) -> Result<ReplaceReceipt, CommitError>;

    fn undo_replace(&self) -> Result<(), CommitError>;
}

pub(crate) fn accept(
    runtime: &AgentRuntime,
    grant: &Grant,
    clock: &TickClock,
    proposal: &ReplaceProposal,
    document: &dyn DocumentCommit,
) -> Result<ReplaceReceipt, ToolError> {
    if proposal.grant != *grant
        || !Arc::ptr_eq(&proposal.origin, &runtime.replace_registry().origin)
    {
        return Err(ReplaceError::BindingMismatch.into());
    }
    // Claim the id before re-reading the document. A second accept, including
    // one that races the first, refuses here and never writes. A failed
    // re-check drops the claim so one later retry can still apply once.
    let guard = runtime.replace_registry().begin_apply(&proposal.id)?;
    if !replacement_allowed(&proposal.replacement) {
        return Err(ReplaceError::InvalidReplacement.into());
    }
    let snapshot = fresh_snapshot(runtime, document, grant, clock)?;
    if !single_paragraph(&snapshot, grant.range()) {
        return Err(ReplaceError::RangeRefused.into());
    }
    let before = snapshot
        .slice_chars(grant.range())
        .ok_or(ToolError::Scope(ScopeError::RangeOutOfBounds))?;
    if before.len() > MAX_REPLACE_TEXT_BYTES {
        return Err(ReplaceError::BeforeTooLarge.into());
    }
    let change = ReplaceCommit {
        document_id: grant.document_id(),
        base_version: grant.base_version(),
        range: grant.range(),
        expected: before,
        replacement: &proposal.replacement,
    };
    commit_gate(runtime, grant)?;
    let receipt = match document.commit_replace(&change, &mut || commit_gate(runtime, grant)) {
        Ok(receipt) => receipt,
        Err(CommitError::Unauthorized) => {
            // The host called `authorize` and it failed. Report the live gate
            // reason when that is why, after releasing the claim on drop.
            commit_gate(runtime, grant)?;
            return Err(CommitError::Unauthorized.into());
        }
        Err(error) => return Err(error.into()),
    };
    guard.commit();
    runtime.note_applied_replacement();
    Ok(receipt)
}

fn commit_gate(runtime: &AgentRuntime, grant: &Grant) -> Result<(), ToolError> {
    let gate = runtime.gate();
    let phase = gate.phase().map_err(ToolError::Gate)?;
    if !matches!(phase, AiPhase::EnabledIdle | AiPhase::Running) {
        return Err(ToolError::Gate(match phase {
            AiPhase::Stopping => GateError::Stopping,
            AiPhase::Disabled => GateError::Disabled,
            AiPhase::EnabledIdle | AiPhase::Running => GateError::Disabled,
        }));
    }
    let generation = gate.generation().map_err(ToolError::Gate)?;
    if generation != grant.issued_generation() {
        return Err(ToolError::Scope(ScopeError::StaleGeneration));
    }
    Ok(())
}

fn single_paragraph(snapshot: &DocumentSnapshot, range: ScopeRange) -> bool {
    // A replace may cross paragraph marks inside the authorized range; the
    // host commits the covered separators as mark deletions (T402). Only an
    // empty range or one past the end of the final-view text is refused
    // here; structural refusals (tables, inline objects) stay with the host.
    !range.is_empty() && range.end() <= snapshot.char_len()
}

fn replacement_allowed(text: &str) -> bool {
    !text.chars().any(|ch| {
        ch == '\u{FDD0}'
            || ch == '\u{FFFC}'
            || ch == '\u{2028}'
            || ch == '\u{2029}'
            || (ch.is_control() && ch != '\t')
    })
}
