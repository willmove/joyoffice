//! Continuous motion curves, 3-D perspective projection and textured surfaces
//! for JoyOffice slide transitions.
use crate::easing::{ease, smooth};
use crate::transition::{Layer, P, Q, R, Source, corners};

/// Smoothstep ease in-out in `[0, 1]`.
pub(crate) fn smoothstep(x: f64) -> f64 {
    smooth(x)
}

/// Quadratic ease in-out in `[0, 1]`.
pub(crate) fn quad_ease(x: f64) -> f64 {
    ease(x, 0.5, 0.5, 0.0)
}

/// Normalized linear progress through `[start, end]`, clamped to `[0, 1]`.
pub(crate) fn span(t: f64, start: f64, end: f64) -> f64 {
    if end <= start {
        return if t >= end { 1. } else { 0. };
    }
    ((t - start) / (end - start)).clamp(0., 1.)
}

/// Deterministic splitmix64 hash in `[0, 1)` for procedural particle scatter.
pub(crate) fn unit_hash(mut seed: u64) -> f64 {
    seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((seed ^ (seed >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
}

/// Parse cardinal/diagonal slide direction options (`l`, `r`, `u`, `d`).
pub(crate) fn direction_vec(option: &str, default: [f64; 2]) -> [f64; 2] {
    let mut v = [0., 0.];
    if option.contains('l') {
        v[0] = -1.;
    }
    if option.contains('r') {
        v[0] = 1.;
    }
    if option.contains('u') {
        v[1] = -1.;
    }
    if option.contains('d') {
        v[1] = 1.;
    }
    if v == [0., 0.] { default } else { v }
}

/// Rotate a slide-space point clockwise about `pivot` preserving aspect ratio.
pub(crate) fn rotate_2d(point: P, pivot: P, degrees: f64, aspect: f64) -> P {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let dx = (point[0] - pivot[0]) * aspect;
    let dy = point[1] - pivot[1];
    [
        pivot[0] + (dx * cos - dy * sin) / aspect,
        pivot[1] + dx * sin + dy * cos,
    ]
}

/// Scale a rectangle about `pivot` and shift by `(dx, dy)`.
pub(crate) fn transform_rect(rect: R, scale: f64, pivot: P, shift: P) -> R {
    [
        pivot[0] + (rect[0] - pivot[0]) * scale + shift[0],
        pivot[1] + (rect[1] - pivot[1]) * scale + shift[1],
        pivot[0] + (rect[2] - pivot[0]) * scale + shift[0],
        pivot[1] + (rect[3] - pivot[1]) * scale + shift[1],
    ]
}

/// 3-D point in slide-height units (`x` right, `y` down, `z` into the screen).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn yaw_about(self, pivot: Self, degrees: f64) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let dx = self.x - pivot.x;
        let dz = self.z - pivot.z;
        Self::new(
            pivot.x + dx * cos - dz * sin,
            self.y,
            pivot.z + dx * sin + dz * cos,
        )
    }

    pub fn pitch_about(self, pivot: Self, degrees: f64) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let dy = self.y - pivot.y;
        let dz = self.z - pivot.z;
        Self::new(
            self.x,
            pivot.y + dy * cos + dz * sin,
            pivot.z - dy * sin + dz * cos,
        )
    }

    pub fn roll_about(self, pivot: Self, degrees: f64) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        let dx = self.x - pivot.x;
        let dy = self.y - pivot.y;
        Self::new(
            pivot.x + dx * cos - dy * sin,
            pivot.y + dx * sin + dy * cos,
            self.z,
        )
    }
}

/// Pinhole perspective camera situated `distance` slide heights in front of the plane.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Camera3D {
    pub aspect: f64,
    pub distance: f64,
}

impl Camera3D {
    pub fn on_plane(self, u: f64, v: f64) -> Vec3 {
        Vec3::new((u - 0.5) * self.aspect, v - 0.5, 0.)
    }

    pub fn rect_corners(self, rect: R) -> [Vec3; 4] {
        corners(rect).map(|p| self.on_plane(p[0], p[1]))
    }

    pub fn project(self, point: Vec3) -> P {
        let scale = self.distance / (self.distance + point.z).max(self.distance * 0.04);
        [point.x * scale / self.aspect + 0.5, point.y * scale + 0.5]
    }

    pub fn project_quad(self, points: [Vec3; 4]) -> Q {
        points.map(|p| self.project(p))
    }

    /// Shading factor in `[0, max_shade]` that darkens a card as it turns edge-on.
    pub fn obliquity_shade(points: [Vec3; 4], max_shade: f64) -> f64 {
        let e1 = Vec3::new(
            points[1].x - points[0].x,
            points[1].y - points[0].y,
            points[1].z - points[0].z,
        );
        let e2 = Vec3::new(
            points[3].x - points[0].x,
            points[3].y - points[0].y,
            points[3].z - points[0].z,
        );
        let nx = e1.y * e2.z - e1.z * e2.y;
        let ny = e1.z * e2.x - e1.x * e2.z;
        let nz = e1.x * e2.y - e1.y * e2.x;
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len <= 1e-12 {
            return max_shade.clamp(0., 1.);
        }
        let facing = (nz.abs() / len).clamp(0., 1.);
        ((1. - facing) * max_shade).clamp(0., 1.)
    }

    /// Project a textured 3-D quad and optionally emit its obliquity shading overlay.
    pub fn push_card(
        self,
        out: &mut Vec<Layer>,
        source: Source,
        src: R,
        points: [Vec3; 4],
        alpha: f64,
        shade: f64,
    ) {
        let quad = self.project_quad(points);
        let area = quad
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let b = quad[(i + 1) % 4];
                a[0] * b[1] - b[0] * a[1]
            })
            .sum::<f64>();
        if area.abs() < 1e-7 {
            return;
        }
        let face = Layer::quad(source, src, quad, alpha);
        out.push(face);
        if shade > 1e-4 {
            out.push(face.shade(shade));
        }
    }
}

/// Interpolate motion landmarks in normalized slide time. Knots at the contact
/// and release of the surface preserve its overshoot when frame rates change.
pub(crate) fn track<const N: usize>(t: f64, keys: &[(f64, [f64; N])]) -> [f64; N] {
    for pair in keys.windows(2) {
        let ((a, start), (b, end)) = (pair[0], pair[1]);
        if t <= b {
            let mix = ((t - a) / (b - a)).clamp(0., 1.);
            return std::array::from_fn(|i| start[i] + mix * (end[i] - start[i]));
        }
    }
    keys[keys.len() - 1].1
}

/// Draw a deformed texture in tiles, with continuous UVs and per-vertex shade.
/// The same geometry is consumed by the CPU and hardware compositors.
pub(crate) fn texture_grid(
    layers: &mut Vec<Layer>,
    source: Source,
    region: [f64; 2],
    columns: usize,
    rows: usize,
    project: impl Fn(P) -> (P, f64),
) {
    for row in 0..rows {
        for column in 0..columns {
            let x = |col: usize| region[0] + (region[1] - region[0]) * col as f64 / columns as f64;
            let rect = [
                x(column),
                row as f64 / rows as f64,
                x(column + 1),
                (row + 1) as f64 / rows as f64,
            ];
            let points = corners(rect).map(&project);
            let tile = Layer::quad(source, rect, points.map(|p| p.0), 1.);
            layers.push(tile);
            let fade = points.map(|p| p.1.clamp(0., 1.));
            if fade.iter().any(|&s| s > 0.) {
                layers.push(tile.shade(1.).faded(fade));
            }
        }
    }
}
