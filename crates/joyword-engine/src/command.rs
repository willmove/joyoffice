//! Typed edit commands (ARCHITECTURE §3). Commands name logical targets by
//! story, node and scalar offset; they cannot carry scripts or raw XML.
//! Positions in a command refer to the document as left by the previous
//! commands of the same transaction.

use std::fmt;
use std::sync::Arc;

use joyword_model::{
    Color, FontRef, Hyperlink, Indent, InlineKind, Justification, Measure, NodeId, NumberingRef,
    PageSetup, Position, RunProps, ScalarOffset, Spacing, StoryId, StyleId, TabStop, TextError,
    Toggle, Underline,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Ordinary text; line and paragraph separators are separate commands.
    InsertText {
        at: Position,
        text: String,
    },
    InsertInline {
        at: Position,
        kind: InlineKind,
    },
    /// Admits still-image bytes and inserts one inline image.
    /// Identical bytes and media type reuse the existing resource.
    InsertImage {
        at: Position,
        data: Arc<[u8]>,
    },
    /// Sets or clears the hyperlink on the runs in `start..end`.
    /// `None` removes the link. One command is one undo step.
    SetHyperlink {
        start: Position,
        end: Position,
        link: Option<Hyperlink>,
    },
    /// Deletes between two positions in the same container, merging the
    /// paragraphs at both ends when they differ.
    DeleteRange {
        start: Position,
        end: Position,
    },
    SplitParagraph {
        at: Position,
    },
    FormatText {
        start: Position,
        end: Position,
        change: RunChange,
    },
    FormatParagraph {
        story: StoryId,
        paragraph: NodeId,
        change: ParagraphChange,
    },
    /// Moves a block before `before` (or to the end) of its own container.
    MoveBlock {
        story: StoryId,
        block: NodeId,
        before: Option<NodeId>,
    },
    RemoveBlock {
        story: StoryId,
        block: NodeId,
    },
    /// Applies a paragraph or character style. A linked style follows
    /// ECMA-376 Part 1 §17.7.4.6: a whole-paragraph span uses the paragraph
    /// style, and a partial span uses the linked character style.
    ApplyStyle {
        start: Position,
        end: Position,
        style: StyleId,
    },
    /// Sets list membership on every paragraph the range touches.
    /// `None` and `num_id` 0 both write `numId` 0, which overrides a style.
    SetNumbering {
        start: Position,
        end: Position,
        numbering: Option<NumberingRef>,
    },
    /// Changes `ilvl` and keeps each paragraph's own `numId`.
    SetNumberingLevel {
        start: Position,
        end: Position,
        level: u8,
    },
    /// Inserts a table before the top-level body block that contains `at`.
    InsertTable {
        at: Position,
        rows: u32,
        columns: u32,
    },
    /// Inserts a row after the row that contains `at`.
    InsertTableRow {
        at: Position,
    },
    /// Deletes the row that contains `at`.
    DeleteTableRow {
        at: Position,
    },
    /// Inserts a column after the cell that contains `at`.
    InsertTableColumn {
        at: Position,
    },
    /// Deletes the column that contains `at`.
    DeleteTableColumn {
        at: Position,
    },
    /// Merges the cell at `at` with the next cell to the right or below.
    MergeTableCells {
        at: Position,
        direction: TableMerge,
    },
    /// Splits a vertical merge, or a horizontal span when there is no
    /// vertical merge. Text stays in the leading cell.
    SplitTableCell {
        at: Position,
    },
    /// Replaces one section's page size, margins and orientation.
    SetPageSetup {
        section: NodeId,
        page: PageSetup,
    },
}

/// Character formatting changes of the M1 subset.
#[derive(Clone, Debug, PartialEq)]
pub enum RunChange {
    Bold(Toggle),
    Italic(Toggle),
    Strike(Toggle),
    Underline(Option<Underline>),
    Size(Option<Measure>),
    Color(Option<Color>),
    AsciiFont(Option<FontRef>),
    EastAsiaFont(Option<FontRef>),
}

