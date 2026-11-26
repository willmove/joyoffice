#![allow(
    clippy::type_complexity,
    reason = "Retain upstream geometry/format data structures"
)]
//! Morph: matching shapes between two slides, interpolating their boxes and building the frames.

use std::collections::{HashMap, VecDeque};

use joydeck_model::resolve::{self, Ctx};
use joydeck_model::{
    ColorRef, Fill, Geom, Presentation, Rgba, Shape, ShapeId, ShapeKind, Slide, Xfrm, walk,
};

use crate::clampf;
use crate::easing::smooth;

fn text_of(s: &Shape) -> String {
    s.text.as_ref().map(|t| t.text()).unwrap_or_default()
}

/// Pairs of (shape on `a`, shape on `b`) that Morph animates from one to the other: first by
/// `!!name` (forced match), then by exact name, then by the same kind and text, then by the same
/// kind and preset geometry if neither carries text. A text box with neither a name nor its text on
/// the other slide stays unpaired and fades in place, as in PowerPoint (#119). Each shape is used
/// once; top-level shapes only (groups move whole).
pub fn morph_pairs(a: &Slide, b: &Slide) -> Vec<(ShapeId, ShapeId)> {
    match_shapes(&a.shapes, &b.shapes)
        .into_iter()
        .map(|(i, j)| (a.shapes[i].id, b.shapes[j].id))
        .collect()
}

#[derive(Clone, Copy)]
enum Rule {
    ExplicitName,
    Name,
    Text,
    Outline,
}

#[derive(PartialEq, Eq, Hash)]
enum Key<'a> {
    Name(&'a str),
    Text(&'static str, &'a str),
    Outline(&'static str, &'a str),
}

impl Rule {
    fn key<'a>(self, shape: &'a Shape, text: &'a str) -> Option<Key<'a>> {
        match self {
            Self::ExplicitName if shape.name.starts_with("!!") => Some(Key::Name(&shape.name)),
            Self::Name if !shape.name.is_empty() => Some(Key::Name(&shape.name)),
            Self::Text if !text.trim().is_empty() => Some(Key::Text(shape.kind_name(), text)),
            Self::Outline if text.trim().is_empty() => {
                Some(Key::Outline(shape.kind_name(), shape.geom.preset_name()?))
            }
            _ => None,
        }
    }
}

fn match_shapes(old: &[Shape], new: &[Shape]) -> Vec<(usize, usize)> {
    let texts_old: Vec<_> = old.iter().map(text_of).collect();
    let texts_new: Vec<_> = new.iter().map(text_of).collect();
    let mut used_old = vec![false; old.len()];
    let mut used_new = vec![false; new.len()];
    let mut matches = Vec::new();
    for rule in [Rule::ExplicitName, Rule::Name, Rule::Text, Rule::Outline] {
        let mut candidates = HashMap::<_, VecDeque<usize>>::new();
        for (j, shape) in new.iter().enumerate() {
            if !used_new[j]
                && let Some(key) = rule.key(shape, &texts_new[j])
            {
                candidates.entry(key).or_default().push_back(j);
            }
        }
        for (i, shape) in old.iter().enumerate() {
            if used_old[i] {
                continue;
            }
            let destination = rule
                .key(shape, &texts_old[i])
                .and_then(|key| candidates.get_mut(&key))
                .and_then(VecDeque::pop_front);
            if let Some(j) = destination {
                used_old[i] = true;
                used_new[j] = true;
                matches.push((i, j));
            }
        }
    }
    matches
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    let v = a + (b - a) * t;
    if v.is_finite() {
        v
    } else if t < 0.5 {
        a
    } else {
        b
    }
}

