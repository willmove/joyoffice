//! Detection of Office owner files and JoyWord lock files next to a source.
//!
//! Detection is deliberately one-sided: a found or undecidable lock is a
//! conflict, but finding none never grants exclusivity — another writer may
//! hold the file through a mechanism we cannot see. Locks are never deleted,
//! overwritten or "expired" by this code; the user resolves them.

use std::fs;
use std::path::{Path, PathBuf};

use super::error::{LockKind, LockState, SaveOriginalError};

/// A lock-named entry that blocks opening or saving.
pub(crate) struct LockFinding {
    pub path: PathBuf,
    pub kind: LockKind,
}

/// JoyWord's own lock file for `name` is the hidden sibling
/// `.<name>.joyword-lock` in the same directory.
pub(crate) fn joyword_lock_name(file_name: &str) -> String {
    format!(".{file_name}.joyword-lock")
}

/// Candidate Word owner-file names for `file_name`. Word writes `~$` plus the
/// document name, truncating the front of the base name for long names to
/// fit short-name limits; the exact truncation rule varies by version, so
/// callers should treat every `~$` sibling matching the document's tail as
/// suspicious rather than guessing one spelling (detection only, never
/// deletion).
fn is_owner_candidate(owner: &[u8], file_name: &[u8]) -> bool {
    // "~$report.docx" matches "report.docx" and "q3-report.docx"; both are
    // conservatively treated as locks on our file.
    owner.len() <= file_name.len() && file_name.ends_with(owner)
}

/// Scan one directory for lock entries concerning `file_name`.
///
/// Returns every finding, or a `LockConflict` with `LockState::Unknown` when
/// the directory cannot be listed or a name cannot be decoded. A missing
/// directory is not a lock problem and yields no findings.
pub(crate) fn scan_directory(
    directory: &Path,
    file_name: &std::ffi::OsStr,
) -> Result<Vec<LockFinding>, SaveOriginalError> {
    let Some(file_name) = file_name.to_str() else {
        return Err(SaveOriginalError::LockConflict {
            lock_path: directory.to_path_buf(),
            kind: LockKind::OfficeOwnerFile,
            state: LockState::Unknown,
        });
    };
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => {
            // The lock state of an unlistable directory is undecidable, not
            // absent; report it as an unknown-state conflict.
            return Err(SaveOriginalError::LockConflict {
                lock_path: directory.to_path_buf(),
                kind: LockKind::OfficeOwnerFile,
                state: LockState::Unknown,
            });
        }
    };
    let mut findings = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| SaveOriginalError::LockConflict {
            lock_path: directory.to_path_buf(),
            kind: LockKind::OfficeOwnerFile,
            state: LockState::Unknown,
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            // An undecodable sibling could itself be a lock; state unknown.
            return Err(SaveOriginalError::LockConflict {
                lock_path: entry.path(),
                kind: LockKind::OfficeOwnerFile,
                state: LockState::Unknown,
            });
        };
        if let Some(stripped) = name.strip_prefix("~$")
            && is_owner_candidate(stripped.as_bytes(), file_name.as_bytes())
        {
            findings.push(LockFinding {
                path: entry.path(),
                kind: LockKind::OfficeOwnerFile,
            });
        }
        if name == joyword_lock_name(file_name) {
            findings.push(LockFinding {
                path: entry.path(),
                kind: LockKind::JoyWordLockFile,
            });
        }
    }
    Ok(findings)
}

/// Scan the directories recorded for a source and report the first conflict.
pub(crate) fn check_directories(
    directories: &[PathBuf],
    file_name: &std::ffi::OsStr,
) -> Result<(), SaveOriginalError> {
    for directory in directories {
        if let Some(finding) = scan_directory(directory, file_name)?.into_iter().next() {
            return Err(SaveOriginalError::LockConflict {
                lock_path: finding.path,
                kind: finding.kind,
                state: LockState::Present,
            });
        }
    }
    Ok(())
}
