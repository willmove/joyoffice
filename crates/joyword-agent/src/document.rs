//! Document access port (T401, AI-002).
//!
//! `check_gates.py` forbids this crate from depending on any document,
//! engine, or network crate, so the agent never touches the document model
//! directly. [`DocumentPort`] is the abstract seam a host (later the shell)
//! implements; [`MemoryDocument`] is the in-crate fake used by contract tests,
//! mirroring the T400 fake-provider style.
//!
//! Privacy is structural: a [`DocumentSnapshot`] only has a channel for the
//! final visible text. Deleted revision content and comments cannot be
//! expressed in the snapshot type, so no tool can project them even if a host
//! tried (AI.md §2: hidden deletions, comments, and other documents are not
//! authorized by selecting body text).

use std::fmt;

use crate::provider::IdError;
use crate::scope::{DocumentId, ScopeRange};

/// Hard budgets for untrusted document input (JoyWord assumption).
pub const MAX_PARAGRAPHS: usize = 4096;
pub const MAX_PARAGRAPH_BYTES: usize = 64 * 1024;
pub const MAX_DOCUMENT_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_HIDDEN_ITEMS: usize = 64;

/// Why a port refused to project a document. Static labels only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortError {
    UnknownDocument,
    BudgetExceeded,
    Inaccessible,
}

impl PortError {
    pub const fn kind(self) -> &'static str {
        match self {
            PortError::UnknownDocument => "unknown-document",
            PortError::BudgetExceeded => "port-budget-exceeded",
            PortError::Inaccessible => "port-inaccessible",
        }
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PortError::UnknownDocument => "document is not accessible through this port",
            PortError::BudgetExceeded => "document exceeds a projection budget",
            PortError::Inaccessible => "document port is inaccessible",
        })
    }
}

impl std::error::Error for PortError {}

/// One paragraph of the final visible projection. `heading_level` 0 is body
/// text; 1..=9 are heading levels. There is no field for hidden content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotParagraph {
    pub heading_level: u8,
    pub visible: String,
}

/// One outline node with absolute character offsets in the snapshot text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutlineNode {
    pub paragraph: u32,
    pub heading_level: u8,
    pub start: u64,
    pub end: u64,
}

/// Immutable projection of the final visible content at one version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSnapshot {
    document_id: DocumentId,
    version: u64,
    text: String,
    char_len: u64,
    outline: Vec<OutlineNode>,
}

impl DocumentSnapshot {
    /// Hosts build snapshots through this constructor so the budgets apply to
    /// every implementation, not just the in-crate fake.
    pub fn new(
        document_id: DocumentId,
        version: u64,
        paragraphs: Vec<SnapshotParagraph>,
    ) -> Result<Self, PortError> {
        if paragraphs.len() > MAX_PARAGRAPHS {
            return Err(PortError::BudgetExceeded);
        }
        let mut text = String::new();
        let mut outline = Vec::with_capacity(paragraphs.len());
        let mut char_pos: u64 = 0;
        for (index, paragraph) in paragraphs.iter().enumerate() {
            if paragraph.visible.len() > MAX_PARAGRAPH_BYTES {
                return Err(PortError::BudgetExceeded);
            }
            if !text.is_empty() {
                text.push('\n');
                char_pos += 1;
            }
            let start = char_pos;
            let chars = u64::try_from(paragraph.visible.chars().count())
                .map_err(|_| PortError::BudgetExceeded)?;
            text.push_str(&paragraph.visible);
            char_pos += chars;
            if text.len() > MAX_DOCUMENT_TEXT_BYTES {
                return Err(PortError::BudgetExceeded);
            }
            let Ok(paragraph_index) = u32::try_from(index) else {
                return Err(PortError::BudgetExceeded);
            };
            outline.push(OutlineNode {
                paragraph: paragraph_index,
                heading_level: paragraph.heading_level,
                start,
                end: char_pos,
            });
        }
        Ok(Self {
            document_id,
            version,
            char_len: char_pos,
            text,
            outline,
        })
    }

