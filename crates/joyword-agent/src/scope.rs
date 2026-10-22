//! Scoped authorization for T401 (AI.md §2, AI-002, AI-004).
//!
//! A [`Grant`] is the capability token a user action produces. It binds a
//! document identity, the base version of the snapshot the user saw, a content
//! fingerprint of the authorized range, the character range itself, the tool
//! allowlist, the AI generation it was issued under, and an expiry tick. Every
//! tool call and every context build re-validates all of it; a mismatch is an
//! explicit, auditable refusal.
//!
//! JoyWord assumption: [`ContentFingerprint`] is a non-cryptographic FNV-1a
//! 64-bit digest. It detects a swapped or edited snapshot, not an adversarial
//! collision; enforcement comes from the version check plus explicit offsets.
//! JoyWord assumption: expiry is driven by a caller-advanced [`TickClock`], so
//! tests and audits are deterministic without a wall-clock dependency.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::document::{DocumentPort, DocumentSnapshot, PortError};
use crate::gate::Generation;
use crate::provider::IdError;
use crate::tools::ToolId;

/// Longest authorized range in characters.
pub const MAX_SCOPE_CHARS: u64 = 1 << 20;
/// Longest grant lifetime in ticks.
pub const MAX_TTL_TICKS: u64 = 1 << 40;

/// Stable document identity. Not a path and never a filesystem handle: this
/// crate has no file access (T111 filesystem allowlist).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(String);

impl DocumentId {
    pub fn parse(value: &str) -> Result<Self, IdError> {
        if value.is_empty()
            || value.len() > 128
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

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// FNV-1a 64-bit digest of authorized content. See the module note.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentFingerprint(u64);

impl ContentFingerprint {
    pub fn compute(text: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self(hash)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for ContentFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentFingerprint({:016x})", self.0)
    }
}

/// Half-open character range `[start, end)` inside the visible text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScopeRange {
    start: u64,
    end: u64,
}

impl ScopeRange {
    pub fn new(start: u64, end: u64) -> Result<Self, ScopeError> {
        if start > end || end - start > MAX_SCOPE_CHARS {
            return Err(ScopeError::RangeOutOfBounds);
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub const fn len_chars(self) -> u64 {
        self.end - self.start
    }
}

/// Tools the user allowed. The model can never widen it; there is no API that
/// mutates an issued allowlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolAllowlist {
    tools: BTreeSet<ToolId>,
}

impl ToolAllowlist {
    pub fn new() -> Self {
        Self {
            tools: BTreeSet::new(),
        }
    }

    /// The read-only set from AI.md §2. Propose tools stay out until T402.
    pub fn read_only() -> Self {
        Self::from_slice(&[
            ToolId::ReadSelection,
            ToolId::ReadStructure,
            ToolId::SearchDocument,
        ])
    }

    pub fn from_slice(tools: &[ToolId]) -> Self {
        let mut set = Self::new();
        for tool in tools {
            set.tools.insert(*tool);
        }
        set
    }

    pub fn with(mut self, tool: ToolId) -> Self {
        self.tools.insert(tool);
        self
    }

    pub fn contains(&self, tool: ToolId) -> bool {
        self.tools.contains(&tool)
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = ToolId> + '_ {
        self.tools.iter().copied()
    }
}

impl Default for ToolAllowlist {
    fn default() -> Self {
        Self::new()
    }
}

/// Caller-driven monotonic clock. Deterministic for tests and audits.
#[derive(Debug, Default)]
pub struct TickClock {
    ticks: AtomicU64,
}

impl TickClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn now(&self) -> u64 {
        self.ticks.load(Ordering::SeqCst)
    }

    pub fn advance(&self, ticks: u64) {
        self.ticks.fetch_add(ticks, Ordering::SeqCst);
    }
}

/// Why a scoped operation was refused. Every variant is a static label plus
/// numbers; none carries document text (QUALITY-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeError {
    DocumentMismatch,
    VersionChanged { expected: u64, found: u64 },
    FingerprintChanged,
    RangeOutOfBounds,
    Expired,
    StaleGeneration,
    TtlInvalid,
    Port(PortError),
}

