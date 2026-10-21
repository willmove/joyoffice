//! Tool names from AI.md §2. Dispatch refuses every tool. Applying a
//! [`ChangeProposal`] is T402 and always returns [`ApplyBlocked::DeferredUntilT402`].

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolId {
    ReadSelection,
    ReadStructure,
    SearchDocument,
    ProposeReplace,
    ProposeStyle,
    ProposeTable,
    ProposeComment,
}

impl ToolId {
    pub const ALL: [ToolId; 7] = [
        ToolId::ReadSelection,
        ToolId::ReadStructure,
        ToolId::SearchDocument,
        ToolId::ProposeReplace,
        ToolId::ProposeStyle,
        ToolId::ProposeTable,
        ToolId::ProposeComment,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            ToolId::ReadSelection => "read_selection",
            ToolId::ReadStructure => "read_structure",
            ToolId::SearchDocument => "search_document",
            ToolId::ProposeReplace => "propose_replace",
            ToolId::ProposeStyle => "propose_style",
            ToolId::ProposeTable => "propose_table",
            ToolId::ProposeComment => "propose_comment",
        }
    }

    /// No tool runs in T400. Comment tools are names only.
    pub const fn dispatch(self) -> Result<(), ToolUnavailable> {
        Err(ToolUnavailable { tool: self })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolUnavailable {
    pub tool: ToolId,
}

impl fmt::Display for ToolUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tool {} is not available", self.tool.name())
    }
}

/// Fields a later milestone will validate. `apply` does not edit a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeProposal {
    pub proposal_id: String,
    pub document_id: String,
    pub base_version: u64,
    pub explanation: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyBlocked {
    DeferredUntilT402,
}

impl ChangeProposal {
    pub fn apply(self) -> Result<(), ApplyBlocked> {
        let _ = self;
        Err(ApplyBlocked::DeferredUntilT402)
    }
}
