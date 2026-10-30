//! Page scene (ARCHITECTURE §6.2, ADR-005): the single description of a
//! laid-out page that the screen, PDF and printing consume. Geometry is in
//! EMU from the page's top-left corner; outputs only convert units and never
//! re-shape or re-break text.

use std::sync::Arc;

use joyword_foundation::{Emu, ScreenScale};

use crate::font::FontKey;
use crate::shape::{PaintedUnderline, ShapedGlyph, ShapedRun, UnderlinePaint};

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
    /// sRGB text fill. Auto and unmarked text are black. Review markup may
    /// replace it with an author colour.
    pub color: [u8; 3],
    /// Underline stroke resolved from the font. Empty means no underline.
    pub underline: Option<UnderlinePaint>,
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
            color: run.color,
            underline: run.underline,
        }
    }

    /// Advance of the placed glyphs, in EMU.
    pub fn advance(&self) -> i64 {
        self.glyphs
            .iter()
            .fold(0i64, |sum, glyph| sum.saturating_add(glyph.advance.raw()))
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

/// Rectangles for this run's underline and, when `marks` is set, the
/// non-document formatting overlay.
///
/// Underline geometry uses the font metrics already stored on the run.
/// A double underline is a second stroke of the same thickness, one
/// thickness below the first. A dotted underline is a row of squares whose
/// side and gap are that thickness. Neither gap is a Word measurement; the
/// OpenType tables describe one stroke.
///
/// Formatting marks do not change advances. A space is a square of one tenth
/// of the font size, centered on the space and one fifth of the size above
/// the baseline. A tab is a horizontal bar plus a square head in a gap
/// recorded while the line was fitted. A paragraph end draws a stem and a
/// square just past the run. The overlay is gray `(128, 128, 128)` so it is
/// not document ink. The stroke is axis-aligned; italic does not shear it.
pub fn decoration_rects(
    run: &GlyphRun,
    marks: bool,
    paragraph_end: bool,
    tab_gaps: &[(i64, i64)],
) -> Vec<SceneRect> {
    let mut rects = Vec::new();
    rects.extend(underline_rects(run));
    if marks {
        rects.extend(mark_rects(run, paragraph_end, tab_gaps));
    }
    rects
}

fn underline_rects(run: &GlyphRun) -> Vec<SceneRect> {
    let Some(paint) = run.underline else {
        return Vec::new();
    };
    let width = run.advance();
    if width <= 0 {
        return Vec::new();
    }
    let x = run.origin_x.raw();
    let top = run.baseline_y.raw().saturating_add(paint.offset.raw());
    let thickness = paint.thickness.raw().max(1);
    let color = run.color;
    match paint.kind {
        PaintedUnderline::Single => vec![bar(x, top, width, thickness, color)],
        PaintedUnderline::Double => vec![
            bar(x, top, width, thickness, color),
            bar(
                x,
                top.saturating_add(thickness.saturating_mul(2)),
                width,
                thickness,
                color,
            ),
        ],
        PaintedUnderline::Dotted => {
            let mut dots = Vec::new();
            let mut cursor = 0i64;
            let step = thickness.saturating_mul(2).max(1);
            while cursor < width {
                let dot = thickness.min(width - cursor);
                if dot <= 0 {
                    break;
                }
                dots.push(bar(x.saturating_add(cursor), top, dot, thickness, color));
                cursor = cursor.saturating_add(step);
            }
            dots
        }
    }
}

const MARK_RED: u8 = 128;
const MARK_GREEN: u8 = 128;
const MARK_BLUE: u8 = 128;

fn mark_rects(run: &GlyphRun, paragraph_end: bool, tab_gaps: &[(i64, i64)]) -> Vec<SceneRect> {
    let mut rects = Vec::new();
    let side = (run.size.raw() / 10).max(1);
    let mid = run.baseline_y.raw().saturating_sub(run.size.raw() / 5);
    let mut byte = 0usize;
    for glyph in &run.glyphs {
        let start = glyph.cluster;
        if start < byte || start > run.text.len() {
            continue;
        }
        let end = run.text[start..]
            .chars()
            .next()
            .map(|ch| start + ch.len_utf8())
            .unwrap_or(start);
        let ch = run.text[start..end].chars().next();
        byte = end;
        let x = run.origin_x.raw().saturating_add(glyph.x.raw());
        let advance = glyph.advance.raw().max(0);
        if matches!(ch, Some(' ' | '\u{00A0}')) {
            let cx = x.saturating_add(advance / 2);
            rects.push(bar(
                cx.saturating_sub(side / 2),
                mid.saturating_sub(side / 2),
                side,
                side,
                [MARK_RED, MARK_GREEN, MARK_BLUE],
            ));
        }
        if ch == Some('\t') && advance > 0 {
            rects.extend(tab_arrow(x, mid, advance, side));
        }
    }
    rects.extend(tab_gaps.iter().copied().flat_map(|(start, width)| {
        let x = run.origin_x.raw().saturating_add(start);
        tab_arrow(x, mid, width, side)
    }));
    if paragraph_end {
        let end = run.origin_x.raw().saturating_add(run.advance());
        let x = end.saturating_add(side);
        let stem_h = (run.size.raw() / 2).max(side);
        rects.push(bar(
            x,
            run.baseline_y.raw().saturating_sub(stem_h),
            (side / 2).max(1),
            stem_h,
            [MARK_RED, MARK_GREEN, MARK_BLUE],
        ));
        rects.push(bar(
            x,
            run.baseline_y.raw().saturating_sub(stem_h),
            side.saturating_mul(2),
            side,
            [MARK_RED, MARK_GREEN, MARK_BLUE],
        ));
    }
    for rect in &mut rects {
        rect.screen_only = true;
    }
    rects
}

fn tab_arrow(x: i64, mid: i64, advance: i64, side: i64) -> Vec<SceneRect> {
    if advance <= 0 {
        return Vec::new();
    }
    let color = [MARK_RED, MARK_GREEN, MARK_BLUE];
    let bar_w = (advance.saturating_mul(6) / 10).max(1);
    let y = mid.saturating_sub(side / 2);
    vec![
        bar(x, y, bar_w, (side / 2).max(1), color),
        bar(
            x.saturating_add(bar_w.saturating_sub(side)),
            mid.saturating_sub(side / 2),
            side,
            side,
            color,
        ),
    ]
}

fn bar(x: i64, y: i64, width: i64, height: i64, color: [u8; 3]) -> SceneRect {
    SceneRect {
        screen_only: false,
        x: Emu::new(x),
        y: Emu::new(y),
        width: Emu::new(width.max(0)),
        height: Emu::new(height.max(0)),
        red: color[0],
        green: color[1],
        blue: color[2],
    }
}

/// A filled rectangle in page coordinates. Borders and cell shading are
/// rectangles so screen and PDF paint the same geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneRect {
    /// Non-document editing overlay. Screen consumers paint it; PDF and
    /// print consumers omit it without changing page or text geometry.
    pub screen_only: bool,
    pub x: Emu,
    pub y: Emu,
    pub width: Emu,
    pub height: Emu,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// A still image painted in the same page coordinates as [`GlyphRun`].
/// Bytes are the admitted raster; outputs decode them and do not fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneImage {
    pub media_type: Arc<str>,
    pub bytes: Arc<[u8]>,
    /// Top-left of the image on the page.
    pub x: Emu,
    pub y: Emu,
    pub width: Emu,
    pub height: Emu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageScene {
    pub width: Emu,
    pub height: Emu,
    pub runs: Vec<GlyphRun>,
    pub rects: Vec<SceneRect>,
    pub images: Vec<SceneImage>,
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