impl ScopeError {
    /// Static label safe for logs and telemetry.
    pub const fn kind(self) -> &'static str {
        match self {
            ScopeError::DocumentMismatch => "scope-document-mismatch",
            ScopeError::VersionChanged { .. } => "scope-version-changed",
            ScopeError::FingerprintChanged => "scope-fingerprint-changed",
            ScopeError::RangeOutOfBounds => "scope-range-out-of-bounds",
            ScopeError::Expired => "scope-expired",
            ScopeError::StaleGeneration => "scope-stale-generation",
            ScopeError::TtlInvalid => "scope-ttl-invalid",
            ScopeError::Port(error) => error.kind(),
        }
    }
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeError::DocumentMismatch => f.write_str("grant does not match this document"),
            ScopeError::VersionChanged { expected, found } => write!(
                f,
                "document version changed since authorization (base {expected}, current {found})"
            ),
            ScopeError::FingerprintChanged => f.write_str("authorized content fingerprint changed"),
            ScopeError::RangeOutOfBounds => f.write_str("authorized range is out of bounds"),
            ScopeError::Expired => f.write_str("authorization expired"),
            ScopeError::StaleGeneration => {
                f.write_str("authorization predates the current ai generation")
            }
            ScopeError::TtlInvalid => f.write_str("authorization lifetime is invalid"),
            ScopeError::Port(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ScopeError {}

/// The capability token. Immutable after issuance; re-validate before every
/// use against a fresh snapshot, the live generation, and the clock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    document_id: DocumentId,
    base_version: u64,
    fingerprint: ContentFingerprint,
    range: ScopeRange,
    tools: ToolAllowlist,
    issued_generation: Generation,
    expires_at_tick: u64,
}

impl Grant {
    /// User-action entry point: reads the current snapshot through the port,
    /// checks the requested range, and binds everything above into a token.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        port: &dyn DocumentPort,
        document: &DocumentId,
        range: ScopeRange,
        tools: ToolAllowlist,
        generation: Generation,
        clock: &TickClock,
        ttl_ticks: u64,
    ) -> Result<Self, ScopeError> {
        if ttl_ticks == 0 || ttl_ticks > MAX_TTL_TICKS {
            return Err(ScopeError::TtlInvalid);
        }
        let snapshot = port.snapshot(document).map_err(ScopeError::Port)?;
        let authorized = snapshot
            .slice_chars(range)
            .ok_or(ScopeError::RangeOutOfBounds)?;
        Ok(Self {
            document_id: document.clone(),
            base_version: snapshot.version(),
            fingerprint: ContentFingerprint::compute(authorized),
            range,
            tools,
            issued_generation: generation,
            expires_at_tick: clock.now().saturating_add(ttl_ticks),
        })
    }

    /// The check every scoped operation runs first.
    pub fn validate(
        &self,
        snapshot: &DocumentSnapshot,
        generation: Generation,
        now_tick: u64,
    ) -> Result<(), ScopeError> {
        if generation != self.issued_generation {
            return Err(ScopeError::StaleGeneration);
        }
        if now_tick > self.expires_at_tick {
            return Err(ScopeError::Expired);
        }
        if snapshot.document_id() != &self.document_id {
            return Err(ScopeError::DocumentMismatch);
        }
        if snapshot.version() != self.base_version {
            return Err(ScopeError::VersionChanged {
                expected: self.base_version,
                found: snapshot.version(),
            });
        }
        let Some(authorized) = snapshot.slice_chars(self.range) else {
            return Err(ScopeError::RangeOutOfBounds);
        };
        if ContentFingerprint::compute(authorized) != self.fingerprint {
            return Err(ScopeError::FingerprintChanged);
        }
        Ok(())
    }

    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    pub const fn base_version(&self) -> u64 {
        self.base_version
    }

    pub const fn fingerprint(&self) -> ContentFingerprint {
        self.fingerprint
    }

    pub const fn range(&self) -> ScopeRange {
        self.range
    }

    pub fn tools(&self) -> &ToolAllowlist {
        &self.tools
    }

    pub const fn issued_generation(&self) -> Generation {
        self.issued_generation
    }

    pub const fn expires_at_tick(&self) -> u64 {
        self.expires_at_tick
    }
}
