//! `p14:prism` turns either whole slides or just their foreground about one axis.
//! The moving edge stays on the screen plane for a cube. Inside a box the camera
//! pulls back and a narrow gap opens between the two walls. Lengths are measured
//! in slide heights, so a vertical turn has the same perspective at every aspect.
use crate::transition::{FULL, Layer, Source, corners};
use std::f64::consts::PI;

struct Wall {
    x: [f64; 2], // homogeneous screen coordinates on the turning axis
    w: [f64; 2], // eye distance of the outer and inner edge
}

impl Wall {
    fn at(t: f64, axis: f64, vertical: bool, inside: bool) -> Self {
        let ease = t * t * (3. - 2. * t);
        let symmetric = 2. * t - 1.;
        let travel =
            ease + (t * (1. - t)).powi(2) * symmetric * (1.29 - 0.89 * symmetric * symmetric);
        let angle = 1.256 * ease + 0.096 * ease * (1. - ease) * (2. * ease - 1.);
        let (sine, cosine) = angle.sin_cos();
        let perspective = axis * (2. / 3.);
        let (gap, inner_depth, slope) = if inside {
            let retreat = if vertical { 0.54 } else { 0.67 };
            (0.057 * ease, retreat * (PI * t).sin().powi(2), -sine)
        } else {
            (0., 0., sine)
        };
        let hinge = 1. - travel - gap;
        let depth = [
            perspective * (inner_depth + slope),
            perspective * inner_depth,
        ];
        Self {
            x: [hinge - cosine + 0.5 * depth[0], hinge + 0.5 * depth[1]],
            w: depth.map(|z| 1. + z),
        }
    }

    fn layer(&self, source: Source, vertical: bool, reverse: bool) -> Option<Layer> {
        if self.x[1] * self.w[0] <= self.x[0] * self.w[1] {
            return None; // The audience sees only the front of each wall.
        }
        // Clip the source in homogeneous space first. A very wide inside wall
        // can cross both the picture edge and the eye plane in one frame.
        let (mut lo, mut hi): (f64, f64) = (0., 1.);
        for [a, b] in [
            self.w.map(|v| v - 0.0001),
            self.x,
            [self.w[0] - self.x[0], self.w[1] - self.x[1]],
        ] {
            if a < 0. && b < 0. {
                return None;
            }
            if a < 0. {
                lo = lo.max(a / (a - b));
            }
            if b < 0. {
                hi = hi.min(a / (a - b));
            }
        }
        if lo >= hi {
            return None;
        }
        if reverse {
            (lo, hi) = (1. - hi, 1. - lo);
        }
        let mut crop = FULL;
        let i = usize::from(vertical);
        crop[i] = lo;
        crop[i + 2] = hi;
        let quad = corners(crop).map(|mut point| {
            let u = if reverse { 1. - point[i] } else { point[i] };
            let w = self.w[0] + u * (self.w[1] - self.w[0]);
            let x = (self.x[0] + u * (self.x[1] - self.x[0])) / w;
            point[i] = if reverse { 1. - x } else { x };
            point[1 - i] = 0.5 + (point[1 - i] - 0.5) / w;
            point
        });
        Some(Layer::quad(source, crop, quad, 1.))
    }
}

pub(crate) fn frame(kind: &str, direction: &str, t: f64, aspect: f64) -> Vec<Layer> {
    let vertical = matches!(direction, "u" | "d");
    let reverse = matches!(direction, "r" | "d");
    let axis = if vertical { 1. } else { aspect };
    let inside = matches!(kind, "box" | "orbit");
    let content = matches!(kind, "rotate" | "orbit");
    let (old, new) = if content {
        (Source::OldContent, Source::NewContent)
    } else {
        (Source::Old, Source::New)
    };
    let mut layers = if content {
        vec![
            Layer::full(Source::OldBack, 1.),
            Layer::full(Source::NewBack, t * t * (3. - 2. * t)),
        ]
    } else {
        vec![Layer::full(Source::Black, 1.)]
    };
    for (source, age, mirror) in [(old, t, reverse), (new, 1. - t, !reverse)] {
        let wall = Wall::at(age, axis, vertical, inside);
        if let Some(layer) = wall.layer(source, vertical, mirror) {
            layers.push(layer);
            let shade = (2.05 * (age - 0.51).max(0.).powf(1.55)).min(0.5);
            if !content && shade > 0. {
                layers.push(layer.shade(shade));
            }
        }
    }
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_directions_reflect_the_same_source_and_destination() {
        for t in [0.08, 0.3, 0.5, 0.71, 0.91] {
            for (forward, backward, axis) in [("l", "r", 0), ("u", "d", 1)] {
                let a = frame("box", forward, t, 4. / 3.);
                let b = frame("box", backward, t, 4. / 3.);
                assert_eq!(a.len(), b.len());
                let mirror = if axis == 0 {
                    [1, 0, 3, 2]
                } else {
                    [3, 2, 1, 0]
                };
                for (left, right) in a.iter().zip(b.iter()) {
                    assert_eq!(left.source, right.source);
                    for (i, j) in mirror.into_iter().enumerate() {
                        assert!((left.quad[i][axis] + right.quad[j][axis] - 1.).abs() < 1e-6);
                        assert!((left.quad[i][1 - axis] - right.quad[j][1 - axis]).abs() < 1e-6);
                    }
                }
            }
        }
    }

    #[test]
    fn wide_inside_turn_clips_textures_and_vertical_turn_is_aspect_independent() {
        for i in 1..100 {
            let t = i as f64 / 100.;
            assert_eq!(frame("orbit", "u", t, 1.), frame("orbit", "u", t, 100.));
            for layer in frame("orbit", "l", t, 100.) {
                assert!(layer.source.is_part());
                assert!(
                    layer
                        .quad
                        .iter()
                        .all(|p| p[0] >= -1e-6 && p[0] <= 1. + 1e-6 && p[1].is_finite())
                );
                assert!(layer.uv.iter().flatten().all(|x| (0. ..=1.).contains(x)));
            }
        }
    }
}
