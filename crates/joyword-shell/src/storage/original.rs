//! Original-file replacement gate (W07): conflict re-verification plus an
//! honest platform capability matrix, and a conservative refusal while the
//! architecture's invariants cannot be proven.
//!
//! The standard save flow (ARCHITECTURE §5) requires proving that (a) the
//! export plan originates from exactly this source package, (b) publication
//! replaces the verified object under a race-safe strategy, and (c) the
//! replaced file keeps permissions/ACL, links, extended attributes, creation
//! time, read-only state and provenance marks. Until every item is provable,
//! `save_original` performs no write at all and reports the durable blockers.
//! This is a real refusal, not a degraded success: the source stays
//! untouched and the verified `save_export_copy` remains the fallback.

use std::path::PathBuf;

use joyword_docx::{ExportPlan, ExportTarget};
use joyword_model::DocumentVersion;

use super::error::{MetadataItem, ReplacementBlocker, SaveOriginalError, SourceStage};
use super::source::{SourceStamp, check_source};

/// Persistence state of a completed original replacement. Reused from the
/// C03 copy path: `FileSynced` means the published file accepted a sync;
/// `FileAndDirectorySynced` adds the containing directory. Neither claims
/// power-loss testing.
pub type OriginalDurability = super::CopyDurability;

/// Receipt for a completed original replacement. It can only be produced by
/// the real replacement write path; while that path is unavailable
/// `save_original` never constructs one, and an error after publication must
/// carry the receipt instead of pretending nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveOriginalReceipt {
    path: PathBuf,
    version: DocumentVersion,
    durability: OriginalDurability,
}

impl SaveOriginalReceipt {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn version(&self) -> DocumentVersion {
        self.version
    }
    pub fn durability(&self) -> OriginalDurability {
        self.durability
    }
}

/// What this build can do with one metadata aspect of a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataCapability {
    /// Observable, and an approved write path (`std::fs::set_permissions`
    /// or the platform read-only attribute) can restore it on a new inode.
    /// This is a statement about capability, not an executed replacement.
    Preservable,
    /// Observable, but no approved dependency can restore it (ownership,
    /// creation time); replacement is refused.
    ObservableNotPreservable,
    /// Not even observable with approved dependencies (ACLs, extended
    /// attributes, provenance marks), so silent loss could not be detected;
    /// replacement is refused.
    Unobservable,
}

/// Per-item capability matrix for original replacement on the current
/// platform build. It grounds the conservative refusal in facts a test can
/// assert; it never authorizes a write by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriginalReplacementCapabilities {
    pub permissions: MetadataCapability,
    pub ownership: MetadataCapability,
    pub acl: MetadataCapability,
    pub extended_attributes: MetadataCapability,
    pub creation_time: MetadataCapability,
    pub links: MetadataCapability,
    pub provenance_marks: MetadataCapability,
}

/// The capability matrix of this build. `std`-only, safe Rust, no new
/// dependencies: permission bits are the only preservable item; ACLs,
/// extended attributes and provenance marks are not even observable, which
/// is why original replacement refuses rather than risking silent loss.
/// Windows entries describe the compiled target, not a tested one.
pub fn original_replacement_capabilities() -> OriginalReplacementCapabilities {
    OriginalReplacementCapabilities {
        permissions: MetadataCapability::Preservable,
        #[cfg(unix)]
        ownership: MetadataCapability::ObservableNotPreservable,
        #[cfg(not(unix))]
        ownership: MetadataCapability::Unobservable,
        acl: MetadataCapability::Unobservable,
        extended_attributes: MetadataCapability::Unobservable,
        creation_time: MetadataCapability::ObservableNotPreservable,
        links: MetadataCapability::ObservableNotPreservable,
        provenance_marks: MetadataCapability::Unobservable,
    }
}

