//! Metafile entry points and the text adapter used by scene playback.

use std::sync::Arc;

use joydeck_fonts::{FontDb, FontFace};
use joydeck_metafile::{HAlign, Text, VAlign};
use kurbo::{Affine, BezPath, Rect, Shape as _};
use skrifa::MetadataProvider as _;
use vello_cpu::{RenderContext, peniko};

pub const DEFAULT_SIDE: u32 = 1024;
const DEFAULT_EM: f64 = 12.0;

pub fn is_metafile(bytes: &[u8]) -> bool {
    joydeck_metafile::is_metafile(bytes)
}

pub fn rasterize(bytes: &[u8], side: u32) -> Option<image::RgbaImage> {
    let picture = joydeck_metafile::parse(bytes).ok()?;
    super::metafile_scene::rasterize(&picture, side)
}

/// One glyph placed in text space (x along the baseline from the string's start, font units at
/// `scale` text units each).
struct Glyph {
    face: Arc<FontFace>,
    gid: u32,
    x: f64,
    y: f64,
}

/// Draws a text item; `m` maps its text space to the output.
pub(super) fn draw_text(ctx: &mut RenderContext, t: &Text, m: Affine) {
    let db = FontDb::global();
    let font = &t.font;
    let family = if font.family.trim().is_empty() {
        "Arial"
    } else {
        font.family.trim()
    };
    let (bold, italic) = (font.weight >= 600, font.italic);
    let style = match (bold, italic) {
        (true, true) => "Bold Italic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (false, false) => "Regular",
    };
    if let Some(indexed) = &t.indexed {
        crate::metafile_glyphs::draw(ctx, t, m, family, style, indexed);
        return;
    }
    let face = db.document_face(family, style);
    let upem = face.upem.max(1.0);
    let (asc, desc) = win_metrics(&face);
    // LOGFONT height: negative is the em, positive the cell (ascent plus descent).
    let h = f64::from(font.height);
    let em = if h < 0.0 {
        -h
    } else if h > 0.0 {
        h * upem / (asc + desc).max(1.0)
    } else {
        DEFAULT_EM
    };
    let k = em / upem;
    if !k.is_finite() || k <= 0.0 {
        return;
    }
    // Characters as drawn: symbol fonts (Wingdings…) map like bullets; each character takes the
    // face that has it.
    let chars: Vec<char> = t
        .text
        .chars()
        .map(|c| joydeck_text::symbol_char(&face, family, c).unwrap_or(c))
        .collect();
    let faces: Vec<Arc<FontFace>> = chars
        .iter()
        .map(|c| {
            if c.is_whitespace() || c.is_control() || face.covers(*c) {
                face.clone()
            } else {
                db.cascade(&[*c], std::slice::from_ref(&face), style)
            }
        })
        .collect();
    // Shape runs of one face; record each glyph's pen position and its character.
    let mut shaped: Vec<(Glyph, usize)> = Vec::with_capacity(chars.len());
    let mut char_pen: Vec<f64> = vec![0.0; chars.len() + 1];
    let mut pen = 0.0;
    let mut i = 0;
    while i < chars.len() {
        let Some(f) = faces.get(i).cloned() else {
            break;
        };
        let mut j = i + 1;
        while faces.get(j).is_some_and(|g| g.id() == f.id()) {
            j += 1;
        }
        let run: String = chars.get(i..j).unwrap_or(&[]).iter().collect();
        // Byte offset in the run → character index in the string.
        let byte_char: Vec<usize> = run.char_indices().map(|(b, _)| b).collect();
        let ku = f.upem.max(1.0);
        for g in joydeck_fonts::shape(&f, &run, &[], |c| c) {
            let ci = i + byte_char.partition_point(|b| *b < g.cluster).min(j - i - 1);
            let s = em / ku;
            shaped.push((
                Glyph {
                    face: f.clone(),
                    gid: g.gid,
                    x: pen + f64::from(g.x_offset) * s,
                    y: -f64::from(g.y_offset) * s,
                },
                ci,
            ));
            pen += f64::from(g.x_advance) * s;
            if let Some(p) = char_pen.get_mut(ci + 1) {
                *p = pen;
            }
        }
        i = j;
    }
    // Character start pens: a character's start is the previous one's end.
    for ci in 1..char_pen.len() {
        let prev = char_pen.get(ci - 1).copied().unwrap_or(0.0);
        if let Some(p) = char_pen.get_mut(ci)
            && *p < prev
        {
            *p = prev;
        }
    }
    // The file's advances place each character; glyphs keep their offset within their character.
    let use_dx = !t.dx.is_empty() && t.dx.len() == chars.len();
    let mut width = pen;
    let mut x_scale = 1.0;
    if use_dx {
        let mut starts = Vec::with_capacity(chars.len() + 1);
        let mut x = 0.0;
        for d in &t.dx {
            starts.push(x);
            x += f64::from(*d);
        }
        width = x;
        for (g, ci) in &mut shaped {
            let (start, pen_start) = (
                starts.get(*ci).copied().unwrap_or(0.0),
                char_pen.get(*ci).copied().unwrap_or(0.0),
            );
            g.x = start + (g.x - pen_start);
        }
    } else if font.width > 0.0 {
        // A LOGFONT width narrows or widens the font against its average character width.
        let avg = avg_char_width(&face) * k;
        if avg > 0.0 {
            x_scale = (f64::from(font.width) / avg).clamp(0.05, 20.0);
            width *= x_scale;
            for (g, _) in &mut shaped {
                g.x *= x_scale;
            }
        }
    }
    let x0 = match t.h_align {
        HAlign::Left => 0.0,
        HAlign::Center => -width / 2.0,
        HAlign::Right => -width,
    };
    let (a, d) = (asc * k, desc * k);
    let base = match t.v_align {
        VAlign::Baseline => 0.0,
        VAlign::Top => a,
        VAlign::Bottom => -d,
    };
    let at = m * Affine::translate((x0, base));
    ctx.set_fill_rule(peniko::Fill::NonZero);
    if let Some(bg) = t.background
        && width > 0.0
    {
        ctx.set_paint(rgba(bg));
        ctx.fill_path(&(at * Rect::new(0.0, -a, width, d).to_path(0.1)));
    }
    let color = rgba(t.color);
    let copy = t.composition == joydeck_metafile::ImageComposition::SourceCopy;
    ctx.set_paint(color);
    let paint = |ctx: &mut RenderContext, path: &BezPath| {
        if copy {
            super::metafile_scene::source_copy(ctx, |ctx| {
                ctx.set_paint(color);
                ctx.fill_path(path);
            });
        } else {
            ctx.fill_path(path);
        }
    };
    for (g, _) in &shaped {
        let path = db.outline(&g.face, g.gid);
        if path.elements().is_empty() {
            continue;
        }
        let s = em / g.face.upem.max(1.0);
        let skew = if italic && !g.face.italic {
            Affine::new([1.0, 0.0, -0.2, 1.0, 0.0, 0.0])
        } else {
            Affine::IDENTITY
        };
        let gm = at
            * Affine::translate((g.x, g.y))
            * Affine::scale_non_uniform(x_scale, 1.0)
            * skew
            * Affine::scale(s);
        let sc = scale_of(gm);
        if !sc.is_finite() || sc > 1e5 {
            continue;
        }
        let p: BezPath = gm * path.as_ref().clone();
        if p.elements().len() > 10_000
            || p.bounding_box().width() > 1e6
            || p.bounding_box().height() > 1e6
        {
            continue;
        }
        paint(ctx, &p);
        if bold && g.face.weight < 600.0 {
            let stroke =
                kurbo::Stroke::new(em * 0.025 * scale_of(at)).with_join(kurbo::Join::Round);
            if copy {
                let outline = kurbo::stroke(p.iter(), &stroke, &kurbo::StrokeOpts::default(), 0.25);
                paint(ctx, &outline);
            } else {
                ctx.set_stroke(stroke);
                ctx.stroke_path(&p);
            }
        }
    }
    // Underline and strike-out: the font's own positions and thickness.
    let metrics = face
        .skrifa()
        .map(|f| f.metrics(skrifa::instance::Size::unscaled(), face.location()));
    let deco =
        |on: bool, d: Option<skrifa::metrics::Decoration>, fallback: (f64, f64)| -> Option<Rect> {
            if !on || width <= 0.0 {
                return None;
            }
            let (off, th) = d.map_or(fallback, |d| {
                (f64::from(d.offset) * k, f64::from(d.thickness) * k)
            });
            let th = if th > 0.0 { th } else { em / 20.0 };
            // Offsets are y-up from the baseline; text space is y-down.
            Some(Rect::new(0.0, -off - th / 2.0, width, -off + th / 2.0))
        };
    let lines = [
        deco(
            font.underline,
            metrics.as_ref().and_then(|m| m.underline),
            (-em * 0.1, em / 20.0),
        ),
        deco(
            font.strike,
            metrics.as_ref().and_then(|m| m.strikeout),
            (em * 0.26, em / 20.0),
        ),
    ];
    for r in lines.into_iter().flatten() {
        paint(ctx, &(at * r.to_path(0.1)));
    }
}

