//! Geometry fast path for a single convex device clip. Shapes wholly inside
//! pass unchanged; shapes crossing the edge are flattened and cut: fills with Sutherland–Hodgman
//! per subpath, outlines with Cyrus–Beck per segment. Bitmaps are cropped (and masked outside a
//! non-rectangular polygon). Text and affine drawings keep the polygon as a scene mask.

use crate::bitmap::Rgba;
use crate::canvas::Raw;
pub(crate) use crate::convex_clip::{clip_polygon, convex};
use crate::convex_clip::{clip_segment, contains};
use crate::geom::{Op, P};

/// Line segments per Bézier when a crossing shape is flattened.
const CURVE_STEPS: usize = 16;

/// The bounding box `[l, t, r, b]` of the points in `ops` (control points included).
pub(crate) fn ops_bbox(ops: &[Op]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    let mut add = |p: P| {
        let v = b.get_or_insert([p.0, p.1, p.0, p.1]);
        *v = [v[0].min(p.0), v[1].min(p.1), v[2].max(p.0), v[3].max(p.1)];
    };
    for op in ops {
        match *op {
            Op::M(p) | Op::L(p) => add(p),
            Op::C(a, c, d) => {
                add(a);
                add(c);
                add(d);
            }
            Op::Z => {}
        }
    }
    b
}

pub(crate) fn intersect(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let r = [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ];
    // An empty intersection stays empty (zero size) so it clips everything away.
    if r[2] < r[0] || r[3] < r[1] {
        [r[0], r[1], r[0], r[1]]
    } else {
        r
    }
}

/// A rectangle `[l, t, r, b]` as a polygon.
pub(crate) fn rect_poly([l, t, r, b]: [f64; 4]) -> Vec<P> {
    vec![(l, t), (r, t), (r, b), (l, b)]
}

pub(crate) fn outline(ring: &[P]) -> kurbo::BezPath {
    let mut path = kurbo::BezPath::new();
    if let Some((&start, rest)) = ring.split_first() {
        path.move_to(start);
        for &point in rest {
            path.line_to(point);
        }
        path.close_path();
    }
    path
}

/// The bounding box of a point list.
pub(crate) fn pts_bbox(pts: &[P]) -> Option<[f64; 4]> {
    let (&f, rest) = pts.split_first()?;
    Some(rest.iter().fold([f.0, f.1, f.0, f.1], |v, p| {
        [v[0].min(p.0), v[1].min(p.1), v[2].max(p.0), v[3].max(p.1)]
    }))
}

/// Whether the clip polygon is an axis-aligned rectangle.
fn is_rect(clip: &[P]) -> bool {
    pts_bbox(clip).is_some_and(|b| {
        clip.len() == 4
            && clip
                .iter()
                .all(|p| (p.0 == b[0] || p.0 == b[2]) && (p.1 == b[1] || p.1 == b[3]))
    })
}

fn disjoint(b: [f64; 4], c: [f64; 4]) -> bool {
    b[2] < c[0] || b[0] > c[2] || b[3] < c[1] || b[1] > c[3] || c[2] <= c[0] || c[3] <= c[1]
}

fn bbox_inside(b: [f64; 4], clip: &[P]) -> bool {
    rect_poly(b).into_iter().all(|p| contains(clip, p))
}

/// Clips one drawn item to a convex device polygon; a shape that is both filled and stroked comes
/// back as two items. An empty polygon clips everything away.
pub(crate) fn clip_raw(raw: Raw, clip: &[P]) -> Vec<Raw> {
    let Some(cb) = pts_bbox(clip).filter(|_| clip.len() >= 3) else {
        return Vec::new();
    };
    match raw {
        Raw::Path {
            ops,
            fill,
            stroke,
            even_odd,
            region,
        } => {
            let Some(b) = ops_bbox(&ops) else {
                return Vec::new();
            };
            if bbox_inside(b, clip) {
                return vec![Raw::Path {
                    ops,
                    fill,
                    stroke,
                    even_odd,
                    region,
                }];
            }
            if disjoint(b, cb) {
                return Vec::new();
            }
            let subs = flatten(&ops);
            let mut out = Vec::new();
            if fill.is_some() {
                let f = clip_fill(&subs, clip);
                if !f.is_empty() {
                    out.push(Raw::Path {
                        ops: f,
                        fill,
                        stroke: None,
                        even_odd,
                        region: region.clone(),
                    });
                }
            }
            if stroke.is_some() {
                let s = clip_stroke(&subs, clip);
                if !s.is_empty() {
                    out.push(Raw::Path {
                        ops: s,
                        fill: None,
                        stroke,
                        even_odd,
                        region,
                    });
                }
            }
            out
        }
        Raw::Pattern {
            ops,
            even_odd,
            tile,
            region,
        } => {
            let Some(b) = ops_bbox(&ops) else {
                return Vec::new();
            };
            if bbox_inside(b, clip) {
                return vec![Raw::Pattern {
                    ops,
                    even_odd,
                    tile,
                    region,
                }];
            }
            if disjoint(b, cb) {
                return Vec::new();
            }
            let f = clip_fill(&flatten(&ops), clip);
            if f.is_empty() {
                Vec::new()
            } else {
                vec![Raw::Pattern {
                    ops: f,
                    even_odd,
                    tile,
                    region,
                }]
            }
        }
        Raw::Bitmap { rect, img, region } => crop(rect, img, clip)
            .map(|(rect, img)| vec![Raw::Bitmap { rect, img, region }])
            .unwrap_or_default(),
        Raw::Text { .. } | Raw::AffineBitmap { .. } | Raw::Nested { .. } => vec![raw],
    }
}

