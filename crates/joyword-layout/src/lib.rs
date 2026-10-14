//! JoyWord text layout foundations (T003 first, then T105).
//!
//! Fonts are identified by their bytes and face index, shaping uses HarfRust
//! with features stated explicitly by the document, and the result is placed
//! in a [`PageScene`] in EMU. HarfRust, read-fonts and the PDF writer share
//! one font-parsing stack (read-fonts 0.39). Line breaking and pagination
//! arrive with T105.

pub mod font;
pub mod line;
pub mod scene;
pub mod shape;

pub use font::{FontError, FontKey, LoadedFont};
pub use line::{
    BreakError, ClusterRange, LayoutLine, LineDiagnostic, LineError, LineLimit, LineLimits,
    LineStats, OverflowKind, ParagraphLines, SourceRange, layout_paragraph,
};
pub use scene::{GlyphRun, PageScene, to_points, to_screen};
pub use shape::{FeatureSet, ShapedGlyph, ShapedRun, shape};
