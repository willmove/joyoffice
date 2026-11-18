//! Preset shape geometry.
//!
//! Each preset is identified by its Office Open XML name (`rect`, `roundRect`, `rightArrow`…) so
//! files round-trip, and the Rust generators construct their outlines, text frames and glue sites
//! from the shape dimensions and preset-specific geometry. Adjust values use the file convention: ratios in 1/100 000 of the shorter side (or
//! of the width/height), angles in 1/60 000 degree.

use kurbo::{BezPath, Point, Rect, Vec2};
use serde::{Deserialize, Serialize};

mod action_buttons;
mod build;
mod builder;
mod catalog;
mod flowchart;
mod line_callout;
mod office_shapes;
mod shapes;

pub use build::build;
#[cfg(test)]
use builder::Pb;
use builder::{fill, fill_only, stroke_only};
pub use catalog::{
    CATALOG, Category, FillMode, Geometry, Handle, PresetInfo, SubPath, adjustment_defaults, info,
    is_line_like, mitred_by_default,
};
pub use line_callout::LineCallout;

#[cfg(test)]
mod extra;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test fixtures use explicit failure assertions"
)]
mod tests;
