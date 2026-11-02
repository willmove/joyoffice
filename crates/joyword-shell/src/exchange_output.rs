//! Explicitly confirmed publication of immutable text-exchange copies.
use crate::exchange::{Format, Plan};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub enum WriteError {
    NeedsConfirmation,
    Cancelled,
    InvalidExtension,
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
            Self::NeedsConfirmation => {
                f.write_str("conversion losses require explicit confirmation")
            }
            Self::Cancelled => f.write_str("text export cancelled"),
            Self::InvalidExtension => {
                f.write_str("output extension does not match conversion format")
            }
            Self::Io(e) => write!(f, "text export I/O: {e}"),
            Self::Cleanup { path, source } => write!(
                f,
                "temporary {} could not be removed: {source}",
                path.display()
            ),
            Self::Published { path, source } => write!(
                f,
                "complete output already published at {}; temporary cleanup failed: {source}",
                path.display()
            ),
        }
    }
}
impl std::error::Error for WriteError {}
impl From<std::io::Error> for WriteError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn valid_extension(format: Format, path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match format {
        Format::Docx => extension == "docx",
        Format::Txt => extension == "txt",
        Format::Markdown => matches!(extension.as_str(), "md" | "markdown"),
        Format::Html => matches!(extension.as_str(), "html" | "htm"),
        Format::Rtf => extension == "rtf",
    }
}

/// No overwrite, including a source path or dangling symlink. Cancellation is
/// checked before IO and again immediately before publication. Once published,
/// success/Published is returned even if cancellation arrives afterwards.
pub fn write_new(
    plan: &Plan,
    destination: &Path,
    accept_losses: bool,
    cancel: &AtomicBool,
) -> Result<PathBuf, WriteError> {
    if !accept_losses {
        return Err(WriteError::NeedsConfirmation);
    }
    if cancel.load(Ordering::Acquire) {
        return Err(WriteError::Cancelled);
    }
    if !valid_extension(plan.target(), destination) {
        return Err(WriteError::InvalidExtension);
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
