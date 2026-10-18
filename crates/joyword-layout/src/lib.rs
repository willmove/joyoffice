//! JoyWord text layout foundations (T003 first, then T105).
//!
//! Fonts are identified by their bytes and face index, shaping uses HarfRust
//! with features stated explicitly by the document, and the result is placed
//! in a [`PageScene`] in EMU. HarfRust, read-fonts and the PDF writer share
//! one font-parsing stack (read-fonts 0.39). T105 currently offers bounded
//! single-font LTR line fitting with explicit or Unicode default breaks and
//! versioned single-section pagination/hit geometry under an explicit preview
//! profile. [`cjk`] holds the measured T207 calculations. Pagination applies
//! a result only when that result's evidence and face match; unverified
//! pairs and other faces keep the T105 advance and line box.

pub mod cache;
pub mod cjk;
pub mod document;
pub mod font;
pub mod line;
mod list_layout;
mod pages;
pub mod scene;
pub mod shape;
pub mod slots;
mod style_paint;

pub use font::{FontError, FontKey, LoadedFont};
pub use line::{
    BreakError, ClusterRange, LayoutLine, LineDiagnostic, LineError, LineLimit, LineLimits,
    LineStats, OverflowKind, ParagraphLines, SourceRange, layout_paragraph, layout_paragraph_auto,
};
pub use scene::{GlyphRun, PageScene, to_points, to_screen};
pub use shape::{FeatureSet, ShapedGlyph, ShapedRun, shape};
pub use slots::{
    FaceSet, FontSlot, MixedLine, MixedParagraph, adjust_cjk_break, prohibited_line_edge, slot_for,
    wrap_mixed,
};

pub use cache::{
    DEFAULT_CACHE_BUDGET, DependencyFlags, ExternalContext, ExternalKey, LayoutInvalidation,
    LayoutReuse, MAX_LAYOUT_PASSES, ParagraphLayoutCache, affected_style_closure, external_key,
    invalidated_paragraphs, layout_incremental, paragraph_dependencies, run_layout_passes,
};
pub use document::{
    CaretGeometry, CaretStop, LayoutCancellation, LayoutDiagnostic, LayoutError, LayoutKey,
    LayoutLimit, LayoutLimits, LayoutNote, LayoutSnapshot, LayoutStats, PageGeometry, PlacedLine,
    SelectionRect, SingleFontProfile, StaleReason, layout_document,
};