impl RunChange {
    pub(crate) fn apply(&self, props: &mut RunProps) {
        match self {
            RunChange::Bold(value) => props.bold = *value,
            RunChange::Italic(value) => props.italic = *value,
            RunChange::Strike(value) => props.strike = *value,
            RunChange::Underline(value) => props.underline = *value,
            RunChange::Size(value) => props.size = *value,
            RunChange::Color(value) => props.color = *value,
            RunChange::AsciiFont(value) => {
                props.fonts.ascii = value.clone();
                props.fonts.h_ansi = value.clone();
            }
            RunChange::EastAsiaFont(value) => props.fonts.east_asia = value.clone(),
        }
    }
}

/// Paragraph formatting changes of the M1 subset.
#[derive(Clone, Debug, PartialEq)]
pub enum ParagraphChange {
    Style(Option<StyleId>),
    Justification(Option<Justification>),
    Indent(Indent),
    Spacing(Spacing),
    /// Replaces the paragraph's direct tab list. `None` restores inheritance.
    Tabs(Option<Vec<TabStop>>),
}

/// Condition checked against the document before any command runs, so a
/// proposal computed on stale content is rejected (AI-004).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Precondition {
    /// Plain text of one paragraph range; inline objects read as U+FFFC.
    Text {
        story: StoryId,
        paragraph: NodeId,
        start: ScalarOffset,
        end: ScalarOffset,
        expected: String,
    },
}

/// Which neighbour [`Command::MergeTableCells`] joins to the anchor cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableMerge {
    Right,
    Down,
}

/// Why a merge or split left the table unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableStructure {
    /// The requested neighbour cell does not exist.
    NoNeighbour,
    /// A horizontal merge refuses a cell that already has `vMerge`.
    VerticalMerge,
    /// The cell below does not start on the same column with the same span.
    SpanMismatch,
    /// Merge and split start at the restart cell, not a continue cell.
    ContinueCell,
    /// The cell below already belongs to a vertical merge.
    VerticalConflict,
    /// The cell has neither a horizontal span nor a vertical merge.
    NotMerged,
    /// The table, row or cell still holds preserved markup.
    Preserved,
    /// The cell holds a block this edit does not move.
    UnsupportedCell,
}

/// Why a command could not apply. The whole transaction is rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandError {
    UnknownStory(StoryId),
    UnknownNode(NodeId),
    NotAParagraph(NodeId),
    /// The positions are in different stories, cells or content controls.
    CrossContainer,
    ReversedRange,
    /// Inside or deleting a locked content control.
    Locked(NodeId),
    /// Line breaks, paragraph separators and control characters are not
    /// ordinary text.
    InvalidText(char),
    MissingResource,
    /// Still-image bytes were refused. The text does not include the payload.
    ImageRefused(&'static str),
    /// A container would be left without blocks.
    LastBlock(NodeId),
    Text(TextError),
    /// No style with this `w:styleId`.
    UnknownStyle(StyleId),
    /// Table and numbering styles are stored but cannot be applied to text.
    UnresolvedStyle(StyleId),
    /// The `basedOn` chain cycles, is too deep, or mixes kinds.
    StyleChain(StyleId),
    /// The selection does not match the style kind and there is no usable link.
    StyleTarget(StyleId),
    /// No numbering instance with this `numId`.
    UnknownNumbering(u32),
    /// `ilvl` is outside 0..=8.
    NumberingLevel(u8),
    /// `list.level` found a paragraph that is not in a list.
    NoNumbering(NodeId),
    /// The position is not inside a top-level table cell.
    NotATable(NodeId),
    /// Rows and columns are limited to 1..=32.
    TableLimit,
    /// A row or column edit would break a vertical or horizontal merge.
    MergedStructure(NodeId),
    /// Column edits require every row to cover the grid with span-1 cells.
    GridGap(NodeId),
    /// A table must keep one row.
    LastRow(NodeId),
    /// A table must keep one column.
    LastColumn(NodeId),
    /// Merge or split would not leave a grid the model accepts.
    TableStructure(NodeId, TableStructure),
    /// Table insertion is a body-story operation.
    NotTopLevel,
    /// Page size, margins or header distance cannot be placed.
    InvalidPage,
    /// The editor refused to store this hyperlink. The text is not a URL.
    Hyperlink(&'static str),
}

impl fmt::Display for TableStructure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TableStructure::NoNeighbour => "there is no cell on that side",
            TableStructure::VerticalMerge => "a horizontal merge cannot cross a vertical merge",
            TableStructure::SpanMismatch => "the cell below does not cover the same columns",
            TableStructure::ContinueCell => "merge or split starts at the restart cell",
            TableStructure::VerticalConflict => "the cell below is already merged",
            TableStructure::NotMerged => "the cell is not merged",
            TableStructure::Preserved => "preserved markup would be rewritten",
            TableStructure::UnsupportedCell => "the cell holds a block this edit cannot move",
        })
    }
}

