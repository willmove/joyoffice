//! Resource budgets and error types for MS-DOC and WPS binary files.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocBudget {
    pub max_file_bytes: usize,
    pub max_text_chars: usize,
    pub max_pieces: usize,
    pub max_paragraphs: usize,
}

impl Default for DocBudget {
    fn default() -> Self {
        Self {
            max_file_bytes: 64 * 1024 * 1024,
            max_text_chars: 16 * 1024 * 1024,
            max_pieces: 65536,
            max_paragraphs: 100_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocError {
    CfbError(String),
    MissingWordDocumentStream,
    MissingTableStream(&'static str),
    InvalidFibMagic,
    UnsupportedFibVersion(u16),
    EncryptedDocument,
    MalformedFib,
    MalformedClx,
    MalformedPieceTable,
    BudgetExceeded(&'static str),
    Utf16Error,
}

impl std::fmt::Display for DocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CfbError(msg) => write!(f, "CFB container error: {msg}"),
            Self::MissingWordDocumentStream => write!(f, "missing 'WordDocument' stream"),
            Self::MissingTableStream(name) => write!(f, "missing table stream '{name}'"),
            Self::InvalidFibMagic => write!(f, "invalid FIB magic number (expected 0xA5EC)"),
            Self::UnsupportedFibVersion(v) => {
                write!(f, "unsupported Word binary FIB version: 0x{v:04X}")
            }
            Self::EncryptedDocument => write!(
                f,
                "encrypted MS-DOC documents are not supported for plain reading"
            ),
            Self::MalformedFib => write!(f, "malformed File Information Block (FIB)"),
            Self::MalformedClx => write!(f, "malformed Clx / piece table in table stream"),
            Self::MalformedPieceTable => write!(f, "malformed PlcPcd piece descriptors"),
            Self::BudgetExceeded(res) => write!(f, "resource budget exceeded: {res}"),
            Self::Utf16Error => write!(f, "invalid UTF-16LE sequence in document stream"),
        }
    }
}

impl std::error::Error for DocError {}
