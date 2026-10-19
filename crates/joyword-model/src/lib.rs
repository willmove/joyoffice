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
mod hyperlink;
mod id;
mod image;
mod numbering;
mod props;
mod resource;
mod revision;
mod style;
mod symbol_font;
mod table_geom;
mod text;
mod theme;
mod tree;
mod validate;

pub use annotation::{Anchor, Annotation, AnnotationKind, Bias, Position};
pub use block::{
    Block, BlockSeq, BorderEdge, BorderKind, BreakKind, Cell, CellBorders, CellMargins, InlineKind,
    InlineObject, OpaqueBlock, PageField, PageFieldKind, Paragraph, Preserved, Removed, Row,
    RowHeight, RowHeightRule, Run, Sdt, SdtLock, SdtProps, Segment, Shading, Table, TableAlign,
    TableLayoutKind, TableLook, TableWidth, VerticalAlign, VerticalMerge,
};
pub use document::{
    DEFAULT_COMPATIBILITY_MODE, Document, DocumentSettings, DocumentSnapshot, DocumentVersion,
    EqualColumns, HeaderFooterKind, NEW_DOCUMENT_COMPATIBILITY_MODE, Orientation, PageNumbering,
    PageSetup, Section, SectionBreak, Story, StoryKind,
};
pub use dump::{DumpOptions, SemanticDifference};
pub use hyperlink::{Hyperlink, MAX_HYPERLINK_CHARS, OpenRefusal, open_target};
pub use id::{AnnotationId, IdAllocator, NodeId, PreservedId, ResourceId, SourceRef, StoryId};
pub use image::{
    AcceptedImage, DecodedImage, ImageLimits, ImageRefuse, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION,
    MAX_IMAGE_PIXELS, accept_image, decode_rgba, display_extent, extent_emu, sample_png_1x1,
    sample_png_overwide,
};
pub use numbering::{
    AbstractNum, IndentEdges, LevelOverride, MAX_LEVEL, NumInstance, NumberFormat, NumberLabel,
    NumberSuffix, NumberingLevel, NumberingProblem, NumberingStore, PlacedTab, ROMAN_MAX,
    ResolvedLevel, displayed_paragraph, indent_edges, list_marks, list_reference, tab_positions,
};
pub use props::{
    Color, FontHint, FontRef, FontSlots, Indent, Justification, Languages, LineSpacing, Measure,
    NumberingRef, ParaProps, RunProps, Spacing, TabAlign, TabLeader, TabStop, ThemeFont, Toggle,
    Underline, merge_tab_stops,
};
pub use resource::{ContentHash, Resource, ResourceTable};
pub use revision::{MoveMarker, MoveSide, RevisionKind, RevisionView, TextRevision};
pub use style::{
    BuiltinStyle, DocDefaults, MAX_STYLE_DEPTH, ResolvedParagraph, ResolvedRun, Style, StyleChain,
    StyleError, StyleId, StyleKind, StyleProblem, StyleSheet, TableCondition, TableConditional,
    TableFormat, overlay_paragraph,
};
pub use symbol_font::{is_mapped_family, map_private_use};
pub use table_geom::{TableMetrics, table_metrics};
pub use text::{
    CHUNK_MAX_BYTES, OBJECT_MARKER, ParagraphText, ScalarOffset, TextError, TextSummary,
    Utf8Offset, Utf16Offset,
};
pub use theme::apply_tint_shade;
pub use theme::{ThemeColorSlot, ThemeFonts, ThemeScheme};
pub use tree::{Item, SeqTree, Summary};
pub use validate::{
    Diagnostic, Diagnostics, Issue, MAX_DIAGNOSTICS, MAX_NESTING, Severity, table_structure,
};
