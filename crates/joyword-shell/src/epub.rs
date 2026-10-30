//! Local EPUB publication of an immutable export plan. No-overwrite, sync and readback.
pub use joyword_epub::{Error as ExportError, Loss, Metadata, Plan, export, utc_timestamp};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub enum WriteError {
    InvalidExtension,
    Cancelled,
    Io(std::io::Error),
    Cleanup {
        path: PathBuf,
        source: std::io::Error,
    },
    Published {
        path: PathBuf,
        source: std::io::Error,
    },
}
impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExtension => f.write_str("EPUB destination must end in .epub"),
            Self::Cancelled => f.write_str("EPUB export cancelled"),
            Self::Io(e) => write!(f, "EPUB I/O: {e}"),
            Self::Cleanup { path, source } => write!(
                f,
                "incomplete temporary file {} could not be removed: {source}",
                path.display()
            ),
            Self::Published { path, source } => write!(
                f,
                "complete EPUB already published at {}; temporary cleanup failed: {source}",
                path.display()
            ),
        }
    }
}
impl std::error::Error for WriteError {}
impl From<std::io::Error> for WriteError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn modified_now() -> Result<String, ExportError> {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .unwrap_or(0);
    utc_timestamp(seconds)
}

/// Publishes a hard link only after complete bytes have been synced and verified.
/// Existing targets (including dangling symlinks) are refused. Directory-sync,
/// unsupported hard-link volumes and actual power loss are not certified here.
pub fn write_new(
    plan: &Plan,
    destination: &Path,
    cancel: &AtomicBool,
) -> Result<PathBuf, WriteError> {
    if !destination
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("epub"))
    {
        return Err(WriteError::InvalidExtension);
    }
    if cancel.load(Ordering::Acquire) {
        return Err(WriteError::Cancelled);
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = std::fs::canonicalize(parent)?;
    let destination = parent.join(
        destination
            .file_name()
            .ok_or(WriteError::InvalidExtension)?,
    );
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let temporary_path = temporary.path().to_path_buf();
    let result = (|| {
        temporary.write_all(plan.bytes())?;
        temporary.as_file().sync_all()?;
        crate::storage::verify_bytes(temporary.as_file_mut(), plan.bytes())?;
        if cancel.load(Ordering::Acquire) {
            return Err(WriteError::Cancelled);
        }
        std::fs::hard_link(temporary.path(), &destination)?;
        Ok(())
    })();
    if let Err(error) = result {
        temporary.close().map_err(|source| WriteError::Cleanup {
            path: temporary_path,
            source,
        })?;
        return Err(error);
    }
    temporary.close().map_err(|source| WriteError::Published {
        path: destination.clone(),
        source,
    })?;
    Ok(destination)
}