/// Durable blockers that make replacement unavailable for this source now.
/// The origin-binding blocker is always present until the ExportPlan API
/// grows a verifiable source token (see the W07 report proposal); every
/// unpreservable or unobservable matrix item adds its own blocker.
fn replacement_blockers(source: &SourceStamp) -> Vec<ReplacementBlocker> {
    let caps = original_replacement_capabilities();
    let mut blockers = vec![ReplacementBlocker::OriginBindingUnproven];
    for (item, capability) in [
        (MetadataItem::Permissions, caps.permissions),
        (MetadataItem::Ownership, caps.ownership),
        (MetadataItem::Acl, caps.acl),
        (MetadataItem::ExtendedAttributes, caps.extended_attributes),
        (MetadataItem::CreationTime, caps.creation_time),
        (MetadataItem::Links, caps.links),
        (MetadataItem::ProvenanceMarks, caps.provenance_marks),
    ] {
        match capability {
            MetadataCapability::Preservable => {}
            MetadataCapability::ObservableNotPreservable => {
                blockers.push(ReplacementBlocker::UnpreservableMetadata(item));
            }
            MetadataCapability::Unobservable => {
                blockers.push(ReplacementBlocker::UnobservableMetadata(item));
            }
        }
    }
    if source.identity().nlink() > 1 {
        blockers.push(ReplacementBlocker::LinkedSourceSemantics);
    }
    blockers
}

/// Replace the opened source file with a verified export plan.
///
/// Re-runs the full conflict verification itself (locks, path resolution,
/// identity, exact bytes) instead of trusting earlier calls, validates the
/// plan's package type against the source extension, and then evaluates the
/// capability blockers. While any blocker stands, this returns
/// [`SaveOriginalError::ReplacementUnavailable`] before writing anything:
/// original replacement is unimplemented/unverified this round, the source
/// file is left byte-identical with its times and links untouched, and no
/// temporary files or locks are created. Callers must report this as
/// "original replacement unavailable", never as a completed save.
pub fn save_original(
    plan: &ExportPlan,
    source: &SourceStamp,
) -> Result<SaveOriginalReceipt, SaveOriginalError> {
    let path = source.path();
    let extension = match plan.target() {
        ExportTarget::Docx => "docx",
        ExportTarget::Dotx => "dotx",
        ExportTarget::Docm => "docm",
        ExportTarget::Dotm => "dotm",
    };
    let matches = path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case(extension));
    if !matches {
        return Err(SaveOriginalError::io(
            SourceStage::Validate,
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "source extension must match the export plan's package type",
            ),
        ));
    }
    // Full re-verification inside this call; earlier check_source passes are
    // not treated as an atomic compare-and-swap against later races.
    check_source(source)?;
    let blockers = replacement_blockers(source);
    if !blockers.is_empty() {
        return Err(SaveOriginalError::ReplacementUnavailable {
            path: path.to_path_buf(),
            blockers,
        });
    }
    // Unreachable today: every build has at least the origin-binding
    // blocker. When a verified binding and a reviewed write path exist,
    // this is where the temp-file/link-verify/publish strategy goes.
    unreachable!("replacement blockers are never empty in this build")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn unix_matrix_is_honest_about_std_only_capabilities() {
        let caps = original_replacement_capabilities();
        assert_eq!(caps.permissions, MetadataCapability::Preservable);
        assert_eq!(caps.ownership, MetadataCapability::ObservableNotPreservable);
        assert_eq!(caps.acl, MetadataCapability::Unobservable);
        assert_eq!(caps.extended_attributes, MetadataCapability::Unobservable);
        assert_eq!(
            caps.creation_time,
            MetadataCapability::ObservableNotPreservable
        );
        assert_eq!(caps.links, MetadataCapability::ObservableNotPreservable);
        assert_eq!(caps.provenance_marks, MetadataCapability::Unobservable);
    }

    #[test]
    fn blockers_always_include_the_origin_binding_gap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.docx");
        std::fs::write(&path, b"0123456789").unwrap();
        let source = super::super::open_source(&path, &joyoffice_opc::Budget::default())
            .unwrap()
            .into_stamp();
        let blockers = replacement_blockers(&source);
        assert!(blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
        assert!(blockers.len() > 1);
    }
}
