//! Shared error surface for opening, conflict-prechecking and saving an
//! opened source document (W07). Every variant names the path it concerned;
//! none of them silently downgrades a failure into success.

use std::fmt;
use std::io;
use std::path::PathBuf;

use super::original::SaveOriginalReceipt;

/// Observable step of the source pipeline. `Io` errors carry the step that
/// failed; the same step set drives deterministic fault injection in tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceStage {
    /// Path resolution (canonicalization) of the requested source.
    Resolve,
    /// Directory scan for owner/lock files.
    LockScan,
    /// stat of the resolved file.
    Stat,
    /// open of the resolved path for the bounded read.
    Open,
    /// The bounded read itself.
    Read,
    /// Re-stat after the read proving the captured baseline was stable.
    Restat,
    /// Argument validation inside save_original.
    Validate,
    /// Read ACLs, extended attributes, provenance marks and creation time.
    /// A failure here refuses replacement and does not write.
    ObserveMetadata,
    /// Create the same-directory temporary file for original replacement.
    CreateTemporary,
    /// Write the verified plan bytes into that temporary file.
    Write,
    /// Sync the temporary file before it may be published.
    SyncFile,
    /// Read the temporary file back and compare it to the plan.
    Verify,
    /// Copy the source permission bits, owner and group (Unix) and creation
    /// time (macOS) onto the temporary file, before it is synced.
    RestorePermissions,
    /// Hard-link the current target under a private backup name.
    Backup,
    /// Read the backup back and compare it to the stamped bytes.
    VerifyBackup,
    /// Exchange the temporary file with the target (macOS `RENAME_SWAP`,
    /// Linux `RENAME_EXCHANGE`). The displaced inode is the previous target.
    Publish,
    /// Read the published target back: it must be the renamed object with
    /// the plan's exact bytes. Only this read yields the receipt and its
    /// refreshed stamp.
    Confirm,
    /// Remove the private backup after the new file is in place.
    Cleanup,
    /// Sync the parent directory after publication.
    SyncDirectory,
}

impl fmt::Display for SourceStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Resolve => "path resolution",
            Self::LockScan => "lock scan",
            Self::Stat => "stat",
            Self::Open => "open",
            Self::Read => "bounded read",
            Self::Restat => "post-read stat",
            Self::Validate => "validation",
            Self::ObserveMetadata => "metadata observation",
            Self::CreateTemporary => "temporary file creation",
            Self::Write => "write",
            Self::SyncFile => "file sync",
            Self::Verify => "read-back verification",
            Self::RestorePermissions => "metadata restore",
            Self::Backup => "backup hard link",
            Self::VerifyBackup => "backup verification",
            Self::Publish => "atomic replace",
            Self::Confirm => "published read-back",
            Self::Cleanup => "backup cleanup",
            Self::SyncDirectory => "directory sync",
        })
    }
}

/// What an Office owner file (`~$…`) or JoyWord lock file was found to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockState {
    /// The lock-named entry exists. Presence alone never grants exclusivity
    /// and expired locks are never deleted by this code.
    Present,
    /// The scan could not decide (unreadable directory, undecodable name).
    /// Treated as a conflict: the conservative direction.
    Unknown,
}

/// The kind of lock-named entry that was found or could not be decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockKind {
    /// A `~$`-prefixed sibling matching the Word owner-file convention.
    OfficeOwnerFile,
    /// JoyWord's own lock file convention, `.<name>.joyword-lock`.
    JoyWordLockFile,
}

