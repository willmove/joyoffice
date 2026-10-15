//! Original-file replacement (W07 and its follow-up): conflict
//! re-verification, origin binding, an honest per-platform capability
//! matrix, and the Unix write path in `publish.rs`.
//!
//! The standard save flow (ARCHITECTURE §5) requires proving that (a) the
//! export plan originates from exactly this source package, (b) publication
//! replaces the verified object under a race-aware strategy, and (c) the
//! replaced file keeps permissions/ACL, links, extended attributes, creation
//! time, read-only state and provenance marks. `save_original` writes only
//! when (a) holds byte-exactly and every metadata item of this particular
//! file is either reproduced and verified on the new object or provably
//! absent. Otherwise it refuses before rewriting the source, and the verified
//! `save_export_copy` remains the fallback.

use std::fs;
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

/// Receipt for a completed original replacement. Only the real write path
/// produces one, after reading the published object back: its stamp holds
/// the identity and bytes of that read, never values copied from the plan.
/// An error after publication carries the receipt instead of pretending
/// nothing was written.
#[derive(Clone, Debug)]
pub struct SaveOriginalReceipt {
    path: PathBuf,
    version: DocumentVersion,
    durability: OriginalDurability,
    stamp: SourceStamp,
}

impl SaveOriginalReceipt {
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) fn new(
        path: PathBuf,
        version: DocumentVersion,
        durability: OriginalDurability,
        stamp: SourceStamp,
    ) -> Self {
        Self {
            path,
            version,
            durability,
            stamp,
        }
    }
    #[cfg_attr(not(unix), allow(dead_code))]
    pub(crate) fn with_durability(self, durability: OriginalDurability) -> Self {
        Self { durability, ..self }
    }
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    /// The document version of the plan that was published.
    pub fn version(&self) -> DocumentVersion {
        self.version
    }
    /// Length of the published file as read back.
    pub fn bytes(&self) -> u64 {
        self.stamp.len()
    }
    pub fn durability(&self) -> OriginalDurability {
        self.durability
    }
    /// Refreshed stamp of the published object, from its read-back. Later
    /// conflict checks and saves of this file must use it; the stamp given
    /// to `save_original` now describes a replaced object.
    pub fn stamp(&self) -> &SourceStamp {
        &self.stamp
    }
    pub fn into_stamp(self) -> SourceStamp {
        self.stamp
    }
}

/// What this build can do with one metadata aspect of a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataCapability {
    /// Restored on the new object with a `std` API and verified by reading
    /// it back before publication and again after it. A source whose value
    /// cannot be applied (for example an owner the process may not give a
    /// file to) is refused before publication.
    Preservable,
    /// Observable, but nothing in this build can reproduce it on a new
    /// object: a source carrying it is refused; a source without it is not.
    ObservableNotPreservable,
    /// Not observable with approved dependencies, so loss could not even be
    /// detected: every replacement is refused.
    Unobservable,
}

/// Per-item capability matrix for original replacement on the current
/// platform build. It grounds refusals in facts a test can assert; it never
/// authorizes a write by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriginalReplacementCapabilities {
    /// Whether this build has a replacement write path at all (Unix only).
    pub write_path: bool,
    pub permissions: MetadataCapability,
    pub ownership: MetadataCapability,
    pub acl: MetadataCapability,
    pub extended_attributes: MetadataCapability,
    pub creation_time: MetadataCapability,
    /// Symbolic links are written through: the resolved file is replaced and
    /// the link itself is untouched. Multiply hard-linked sources are refused.
    pub links: MetadataCapability,
    pub provenance_marks: MetadataCapability,
    pub file_flags: MetadataCapability,
}