/// Subpaths as point lists with their closed flag.
pub(crate) fn flatten(ops: &[Op]) -> Vec<(Vec<P>, bool)> {
    let mut subs: Vec<(Vec<P>, bool)> = Vec::new();
    for op in ops {
        match *op {
            Op::M(p) => subs.push((vec![p], false)),
            Op::L(p) => match subs.last_mut() {
                Some(s) => s.0.push(p),
                None => subs.push((vec![p], false)),
            },
            Op::C(a, b, d) => {
                let Some(s) = subs.last_mut() else { continue };
                let Some(&p0) = s.0.last() else { continue };
                for i in 1..=CURVE_STEPS {
                    let t = i as f64 / CURVE_STEPS as f64;
                    let u = 1.0 - t;
                    let k = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    s.0.push((
                        k[0] * p0.0 + k[1] * a.0 + k[2] * b.0 + k[3] * d.0,
                        k[0] * p0.1 + k[1] * a.1 + k[2] * b.1 + k[3] * d.1,
                    ));
                }
            }
            Op::Z => {
                if let Some(s) = subs.last_mut() {
                    s.1 = true;
                    // Drawing after a close starts from the subpath's first point.
                    let first = s.0.first().copied();
                    if let Some(f) = first {
                        subs.push((vec![f], false));
                    }
                }
            }
        }
    }
    subs.retain(|s| s.0.len() > 1);
    subs
}

fn clip_fill(subs: &[(Vec<P>, bool)], clip: &[P]) -> Vec<Op> {
    let mut ops = Vec::new();
    for (pts, _) in subs {
        let poly = clip_polygon(pts, clip);
        if poly.len() >= 3 {
            ops.push(Op::M(poly[0]));
            ops.extend(poly[1..].iter().map(|p| Op::L(*p)));
            ops.push(Op::Z);
        }
    }
    ops
}

fn clip_stroke(subs: &[(Vec<P>, bool)], clip: &[P]) -> Vec<Op> {
    let mut ops = Vec::new();
    for (pts, closed) in subs {
        let mut pts = pts.clone();
        if *closed && let Some(&f) = pts.first() {
            pts.push(f);
        }
        let mut last: Option<P> = None;
        for w in pts.windows(2) {
            let Some((a, b)) = clip_segment(w[0], w[1], clip) else {
                last = None;
                continue;
            };
            if last != Some(a) {
                ops.push(Op::M(a));
            }
            ops.push(Op::L(b));
            last = Some(b);
        }
    }
    ops
}

/// Crops a bitmap drawn into `rect` = `[x, y, w, h]` (device, positive extents) to the clip
/// polygon's box; pixels outside a non-rectangular polygon turn transparent.
fn crop(rect: [f64; 4], img: Rgba, clip: &[P]) -> Option<([f64; 4], Rgba)> {
    let [x, y, w, h] = rect;
    let b = [x, y, x + w, y + h];
    if bbox_inside(b, clip) {
        return Some((rect, img));
    }
    let c = pts_bbox(clip)?;
    if disjoint(b, c) || w <= 0.0 || h <= 0.0 || img.w == 0 || img.h == 0 {
        return None;
    }
    let i = intersect(b, c);
    let (kx, ky) = (img.w as f64 / w, img.h as f64 / h);
    let x0 = (((i[0] - x) * kx).floor().max(0.0) as usize).min(img.w);
    let x1 = (((i[2] - x) * kx).ceil().max(0.0) as usize).min(img.w);
    let y0 = (((i[1] - y) * ky).floor().max(0.0) as usize).min(img.h);
    let y1 = (((i[3] - y) * ky).ceil().max(0.0) as usize).min(img.h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mask = !is_rect(clip);
    let total_pixels = (x1 - x0).saturating_mul(y1 - y0);
    if mask && total_pixels.saturating_mul(clip.len()) > 5_000_000 {
        return None;
    }
    let mut px = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
    for row in y0..y1 {
        // Rows are top-down unless the image says otherwise.
        let r = if img.bottom_up { img.h - 1 - row } else { row };
        let s = (r * img.w + x0) * 4;
        let start = px.len();
        px.extend_from_slice(img.px.get(s..s + (x1 - x0) * 4)?);
        if mask {
            let cy = y + (row as f64 + 0.5) / ky;
            for (k, p) in px[start..].as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let cx = x + ((x0 + k) as f64 + 0.5) / kx;
                if !contains(clip, (cx, cy)) {
                    p[3] = 0;
                }
            }
        }
    }
    let out = [
        x + x0 as f64 / kx,
        y + y0 as f64 / ky,
        (x1 - x0) as f64 / kx,
        (y1 - y0) as f64 / ky,
    ];
    Some((
        out,
        Rgba {
            w: x1 - x0,
            h: y1 - y0,
            bottom_up: false,
            px,
        },
    ))
}
