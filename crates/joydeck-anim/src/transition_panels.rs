//! Tiled and banded slide transitions. Each face samples its own rectangle of
//! the original slide, so positions remain continuous as the face turns.
use crate::transition::{Layer, P, R, Source};
use crate::transition_geom::{Cam, card, center, offset};
use std::f64::consts::TAU;

fn band(vertical: bool, i: usize, count: usize) -> R {
    let lo = i as f64 / count as f64;
    let hi = (i + 1) as f64 / count as f64;
    if vertical {
        [lo, 0., hi, 1.]
    } else {
        [0., lo, 1., hi]
    }
}

pub(crate) fn comb(vertical: bool, t: f64) -> Vec<Layer> {
    let count = if vertical { 10 } else { 7 };
    let mut layers = vec![Layer::full(Source::New, 1.)];
    for i in 0..count {
        // Horizontal teeth leave successively from the bottom of the page.
        // The vertical teeth travel together, starting at alternating ends.
        let travel = if vertical {
            2.08 * t - 0.30
        } else {
            1.57 * t - 0.46 + 0.077 * i as f64
        };
        let distance = travel.max(0.).powi(2);
        if distance < 1. {
            let r = band(vertical, i, count);
            let positive = (i % 2 == 0) != vertical;
            let d = if positive { distance } else { -distance };
            let dest = if vertical {
                offset(r, 0., d)
            } else {
                offset(r, d, 0.)
            };
            layers.push(Layer::rect(Source::Old, r, dest, 1.));
        }
    }
    layers
}

// Half-turn offsets in hundredths of the duration from a horizontal checker
// export. The same ordering is used when the sweep travels down the page.
const TURN_OFFSETS: [[u8; 7]; 5] = [
    [30, 45, 25, 24, 24, 36, 34],
    [45, 40, 41, 36, 24, 44, 24],
    [38, 35, 30, 42, 25, 33, 38],
    [46, 31, 45, 28, 43, 24, 23],
    [23, 29, 39, 25, 29, 31, 29],
];

pub(crate) fn checker(vertical: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let camera = Cam { a: aspect, d: 3.0 };
    let mut out = vec![Layer::full(Source::Black, 1.)];
    for (row, offsets) in TURN_OFFSETS.iter().enumerate() {
        for (col, &phase) in offsets.iter().enumerate() {
            let tile = [
                col as f64 / 7.,
                row as f64 / 5.,
                (col + 1) as f64 / 7.,
                (row + 1) as f64 / 5.,
            ];
            let half = f64::from(phase) * 0.01
                + if vertical {
                    0.072 * row as f64 - 0.01
                } else {
                    0.046 * col as f64
                };
            let turn = ((t - half) / 0.34 + 0.5).clamp(0., 1.);
            let (source, angle) = if turn < 0.5 {
                (Source::Old, turn * 180.)
            } else {
                (Source::New, (turn - 1.) * 180.)
            };
            if angle == 0. {
                out.push(Layer::inplace(source, tile, 1.));
            } else {
                let middle = center(tile);
                let pivot = camera.unit(middle[0], middle[1]);
                let face = camera.rect(tile).map(|p| {
                    if vertical {
                        p.pitch(pivot, angle)
                    } else {
                        p.yaw(pivot, angle)
                    }
                });
                let shadow = 0.42 * (1. - angle.to_radians().cos());
                out.extend(card(&camera, source, tile, face, 1., shadow, true));
            }
        }
    }
    out
}

pub(crate) fn blinds(vertical: bool, t: f64) -> Vec<Layer> {
    let count = if vertical { 18 } else { 14 };
    let mut out = vec![Layer::full(Source::Black, 1.)];
    for i in 0..count {
        let full = band(vertical, i, count);
        let along = (i as f64 + 0.5) / count as f64;
        let location = 2. * along - 1.;
        let (sweep, skew, span, shadow_width) = if vertical {
            (0.36, 0.05, 0.05, 0.11)
        } else {
            (0.34, 0.02, 0.15, 0.07)
        };
        let clock = t - (0.24 + location.abs() * sweep + location * skew);
        let old_width = (0.5 - clock / span).clamp(0., 1.);
        let shadow = 0.45 * (1. - (clock / shadow_width).abs()).max(0.).powf(1.3);
        let side = if vertical { 0 } else { 1 };
        let seam = full[side] + (full[side + 2] - full[side]) * old_width;
        let mut old = full;
        let mut new = full;
        old[side + 2] = seam;
        new[side] = seam;
        for (source, rect) in [(Source::Old, old), (Source::New, new)] {
            if rect[side + 2] > rect[side] {
                let face = Layer::rect(source, full, rect, 1.);
                out.push(face);
                if shadow > 0. {
                    out.push(face.shade(shadow));
                }
            }
        }
    }
    out
}