/// Interpolate a box from `a` to `b` at `t` (0..1): position and size linearly, rotation along the
/// shortest way round; flips switch at the midpoint.
pub fn morph_xfrm(a: Xfrm, b: Xfrm, t: f64) -> Xfrm {
    let t = clampf(t, 0.0, 1.0, 0.0);
    if t >= 1.0 {
        return b;
    }
    if t <= 0.0 {
        return a;
    }
    let ra = if a.rot.is_finite() { a.rot } else { 0.0 };
    let rb = if b.rot.is_finite() { b.rot } else { 0.0 };
    let mut d = (rb - ra).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    let rot = (ra + d * t).rem_euclid(360.0);
    let late = t >= 0.5;
    Xfrm {
        x: lerp(a.x, b.x, t),
        y: lerp(a.y, b.y, t),
        w: lerp(a.w, b.w, t),
        h: lerp(a.h, b.h, t),
        rot: if rot.is_finite() { rot } else { 0.0 },
        flip_h: if late { b.flip_h } else { a.flip_h },
        flip_v: if late { b.flip_v } else { a.flip_v },
    }
}

/// One frame of a Morph transition, ready for the renderer's `render_blend`: the backdrops of the
/// two slides cross-fade by [`MorphFrame::mix`], the shapes are drawn in order on top.
#[derive(Clone, Debug, Default)]
pub struct MorphFrame {
    /// Shapes with fresh ids, each flagged `true` when it comes from the old slide (it resolves
    /// its placeholder, theme and style against that slide).
    pub shapes: Vec<(Shape, bool)>,
    /// Opacity of the frame's shapes by id; shapes not listed are opaque.
    pub opacity: Vec<(ShapeId, f64)>,
    pub mix: f64,
    /// With the Words or Characters option: pairs of frame shapes whose texts morph by words or
    /// characters instead of cross-fading (the renderer's `TextMorph`).
    pub text: Vec<MorphText>,
    /// Frame shapes whose outline turns from another geometry into their own (the renderer's
    /// `PathMorph`).
    pub paths: Vec<MorphPath>,
}

/// A frame shape whose outline morphs from `from` (the old shape's geometry) into its own.
#[derive(Clone, Debug, PartialEq)]
pub struct MorphPath {
    pub id: ShapeId,
    pub from: Geom,
    /// The boxes of the old and the new shape on their own slides.
    pub from_box: Xfrm,
    pub to_box: Xfrm,
}

/// Two frame shapes whose texts morph by words or characters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphText {
    /// Frame shape ids: from the old slide, from the new slide.
    pub old: ShapeId,
    pub new: ShapeId,
    /// The boxes of the two shapes on their own slides.
    pub from: Xfrm,
    pub to: Xfrm,
    pub chars: bool,
}

impl MorphFrame {
    /// Opacity of shape `id` of the frame.
    pub fn opacity_of(&self, id: ShapeId) -> f64 {
        self.opacity
            .iter()
            .find(|(s, _)| *s == id)
            .map(|(_, o)| *o)
            .unwrap_or(1.0)
    }
}

fn mix_color(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let m = |x: u8, y: u8| clampf(lerp(x as f64, y as f64, t), 0.0, 255.0, 0.0).round() as u8;
    Rgba {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
        a: m(a.a, b.a),
    }
}

fn solid_color(ctx: Option<&Ctx>, s: &Shape) -> Option<Rgba> {
    let ctx = ctx?;
    match resolve::fill(ctx, s) {
        (Some(Fill::Solid { color }), ph) => Some(ctx.color(&color, ph)),
        _ => None,
    }
}

fn box_of(ctx: Option<&Ctx>, s: &Shape) -> Option<Xfrm> {
    match ctx {
        Some(c) => Some(resolve::xfrm(c, s)),
        None => s.xfrm,
    }
}

/// The content of a shape apart from what Morph interpolates (identity, box and fill).
fn content(s: &Shape) -> Shape {
    Shape {
        id: ShapeId(0),
        name: String::new(),
        xfrm: None,
        fill: None,
        ..s.clone()
    }
}

/// Whether two shapes differ in their geometry only, and it is drawn as an outline (shapes,
/// pictures, connectors), so the outline can morph.
fn reshaped(a: &Shape, b: &Shape) -> bool {
    let outlined = |s: &Shape| {
        matches!(
            s.kind,
            ShapeKind::Shape | ShapeKind::Picture { .. } | ShapeKind::Connector { .. }
        )
    };
    a.geom != b.geom
        && outlined(a)
        && outlined(b)
        && Shape {
            geom: Geom::default(),
            ..content(a)
        } == Shape {
            geom: Geom::default(),
            ..content(b)
        }
}