    pub fn document_id(&self) -> &DocumentId {
        &self.document_id
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn char_len(&self) -> u64 {
        self.char_len
    }

    pub fn outline(&self) -> &[OutlineNode] {
        &self.outline
    }

    /// Character-offset slice, or `None` when the range leaves the text.
    pub fn slice_chars(&self, range: ScopeRange) -> Option<&str> {
        if range.start() > self.char_len || range.end() > self.char_len {
            return None;
        }
        let mut start_byte = self.text.len();
        let mut start_found = range.start() == self.char_len;
        if range.start() == 0 {
            start_byte = 0;
            start_found = true;
        }
        for (char_index, (byte_index, _)) in self.text.char_indices().enumerate() {
            let position = char_index as u64;
            if position == range.start() {
                start_byte = byte_index;
                start_found = true;
            }
            if position == range.end() {
                return start_found.then(|| &self.text[start_byte..byte_index]);
            }
        }
        start_found.then(|| &self.text[start_byte..])
    }
}

/// Read-only seam to the host's document. One document per call, identified
/// explicitly; there is no enumeration and no filesystem access.
pub trait DocumentPort {
    fn snapshot(&self, document: &DocumentId) -> Result<DocumentSnapshot, PortError>;
}

/// A paragraph the fake keeps locally. `deleted` and `comments` model hidden
/// revision content and comments: they exist in the source but the projection
/// drops them, exactly like a host port must.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryParagraph {
    pub heading_level: u8,
    pub visible: String,
    pub deleted: Vec<String>,
    pub comments: Vec<String>,
}

impl MemoryParagraph {
    pub fn body(visible: &str) -> Self {
        Self {
            heading_level: 0,
            visible: visible.to_owned(),
            deleted: Vec::new(),
            comments: Vec::new(),
        }
    }

    pub fn heading(level: u8, visible: &str) -> Self {
        Self {
            heading_level: level,
            visible: visible.to_owned(),
            deleted: Vec::new(),
            comments: Vec::new(),
        }
    }
}

/// In-memory fake document for contract tests. Version 1 at creation; every
/// accepted edit bumps the version so stale grants fail validation.
#[derive(Clone, Debug)]
pub struct MemoryDocument {
    id: DocumentId,
    version: u64,
    paragraphs: Vec<MemoryParagraph>,
}

impl MemoryDocument {
    pub fn new(id: &str) -> Result<Self, IdError> {
        Ok(Self {
            id: DocumentId::parse(id)?,
            version: 1,
            paragraphs: Vec::new(),
        })
    }

    pub fn document_id(&self) -> &DocumentId {
        &self.id
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn push_paragraph(&mut self, paragraph: MemoryParagraph) -> Result<(), PortError> {
        if self.paragraphs.len() >= MAX_PARAGRAPHS
            || paragraph.visible.len() > MAX_PARAGRAPH_BYTES
            || paragraph.deleted.len() > MAX_HIDDEN_ITEMS
            || paragraph.comments.len() > MAX_HIDDEN_ITEMS
            || paragraph
                .deleted
                .iter()
                .chain(paragraph.comments.iter())
                .any(|item| item.len() > MAX_PARAGRAPH_BYTES)
        {
            return Err(PortError::BudgetExceeded);
        }
        self.paragraphs.push(paragraph);
        Ok(())
    }

    /// A user edit: replaces visible text and bumps the version. Grants bound
    /// to the old version must fail from here on.
    pub fn edit_visible(&mut self, index: usize, visible: &str) -> Result<(), PortError> {
        if visible.len() > MAX_PARAGRAPH_BYTES || self.version == u64::MAX {
            return Err(PortError::BudgetExceeded);
        }
        let paragraph = self
            .paragraphs
            .get_mut(index)
            .ok_or(PortError::UnknownDocument)?;
        paragraph.visible = visible.to_owned();
        self.version += 1;
        Ok(())
    }

    fn projected(&self) -> Vec<SnapshotParagraph> {
        self.paragraphs
            .iter()
            .map(|paragraph| SnapshotParagraph {
                heading_level: paragraph.heading_level,
                visible: paragraph.visible.clone(),
            })
            .collect()
    }
}

impl DocumentPort for MemoryDocument {
    fn snapshot(&self, document: &DocumentId) -> Result<DocumentSnapshot, PortError> {
        if document != &self.id {
            return Err(PortError::UnknownDocument);
        }
        DocumentSnapshot::new(self.id.clone(), self.version, self.projected())
    }
}
