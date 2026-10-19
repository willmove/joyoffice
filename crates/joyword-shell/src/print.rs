//! Print-via-PDF (PLATFORM-007).
//!
//! The shell does not paginate. Callers hand it PDF bytes that were already
//! produced from the same page scene as the screen. The headless backend
//! publishes that file with the same write, sync, read-back and hard-link
//! steps as Save As. `copies` is recorded on the receipt; the file is the
//! PDF once. Native print dialogs are compiled per platform and return
//! [`PrintError::NotVerified`] without calling an operating-system API.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::storage::{sync_directory, verify_bytes};

const MAX_PDF: usize = 256 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrintReceipt {
    path: PathBuf,
    bytes: u64,
    copies: u16,
    range: String,
}

impl PrintReceipt {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    pub fn copies(&self) -> u16 {
        self.copies
    }

    pub fn range(&self) -> &str {
        &self.range
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PrintError {
    InvalidPdf,
    Extension,
    Copies,
    Range,
    Marks,
    Io(&'static str),
    NotVerified(&'static str),
}

impl std::fmt::Display for PrintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPdf => formatter.write_str("pdf"),
            Self::Extension => formatter.write_str("pdf extension"),
            Self::Copies => formatter.write_str("copies"),
            Self::Range => formatter.write_str("range"),
            Self::Marks => formatter.write_str("marks"),
            Self::Io(message) => formatter.write_str(message),
            Self::NotVerified(message) => formatter.write_str(message),
        }
    }
}

/// Inclusive 1-based `N` or `N-M`, or `all` / absent for every page.
/// The result is a half-open index range into an already paginated scene.
pub fn page_span(
    spec: Option<&str>,
    page_count: usize,
) -> Result<std::ops::Range<usize>, PrintError> {
    if page_count == 0 {
        return Err(PrintError::Range);
    }
    let Some(spec) = spec.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(0..page_count);
    };
    if spec.eq_ignore_ascii_case("all") {
        return Ok(0..page_count);
    }
    let (start, end) = if let Some((left, right)) = spec.split_once('-') {
        (page_number(left)?, page_number(right)?)
    } else {
        let page = page_number(spec)?;
        (page, page)
    };
    if start == 0 || end == 0 || start > end || end > page_count {
        return Err(PrintError::Range);
    }
    Ok((start - 1)..end)
}

fn page_number(text: &str) -> Result<usize, PrintError> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PrintError::Range);
    }
    text.parse().map_err(|_| PrintError::Range)
}

pub fn copies_of(spec: Option<&str>) -> Result<u16, PrintError> {
    let Some(spec) = spec.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(1);
    };
    if !spec.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PrintError::Copies);
    }
    let value: u16 = spec.parse().map_err(|_| PrintError::Copies)?;
    if (1..=99).contains(&value) {
        Ok(value)
    } else {
        Err(PrintError::Copies)
    }
}

/// Only the laid-out scene is printable. Markup and original review views
/// are refused because tracked-change printing is not implemented.
pub fn review_marks(spec: Option<&str>) -> Result<(), PrintError> {
    match spec.map(str::trim).filter(|text| !text.is_empty()) {
        None | Some("laid-out") => Ok(()),
        Some(_) => Err(PrintError::Marks),
    }
}

#[cfg(target_os = "macos")]
pub fn macos_print_dialog() -> Result<(), PrintError> {
    Err(PrintError::NotVerified(
        "macOS print dialog is not verified",
    ))
}

#[cfg(target_os = "windows")]
pub fn windows_print_dialog() -> Result<(), PrintError> {
    Err(PrintError::NotVerified(
        "Windows print dialog is not verified",
    ))
}

#[cfg(target_os = "linux")]
pub fn linux_print_dialog() -> Result<(), PrintError> {
    Err(PrintError::NotVerified(
        "Linux print dialog is not verified",
    ))
}

/// The native dialog for this build. It does not open a window or write a file.
pub fn platform_print_dialog() -> Result<(), PrintError> {
    #[cfg(target_os = "macos")]
    {
        macos_print_dialog()
    }
    #[cfg(target_os = "windows")]
    {
        windows_print_dialog()
    }
    #[cfg(target_os = "linux")]
    {
        linux_print_dialog()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(PrintError::NotVerified("print dialog is not verified"))
    }
}

pub struct PrintToFile<'a> {
    pub destination: &'a Path,
    pub pdf: &'a [u8],
    pub copies: u16,
    pub range: &'a str,
}

/// Headless print-to-file. `copies` is stored on the receipt and is not
/// repeated inside the PDF.
pub fn print_to_file(request: PrintToFile<'_>) -> Result<PrintReceipt, PrintError> {
    if !(1..=99).contains(&request.copies) {
        return Err(PrintError::Copies);
    }
    if request.range.trim().is_empty() {
        return Err(PrintError::Range);
    }
    let mut receipt = publish_pdf(request.destination, request.pdf)?;
    receipt.copies = request.copies;
    receipt.range = request.range.to_string();
    Ok(receipt)
}

/// Write-then-verify publish of one PDF. An existing path is left untouched.
pub fn publish_pdf(destination: &Path, bytes: &[u8]) -> Result<PrintReceipt, PrintError> {
    if !pdf_bytes(bytes) {
        return Err(PrintError::InvalidPdf);
    }
    if destination.file_name().is_none()
        || !destination
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
    {
        return Err(PrintError::Extension);
    }
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent =
        fs::canonicalize(parent).map_err(|_| PrintError::Io("cannot inspect output directory"))?;
    let Some(file_name) = destination.file_name() else {
        return Err(PrintError::Extension);
    };
    let target = parent.join(file_name);
    match fs::symlink_metadata(&target) {
        Ok(_) => return Err(PrintError::Io("destination exists")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(PrintError::Io("cannot inspect destination")),
    }
    let mut temporary = tempfile::Builder::new()
        .prefix(".joyword-pdf-")
        .tempfile_in(&parent)
        .map_err(|_| PrintError::Io("cannot create temporary file"))?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| PrintError::Io("cannot write output"))?;
    verify_bytes(temporary.as_file_mut(), bytes)
        .map_err(|_| PrintError::Io("verification failed"))?;
    match fs::symlink_metadata(&target) {
        Ok(_) => return Err(PrintError::Io("destination exists")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(PrintError::Io("cannot inspect destination")),
    }
    fs::hard_link(temporary.path(), &target)
        .map_err(|_| PrintError::Io("cannot publish output"))?;
    temporary
        .close()
        .map_err(|_| PrintError::Io("temporary cleanup failed"))?;
    sync_directory(&parent).map_err(|_| PrintError::Io("directory sync failed"))?;
    let published = fs::read(&target).map_err(|_| PrintError::Io("read-back failed"))?;
    if published != bytes {
        return Err(PrintError::Io("read-back mismatch"));
    }
    Ok(PrintReceipt {
        path: target,
        bytes: bytes.len() as u64,
        copies: 1,
        range: "all".to_string(),
    })
}

fn pdf_bytes(bytes: &[u8]) -> bool {
    bytes.len() > 8
        && bytes.len() <= MAX_PDF
        && bytes.starts_with(b"%PDF-")
        && bytes.windows(5).any(|window| window == b"%%EOF")
}