/// The content of a shape apart from what Morph interpolates, with a picture's crop left out.
fn uncropped(s: &Shape) -> Shape {
    let mut c = content(s);
    if let ShapeKind::Picture { fill } = &mut c.kind {
        fill.crop = [0.0; 4];
    }
    c
}

fn crop_of(s: &Shape) -> [f64; 4] {
    match &s.kind {
        ShapeKind::Picture { fill } => fill.crop,
        _ => [0.0; 4],
    }
}

/// Whether two pictures differ in their crop only (apart from box and fill), so the crop can morph.
fn recropped(a: &Shape, b: &Shape) -> bool {
    matches!(a.kind, ShapeKind::Picture { .. })
        && crop_of(a) != crop_of(b)
        && uncropped(a) == uncropped(b)
}

/// The crop (left, top, right, bottom) of a picture morphing from box `xa` cropped by `ca` to box
/// `xb` cropped by `cb`, when its box is `x` at progress `t`: the whole image moves and scales
/// linearly between where it lies on either slide, and the crop is what of it `x` shows. Falls back
/// to blending the crops when a crop leaves nothing visible.
pub fn morph_crop(ca: [f64; 4], xa: Xfrm, cb: [f64; 4], xb: Xfrm, x: Xfrm, t: f64) -> [f64; 4] {
    // Along one axis: (box start, box size, crop before, crop after) → the crops at `t`.
    let axis = |(pa, sa, la, ha): (f64, f64, f64, f64),
                (pb, sb, lb, hb): (f64, f64, f64, f64),
                (p, s): (f64, f64)| {
        let (va, vb) = (1.0 - la - ha, 1.0 - lb - hb);
        if va <= 1e-6 || vb <= 1e-6 {
            return (lerp(la, lb, t), lerp(ha, hb, t));
        }
        // The whole image: its extent and start on either slide.
        let (ea, eb) = (sa / va, sb / vb);
        let e = lerp(ea, eb, t);
        if !e.is_finite() || e.abs() <= 1e-9 {
            return (lerp(la, lb, t), lerp(ha, hb, t));
        }
        let o = lerp(pa - la * ea, pb - lb * eb, t);
        let lo = (p - o) / e;
        (lo, 1.0 - lo - s / e)
    };
    let (l, r) = axis(
        (xa.x, xa.w, ca[0], ca[2]),
        (xb.x, xb.w, cb[0], cb[2]),
        (x.x, x.w),
    );
    let (tp, bt) = axis(
        (xa.y, xa.h, ca[1], ca[3]),
        (xb.y, xb.h, cb[1], cb[3]),
        (x.y, x.h),
    );
    let c = [l, tp, r, bt];
    if c.iter().all(|v| v.is_finite()) {
        c
    } else {
        cb
    }
}

