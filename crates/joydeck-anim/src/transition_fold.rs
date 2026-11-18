//! A paper dart and a flapping bird. Faces keep their source UVs as their
//! shared creases move, including when a wing turns inside the silhouette.
use crate::transition::{Layer, P, Source};
use crate::transition_geom::{ss, win};
use crate::transition_surface::track;

fn blend(a: P, b: P, t: f64) -> P {
    std::array::from_fn(|axis| a[axis] + (b[axis] - a[axis]) * t)
}

fn facet(frame: &mut Vec<Layer>, right: bool, vertices: [P; 3], uv: [P; 3], dark: f64) {
    let mirror = |[x, y]: P| [if right { 1. - x } else { x }, y];
    let face = Layer::tri_uv(
        Source::Old,
        vertices.map(mirror),
        uv.map(mirror),
        [1.; 3],
        1.,
    );
    frame.push(face);
    if dark > 0. {
        frame.push(face.shade(dark));
    }
}

pub(crate) fn airplane(right: bool, t: f64) -> Vec<Layer> {
    let mut frame = vec![Layer::full(Source::New, 1.)];
    if t >= 0.94 {
        return frame;
    }
    // Nose and two wing tips define the dart in slide coordinates. The tail
    // protrudes below the slide briefly before the dart is thrown.
    let [nx, ny, lx, ly, rx, ry] = track(
        t,
        &[
            (0., [0.5, 0., 0., 1., 1., 1.]),
            (0.125, [0.493, 0.035, 0.162, 1.025, 0.90, 0.966]),
            (0.25, [0.45, 0.163, 0.323, 1.065, 0.966, 0.803]),
            (0.375, [0.449, 0.325, 0.49, 1.06, 1.015, 0.65]),
            (0.5, [0.475, 0.426, 0.62, 1.042, 1.04, 0.644]),
            (0.625, [0.51, 0.46, 0.655, 0.989, 1.055, 0.673]),
            (0.78, [0.538, 0.509, 0.684, 1.01, 1.073, 0.684]),
            (0.82, [0.465, 0.373, 0.63, 0.954, 1.022, 0.56]),
            (0.875, [0.164, -0.038, 0.169, 0.245, 0.586, 0.09]),
            (0.94, [-0.32, -0.61, -0.26, -0.25, 0.085, -0.45]),
        ],
    );
    let nose = [nx, ny];
    let left = [lx, ly];
    let right_tip = [rx, ry];
    let crease = ss(win(t, 0., 0.22));
    let ridge = blend(left, right_tip, 0.5);
    let tail = [ridge[0] + 0.04 * crease, ridge[1] + 0.20 * crease];
    let uv = [[0.5, 0.], [0., 0.], [0., 1.], [0.5, 1.], [1., 1.], [1., 0.]];
    let mut positions = [nose, [0., 0.], left, tail, right_tip, [1., 0.]];
    // Two corner flaps close around the long diagonals. Their free corners
    // cross the crease, exposing their reverse sides on the centre panels.
    for (corner, tip) in [(1, left), (5, right_tip)] {
        let hinge = blend(nose, tip, 0.48);
        let closed = blend(hinge, tail, 0.36);
        positions[corner] = blend(uv[corner], closed, ss(win(t, 0., 0.145)));
    }
    for (indices, shade) in [
        ([0, 2, 3], 0.015),
        ([0, 3, 4], 0.10),
        ([0, 1, 2], 0.23),
        ([0, 4, 5], 0.12),
    ] {
        facet(
            &mut frame,
            right,
            indices.map(|i| positions[i]),
            indices.map(|i| uv[i]),
            shade * crease,
        );
    }
    frame
}

pub(crate) fn origami(right: bool, t: f64) -> Vec<Layer> {
    let mut frame = vec![Layer::full(Source::New, 1.)];
    if t >= 0.94 {
        return frame;
    }
    // Eight folds surround the central joint. Lift the two wing tips
    // independently while the folded bird accelerates off the upper edge.
    let joint_src = [0.5, 0.5];
    let contour = [
        [0., 0.],
        [0.36, 0.],
        [1., 0.],
        [1., 0.40],
        [1., 1.],
        [0.56, 1.],
        [0.30, 1.],
        [0., 0.62],
    ];
    let [jx, jy, width, height, flap] = track(
        t,
        &[
            (0., [0.50, 0.5, 1., 1., 0.]),
            (0.0625, [0.496, 0.49, 1., 1., 0.]),
            (0.125, [0.478, 0.366, 1., 1., 0.]),
            (0.25, [0.473, 0.354, 1., 1., 0.14]),
            (0.375, [0.483, 0.423, 1.06, 1.04, 0.43]),
            (0.50, [0.41, 0.451, 0.98, 1.08, 0.99]),
            (0.625, [0.21, 0.36, 0.82, 0.92, 1.]),
            (0.75, [0.024, 0.169, 0.58, 0.74, 0.87]),
            (0.875, [-0.097, -0.014, 0.44, 0.60, 0.76]),
            (0.94, [-0.28, -0.20, 0.38, 0.48, 0.64]),
        ],
    );
    let flying = [
        [-0.30, -0.11 - 0.04 * flap],
        [-0.13, 0.025],
        [0.075, -0.237 + 0.15 * flap],
        [0.096, 0.081 - 0.025 * flap],
        [0.335, 0.371 - 0.11 * flap],
        [0.009, 0.463 - 0.08 * flap],
        [-0.094 - 0.045 * flap, 0.487 + 0.17 * flap],
        [-0.115, 0.237],
    ];
    let folding = ss(win(t, 0., 0.139));
    let joint = blend(joint_src, [jx, jy], folding);
    let vertices: [P; 8] = std::array::from_fn(|i| {
        blend(
            contour[i],
            [jx + flying[i][0] * width, jy + flying[i][1] * height],
            folding,
        )
    });
    for (i, dark) in [0.12, 0.025, 0.17, 0.03, 0.14, 0.32, 0.24, 0.06]
        .into_iter()
        .enumerate()
    {
        let next = (i + 1) % contour.len();
        facet(
            &mut frame,
            right,
            [joint, vertices[i], vertices[next]],
            [joint_src, contour[i], contour[next]],
            dark * folding,
        );
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverted_flights_reflect_geometry_and_texture_together() {
        for build in [airplane, origami] {
            for t in [0.06, 0.25, 0.5, 0.875] {
                let left = build(false, t);
                let right = build(true, t);
                let panels: Vec<_> = left.iter().filter(|l| l.source == Source::Old).collect();
                assert!(panels.len() >= 4);
                for (a, b) in left.iter().skip(1).zip(right.iter().skip(1)) {
                    assert_eq!(a.source, b.source);
                    assert_eq!(a.quad[2], a.quad[3]);
                    for (p, q) in a.quad.iter().chain(&a.uv).zip(b.quad.iter().chain(&b.uv)) {
                        assert!((p[0] + q[0] - 1.).abs() < 1e-9 && p[1] == q[1]);
                    }
                }
            }
            assert_eq!(build(false, 0.96), [Layer::full(Source::New, 1.)]);
        }
    }
}