/// The capability matrix of this build: `std` only, safe Rust, no new
/// dependencies.
///
/// - macOS: owner/group (`fchown`), permission bits and creation time
///   (`FileTimesExt::set_created`) are restored and verified. Extended
///   attributes, ACLs and provenance marks are observed through `/bin/ls`
///   (see `observe.rs`) and refused when present; `std::fs::copy` is not
///   used to carry them because on this host its clone path dropped an ACL
///   and its copyfile path rewrote the quarantine value and creation time.
///   File flags are observed (`st_flags`) and refused when set.
/// - Other Unix: extended attributes (including POSIX ACLs and download
///   origin records) cannot be listed without new dependencies, and a birth
///   time cannot be set, so replacement is refused.
/// - Windows: no write path; the entries describe the compiled target only.
pub fn original_replacement_capabilities() -> OriginalReplacementCapabilities {
    use MetadataCapability as M;
    #[cfg(target_os = "macos")]
    let caps = OriginalReplacementCapabilities {
        write_path: true,
        permissions: M::Preservable,
        ownership: M::Preservable,
        acl: M::ObservableNotPreservable,
        extended_attributes: M::ObservableNotPreservable,
        creation_time: M::Preservable,
        links: M::ObservableNotPreservable,
        provenance_marks: M::ObservableNotPreservable,
        file_flags: M::ObservableNotPreservable,
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let caps = OriginalReplacementCapabilities {
        write_path: true,
        permissions: M::Preservable,
        ownership: M::Preservable,
        acl: M::Unobservable,
        extended_attributes: M::Unobservable,
        creation_time: M::ObservableNotPreservable,
        links: M::ObservableNotPreservable,
        provenance_marks: M::Unobservable,
        file_flags: M::Unobservable,
    };
    #[cfg(not(unix))]
    let caps = OriginalReplacementCapabilities {
        write_path: false,
        permissions: M::ObservableNotPreservable,
        ownership: M::Unobservable,
        acl: M::Unobservable,
        extended_attributes: M::Unobservable,
        creation_time: M::ObservableNotPreservable,
        links: M::ObservableNotPreservable,
        provenance_marks: M::Unobservable,
        file_flags: M::Unobservable,
    };
    caps
}

/// Metadata items the source carries that a replacement cannot reproduce,
/// or `Err` when they cannot be determined. Only items the matrix marks
/// `ObservableNotPreservable` are asked about; links are reported through
/// [`ReplacementBlocker::LinkedSourceSemantics`] instead.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn present_unpreservable(
    source: &SourceStamp,
    metadata: &fs::Metadata,
) -> Result<Vec<MetadataItem>, ()> {
    #[cfg(target_os = "macos")]
    {
        use std::os::darwin::fs::MetadataExt as _;
        let mut items = super::observe::observe(source.path())
            .map_err(|_| ())?
            .present_items();
        if metadata.st_flags() != 0 {
            items.push(MetadataItem::FileFlags);
        }
        Ok(items)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut items = Vec::new();
        if metadata.created().is_ok() {
            items.push(MetadataItem::CreationTime);
        }
        Ok(items)
    }
}

/// Durable reasons this plan may not replace this source. Empty means the
/// write path may run; it still restores, verifies and re-checks everything.
fn replacement_blockers(
    plan: &ExportPlan,
    source: &SourceStamp,
    metadata: &fs::Metadata,
) -> Vec<ReplacementBlocker> {
    let caps = original_replacement_capabilities();
    let mut blockers = Vec::new();
    // W10 binding: the backing bytes the plan was built from. A plan with
    // no proven origin (`None`) is refused like a mismatched one.
    if plan.origin_bytes() != Some(source.bytes()) {
        blockers.push(ReplacementBlocker::OriginBindingUnproven);
    }
    if !caps.write_path {
        blockers.push(ReplacementBlocker::UnsupportedPlatform);
    }
    if metadata.permissions().readonly() {
        blockers.push(ReplacementBlocker::ReadOnlySource);
    }
    if source.identity().nlink() > 1 {
        blockers.push(ReplacementBlocker::LinkedSourceSemantics);
    }
    let matrix = [
        (MetadataItem::Permissions, caps.permissions),
        (MetadataItem::Ownership, caps.ownership),
        (MetadataItem::Acl, caps.acl),
        (MetadataItem::ExtendedAttributes, caps.extended_attributes),
        (MetadataItem::CreationTime, caps.creation_time),
        (MetadataItem::Links, caps.links),
        (MetadataItem::ProvenanceMarks, caps.provenance_marks),
        (MetadataItem::FileFlags, caps.file_flags),
    ];
    let observable = |item| {
        matrix
            .iter()
            .any(|&(i, c)| i == item && c == MetadataCapability::ObservableNotPreservable)
    };
    for &(item, capability) in &matrix {
        if capability == MetadataCapability::Unobservable {
            blockers.push(ReplacementBlocker::UnobservableMetadata(item));
        }
    }
    match present_unpreservable(source, metadata) {
        Ok(items) => {
            for item in items {
                if item != MetadataItem::Links && observable(item) {
                    blockers.push(ReplacementBlocker::UnpreservableMetadata(item));
                }
            }
        }
        Err(()) => {
            // The observer failed; nothing it covers can be vouched for.
            for &(item, capability) in &matrix {
                if capability == MetadataCapability::ObservableNotPreservable
                    && item != MetadataItem::Links
                {
                    blockers.push(ReplacementBlocker::UnobservableMetadata(item));
                }
            }
        }
    }
    blockers
}

