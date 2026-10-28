//! Untrusted-input resource budgets for RTF parsing.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtfBudget {
    pub max_group_depth: usize,
    pub max_text_bytes: usize,
    pub max_control_word_len: usize,
}

impl Default for RtfBudget {
    fn default() -> Self {
        Self {
            max_group_depth: 64,
            max_text_bytes: 32 * 1024 * 1024,
            max_control_word_len: 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtfError {
    UnexpectedEof,
    InvalidHeader,
    GroupDepthExceeded,
    MalformedHexByte,
    BudgetExceeded(&'static str),
}

impl std::fmt::Display for RtfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of RTF stream"),
            Self::InvalidHeader => write!(f, "missing or invalid {{{{rtf header"),
            Self::GroupDepthExceeded => write!(f, "RTF group nesting depth exceeded budget"),
            Self::MalformedHexByte => write!(f, "malformed hex escape \\'xx"),
            Self::BudgetExceeded(msg) => write!(f, "RTF budget exceeded: {msg}"),
        }
    }
}

impl std::error::Error for RtfError {}
