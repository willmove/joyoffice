//! Shared error surface for opening, conflict-prechecking and saving an
//! opened source document (W07). Every variant names the path it concerned;
//! none of them silently downgrades a failure into success.

use std::fmt;
use std::io;
use std::path::PathBuf;

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
    /// Modification or (Unix) status-change time differs. Content may or may
    /// not have changed; identical content with new times still conflicts.
    TimesChanged,
    /// Permission bits differ (Unix).
    ModeChanged,
    /// Owner or group differs (Unix). Only reported where status-change
    /// time cannot be compared, such as the replacement's backup link.
    OwnershipChanged,
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
            Self::OwnershipChanged => "source owner or group changed",
            Self::LinkCountChanged => "source hard-link count changed",
            Self::ContentChanged => "source content changed",
            Self::PathResolutionChanged => "request path resolves to a different object",
        }
    }
}

/// Why original-file replacement is unavailable for this plan and source.
/// These are durable facts, not transient failures; retrying the same call
/// cannot fix them. Nothing was written to the source when one is reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacementBlocker {
    /// The plan was not built from exactly the package bytes the stamp read:
    /// another document, an older or newer version of this file, or a
    /// session that has not been re-based after an earlier save. Equal
    /// version numbers never prove that a plan and a source belong together.
    OriginBindingUnproven,
    /// The metadata item cannot even be observed with the approved
    /// dependencies, so losing it could not be detected, let alone restored.
    UnobservableMetadata(MetadataItem),
    /// The source carries a value of this item that the replacement cannot
    /// reproduce on the new object (or the new object could not be made to
    /// carry it), so replacing would lose or change it.
    UnpreservableMetadata(MetadataItem),
    /// The source has several hard links; replacing one path leaves the other
    /// links on the old object. A product decision is required first.
    LinkedSourceSemantics,
    /// No write permission bit is set on the source. Replacing it through
    /// the directory would bypass that read-only intent.
    ReadOnlySource,
    /// This platform has no original-replacement write path in this build.
    UnsupportedPlatform,
}

impl ReplacementBlocker {
    fn message(&self) -> &'static str {
        match self {
            Self::OriginBindingUnproven => {
                "the export plan cannot be proven to originate from this source package"
            }
            Self::UnobservableMetadata(_) => {
                "metadata item cannot be observed with approved dependencies"
            }
            Self::UnpreservableMetadata(_) => {
                "metadata item is present but cannot be reproduced on the replacement"
            }
            Self::LinkedSourceSemantics => {
                "source is hard-linked; replacement semantics for other links are undecided"
            }
            Self::ReadOnlySource => "source is read-only",
            Self::UnsupportedPlatform => "original replacement is not implemented on this platform",
        }
    }
}

/// Step of the original-replacement write path. `Replace` errors carry the
/// step that failed; the same steps drive deterministic fault injection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplaceStage {
    /// Private temporary file in the source's directory.
    CreateTemporary,
    /// Writing the plan bytes into the temporary file.
    Write,
    /// Restoring owner, group, permission bits and (macOS) creation time.
    RestoreMetadata,
    /// Sync of the temporary file.
    SyncFile,
    /// Byte-for-byte and metadata read-back of the temporary file.
    VerifyTemporary,
    /// Full conflict re-verification of the source just before the backup.
    Recheck,
    /// Hard link of the current source object under a private backup name.
    Backup,
    /// Proof that the backup names the verified object with its exact bytes.
    VerifyBackup,
    /// Atomic rename of the temporary file over the source path.
    Publish,
    /// Bounded read-back of the published path; yields receipt and stamp.
    Confirm,
    /// Removal of the backup link after confirmation.
    RemoveBackup,
    /// Sync of the containing directory.
    SyncDirectory,
}