/// Mean scale of an affine transform's linear part.
fn scale_of(m: Affine) -> f64 {
    m.determinant().abs().sqrt()
}

/// The face's Windows ascent and descent (OS/2 `usWinAscent`/`usWinDescent`, as GDI measures the
/// cell), in font units; the typographic ones when the font has no OS/2 table.
fn win_metrics(face: &FontFace) -> (f64, f64) {
    use skrifa::raw::TableProvider as _;
    let os2 = face
        .skrifa()
        .and_then(|f| f.os2().ok())
        .map(|o| (f64::from(o.us_win_ascent()), f64::from(o.us_win_descent())));
    match os2 {
        Some((a, d)) if a + d > 0.0 => (a, d),
        _ => (face.ascent.max(0.0), face.descent.max(0.0)),
    }
}

/// OS/2 `xAvgCharWidth` in font units (half an em without it).
fn avg_char_width(face: &FontFace) -> f64 {
    use skrifa::raw::TableProvider as _;
    face.skrifa()
        .and_then(|f| f.os2().ok())
        .map(|o| f64::from(o.x_avg_char_width()))
        .filter(|w| *w > 0.0)
        .unwrap_or(face.upem * 0.5)
}

pub(super) fn rgba(c: [u8; 4]) -> peniko::Color {
    peniko::Color::from_rgba8(c[0], c[1], c[2], c[3])
}
