//! Page scene (ARCHITECTURE §6.2, ADR-005): the single description of a
//! laid-out page that the screen, PDF and printing consume. Geometry is in
//! EMU from the page's top-left corner; outputs only convert units and never
//! re-shape or re-break text.

use std::sync::Arc;

use joyword_foundation::{Emu, ScreenScale};

use crate::font::FontKey;
use crate::shape::{ShapedGlyph, ShapedRun};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphRun {
    pub font: FontKey,
    pub size: Emu,
    /// Baseline start of the run on the page.
    pub origin_x: Emu,
    pub baseline_y: Emu,
    /// Source text; glyph clusters are byte offsets into it.
    pub text: String,
    pub glyphs: Vec<ShapedGlyph>,
    /// Resolved weight, posture and requested family names. Outputs paint
    /// weight and posture; the shaping face stays `font`.
    pub bold: bool,
    pub italic: bool,
    pub latin_family: Option<Arc<str>>,
    pub east_asia_family: Option<Arc<str>>,
}

impl GlyphRun {
    pub fn place(run: &ShapedRun, origin_x: Emu, baseline_y: Emu) -> Self {
        Self {
            font: run.font,
            size: run.size,
            origin_x,
            baseline_y,
            text: run.text.clone(),
            glyphs: run.glyphs.clone(),
            bold: run.bold,
            italic: run.italic,
            latin_family: run.latin_family.clone(),
            east_asia_family: run.east_asia_family.clone(),
        }
    }

    /// Page position of glyph `index`'s origin, before glyph offsets.
    pub fn glyph_x(&self, index: usize) -> Emu {
        Emu::new(self.origin_x.raw() + self.glyphs[index].x.raw())
    }

    /// Byte range of the cluster containing glyph `index`: from its cluster
    /// start to the next greater cluster start (or the end of the text).
    pub fn cluster_range(&self, index: usize) -> std::ops::Range<usize> {
        let start = self.glyphs[index].cluster;
        let end = self
            .glyphs
            .iter()
            .map(|glyph| glyph.cluster)
            .filter(|&cluster| cluster > start)
            .min()
            .unwrap_or(self.text.len());
        start..end
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageScene {
    pub width: Emu,
    pub height: Emu,
    pub runs: Vec<GlyphRun>,
}

/// Points as PDF and print coordinates use them.
pub fn to_points(length: Emu) -> f64 {
    length.points()
}

/// Logical screen pixels at a zoom level.
pub fn to_screen(length: Emu, scale: ScreenScale) -> f64 {
    scale.logical_pixels(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_and_pdf_agree_within_a_hundredth_of_a_point() {
        // The same scene coordinate through both outputs and back.
        for zoom in [0.5, 1.0, 1.37, 2.0, 4.0] {
            let scale = ScreenScale::new(zoom).unwrap();
            for raw in [0_i64, 1, 635, 12_700, 914_400, 7_560_000, 10_692_000] {
                let length = Emu::new(raw);
                let from_screen = to_screen(length, scale) / zoom * 72.0 / 96.0;
                assert!(
                    (from_screen - to_points(length)).abs() <= 0.01,
                    "{zoom} {raw}"
                );
            }
        }
    }
}