pub(crate) fn ripple(option: &str, t: f64, aspect: f64) -> Vec<Layer> {
    let origin: P = match option {
        "lu" => [1., 1.],
        "ru" => [0., 1.],
        "ld" => [1., 0.],
        "rd" => [0., 0.],
        _ => [0.5, 0.5],
    };
    let far = ((origin[0].max(1. - origin[0]) * aspect).powi(2)
        + origin[1].max(1. - origin[1]).powi(2))
    .sqrt();
    let front = 0.98 * t - 0.09;
    let intensity = |radius: f64| {
        let offset = radius - front;
        let mix = (0.5 - (offset + 0.003) / 0.223).clamp(0., 1.);
        // A crest darkens both textures. The trailing crests lose energy
        // and only the leading shoulder refracts the outgoing slide.
        let phase = TAU * (offset - 0.0255) / (1. / 3.);
        let damping = (0.73 * (offset - 0.0255).min(0.)).exp();
        let shade = 0.173 * phase.cos().max(0.) * damping * (mix * 7.).min(1.);
        let old_shift = 0.0016 * (1. - ((offset - 0.075) / 0.064).powi(2)).max(0.);
        let new_shift = -0.0015 * (TAU * (offset + 0.21) / 0.33).cos() * damping;
        (mix, shade, old_shift, new_shift)
    };
    let mut out = vec![Layer::full(Source::Old, 1.)];
    // Join each ray to a fixed point on the frame. Clamping the radii at
    // the edge includes the four corners and keeps all UVs on the slide.
    let reach = |p: P| ((p[0] * aspect).powi(2) + p[1].powi(2)).sqrt() / far;
    for side in 0..4 {
        let ray = |i: usize| -> P {
            let u = i as f64 / 12.;
            let end = match side {
                0 => [u, 0.],
                1 => [1., u],
                2 => [1. - u, 1.],
                _ => [0., 1. - u],
            };
            [end[0] - origin[0], end[1] - origin[1]]
        };
        for sector in 0..12 {
            let (left, right) = (ray(sector), ray(sector + 1));
            if (left[0] * right[1] - left[1] * right[0]).abs() < 1e-12 {
                continue;
            }
            let (lr, rr) = (reach(left), reach(right));
            let point = |dir: P, limit: f64, r: f64| -> P {
                let k = (r / limit).min(1.);
                [origin[0] + k * dir[0], origin[1] + k * dir[1]]
            };
            for step in 0..36 {
                let r0 = step as f64 / 36.;
                if r0 >= lr.max(rr) || r0 >= front + 0.124 {
                    break;
                }
                let r1 = (step + 1) as f64 / 36.;
                let quad = [
                    point(left, lr, r0),
                    point(left, lr, r1),
                    point(right, rr, r1),
                    point(right, rr, r0),
                ];
                let radii = [r0.min(lr), r1.min(lr), r1.min(rr), r0.min(rr)];
                let values = radii.map(intensity);
                let refract = |i: usize, shift: f64| {
                    let radius = radii[i];
                    if radius == 0. {
                        return origin;
                    }
                    let k = 1. + shift / radius;
                    [0, 1].map(|axis| {
                        (origin[axis] + (quad[i][axis] - origin[axis]) * k).clamp(0., 1.)
                    })
                };
                for inds in [[0, 1, 2], [0, 2, 3]] {
                    let p = inds.map(|i| quad[i]);
                    if p[0] == p[1] || p[1] == p[2] || p[0] == p[2] {
                        continue;
                    }
                    if inds.iter().any(|&i| values[i].2 > 0.) {
                        out.push(Layer::tri_uv(
                            Source::Old,
                            p,
                            inds.map(|i| refract(i, values[i].2)),
                            [1.; 3],
                            1.,
                        ));
                    }
                    out.push(Layer::tri_uv(
                        Source::New,
                        p,
                        inds.map(|i| refract(i, values[i].3)),
                        inds.map(|i| values[i].0),
                        1.,
                    ));
                    let shade = inds.map(|i| values[i].1);
                    if shade.iter().any(|&v| v > 0.) {
                        out.push(Layer::tri_uv(Source::Black, p, p, shade, 1.));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_ripple_textures_refract_inside_the_slide_in_all_five_directions() {
        for option in ["center", "lu", "ru", "ld", "rd"] {
            for aspect in [4. / 3., 16. / 9.] {
                let layers = ripple(option, 0.5, aspect);
                for source in [Source::Old, Source::New] {
                    assert!(
                        layers
                            .iter()
                            .filter(|face| face.source == source)
                            .any(|face| {
                                face.uv.iter().zip(face.quad).any(|(uv, p)| {
                                    (uv[0] - p[0]).abs() + (uv[1] - p[1]).abs() > 1e-5
                                })
                            })
                    );
                }
                assert!(
                    layers
                        .iter()
                        .flat_map(|l| l.uv)
                        .flatten()
                        .all(|coord| (0.0..=1.).contains(&coord))
                );
            }
        }
    }
}
