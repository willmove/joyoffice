//! Review projection and markup paint (T300).
//!
//! Final and Original drop hidden runs before shaping, so line breaks follow
//! the visible text. Final with markup shapes every run and paints insertions
//! with an underline and author colour, deletions with a strikethrough. The
//! colour is an 8-entry palette of our own, hashed from the author. The
//! underline and strike are a preview measurement, not a Word match.

use std::ops::Range;

use joyword_foundation::Emu;
use joyword_model::{Paragraph, RevisionKind, RevisionView};

use crate::line::{LayoutLine, LineRevision};
use crate::scene::{GlyphRun, SceneRect};
use crate::{LayoutError, ShapedGlyph};

pub(crate) struct Projection {
    pub text: String,
    pub map: Vec<usize>,
}

/// `Ok(None)` keeps the paragraph text. `Err` refuses a projection that
/// would have to guess around an image, a page break or list layout.
pub(crate) fn project(
    paragraph: &Paragraph,
    view: RevisionView,
    list_layout: bool,
) -> Result<Option<Projection>, LayoutError> {
    let hidden = paragraph.runs().iter().any(|run| {
        run.revision
            .as_ref()
            .is_some_and(|revision| !revision.kind.visible_in(view))
    });
    if !hidden {
        return Ok(None);
    }
    if list_layout || !paragraph.inlines().is_empty() {
        return Err(crate::document::unsupported(
            Some(paragraph.id()),
            "revision projection with an image, break or list",
        ));
    }
    let mut text = String::new();
    let mut map = Vec::new();
    let mut offset = 0usize;
    let plain = paragraph.plain_text(|_| String::new());
    let mut chars = plain.chars();
    for run in paragraph.runs() {
        let show = run
            .revision
            .as_ref()
            .map(|revision| revision.kind.visible_in(view))
            .unwrap_or(true);
        for _ in 0..run.len {
            let Some(ch) = chars.next() else {
                break;
            };
            if show {
                text.push(ch);
                map.push(offset);
            }
            offset += 1;
        }
    }
    Ok(Some(Projection { text, map }))
}

pub(crate) fn remap(lines: &mut [LayoutLine], map: &[usize], model_len: usize) {
    for line in &mut *lines {
        remap_range(&mut line.source.scalars, map);
        for cluster in &mut line.clusters {
            remap_range(&mut cluster.source.scalars, map);
        }
    }
    if let Some(first) = lines.first_mut() {
        first.source.scalars.start = 0;
    }
    if let Some(last) = lines.last_mut()
        && last.source.scalars.end < model_len
    {
        last.source.scalars.end = model_len;
    }
}

fn remap_range(range: &mut Range<usize>, map: &[usize]) {
    let start = mapped(map, range.start);
    let end = mapped(map, range.end);
    *range = start..end.max(start);
}

fn mapped(map: &[usize], visible: usize) -> usize {
    if visible < map.len() {
        map[visible]
    } else {
        map.last().map(|scalar| scalar + 1).unwrap_or(0)
    }
}

pub(crate) fn annotate(lines: &mut [LayoutLine], paragraph: &Paragraph, view: RevisionView) {
    if view != RevisionView::FinalWithMarkup {
        return;
    }
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for run in paragraph.runs() {
        let end = offset + run.len;
        if let Some(revision) = &run.revision {
            spans.push((offset..end, revision.kind, author_color(&revision.author)));
        }
        offset = end;
    }
    if spans.is_empty() {
        return;
    }
    for line in lines {
        for (range, kind, color) in &spans {
            let start = range.start.max(line.source.scalars.start);
            let end = range.end.min(line.source.scalars.end);
            if start < end {
                line.revisions.push(LineRevision {
                    scalars: start..end,
                    kind: *kind,
                    red: color.0,
                    green: color.1,
                    blue: color.2,
                });
            }
        }
    }
}

pub(crate) fn author_color(author: &str) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 8] = [
        (0xC0, 0x39, 0x2B),
        (0x29, 0x80, 0xB9),
        (0x27, 0xAE, 0x60),
        (0x8E, 0x44, 0xAD),
        (0xD3, 0x54, 0x00),
        (0x16, 0xA0, 0x85),
        (0x2C, 0x3E, 0x50),
        (0xB7, 0x95, 0x0B),
    ];
    let mut hash = 0x811c_9dc5u32;
    for byte in author.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    PALETTE[(hash as usize) % PALETTE.len()]
}