/// Morph frame from slide `a` to slide `b` at progress `t` (0..1, eased here). Paired shapes
/// ([`morph_pairs`]) move along [`morph_xfrm`]; when they differ only in solid fill colour the
/// colour blends, when pictures differ only in crop the crop morphs ([`morph_crop`]), when only
/// in geometry (and fill colour) the outline morphs ([`MorphFrame::paths`]),
/// otherwise the new shape fades in under the fading old one on the same moving
/// box; with the transition option `words` or `characters` their texts are listed in
/// [`MorphFrame::text`] to morph word by word or character by character. Unpaired shapes of `a` fade out, unpaired shapes of `b` fade in, and shapes of `b` for
/// which `hidden_b` is true (entrance animations, hidden media) stay out of the frame.
pub fn morph_frame(
    pres: &Presentation,
    a: &Slide,
    b: &Slide,
    t: f64,
    hidden_b: &dyn Fn(ShapeId) -> bool,
) -> MorphFrame {
    let s = smooth(clampf(t, 0.0, 1.0, 0.0));
    let ca = Ctx::for_slide(pres, a);
    let cb = Ctx::for_slide(pres, b);
    let visible: Vec<_> = b.shapes.iter().map(|shape| !hidden_b(shape.id)).collect();
    let mut paired_old = vec![false; a.shapes.len()];
    let mut origin = vec![None; b.shapes.len()];
    for (i, j) in match_shapes(&a.shapes, &b.shapes) {
        if visible[j] {
            paired_old[i] = true;
            origin[j] = Some(i);
        }
    }
    let mut next = 1u32;
    walk(&a.shapes, &mut |x, _| {
        next = next.max(x.id.0.saturating_add(1))
    });
    walk(&b.shapes, &mut |x, _| {
        next = next.max(x.id.0.saturating_add(1))
    });
    let mut f = MorphFrame {
        mix: s,
        ..Default::default()
    };
    let by = b
        .transition
        .as_ref()
        .map(|tr| tr.option.as_str())
        .unwrap_or("");
    let (by_text, chars) = (by == "words" || by == "characters", by == "characters");
    let has_text = |x: &Shape| {
        x.text.as_ref().is_some_and(|t| !t.is_empty())
            && !matches!(x.kind, ShapeKind::Group { .. } | ShapeKind::Table(_))
    };
    let mut text = Vec::new();
    let mut paths = Vec::new();
    let mut push = |mut sh: Shape, old: bool, op: f64| {
        let id = ShapeId(next);
        sh.id = id;
        next = next.saturating_add(1);
        if op < 1.0 {
            f.opacity.push((id, op));
        }
        f.shapes.push((sh, old));
        id
    };
    for (i, sa) in a.shapes.iter().enumerate() {
        if !paired_old[i] {
            push(sa.clone(), true, 1.0 - s);
        }
    }
    for (j, sb) in b.shapes.iter().enumerate() {
        if !visible[j] {
            continue;
        }
        let Some(sa) = origin[j].map(|i| &a.shapes[i]) else {
            push(sb.clone(), false, s);
            continue;
        };
        let (xa, xb) = (box_of(ca.as_ref(), sa), box_of(cb.as_ref(), sb));
        let x = match (xa, xb) {
            (Some(xa), Some(xb)) => Some(morph_xfrm(xa, xb, s)),
            (_, xb) => xb,
        };
        let reshape = reshaped(sa, sb);
        let recrop = recropped(sa, sb);
        if reshape || recrop || content(sa) == content(sb) {
            let mut m = Shape {
                xfrm: x,
                ..sb.clone()
            };
            if recrop && let ShapeKind::Picture { fill } = &mut m.kind {
                fill.crop = match (xa, xb, x) {
                    (Some(xa), Some(xb), Some(x)) => {
                        morph_crop(crop_of(sa), xa, crop_of(sb), xb, x, s)
                    }
                    _ => {
                        let (ca, cb) = (crop_of(sa), crop_of(sb));
                        [
                            lerp(ca[0], cb[0], s),
                            lerp(ca[1], cb[1], s),
                            lerp(ca[2], cb[2], s),
                            lerp(ca[3], cb[3], s),
                        ]
                    }
                };
            }
            if let (Some(fa), Some(fb)) =
                (solid_color(ca.as_ref(), sa), solid_color(cb.as_ref(), sb))
                && fa != fb
            {
                m.fill = Some(Fill::solid(ColorRef::rgb(mix_color(fa, fb, s))));
            }
            let id = push(m, false, 1.0);
            if reshape && let (Some(from_box), Some(to_box)) = (xa, xb) {
                paths.push(MorphPath {
                    id,
                    from: sa.geom.clone(),
                    from_box,
                    to_box,
                });
            }
        } else {
            let new = push(
                Shape {
                    xfrm: x,
                    ..sb.clone()
                },
                false,
                s,
            );
            let old = push(
                Shape {
                    xfrm: x,
                    ..sa.clone()
                },
                true,
                1.0 - s,
            );
            // The same text in shapes that differ otherwise (a title slide's centred title and a
            // content slide's title: other placeholder, size, alignment) moves and resizes word
            // by word even with the Objects option, as PowerPoint does, instead of cross-fading.
            let same_text = text_of(sa).trim() == text_of(sb).trim();
            if (by_text || same_text)
                && has_text(sa)
                && has_text(sb)
                && let (Some(from), Some(to)) = (xa, xb)
            {
                text.push(MorphText {
                    old,
                    new,
                    from,
                    to,
                    chars,
                });
            }
        }
    }
    f.text = text;
    f.paths = paths;
    f
}
