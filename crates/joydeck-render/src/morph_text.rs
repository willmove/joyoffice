//! Morph by words or characters: the words (or characters) two texts share travel from their place
//! in the old text to their place in the new one, growing or shrinking to the new size; the rest
//! of the old text fades out and the rest of the new one fades in, both riding the moving box.

use std::collections::{HashMap, VecDeque};

use joydeck_geom::Xfrm;
use joydeck_model::resolve::Ctx;
use joydeck_model::{Shape, ShapeId};
use joydeck_text::{Fields, GlyphRun, Opts, TextLayout};
use kurbo::{Affine, Point, Vec2};
use vello_cpu::RenderContext;

use crate::{color, draw_glyph, draw_layout, shape_geometry, text_transform};

/// The texts of two frame shapes (see [`crate::render_blend`]) morphed by words or characters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMorph {
    /// The frame shape from the old slide and the one from the new slide.
    pub old: ShapeId,
    pub new: ShapeId,
    /// Their boxes on their own slides: where the shared words start and end.
    pub from: Xfrm,
    pub to: Xfrm,
    /// Characters instead of words.
    pub chars: bool,
    /// Progress, 0..1 (already eased).
    pub t: f64,
}

/// A word or character: its text, its glyphs (run, glyph) and the origin of its first glyph.
struct Token {
    key: String,
    glyphs: Vec<(usize, usize)>,
    origin: Point,
    run: usize,
}

/// Words (or characters) of a layout in reading order, plus glyphs that belong to none (bullets).
fn tokens(l: &TextLayout, chars: bool) -> (Vec<Token>, Vec<(usize, usize)>) {
    let mut cells: Vec<(usize, usize, char, usize, usize)> = Vec::new();
    let mut loose = Vec::new();
    for (ri, run) in l.runs.iter().enumerate() {
        for gi in 0..run.glyphs.len() {
            match run.cells.get(gi) {
                Some((ci, ch)) => cells.push((run.para, *ci, *ch, ri, gi)),
                None => loose.push((ri, gi)),
            }
        }
    }
    cells.sort_by_key(|c| (c.0, c.1, c.3, c.4));
    let mut out: Vec<Token> = Vec::new();
    let mut last: Option<(usize, usize)> = None;
    for (para, ci, ch, ri, gi) in cells {
        if ch.is_whitespace() {
            last = None;
            continue;
        }
        let same_char = last == Some((para, ci));
        let next_char = last.is_some_and(|(p, c)| p == para && c.checked_add(1) == Some(ci));
        let extend = same_char || (!chars && next_char);
        match out.last_mut() {
            Some(tok) if extend => {
                if !same_char {
                    tok.key.push(ch);
                }
                tok.glyphs.push((ri, gi));
            }
            _ => {
                let origin = l
                    .runs
                    .get(ri)
                    .and_then(|r| r.glyphs.get(gi))
                    .map(|g| Point::new(g.1, g.2))
                    .unwrap_or(Point::ZERO);
                out.push(Token {
                    key: ch.to_string(),
                    glyphs: vec![(ri, gi)],
                    origin,
                    run: ri,
                });
            }
        }
        last = Some((para, ci));
    }
    (out, loose)
}

/// For each token of `b`, the token of `a` with the same text, in reading order, each used once.
fn matches(a: &[Token], b: &[Token]) -> Vec<Option<usize>> {
    let mut available = HashMap::<&str, VecDeque<usize>>::new();
    for (i, token) in a.iter().enumerate() {
        available.entry(&token.key).or_default().push_back(i);
    }
    b.iter()
        .map(|token| available.get_mut(token.key.as_str())?.pop_front())
        .collect()
}

fn layout(ctx: &Ctx, s: &Shape, x: Xfrm, fields: &dyn Fields) -> Option<TextLayout> {
    let body = s.text.as_ref().filter(|b| !b.is_empty())?;
    let rect = shape_geometry(s, x.w, x.h).text_rect;
    Some(joydeck_text::layout(
        ctx,
        s,
        body,
        &Opts {
            rect,
            fields,
            prompt_color: None,
            no_shrink: false,
        },
    ))
}

fn glyph_at(l: &TextLayout, (ri, gi): (usize, usize)) -> Option<(&GlyphRun, u32, Point)> {
    let run = l.runs.get(ri)?;
    let g = run.glyphs.get(gi)?;
    Some((run, g.0, Point::new(g.1, g.2)))
}