/// How the on-disk state stopped matching the captured `SourceStamp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictDetail {
    /// The resolved path no longer exists.
    Missing,
    /// The path now names something other than a regular file.
    NotARegularFileAnymore,
    /// Same path, different file-system object (delete/recreate, or a new
    /// inode after replacement by another writer).
    InodeChanged,
    /// Length differs from the captured baseline.
    SizeChanged,
    /// Modification, status-change, or birth time differs. Content may or may
    /// not have changed; identical content with new times still conflicts.
    TimesChanged,
    /// Permission bits differ (Unix).
    ModeChanged,
    /// Hard-link count differs (Unix); alias links appeared or disappeared.
    LinkCountChanged,
    /// Bytes differ from the captured baseline. Caught by exact byte
    /// comparison, including same-size/same-mtime rewrites.
    ContentChanged,
    /// The recorded request path now resolves elsewhere (symlink retargeted,
    /// path moved), so the stamp no longer names the same object.
    PathResolutionChanged,
}

impl ConflictDetail {
    fn message(&self) -> &'static str {
        match self {
            Self::Missing => "source no longer exists",
            Self::NotARegularFileAnymore => "source is no longer a regular file",
            Self::InodeChanged => "source object identity changed",
            Self::SizeChanged => "source length changed",
            Self::TimesChanged => "source times changed",
            Self::ModeChanged => "source permission bits changed",
            Self::LinkCountChanged => "source hard-link count changed",
            Self::ContentChanged => "source content changed",
            Self::PathResolutionChanged => "request path resolves to a different object",
        }
    }
}

/// Why original-file replacement is unavailable. These are durable policy
/// facts, not transient failures; retrying cannot fix them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacementBlocker {
    /// `ExportPlan::origin_bytes()` is `None`, so the plan cannot prove which
    /// package it came from. Equal version numbers are not a substitute.
    OriginBindingUnproven,
    /// This build has no reviewed atomic replace for the current platform.
    /// Unix and Windows have dedicated implementations; other targets refuse.
    AtomicReplaceUnverified,
    /// The metadata item cannot even be observed with the approved
    /// dependencies, so losing it could not be detected, let alone restored.
    UnobservableMetadata(MetadataItem),
    /// The metadata item is observable but no approved write path can restore
    /// it on a replaced inode.
    UnpreservableMetadata(MetadataItem),
    /// The source has several hard links; replacing one path leaves the other
    /// links on the old object. A product decision is required first.
    LinkedSourceSemantics,
    /// No write permission bit is set on the source. Replacing it through
    /// the directory would bypass that read-only intent.
    ReadOnlySource,
    /// The caller did not explicitly confirm replacement. The default is to
    /// leave the source untouched (and write a new copy elsewhere).
    ConfirmationRequired,
}

impl ReplacementBlocker {
    fn message(&self) -> &'static str {
        match self {
            Self::OriginBindingUnproven => {
                "the export plan cannot be proven to originate from this source package"
            }
            Self::AtomicReplaceUnverified => {
                "atomic replacement of an existing file is not verified on this platform"
            }
            Self::UnobservableMetadata(_) => {
                "metadata item cannot be observed with approved dependencies"
            }
            Self::UnpreservableMetadata(_) => {
                "metadata item is observable but cannot be reproduced on the replacement"
            }
            Self::LinkedSourceSemantics => {
                "source is hard-linked; replacement semantics for other links are undecided"
            }
            Self::ReadOnlySource => "source is read-only",
            Self::ConfirmationRequired => {
                "replacement requires explicit confirmation; the default is a new copy"
            }
        }
    }
}

/// One metadata aspect the architecture requires an atomic replacement to
/// preserve (ARCHITECTURE §5: links, permissions/ACL, extended attributes,
/// creation time, read-only attribute, provenance marks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataItem {
    Permissions,
    Ownership,
    Acl,
    ExtendedAttributes,
    CreationTime,
    Links,
    /// Windows Mark of the Web, macOS quarantine flags.
    ProvenanceMarks,
}

impl MetadataItem {
    fn message(&self) -> &'static str {
        match self {
            Self::Permissions => "permission bits / read-only attribute",
            Self::Ownership => "file ownership",
            Self::Acl => "access control lists",
            Self::ExtendedAttributes => "extended attributes",
            Self::CreationTime => "creation time",
            Self::Links => "symbolic and hard links",
            Self::ProvenanceMarks => "provenance marks (MOTW, quarantine)",
        }
    }
}

