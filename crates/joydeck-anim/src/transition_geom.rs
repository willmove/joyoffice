//! Geometry for transition effects: easing, 3-D cards, opacity fields, polygons.

#[cfg(test)]
use crate::easing::ease;
use crate::easing::smooth;
use crate::transition::{Layer, P, Q, R, Source, corners};

// ---------- easing ----------

/// Smoothstep ease in-out (PowerPoint's fades).
pub(crate) fn ss(x: f64) -> f64 {
    smooth(x)
}

/// Quadratic ease in-out: velocity ramps up for the first half and down for the second
/// (PowerPoint's moves: Push, Cover, Uncover, Pan…).
#[cfg(test)]
pub(crate) fn q2(x: f64) -> f64 {
    ease(x, 0.5, 0.5, 0.0)
}

/// Progress through the window [a, b] of t, clamped to 0..1.
pub(crate) fn win(t: f64, a: f64, b: f64) -> f64 {
    if b <= a {
        return if t >= b { 1.0 } else { 0.0 };
    }
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

// ---------- randomness ----------

pub(crate) fn hash64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Deterministic pseudo-random value in [0, 1).
pub(crate) fn rnd(i: u64) -> f64 {
    (hash64(i) >> 11) as f64 / (1u64 << 53) as f64
}

// ---------- 2-D ----------

pub(crate) fn offset(r: R, dx: f64, dy: f64) -> R {
    [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy]
}

pub(crate) fn center(r: R) -> P {
    [(r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0]
}

// ---------- polygons ----------

/// Keep the part of a convex polygon where `n · p <= d`.
pub(crate) fn clip(poly: &[P], n: P, d: f64) -> Vec<P> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    let side = |p: P| n[0] * p[0] + n[1] * p[1] - d;
    for (i, p) in poly.iter().enumerate() {
        let q = poly.get((i + 1) % poly.len()).copied().unwrap_or(*p);
        let (sp, sq) = (side(*p), side(q));
        if sp <= 0.0 {
            out.push(*p);
        }
        if (sp < 0.0 && sq > 0.0) || (sp > 0.0 && sq < 0.0) {
            let k = sp / (sp - sq);
            out.push([p[0] + (q[0] - p[0]) * k, p[1] + (q[1] - p[1]) * k]);
        }
    }
    out
}

/// A convex polygon as triangles: vertices drawn at `map(p)` with texture coordinate `p`.
pub(crate) fn poly_layers(
    source: Source,
    poly: &[P],
    alpha: f64,
    map: &dyn Fn(P) -> P,
) -> Vec<Layer> {
    let Some(first) = poly.first().copied() else {
        return vec![];
    };
    poly.windows(2)
        .skip(1)
        .filter_map(|w| match w {
            [b, c] => Some(Layer::tri_uv(
                source,
                [map(first), map(*b), map(*c)],
                [first, *b, *c],
                [1.0; 3],
                alpha,
            )),
            _ => None,
        })
        .collect()
}

// ---------- 3-D ----------

/// A point in the 3-D scene: x right and y down in slide heights (the slide spans x ∈ [−a/2, a/2],
/// y ∈ [−½, ½] at z = 0), z away from the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub(crate) fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    /// Rotate about the vertical axis through `c` (degrees; positive turns the right side away).
    pub fn yaw(self, c: V3, deg: f64) -> V3 {
        let (s, co) = deg.to_radians().sin_cos();
        let p = self.sub(c);
        v3(p.x * co - p.z * s, p.y, p.x * s + p.z * co).add(c)
    }
    /// Rotate about the horizontal axis through `c` (degrees; positive turns the top away).
    pub fn pitch(self, c: V3, deg: f64) -> V3 {
        let (s, co) = deg.to_radians().sin_cos();
        let p = self.sub(c);
        v3(p.x, p.y * co + p.z * s, -p.y * s + p.z * co).add(c)
    }
}

/// A perspective camera looking at the slide plane from `d` slide heights away.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cam {
    pub a: f64,
    pub d: f64,
}

impl Cam {
    pub fn p(&self, v: V3) -> P {
        let k = self.d / (self.d + v.z).max(self.d * 0.02);
        [v.x * k / self.a + 0.5, v.y * k + 0.5]
    }
    /// The 3-D corners (tl, tr, br, bl) of the unit-space rectangle `r` lying on the slide plane.
    pub fn rect(&self, r: R) -> [V3; 4] {
        corners(r).map(|p| v3((p[0] - 0.5) * self.a, p[1] - 0.5, 0.0))
    }
    /// The unit-space point of a 3-D point on the slide plane.
    pub fn unit(&self, x: f64, y: f64) -> V3 {
        v3((x - 0.5) * self.a, y - 0.5, 0.0)
    }
    pub fn quad(&self, c: [V3; 4]) -> Q {
        c.map(|v| self.p(v))
    }
}

/// Signed area of a quad (positive: front side facing the viewer).
pub(crate) fn area(q: &Q) -> f64 {
    let mut s = 0.0;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        s += a[0] * b[1] - b[0] * a[1];
    }
    s / 2.0
}

/// A card: `src` of the source mapped onto 3-D corners, if it faces the viewer; `shade` (0..1)
/// darkens it. Seen from behind it is left out unless `back` is set (then drawn mirrored).
pub(crate) fn card(
    cam: &Cam,
    source: Source,
    src: R,
    c: [V3; 4],
    alpha: f64,
    shade: f64,
    back: bool,
) -> Vec<Layer> {
    let q = cam.quad(c);
    if !q.iter().all(|p| p[0].is_finite() && p[1].is_finite()) {
        return vec![];
    }
    let front = area(&q) > 0.0;
    if !front && !back {
        return vec![];
    }
    let l = Layer::quad(source, src, q, alpha);
    let mut v = vec![l];
    if shade > 0.001 {
        v.push(l.shade(shade));
    }
    v
}
