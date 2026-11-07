//! Detection of Office owner files and JoyWord lock files next to a source.
//!
//! Detection is deliberately one-sided: a found or undecidable lock is a
//! conflict, but finding none never grants exclusivity — another writer may
//! hold the file through a mechanism we cannot see. Locks are never deleted,
//! overwritten or "expired" by this code; the user resolves them.
//!
//! A caller that already holds the JoyWord lock may pass its token. Only a
//! regular file whose entire contents equal that token is skipped. Every
//! other JoyWord lock, every Office `~$` file, and an unreadable lock stay
//! conflicts. `open_source` never passes a token.

use std::fs::{self, File};
use std::io::{self, Read};
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
    owner_token: Option<&str>,
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
        if name == joyword_lock_name(file_name) && !joyword_lock_is_own(&entry.path(), owner_token)
        {
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
    owner_token: Option<&str>,
) -> Result<(), SaveOriginalError> {
    for directory in directories {
        if let Some(finding) = scan_directory(directory, file_name, owner_token)?
            .into_iter()
            .next()
        {
            return Err(SaveOriginalError::LockConflict {
                lock_path: finding.path,
                kind: finding.kind,
                state: LockState::Present,
            });
        }
    }
    Ok(())
}

/// True only when `path` is a regular file and its whole contents equal
/// `owner_token`. An empty token never matches. The file is not modified.
fn joyword_lock_is_own(path: &Path, owner_token: Option<&str>) -> bool {
    let Some(token) = owner_token.filter(|token| !token.is_empty()) else {
        return false;
    };
    read_own_lock(path, token).unwrap_or(false)
}

fn open_lock(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        use rustix::fs::{Mode, OFlags, open};
        // A lock replaced after lstat must not redirect the read or block on a FIFO.
        Ok(open(
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?
        .into())
    }
    #[cfg(not(unix))]
    {
        File::open(path)
    }
}

fn read_own_lock(path: &Path, token: &str) -> io::Result<bool> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.len() != token.len() as u64 {
        return Ok(false);
    }
    let file = open_lock(path)?;
    let opened = file.metadata()?;
    if !opened.is_file() || opened.len() != before.len() {
        return Ok(false);
    }
    let matches = matching_token(file, token)?;
    let after = fs::symlink_metadata(path)?;
    if !after.is_file() || after.len() != before.len() {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use super::source::SourceIdentity;
        if SourceIdentity::capture(&before) != SourceIdentity::capture(&opened)
            || SourceIdentity::capture(&before) != SourceIdentity::capture(&after)
        {
            return Ok(false);
        }
    }
    Ok(matches)
}

fn matching_token(reader: impl Read, token: &str) -> io::Result<bool> {
    let mut bytes = Vec::new();
    // A concurrent writer can grow the file after metadata inspection. Never
    // let lock verification allocate/read beyond the expected token plus one.
    reader
        .take((token.len() as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes == token.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_nonempty_regular_lock_tokens_are_owned()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("lock");
        fs::write(&path, "owner-token")?;
        assert!(joyword_lock_is_own(&path, Some("owner-token")));
        for token in [None, Some(""), Some("owner"), Some("other-token")] {
            assert!(!joyword_lock_is_own(&path, token));
        }
        fs::write(&path, "owner-token-extra")?;
        assert!(!joyword_lock_is_own(&path, Some("owner-token")));
        assert!(!joyword_lock_is_own(dir.path(), Some("owner-token")));
        Ok(())
    }

    #[test]
    fn growing_lock_read_stops_after_token_length_plus_one() -> io::Result<()> {
        assert!(!matching_token(io::repeat(b'x'), "x")?);
        assert!(matching_token(io::Cursor::new(b"owner"), "owner")?);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_matching_token_remains_a_conflicting_lock()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir()?;
        let target = dir.path().join("token");
        fs::write(&target, "owner-token")?;
        let lock = dir.path().join(joyword_lock_name("document.docx"));
        symlink(&target, &lock)?;
        assert!(!joyword_lock_is_own(&lock, Some("owner-token")));
        assert!(open_lock(&lock).is_err());
        let findings = scan_directory(
            dir.path(),
            std::ffi::OsStr::new("document.docx"),
            Some("owner-token"),
        )?;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].path, lock);
        assert_eq!(fs::read_to_string(&target)?, "owner-token");
        Ok(())
    }
}
