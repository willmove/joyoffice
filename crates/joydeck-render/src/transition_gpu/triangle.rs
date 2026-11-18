//! Analytic coverage at the CPU compositor's pixel centre. Hardware triangles snap
//! vertices to a subpixel grid; thin tiles would otherwise reveal the background.
use joydeck_anim::MeshVertex;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Triangle {
    bounds: [f32; 4],
    edges: [[f32; 4]; 3],
    origins: [f32; 3],
    factors: [f32; 3],
    flags: u32,
    attributes: [[f32; 3]; 3],
}

impl Triangle {
    pub(super) fn new(
        vertices: [MeshVertex; 3],
        size: [f64; 2],
        source: u32,
    ) -> Option<(Self, u64)> {
        let points = vertices.map(|v| [v.pos[0] * size[0], v.pos[1] * size[1]]);
        let [a, b, c] = points;
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if !area.is_finite() || area.abs() < 1e-12 {
            return None;
        }
        let mut bounds = [0.; 4];
        let mut pixels = 1u64;
        for axis in 0..2 {
            let lo = a[axis]
                .min(b[axis])
                .min(c[axis])
                .floor()
                .clamp(0., size[axis]);
            let hi = a[axis]
                .max(b[axis])
                .max(c[axis])
                .ceil()
                .clamp(0., size[axis]);
            if hi <= lo {
                return None;
            }
            pixels *= (hi - lo) as u64;
            bounds[axis] = (lo / size[axis]) as f32;
            bounds[axis + 2] = (hi / size[axis]) as f32;
        }
        let mut edges = [[0.; 4]; 3];
        let mut origins = [0.; 3];
        let mut factors = [0.; 3];
        let mut flags = source;
        let centre = [0.5 + 1.17e-6, 0.5 + 2.31e-6];
        for (edge, (a, b)) in [(b, c), (c, a), (a, b)].into_iter().enumerate() {
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            // Solve against the dominant normal. Splitting the slope and separating the
            // integer intercept keeps shared edges stable at pixel centres on large slides.
            let (dependent, scale) = if dx.abs() > dy.abs() {
                flags |= 1 << (3 + edge);
                (1, dx)
            } else {
                (0, -dy)
            };
            let other = 1 - dependent;
            let slope = (b[dependent] - a[dependent]) / (b[other] - a[other]);
            let high = slope as f32;
            // Pick the same origin for either direction of a shared edge. At fan or
            // sliver vertices, subtracting the pixel origin before multiplication keeps
            // the tiny centre offset distinguishable from zero.
            let anchor = if (a[other], a[dependent]) < (b[other], b[dependent]) {
                a
            } else {
                b
            };
            let origin = (anchor[other] - centre[other]).round() as f32;
            let intercept = anchor[dependent] - centre[dependent]
                + slope * (f64::from(origin) - anchor[other] + centre[other]);
            let integer = intercept.round() as f32;
            origins[edge] = origin;
            edges[edge] = [
                high,
                (slope - f64::from(high)) as f32,
                integer,
                (intercept - f64::from(integer)) as f32,
            ];
            factors[edge] = (scale / area) as f32;
        }
        Some((
            Self {
                bounds,
                edges,
                origins,
                factors,
                flags,
                attributes: vertices
                    .map(|v| [v.uv[0] as f32, v.uv[1] as f32, v.a.clamp(0., 1.) as f32]),
            },
            pixels,
        ))
    }
}
