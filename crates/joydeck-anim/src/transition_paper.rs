//! Textured paper fragments, crumpling, diagonal peels and bound page turns.
//! All positions are in slide coordinates; CPU and GPU draw the same triangles.
use crate::transition::{FULL, Layer, P, Source, corners};
use crate::transition_geom::{clip, poly_layers, rnd, ss, win};
use crate::transition_surface::{texture_grid, track};
use std::f64::consts::PI;

pub(crate) fn fracture(t: f64, aspect: f64) -> Vec<Layer> {
    let mut frame = vec![Layer::full(Source::New, 1.)];
    if t >= 0.93 {
        return frame;
    }
    if t < 0.035 {
        frame.push(Layer::full(Source::Old, 1.));
        return frame;
    }
    let impact = [0.47, 0.50];
    // Rays intersect the edges of the actual slide, then meet an irregular inner ring.
    // Adjacent shards share their original UV vertices even at the four corners.
    let perimeter: [P; 16] = [
        [0., 0.],
        [0.19, 0.],
        [0.43, 0.],
        [0.69, 0.],
        [1., 0.],
        [1., 0.24],
        [1., 0.48],
        [1., 0.74],
        [1., 1.],
        [0.78, 1.],
        [0.53, 1.],
        [0.22, 1.],
        [0., 1.],
        [0., 0.81],
        [0., 0.53],
        [0., 0.27],
    ];
    let inner = |i: usize| {
        let f = 0.31 + 0.34 * rnd(i as u64 + 23);
        std::array::from_fn(|axis| impact[axis] + (perimeter[i][axis] - impact[axis]) * f)
    };
    let split = ss(win(t, 0.08, 0.54));
    let time = (t - 0.07).max(0.);
    for (i, &edge) in perimeter.iter().enumerate() {
        let j = (i + 1) % perimeter.len();
        let (a, b) = (inner(i), inner(j));
        for (piece, uv) in [
            [impact, a, b],
            [a, edge, perimeter[j]],
            [a, perimeter[j], b],
        ]
        .into_iter()
        .enumerate()
        {
            let seed = (3 * i + piece) as u64;
            let center: P =
                std::array::from_fn(|axis| uv.iter().map(|p| p[axis]).sum::<f64>() / 3.);
            let drift = (rnd(seed + 431) - 0.5) * 0.074 * split;
            let gravity = (1.36 + 0.69 * rnd(seed + 119)) * time * time;
            let twist = (rnd(seed + 223) - 0.5) * 0.71 * split;
            let (sin, cos) = twist.sin_cos();
            let fold = 1. - (0.08 + 0.35 * rnd(seed + 61)) * split;
            let face = uv.map(|p| {
                let (x, y) = ((p[0] - center[0]) * aspect, (p[1] - center[1]) * fold);
                [
                    center[0] + drift + (cos * x - sin * y) / aspect,
                    center[1] + gravity + (sin * x + cos * y),
                ]
            });
            let shard = Layer::tri_uv(Source::Old, face, uv, [1.; 3], 1.);
            frame.push(shard);
            frame.push(shard.shade((1. - fold) * 0.25));
        }
    }
    frame
}