/// Errors from opening a source, conflict prechecks and original replacement.
#[derive(Debug)]
pub enum SaveOriginalError {
    /// File-system failure at the named stage. `published` is set only after
    /// the target name already points at the new bytes.
    Io {
        stage: SourceStage,
        path: PathBuf,
        source: io::Error,
        published: Option<SaveOriginalReceipt>,
    },
    /// The plan's origin bytes are present and are not the stamped package.
    /// No write was attempted. Version equality is irrelevant.
    OriginMismatch { path: PathBuf },
    /// The resolved path names a directory, device or other non-regular file.
    NotARegularFile { path: PathBuf },
    /// The file exceeds the byte budget of the read. Never read unbounded.
    SourceTooLarge { path: PathBuf, limit: u64 },
    /// Identity observations around one bounded read disagreed, so the read
    /// bytes are not a stable baseline and no stamp was produced.
    UnstableRead { path: PathBuf },
    /// An owner/lock file exists, or its state could not be determined.
    /// Locks are never deleted or stolen.
    LockConflict {
        lock_path: PathBuf,
        kind: LockKind,
        state: LockState,
    },
    /// Current state stopped matching the stamp (`check_source`, and the
    /// internal re-verification of `save_original`).
    Conflict {
        path: PathBuf,
        detail: ConflictDetail,
    },
    /// Original replacement is refused before any write; each blocker names
    /// a durable reason. The source file is untouched.
    ReplacementUnavailable {
        path: PathBuf,
        blockers: Vec<ReplacementBlocker>,
    },
    /// The rename onto `path` happened, but reading it back did not show the
    /// renamed object with the plan's exact bytes, so no receipt or stamp is
    /// issued. The verified original's bytes stay under `backup`, which is
    /// never deleted automatically; nothing is retried.
    PublishedUnconfirmed {
        path: PathBuf,
        source: io::Error,
        backup: PathBuf,
    },
    /// Metadata could not be observed. Replacement is refused even when the
    /// caller asked to confirm, because the loss list would be incomplete.
    ObservationFailed { path: PathBuf, source: io::Error },
}

impl SaveOriginalError {
    /// The path the error is about: the source, the lock or the target.
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::Io { path, .. }
            | Self::NotARegularFile { path }
            | Self::SourceTooLarge { path, .. }
            | Self::UnstableRead { path }
            | Self::Conflict { path, .. }
            | Self::OriginMismatch { path }
            | Self::ReplacementUnavailable { path, .. }
            | Self::PublishedUnconfirmed { path, .. }
            | Self::ObservationFailed { path, .. } => path,
            Self::LockConflict { lock_path, .. } => lock_path,
        }
    }
    pub fn stage(&self) -> Option<SourceStage> {
        match self {
            Self::Io { stage, .. } => Some(*stage),
            Self::PublishedUnconfirmed { .. } => Some(SourceStage::Confirm),
            Self::ObservationFailed { .. } => Some(SourceStage::ObserveMetadata),
            _ => None,
        }
    }
    pub fn io_error(&self) -> Option<&io::Error> {
        match self {
            Self::Io { source, .. }
            | Self::PublishedUnconfirmed { source, .. }
            | Self::ObservationFailed { source, .. } => Some(source),
            _ => None,
        }
    }
    /// Where the verified original's bytes were kept after an unconfirmed
    /// publication.
    pub fn retained_backup(&self) -> Option<&std::path::Path> {
        match self {
            Self::PublishedUnconfirmed { backup, .. } => Some(backup),
            _ => None,
        }
    }
    /// The conflict detail, when this is a `Conflict`.
    pub fn conflict(&self) -> Option<ConflictDetail> {
        match self {
            Self::Conflict { detail, .. } => Some(*detail),
            _ => None,
        }
    }
    /// The lock kind/state, when this is a `LockConflict`.
    pub fn lock(&self) -> Option<(LockKind, LockState)> {
        match self {
            Self::LockConflict { kind, state, .. } => Some((*kind, *state)),
            _ => None,
        }
    }
    /// The durable blockers, when replacement was refused.
    pub fn blockers(&self) -> Option<&[ReplacementBlocker]> {
        match self {
            Self::ReplacementUnavailable { blockers, .. } => Some(blockers),
            _ => None,
        }
    }
    /// The plan had origin bytes and they were not the stamped package.
    pub fn is_origin_mismatch(&self) -> bool {
        matches!(self, Self::OriginMismatch { .. })
    }
    /// Set only when the target name already points at the replacement.
    pub fn published(&self) -> Option<&SaveOriginalReceipt> {
        match self {
            Self::Io { published, .. } => published.as_ref(),
            _ => None,
        }
    }
}