impl From<TextError> for CommandError {
    fn from(error: TextError) -> Self {
        CommandError::Text(error)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::UnknownStory(story) => write!(f, "unknown story {story}"),
            CommandError::UnknownNode(node) => write!(f, "unknown node {node}"),
            CommandError::NotAParagraph(node) => write!(f, "{node} is not a paragraph"),
            CommandError::CrossContainer => f.write_str("range crosses a container boundary"),
            CommandError::ReversedRange => f.write_str("range ends before it starts"),
            CommandError::Locked(node) => write!(f, "content control {node} is locked"),
            CommandError::InvalidText(ch) => write!(f, "U+{:04X} is not ordinary text", *ch as u32),
            CommandError::MissingResource => f.write_str("image resource does not exist"),
            CommandError::ImageRefused(reason) => write!(f, "{reason}"),
            CommandError::LastBlock(node) => write!(f, "{node} is the last block of its container"),
            CommandError::Text(error) => write!(f, "{error}"),
            CommandError::UnknownStyle(style) => write!(f, "unknown style {style}"),
            CommandError::UnresolvedStyle(style) => {
                write!(f, "style {style} is not a paragraph or character style")
            }
            CommandError::StyleChain(style) => {
                write!(
                    f,
                    "style {style} has a cycle, a kind mismatch, or is too deep"
                )
            }
            CommandError::StyleTarget(style) => {
                write!(f, "style {style} does not apply to this selection")
            }
            CommandError::UnknownNumbering(id) => write!(f, "unknown numbering instance {id}"),
            CommandError::NumberingLevel(level) => {
                write!(f, "numbering level {level} is outside 0..=8")
            }
            CommandError::NoNumbering(node) => write!(f, "{node} is not in a list"),
            CommandError::NotATable(node) => write!(f, "{node} is not in a table cell"),
            CommandError::TableLimit => f.write_str("table is limited to 32 rows and columns"),
            CommandError::MergedStructure(node) => {
                write!(f, "table {node} has merged cells")
            }
            CommandError::GridGap(node) => write!(f, "table {node} has grid gaps"),
            CommandError::LastRow(node) => write!(f, "table {node} has one row"),
            CommandError::LastColumn(node) => write!(f, "table {node} has one column"),
            CommandError::TableStructure(node, reason) => {
                write!(f, "table {node} refuses the edit: {reason}")
            }
            CommandError::NotTopLevel => f.write_str("tables are inserted in the body"),
            CommandError::InvalidPage => f.write_str("page setup is not usable"),
            CommandError::Hyperlink(reason) => f.write_str(reason),
        }
    }
}

/// Ordinary text excludes C0/C1 controls other than tab, and line or
/// paragraph separators (U+2028, U+2029), which have typed commands.
pub(crate) fn check_text(text: &str) -> Result<(), CommandError> {
    match text
        .chars()
        .find(|&ch| (ch.is_control() && ch != '\t') || matches!(ch, '\u{2028}' | '\u{2029}'))
    {
        Some(ch) => Err(CommandError::InvalidText(ch)),
        None => Ok(()),
    }
}