pub(crate) fn crush(t: f64) -> Vec<Layer> {
    let mut frame = vec![Layer::full(Source::New, 1.)];
    if t >= 0.99 {
        return frame;
    }
    // The folds first gather in place, hold their volume, and then fall together.
    let [cx, cy, rx, ry, fold] = track(
        t,
        &[
            (0., [0.5, 0.5, 0.5, 0.5, 0.]),
            (0.125, [0.518, 0.487, 0.486, 0.505, 0.14]),
            (0.25, [0.481, 0.513, 0.419, 0.439, 0.51]),
            (0.375, [0.493, 0.526, 0.308, 0.383, 0.82]),
            (0.5, [0.501, 0.523, 0.213, 0.304, 1.]),
            (0.625, [0.504, 0.535, 0.196, 0.29, 1.]),
            (0.75, [0.497, 0.693, 0.211, 0.289, 1.]),
            (0.875, [0.497, 0.977, 0.211, 0.282, 1.]),
            (1., [0.5, 1.47, 0.207, 0.28, 1.]),
        ],
    );
    let project = |[u, v]: P| {
        let (x, y) = (2. * u - 1., 2. * v - 1.);
        let cut_x = 1. - fold * (1. - (1. - 0.53 * y * y).sqrt());
        let cut_y = 1. - fold * (1. - (1. - 0.58 * x * x).sqrt());
        let crease = (13. * u - 7. * v).sin() * (8. * v + 3. * u).cos();
        let ridge = (12. * v + 4. * u).sin() * (11. * u + v).sin();
        let folded = [
            cx + rx * (x * cut_x + fold * (0.13 * crease - 0.16 * y + 0.033)),
            cy + ry * (y * cut_y + fold * (0.18 * ridge + 0.07)),
        ];
        (folded, fold * (0.10 + 0.16 * crease + 0.07 * ridge).max(0.))
    };
    // A crease can invert one half of a tile. A projective quad has a pole
    // in that case; two planar triangles keep both halves inside the paper.
    for row in 0..8 {
        for col in 0..9 {
            let uv = corners([
                col as f64 / 9.,
                row as f64 / 8.,
                (col + 1) as f64 / 9.,
                (row + 1) as f64 / 8.,
            ]);
            let vertices = uv.map(project);
            for indices in [[0, 1, 2], [0, 2, 3]] {
                let face = Layer::tri_uv(
                    Source::Old,
                    indices.map(|i| vertices[i].0),
                    indices.map(|i| uv[i]),
                    [1.; 3],
                    1.,
                );
                frame.push(face);
                let shade = indices.map(|i| vertices[i].1);
                frame.push(
                    face.shade(1.)
                        .faded([shade[0], shade[1], shade[2], shade[2]]),
                );
            }
        }
    }
    frame
}

fn mirror(frame: &mut [Layer], right: bool) {
    if right {
        for layer in frame.iter_mut().skip(1) {
            for point in layer.quad.iter_mut().chain(&mut layer.uv) {
                point[0] = 1. - point[0];
            }
            let [left, top, right, bottom] = layer.src;
            layer.src = [1. - right, top, 1. - left, bottom];
        }
    }
}

/// Peel from the lower free corner around a moving diagonal cylinder. The
/// unturned, curved and turned portions meet at the same positions and UVs.
pub(crate) fn peel(right: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let mut frame = vec![Layer::full(Source::New, 1.)];
    if t >= 0.85 {
        return frame;
    }
    let slope = 0.84 / aspect;
    let [middle] = track(
        t,
        &[
            (0., [1.32]),
            (0.125, [1.29]),
            (0.25, [1.10]),
            (0.375, [0.936]),
            (0.4375, [0.833]),
            (0.5, [0.725]),
            (0.5625, [0.559]),
            (0.625, [0.347]),
            (0.6875, [0.205]),
            (0.75, [0.055]),
            (0.8125, [-0.17]),
            (0.85, [-0.32]),
        ],
    );
    let radius = (0.025 + 0.055 * ss(win(t, 0.10, 0.60))) * 1.77 / aspect;
    let normal = [1., slope];
    let tangent_start = middle + 0.5 * slope - radius;
    let s = |p: P| p[0] + slope * p[1];
    let denom = 1. + (slope * aspect).powi(2);
    let project = |p: P| {
        let d = (s(p) - tangent_start).max(0.);
        let angle = (d / radius).min(PI);
        let shift = radius * angle.sin() - (d - PI * radius).max(0.) - d;
        [
            p[0] + shift / denom,
            p[1] + shift * slope * aspect * aspect / denom,
        ]
    };
    let square = corners(FULL);
    frame.extend(poly_layers(
        Source::Old,
        &clip(&square, normal, tangent_start),
        1.,
        &project,
    ));
    let cuts = std::iter::once(tangent_start)
        .chain((1..=10).map(|i| tangent_start + radius * PI * i as f64 / 10.))
        .chain(std::iter::once(1. + slope + 1.));
    let mut prev = tangent_start;
    for bound in cuts.skip(1) {
        if bound <= prev {
            continue;
        }
        let polygon = clip(&clip(&square, normal, bound), [-1., -slope], -prev);
        let angle = ((0.5 * (bound + prev) - tangent_start) / radius).clamp(0., PI);
        let dark = 0.13 * (1. - angle.cos()) + 0.10 * angle.sin();
        for facet in poly_layers(Source::Old, &polygon, 1., &project) {
            frame.push(facet);
            frame.push(facet.shade(dark));
        }
        prev = bound;
    }
    mirror(&mut frame, right);
    frame
}