/// Replace the opened source file with a verified export plan.
///
/// Validates the plan's package type against the source extension and its
/// size against the stamp's read cap, re-runs the full conflict verification
/// itself (locks, path resolution, identity, exact bytes) instead of trusting
/// earlier calls, then requires the plan to originate byte-exactly from the
/// stamp's package and evaluates the per-file metadata gate. Any blocker is
/// reported as [`SaveOriginalError::ReplacementUnavailable`] before anything
/// is written. Otherwise the Unix write path runs (see `publish.rs`); its
/// failures are [`SaveOriginalError::Replace`] errors that state whether the
/// plan was published.
///
/// A successful receipt carries the refreshed stamp read back from disk.
/// The session that produced `plan` still holds the pre-save package, so a
/// later plan from it no longer originates from the file on disk and is
/// refused until the session is re-based on the saved bytes.
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
    // The refreshed stamp is read back under the same cap.
    if plan.bytes().len() as u64 > source.budget_limit() {
        return Err(SaveOriginalError::io(
            SourceStage::Validate,
            path,
            std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "export plan exceeds the source read budget",
            ),
        ));
    }
    // Full re-verification inside this call; earlier check_source passes are
    // not treated as an atomic compare-and-swap against later races.
    check_source(source)?;
    let metadata =
        fs::metadata(path).map_err(|e| SaveOriginalError::io(SourceStage::Stat, path, e))?;
    let blockers = replacement_blockers(plan, source, &metadata);
    if !blockers.is_empty() {
        return Err(SaveOriginalError::ReplacementUnavailable {
            path: path.to_path_buf(),
            blockers,
        });
    }
    #[cfg(unix)]
    {
        let expected = super::publish::Expected::of(source, &metadata);
        super::publish::replace(plan, source, &expected)
    }
    #[cfg(not(unix))]
    Err(SaveOriginalError::ReplacementUnavailable {
        path: path.to_path_buf(),
        blockers: vec![ReplacementBlocker::UnsupportedPlatform],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::DocxSession;
    use joyoffice_opc::Budget;

    fn fixture(id: u8) -> Vec<u8> {
        std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
        )
        .unwrap()
    }

    fn stamp_of(dir: &std::path::Path, bytes: &[u8]) -> (SourceStamp, fs::Metadata) {
        let path = dir.join("a.docx");
        fs::write(&path, bytes).unwrap();
        let stamp = super::super::open_source(&path, &Budget::default())
            .unwrap()
            .into_stamp();
        let metadata = fs::metadata(stamp.path()).unwrap();
        (stamp, metadata)
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_matrix_restores_what_std_can_and_observes_the_rest() {
        let caps = original_replacement_capabilities();
        assert!(caps.write_path);
        assert_eq!(caps.permissions, MetadataCapability::Preservable);
        assert_eq!(caps.ownership, MetadataCapability::Preservable);
        assert_eq!(caps.creation_time, MetadataCapability::Preservable);
        for item in [
            caps.acl,
            caps.extended_attributes,
            caps.links,
            caps.provenance_marks,
            caps.file_flags,
        ] {
            assert_eq!(item, MetadataCapability::ObservableNotPreservable);
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn other_unix_matrix_cannot_see_extended_attributes() {
        let caps = original_replacement_capabilities();
        assert!(caps.write_path);
        assert_eq!(caps.permissions, MetadataCapability::Preservable);
        assert_eq!(caps.ownership, MetadataCapability::Preservable);
        assert_eq!(caps.acl, MetadataCapability::Unobservable);
        assert_eq!(caps.extended_attributes, MetadataCapability::Unobservable);
        assert_eq!(caps.provenance_marks, MetadataCapability::Unobservable);
        assert_eq!(caps.file_flags, MetadataCapability::Unobservable);
        assert_eq!(
            caps.creation_time,
            MetadataCapability::ObservableNotPreservable
        );
    }

    #[cfg(not(unix))]
    #[test]
    fn non_unix_matrix_has_no_write_path() {
        assert!(!original_replacement_capabilities().write_path);
    }

    #[test]
    fn a_plan_from_other_bytes_is_never_bound_to_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let (stamp, metadata) = stamp_of(dir.path(), &fixture(2));
        let other = DocxSession::open(&fixture(1), &Budget::default())
            .unwrap()
            .export_plan()
            .unwrap();
        let blockers = replacement_blockers(&other, &stamp, &metadata);
        assert!(blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
        let own = DocxSession::open(&fixture(2), &Budget::default())
            .unwrap()
            .export_plan()
            .unwrap();
        let blockers = replacement_blockers(&own, &stamp, &metadata);
        assert!(!blockers.contains(&ReplacementBlocker::OriginBindingUnproven));
        // Only macOS can vouch for every metadata item of a plain file.
        assert_eq!(blockers.is_empty(), cfg!(target_os = "macos"));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn other_platforms_refuse_even_a_bound_plan() {
        let dir = tempfile::tempdir().unwrap();
        let (stamp, metadata) = stamp_of(dir.path(), &fixture(1));
        let plan = DocxSession::open(&fixture(1), &Budget::default())
            .unwrap()
            .export_plan()
            .unwrap();
        let blockers = replacement_blockers(&plan, &stamp, &metadata);
        assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(
            MetadataItem::ExtendedAttributes
        )));
        assert!(blockers.contains(&ReplacementBlocker::UnobservableMetadata(MetadataItem::Acl)));
    }
}