impl fmt::Display for SaveOriginalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                stage,
                path,
                source,
                published,
            } => {
                write!(f, "source {} at {:?}: {}", stage, path.display(), source)?;
                if published.is_some() {
                    f.write_str(" (replacement already published)")?;
                }
                Ok(())
            }
            Self::OriginMismatch { path } => write!(
                f,
                "export plan origin does not match source {:?}",
                path.display()
            ),
            Self::NotARegularFile { path } => {
                write!(f, "source {:?} is not a regular file", path.display())
            }
            Self::SourceTooLarge { path, limit } => {
                write!(
                    f,
                    "source {:?} exceeds the read budget of {} bytes",
                    path.display(),
                    limit
                )
            }
            Self::UnstableRead { path } => write!(
                f,
                "source {:?} changed identity during the bounded read; no baseline captured",
                path.display()
            ),
            Self::LockConflict {
                lock_path,
                kind,
                state,
            } => write!(
                f,
                "{:?} at {:?}: {}",
                kind,
                lock_path.display(),
                match state {
                    LockState::Present => "lock entry exists",
                    LockState::Unknown => "lock state could not be determined",
                }
            ),
            Self::Conflict { path, detail } => {
                write!(f, "source {:?}: {}", path.display(), detail.message())
            }
            Self::ReplacementUnavailable { path, blockers } => {
                write!(
                    f,
                    "original replacement unavailable for {:?}",
                    path.display()
                )?;
                for blocker in blockers {
                    write!(f, "; {}", blocker.message())?;
                    match blocker {
                        ReplacementBlocker::UnobservableMetadata(item)
                        | ReplacementBlocker::UnpreservableMetadata(item) => {
                            write!(f, " ({})", item.message())?;
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            Self::PublishedUnconfirmed {
                path,
                source,
                backup,
            } => write!(
                f,
                "replacement of {:?} was renamed into place but not confirmed: {}; original kept at {:?}",
                path.display(),
                source,
                backup.display()
            ),
            Self::ObservationFailed { path, source } => write!(
                f,
                "metadata observation failed for {:?}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SaveOriginalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. }
            | Self::PublishedUnconfirmed { source, .. }
            | Self::ObservationFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl SaveOriginalError {
    pub(crate) fn observation(path: &std::path::Path, source: io::Error) -> Self {
        Self::ObservationFailed {
            path: path.to_path_buf(),
            source,
        }
    }
    pub fn is_observation_failure(&self) -> bool {
        matches!(self, Self::ObservationFailed { .. })
    }
    pub(crate) fn io(stage: SourceStage, path: &std::path::Path, source: io::Error) -> Self {
        Self::Io {
            stage,
            path: path.to_path_buf(),
            source,
            published: None,
        }
    }
    #[cfg(any(unix, windows))]
    pub(crate) fn io_published(
        stage: SourceStage,
        path: &std::path::Path,
        source: io::Error,
        receipt: SaveOriginalReceipt,
    ) -> Self {
        Self::Io {
            stage,
            path: path.to_path_buf(),
            source,
            published: Some(receipt),
        }
    }
    pub(crate) fn conflict_error(path: &std::path::Path, detail: ConflictDetail) -> Self {
        Self::Conflict {
            path: path.to_path_buf(),
            detail,
        }
    }
}