fn same_look(a: &GlyphRun, b: &GlyphRun) -> bool {
    a.face.id() == b.face.id()
        && a.size == b.size
        && a.color == b.color
        && a.alpha == b.alpha
        && a.outline == b.outline
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

struct SharedGlyphs {
    old: Point,
    new: Point,
    ratio: f64,
}

fn draw_token(
    ctx: &mut RenderContext,
    layout: &TextLayout,
    token: &Token,
    transform: Affine,
    opacity: f64,
) {
    for &glyph in &token.glyphs {
        if let Some((run, id, pos)) = glyph_at(layout, glyph) {
            draw_glyph(
                ctx,
                run,
                id,
                transform * Affine::translate(pos - token.origin),
                opacity,
            );
        }
    }
}

/// Draw the morphing text of `a` (old slide, `ca`) and `b` (new slide, `cb`); `now` is the box both
/// frame shapes share at this moment, `view` the slide-to-pixels transform.
pub(crate) fn draw(
    ctx: &mut RenderContext,
    (ca, a, fa): (&Ctx, &Shape, &dyn Fields),
    (cb, b, fb): (&Ctx, &Shape, &dyn Fields),
    m: &TextMorph,
    now: Xfrm,
    view: Affine,
) {
    let t = if m.t.is_finite() {
        m.t.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let la = layout(ca, a, m.from, fa);
    let lb = layout(cb, b, m.to, fb);
    let mt = view * now.affine();
    // Rotated (vertical) text: no per-word travel, the two texts cross-fade on the moving box.
    let rotated = la.as_ref().is_some_and(|l| l.rotation != 0.0)
        || lb.as_ref().is_some_and(|l| l.rotation != 0.0);
    if rotated || la.is_none() || lb.is_none() {
        for (l, s, alpha, x) in [(la, a, 1.0 - t, m.from), (lb, b, t, m.to)] {
            if let Some(l) = l
                && alpha > 0.001
            {
                ctx.push_layer(None, None, Some(alpha as f32), None, None);
                draw_layout(ctx, &l, shape_geometry(s, x.w, x.h).text_rect, mt);
                ctx.pop_layer();
            }
        }
        return;
    }
    let (Some(la), Some(lb)) = (la, lb) else {
        return;
    };
    let (ta, loose_a) = tokens(&la, m.chars);
    let (tb, loose_b) = tokens(&lb, m.chars);
    let pairs = matches(&ta, &tb);
    // Flipped boxes keep their text readable, as when drawn (see `text_transform`).
    let (tra, trb) = (
        shape_geometry(a, m.from.w, m.from.h).text_rect,
        shape_geometry(b, m.to.w, m.to.h).text_rect,
    );
    let (ma, mb) = (
        text_transform(view * m.from.affine(), tra, 0.0),
        text_transform(view * m.to.affine(), trb, 0.0),
    );
    let (mta, mtb) = (text_transform(mt, tra, 0.0), text_transform(mt, trb, 0.0));
    let c = mta.as_coeffs();
    let linear = Affine::new([c[0], c[1], c[2], c[3], 0.0, 0.0]);
    let mut moved_a = vec![false; ta.len()];
    let mut shared = Vec::new();
    for (j, i) in pairs.iter().enumerate() {
        let (Some(i), Some(tok_b)) = (*i, tb.get(j)) else {
            continue;
        };
        let Some(tok_a) = ta.get(i) else { continue };
        if let Some(f) = moved_a.get_mut(i) {
            *f = true;
        }
        let (Some(ra), Some(rb)) = (la.runs.get(tok_a.run), lb.runs.get(tok_b.run)) else {
            continue;
        };
        let ratio = if ra.size > 0.0 && rb.size > 0.0 {
            rb.size / ra.size
        } else {
            1.0
        };
        let old = ma * tok_a.origin;
        let new = mb * tok_b.origin;
        shared.push(SharedGlyphs { old, new, ratio });
        let at = Affine::translate(old.lerp(new, t).to_vec2()) * linear;
        let same = same_look(ra, rb);
        draw_token(
            ctx,
            &la,
            tok_a,
            at * Affine::scale(lerp(1.0, ratio, t)),
            if same { 1.0 } else { 1.0 - t },
        );
        if !same {
            draw_token(
                ctx,
                &lb,
                tok_b,
                at * Affine::scale(lerp(1.0 / ratio, 1.0, t)),
                t,
            );
        }
    }
    // The rest fades while following the shared tokens: they define a scale and shift from the old
    // text to the new one (F(p) = s·p + c, in pixels), so a line grows, shrinks and rearranges as
    // one. With nothing shared it rides the moving box.
    let n = shared.len().max(1) as f64;
    let s = shared.iter().map(|pair| pair.ratio).sum::<f64>() / n;
    let c = shared.iter().fold(Vec2::ZERO, |acc, pair| {
        acc + (pair.new.to_vec2() - pair.old.to_vec2() * s)
    }) / n;
    let (place_a, place_b) = if !shared.is_empty() && s.is_finite() && s > 0.0 && c.is_finite() {
        // Old text: p ↦ lerp(p, F(p), t). New text: q ↦ lerp(F⁻¹(q), q, t).
        let a = Affine::translate(c * t) * Affine::scale(lerp(1.0, s, t));
        let b = Affine::translate(-c * ((1.0 - t) / s)) * Affine::scale((1.0 - t) / s + t);
        (a * ma, b * mb)
    } else {
        (mta, mtb)
    };
    let unmatched_a = ta
        .iter()
        .enumerate()
        .filter(|(i, _)| !moved_a.get(*i).copied().unwrap_or(false))
        .flat_map(|(_, tok)| tok.glyphs.iter().copied());
    for g in unmatched_a.chain(loose_a) {
        if let Some((run, gid, pos)) = glyph_at(&la, g) {
            draw_glyph(
                ctx,
                run,
                gid,
                place_a * Affine::translate(pos.to_vec2()),
                1.0 - t,
            );
        }
    }
    let unmatched_b = tb
        .iter()
        .enumerate()
        .filter(|(j, _)| pairs.get(*j).is_none_or(Option::is_none))
        .flat_map(|(_, tok)| tok.glyphs.iter().copied());
    for g in unmatched_b.chain(loose_b) {
        if let Some((run, gid, pos)) = glyph_at(&lb, g) {
            draw_glyph(ctx, run, gid, place_b * Affine::translate(pos.to_vec2()), t);
        }
    }
    // Underlines, strikethroughs and highlights fade with their text.
    for (l, alpha, place) in [(&la, 1.0 - t, place_a), (&lb, t, place_b)] {
        for d in &l.decos {
            ctx.set_transform(place);
            ctx.set_paint(color(d.color, alpha));
            crate::fill_deco(ctx, d);
        }
    }
}

#[cfg(test)]
pub(crate) fn token_keys(l: &TextLayout, chars: bool) -> Vec<String> {
    tokens(l, chars).0.into_iter().map(|t| t.key).collect()
}