pub(crate) fn turn(option: &str, t: f64) -> Vec<Layer> {
    let single = matches!(option, "singleL" | "singleR");
    let right = matches!(option, "r" | "singleR");
    let mut frame = vec![Layer::full(Source::New, 1.)];
    let [edge] = if single {
        track(
            t,
            &[
                (0., [1.]),
                (0.25, [1.]),
                (0.3125, [0.982]),
                (0.375, [0.954]),
                (0.4375, [0.895]),
                (0.5, [0.767]),
                (0.5625, [0.642]),
                (0.625, [0.517]),
                (0.6875, [0.33]),
                (0.75, [0.058]),
                (0.80, [-0.18]),
                (1., [-1.]),
            ],
        )
    } else {
        track(
            t,
            &[
                (0., [1.]),
                (0.125, [0.997]),
                (0.25, [0.915]),
                (0.375, [0.807]),
                (0.4375, [0.711]),
                (0.5, [0.561]),
                (0.5625, [0.352]),
                (0.625, [0.143]),
                (0.6875, [0.018]),
                (0.75, [0.]),
                (1., [0.]),
            ],
        )
    };
    if edge <= 0. {
        return frame;
    }
    if single {
        // A loose sheet slides past the binding. Its lifted edge grows in
        // perspective while the incoming page is already flat behind it.
        texture_grid(&mut frame, Source::Old, [0., 1.], 12, 1, |[u, v]| {
            let bend = (PI * u).sin() * ss(win(t, 0.10, 0.5));
            (
                [
                    edge - 1. + u - 0.03 * bend,
                    (v - 0.5) * (1. + 0.24 * bend) + 0.5,
                ],
                0.04 * bend,
            )
        });
    } else {
        frame.push(Layer::inplace(Source::Old, [0., 0., 0.5, 1.], 1.));
        let front = edge >= 0.5;
        let (source, range) = if front {
            (Source::Old, [0.5, 1.])
        } else {
            (Source::New, [0., 0.5])
        };
        texture_grid(&mut frame, source, range, 16, 1, |[u, v]| {
            let across = 2. * (u - 0.5).abs();
            let angle = ((edge - 0.5).abs() * 2.).clamp(0., 1.).acos();
            let bend = (PI * across).sin() * angle.sin();
            let x = 0.5 + (edge - 0.5) * across + if front { 0.022 * bend } else { -0.022 * bend };
            let y = (v - 0.5) * (1. + 0.22 * bend + 0.12 * across * angle.sin()) + 0.5;
            ([x, y], 0.10 * bend + 0.10 * across * angle.sin())
        });
    }
    mirror(&mut frame, right);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::layer_mesh;

    #[test]
    fn fully_gathered_crush_projects_inside_the_folded_sheet() {
        assert!(
            crush(0.5)
                .iter()
                .filter(|l| l.source == Source::Old)
                .all(|layer| {
                    let (vertices, _) = layer_mesh(layer);
                    vertices.iter().all(|v| {
                        (0.23..0.77).contains(&v.pos[0]) && (0.12..0.91).contains(&v.pos[1])
                    })
                })
        );
        let lowest = |t| {
            crush(t)
                .into_iter()
                .filter(|l| l.source == Source::Old)
                .flat_map(|l| l.quad.map(|p| p[1]))
                .fold(0., f64::max)
        };
        assert!(lowest(0.875) > lowest(0.5) + 0.35);
    }

    #[test]
    fn two_bound_pages_turn_about_the_same_spine() {
        let front = turn("l", 0.375);
        let back = turn("l", 0.625);
        assert!(
            front
                .iter()
                .skip(2)
                .any(|l| l.source == Source::Old && l.src[0] >= 0.5)
        );
        assert!(
            back.iter()
                .skip(2)
                .any(|l| l.source == Source::New && l.src[2] <= 0.5)
        );
        for (l, r) in back.iter().skip(1).zip(turn("r", 0.625).iter().skip(1)) {
            assert_eq!(l.source, r.source);
            for (&[x, y], &[right_x, right_y]) in l.quad.iter().zip(&r.quad) {
                assert!((1. - x - right_x).abs() < 1e-12 && y == right_y);
            }
        }
        assert!(
            turn("singleL", 0.625)
                .iter()
                .skip(1)
                .all(|l| l.source != Source::New)
        );
    }
}