/// A vertical bar in the page's left margin beside a changed line.
///
/// The bar uses the line's first author colour. It sits in the margin, short
/// of the content edge, so screen and PDF paint the same indicator. It is a
/// JoyWord mark, not a comment balloon.
pub(crate) fn margin_bar(
    geometry: crate::document::PageGeometry,
    line: &LayoutLine,
    top: Emu,
    height: Emu,
) -> Option<SceneRect> {
    let mark = line.revisions.first()?;
    if height.raw() <= 0 {
        return None;
    }
    let margin = geometry.left.raw();
    let gap = 4 * Emu::PER_TWIP;
    let width = Emu::PER_POINT.min(margin / 3).max(2 * Emu::PER_TWIP);
    if margin < width.saturating_add(gap) {
        return None;
    }
    Some(SceneRect {
        x: Emu::new(margin - gap - width),
        y: top,
        width: Emu::new(width),
        height,
        red: mark.red,
        green: mark.green,
        blue: mark.blue,
    })
}

/// Splits one fitted line into glyph runs. Insertions and move destinations
/// are coloured and underlined. Deletions and move sources are struck through.
pub(crate) fn marked_runs(
    line: &LayoutLine,
    origin_x: Emu,
    baseline_y: Emu,
) -> (Vec<GlyphRun>, Vec<SceneRect>) {
    if line.revisions.is_empty() || line.run.glyphs.is_empty() {
        return (vec![line.to_glyph_run(origin_x, baseline_y)], Vec::new());
    }
    let mut groups: Vec<(Range<usize>, Option<usize>)> = Vec::new();
    for (index, cluster) in line.clusters.iter().enumerate() {
        let mark = line.revisions.iter().position(|revision| {
            cluster.source.scalars.start < revision.scalars.end
                && revision.scalars.start < cluster.source.scalars.end
        });
        if let Some((range, current)) = groups.last_mut()
            && *current == mark
        {
            range.end = index + 1;
        } else {
            groups.push((index..index + 1, mark));
        }
    }
    if groups.is_empty() {
        return (vec![line.to_glyph_run(origin_x, baseline_y)], Vec::new());
    }
    let mut runs = Vec::new();
    let mut rects = Vec::new();
    for (clusters, mark) in groups {
        let glyphs =
            line.clusters[clusters.start].glyphs.start..line.clusters[clusters.end - 1].glyphs.end;
        if glyphs.start >= glyphs.end || glyphs.end > line.run.glyphs.len() {
            continue;
        }
        let piece = &line.run.glyphs[glyphs.clone()];
        let byte_start = piece[0].cluster;
        let byte_end = line
            .run
            .glyphs
            .get(glyphs.end)
            .map(|glyph| glyph.cluster)
            .unwrap_or(line.run.text.len())
            .max(byte_start)
            .min(line.run.text.len());
        if !line.run.text.is_char_boundary(byte_start) || !line.run.text.is_char_boundary(byte_end)
        {
            continue;
        }
        let first_x = piece[0].x.raw();
        let mut glyphs_out: Vec<ShapedGlyph> = piece.to_vec();
        for glyph in &mut glyphs_out {
            glyph.cluster = glyph.cluster.saturating_sub(byte_start);
            glyph.x = Emu::new(glyph.x.raw().saturating_sub(first_x));
        }
        let mark = mark.and_then(|index| line.revisions.get(index));
        let color = mark
            .map(|mark| [mark.red, mark.green, mark.blue])
            .unwrap_or([0, 0, 0]);
        let mut run = GlyphRun::place(&line.run, Emu::new(origin_x.raw() + first_x), baseline_y);
        run.text = line.run.text[byte_start..byte_end].to_string();
        run.glyphs = glyphs_out;
        run.color = color;
        let width = piece
            .last()
            .map(|glyph| glyph.x.raw() + glyph.advance.raw() - first_x)
            .unwrap_or(0);
        if let Some(mark) = mark
            && width > 0
        {
            let thickness = (line.run.size.raw() / 16).max(Emu::PER_TWIP);
            let (y, draw) = match mark.kind {
                RevisionKind::Insert | RevisionKind::MoveTo => {
                    (baseline_y.raw() + line.run.size.raw() / 10, true)
                }
                RevisionKind::Delete | RevisionKind::MoveFrom => {
                    (baseline_y.raw() - line.run.size.raw() / 3, true)
                }
            };
            if draw {
                rects.push(SceneRect {
                    x: Emu::new(origin_x.raw() + first_x),
                    y: Emu::new(y),
                    width: Emu::new(width),
                    height: Emu::new(thickness),
                    red: mark.red,
                    green: mark.green,
                    blue: mark.blue,
                });
            }
        }
        runs.push(run);
    }
    if runs.is_empty() {
        (vec![line.to_glyph_run(origin_x, baseline_y)], Vec::new())
    } else {
        (runs, rects)
    }
}
