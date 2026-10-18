//! JoyWord typed document model (T100; ARCHITECTURE §2, ADR-003).
//!
//! Pure data and invariants: no GPUI, file dialogs, network or AI providers.
//! Stories hold blocks in a persistent tree, paragraphs hold chunked text with
//! typed runs and inline objects, and range annotations share one position
//! space. Everything is structurally shared, so snapshots cost O(1) and an
//! edit copies O(log n) nodes. Editing transactions, position mapping and undo
//! belong to the engine (T101); file formats belong to the format layer (T103).

mod annotation;
mod block;
mod document;
mod dump;
mod id;
mod numbering;
mod props;
mod resource;
mod style;
mod text;
mod theme;
mod tree;
mod validate;

pub use annotation::{Anchor, Annotation, AnnotationKind, Bias, Position};
pub use block::{
    Block, BlockSeq, BorderEdge, BorderKind, BreakKind, Cell, CellBorders, InlineKind,
    InlineObject, OpaqueBlock, Paragraph, Preserved, Removed, Row, Run, Sdt, SdtLock, SdtProps,
    Segment, Shading, Table, TableWidth, VerticalMerge,
};
pub use document::{
    DEFAULT_COMPATIBILITY_MODE, Document, DocumentSettings, DocumentSnapshot, DocumentVersion,
    HeaderFooterKind, NEW_DOCUMENT_COMPATIBILITY_MODE, Orientation, PageSetup, Section, Story,
    StoryKind,
};
pub use dump::{DumpOptions, SemanticDifference};
pub use id::{AnnotationId, IdAllocator, NodeId, PreservedId, ResourceId, SourceRef, StoryId};
pub use numbering::{
    AbstractNum, IndentEdges, LevelOverride, MAX_LEVEL, NumInstance, NumberFormat, NumberLabel,
    NumberSuffix, NumberingLevel, NumberingProblem, NumberingStore, PlacedTab, ResolvedLevel,
    displayed_paragraph, indent_edges, list_marks, list_reference, tab_positions,
};
pub use props::{
    Color, FontHint, FontRef, FontSlots, Indent, Justification, Languages, LineSpacing, Measure,
    NumberingRef, ParaProps, RunProps, Spacing, TabAlign, TabLeader, TabStop, ThemeFont, Toggle,
    Underline, merge_tab_stops,
};
pub use resource::{ContentHash, Resource, ResourceTable};
pub use style::{
    BuiltinStyle, DocDefaults, MAX_STYLE_DEPTH, ResolvedParagraph, ResolvedRun, Style, StyleChain,
    StyleError, StyleId, StyleKind, StyleProblem, StyleSheet, overlay_paragraph,
};
pub use text::{
    CHUNK_MAX_BYTES, OBJECT_MARKER, ParagraphText, ScalarOffset, TextError, TextSummary,
    Utf8Offset, Utf16Offset,
};
pub use theme::{ThemeColorSlot, ThemeFonts, ThemeScheme};
pub use tree::{Item, SeqTree, Summary};
pub use validate::{
    Diagnostic, Diagnostics, Issue, MAX_DIAGNOSTICS, MAX_NESTING, Severity, table_structure,
};
