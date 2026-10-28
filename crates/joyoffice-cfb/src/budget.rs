//! Untrusted-input resource budgets for Compound File Binary parsing (QUALITY-005).

use core::fmt;

/// Resource limits applied during Compound File Binary parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfbBudget {
    /// Maximum allowed file size in bytes (default 128 MiB).
    pub max_file_size: usize,
    /// Maximum number of sectors parsed (default 262,144 sectors = 128 MiB at 512B/sector).
    pub max_sectors: usize,
    /// Maximum number of directory entries allowed (default 8,192).
    pub max_entries: usize,
    /// Maximum single stream size in bytes (default 64 MiB).
    pub max_stream_size: usize,
    /// Maximum depth of directory tree (default 32).
    pub max_tree_depth: usize,
}

impl Default for CfbBudget {
    fn default() -> Self {
        Self {
            max_file_size: 128 * 1024 * 1024,
            max_sectors: 262_144,
            max_entries: 8_192,
            max_stream_size: 64 * 1024 * 1024,
            max_tree_depth: 32,
        }
    }
}

/// Errors raised when parsing exceeds resource budgets or encounters malformed structures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfbError {
    /// File is smaller than the 512-byte CFB header.
    TruncatedHeader,
    /// Header signature does not match MS-CFB `0xD0CF11E0A1B11AE1`.
    InvalidSignature,
    /// Byte order mark is not little-endian (0xFFFE).
    UnsupportedByteOrder(u16),
    /// Sector size is not 512 (v3) or 4096 (v4).
    UnsupportedSectorShift(u16),
    /// Mini sector size is not 64 (shift 6).
    UnsupportedMiniSectorShift(u16),
    /// File size exceeds budget or does not match sector count.
    BudgetExceeded(&'static str),
    /// Sector chain loop or out-of-bounds sector referenced.
    InvalidSectorChain(u32),
    /// Directory entry is malformed or invalid name encoding.
    MalformedDirectory(&'static str),
    /// Requested stream was not found.
    StreamNotFound(String),
    /// Unexpected end of data while reading stream or sector.
    UnexpectedEof,
}

impl fmt::Display for CfbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader => write!(f, "file truncated: cannot read 512-byte CFB header"),
            Self::InvalidSignature => write!(f, "invalid CFB signature magic"),
            Self::UnsupportedByteOrder(bo) => write!(f, "unsupported byte order 0x{bo:04X}"),
            Self::UnsupportedSectorShift(ss) => write!(f, "unsupported sector shift {ss}"),
            Self::UnsupportedMiniSectorShift(ss) => {
                write!(f, "unsupported mini sector shift {ss}")
            }
            Self::BudgetExceeded(msg) => write!(f, "resource budget exceeded: {msg}"),
            Self::InvalidSectorChain(sec) => write!(f, "invalid sector in chain: {sec}"),
            Self::MalformedDirectory(msg) => write!(f, "malformed directory: {msg}"),
            Self::StreamNotFound(name) => write!(f, "stream '{name}' not found"),
            Self::UnexpectedEof => write!(f, "unexpected EOF reading compound file data"),
        }
    }
}

impl std::error::Error for CfbError {}
