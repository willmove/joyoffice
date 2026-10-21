//! AIGC labeling placeholder (AI.md §6).
//!
//! GB 45438-2025 and EU AI Act article 50 are in force, but whether a desktop
//! application whose user configures a provider is an obligated party was not
//! assessed for this milestone. This module does not write explicit labels or
//! file metadata, and it does not claim compliance. Existing label metadata in
//! a package stays the format layer's concern; nothing here strips it.

/// The legal assessment has not been recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AigcStatus {
    /// Do not emit implicit or explicit AIGC labels from this result.
    AssessmentPending,
}

/// Always [`AigcStatus::AssessmentPending`] until an assessment is recorded.
pub fn labeling_status() -> AigcStatus {
    AigcStatus::AssessmentPending
}

/// Refuses to write an AIGC label. There is no success path in T400.
pub fn write_label() -> Result<(), AigcStatus> {
    Err(labeling_status())
}