/// What a failed replacement left at the source path. Nothing is rolled
/// back or retried automatically once the rename has happened.
#[derive(Clone, Debug)]
pub enum Publication {
    /// No rename happened: the source path still names the verified original
    /// object with its bytes, mode and modification time.
    NotPublished,
    /// The rename happened but read-back could not confirm the published
    /// object. The original's exact bytes are kept under the retained backup.
    Unconfirmed,
    /// Read-back confirmed the published plan; a later cleanup or sync step
    /// failed. The receipt and its refreshed stamp are valid.
    Confirmed(Box<super::original::SaveOriginalReceipt>),
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
    /// BSD file flags (macOS `chflags`: hidden, immutable, append-only…).
    FileFlags,
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
            Self::FileFlags => "file flags",
        }
    }
}

/// Errors from opening a source, conflict prechecks and original replacement.
#[derive(Debug)]
pub enum SaveOriginalError {
    /// File-system failure at the named stage.
    Io {
        stage: SourceStage,
        path: PathBuf,
        source: io::Error,
    },
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
    /// Original replacement is refused before the source is rewritten; each
    /// blocker names a durable reason. The source file is untouched and no
    /// temporary or backup file is left behind.
    ReplacementUnavailable {
        path: PathBuf,
        blockers: Vec<ReplacementBlocker>,
    },
    /// The replacement write path failed at `stage`. `publication` states
    /// what the source path holds now. `retained_backup` names a private hard
    /// link to the verified original that was kept on purpose (publication
    /// not confirmed) or could not be removed; it is never deleted
    /// automatically once a rename has happened.
    Replace {
        stage: ReplaceStage,
        path: PathBuf,
        source: io::Error,
        publication: Publication,
        retained_backup: Option<PathBuf>,
    },
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
            | Self::ReplacementUnavailable { path, .. }
            | Self::Replace { path, .. } => path,
            Self::LockConflict { lock_path, .. } => lock_path,
        }
    }
    pub fn stage(&self) -> Option<SourceStage> {
        match self {
            Self::Io { stage, .. } => Some(*stage),
            _ => None,
        }
    }
    pub fn io_error(&self) -> Option<&io::Error> {
        match self {
            Self::Io { source, .. } | Self::Replace { source, .. } => Some(source),
            _ => None,
        }
    }
    /// The replacement step that failed, when this is a `Replace` error.
    pub fn replace_stage(&self) -> Option<ReplaceStage> {
        match self {
            Self::Replace { stage, .. } => Some(*stage),
            _ => None,
        }
    }
    /// What the source path holds after a failed replacement.
    pub fn publication(&self) -> Option<&Publication> {
        match self {
            Self::Replace { publication, .. } => Some(publication),
            _ => None,
        }
    }
    /// The confirmed receipt, when the plan was published and read back
    /// before a later step failed.
    pub fn published(&self) -> Option<&super::original::SaveOriginalReceipt> {
        match self.publication() {
            Some(Publication::Confirmed(receipt)) => Some(receipt),
            _ => None,
        }
    }
    /// A backup link to the verified original left on disk.
    pub fn retained_backup(&self) -> Option<&std::path::Path> {
        match self {
            Self::Replace {
                retained_backup, ..
            } => retained_backup.as_deref(),
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
}

impl fmt::Display for SaveOriginalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                stage,
                path,
                source,
            } => {
                write!(f, "source {} at {:?}: {}", stage, path.display(), source)
            }
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
            Self::Replace {
                stage,
                path,
                source,
                publication,
                retained_backup,
            } => {
                write!(
                    f,
                    "original replacement of {:?} at {:?}: {}",
                    path.display(),
                    stage,
                    source
                )?;
                f.write_str(match publication {
                    Publication::NotPublished => " (original kept)",
                    Publication::Unconfirmed => " (published but not confirmed)",
                    Publication::Confirmed(_) => " (plan already published and confirmed)",
                })?;
                if let Some(backup) = retained_backup {
                    write!(f, "; original retained at {:?}", backup.display())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for SaveOriginalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::Replace { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl SaveOriginalError {
    pub(crate) fn io(stage: SourceStage, path: &std::path::Path, source: io::Error) -> Self {
        Self::Io {
            stage,
            path: path.to_path_buf(),
            source,
        }
    }
    pub(crate) fn conflict_error(path: &std::path::Path, detail: ConflictDetail) -> Self {
        Self::Conflict {
            path: path.to_path_buf(),
            detail,
        }
    }
}
