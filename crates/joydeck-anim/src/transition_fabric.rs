//! Projected sheet and cloth presets. Motion is measured in fractions of the
//! slide, including at 4:3. The incoming texture stays fixed behind the curtain.
use crate::transition::{FULL, Layer, Source};
use crate::transition_surface::{texture_grid, track};
use std::f64::consts::PI;

pub(crate) fn fall_over(right: bool, t: f64) -> Vec<Layer> {
    let mut layers = vec![Layer::full(Source::New, 1.)];
    if t >= 0.90 {
        return layers;
    }
    // The upper edge narrows and accelerates downward. The lower corners
    // slide apart as they pass the bottom edge of the viewport.
    let [left, top_left, right_edge, top_right, foot, darkness] = track(
        t,
        &[
            (0., [0., 0., 1., 0., 0., 0.]),
            (0.25, [0.02, 0.025, 0.996, 0.005, 0.006, 0.]),
            (0.375, [0.039, 0.048, 0.979, 0.024, 0.009, 0.005]),
            (0.5, [0.072, 0.104, 0.953, 0.062, 0.015, 0.02]),
            (0.625, [0.125, 0.236, 0.906, 0.161, 0.025, 0.06]),
            (0.75, [0.186, 0.517, 0.846, 0.417, 0.032, 0.21]),
            (0.8125, [0.203, 0.744, 0.818, 0.653, 0.026, 0.36]),
            (0.875, [0.214, 1.065, 0.807, 0.935, 0.017, 0.51]),
            (0.90, [0.22, 1.19, 0.80, 1.07, 0.01, 0.55]),
        ],
    );
    let mut quad = [
        [left, top_left],
        [right_edge, top_right],
        [1. + foot * 0.5, 1.],
        [foot, 1. + foot],
    ];
    if right {
        quad = [quad[1], quad[0], quad[3], quad[2]].map(|[x, y]| [1. - x, y]);
    }
    let sheet = Layer::quad(Source::Old, FULL, quad, 1.);
    layers.push(sheet);
    if darkness > 0. {
        layers.push(sheet.shade(darkness));
    }
    layers
}

pub(crate) fn drape(right: bool, t: f64) -> Vec<Layer> {
    let mut layers = vec![Layer::full(Source::Old, 1.)];
    if t <= 0.29 {
        return layers;
    }
    // Separate free corners drop at different speeds. After the first contact
    // the hem swings back, narrowing at the sides before it comes to rest.
    let [left, right_edge, gather, curved, shade] = track(
        t,
        &[
            (0.29, [0., 0., 0., 0.22, 0.62]),
            (0.3125, [0.044, 0.023, 0., 0.22, 0.43]),
            (0.375, [0.25, 0.27, 0., 0.22, 0.16]),
            (0.4375, [0.51, 0.58, 0., 0.18, 0.12]),
            (0.5, [0.90, 0.99, 0., 0.05, 0.06]),
            (0.54, [1.09, 1.10, -0.018, 0., 0.015]),
            (0.625, [0.935, 0.987, 0.049, 0.017, 0.012]),
            (0.69, [0.921, 0.975, 0.060, 0.020, 0.020]),
            (0.75, [0.931, 0.975, 0.050, 0.018, 0.018]),
            (0.875, [0.978, 0.992, 0.017, 0.008, 0.009]),
            (1., [1., 1., 0., 0., 0.]),
        ],
    );
    texture_grid(&mut layers, Source::New, [0., 1.], 5, 6, |[u, v]| {
        let x = if right { 1. - u } else { u };
        let height = left + (right_edge - left) * x;
        let inset = gather * v * (1. - 1.55 * x);
        let y = height * (v + curved * (PI * v).sin());
        let darkness = shade * (0.35 + 1.30 * v.powi(3));
        ([if right { u - inset } else { u + inset }, y], darkness)
    });
    layers
}

pub(crate) fn curtains(t: f64) -> Vec<Layer> {
    let mut layers = vec![Layer::full(Source::New, 1.)];
    if t >= 0.86 {
        return layers;
    }
    // The hems lag the header, rebound toward the middle, then both panels
    // are hauled up and out. Their top and bottom widths cross twice.
    let [top, middle, hem, travel, lift] = track(
        t,
        &[
            (0., [0.5, 0.5, 0.5, 0., 0.]),
            (0.125, [0.396, 0.44, 0.482, 0., 0.]),
            (0.25, [0.308, 0.316, 0.323, 0., 0.]),
            (0.375, [0.288, 0.255, 0.185, 0., 0.]),
            (0.44, [0.296, 0.275, 0.205, 0., 0.]),
            (0.5, [0.299, 0.298, 0.285, 0., 0.]),
            (0.5625, [0.294, 0.339, 0.367, 0.005, 0.]),
            (0.625, [0.254, 0.346, 0.402, 0.04, 0.035]),
            (0.6875, [0.17, 0.285, 0.355, 0.13, 0.10]),
            (0.75, [0.08, 0.18, 0.242, 0.22, 0.08]),
            (0.8125, [-0.008, 0.055, 0.078, 0.32, 0.02]),
            (0.86, [-0.14, -0.11, -0.09, 0.43, 0.]),
        ],
    );
    let gathered = (4. * t).min(1.);
    for right in [false, true] {
        let region = if right { [0.5, 1.] } else { [0., 0.5] };
        texture_grid(&mut layers, Source::Old, region, 8, 6, |[u, v]| {
            let s = (if right { 1. - u } else { u }) * 2.;
            let width = (1. - v) * top + v * hem + 4. * v * (1. - v) * (middle - (top + hem) / 2.);
            let outside = -0.045 * gathered - travel;
            let fold = (s * 4. * PI).sin();
            let x = outside + s * (width - outside) + 0.007 * gathered * fold;
            let y = v * (1. + 0.04 * gathered) - 0.02 * gathered - lift * s * v;
            (
                [if right { 1. - x } else { x }, y],
                gathered * (0.04 + 0.11 * fold * fold),
            )
        });
    }
    layers
}

/// A gust reaches the free edge and sets a travelling ripple across the sheet.
/// The two file directions are reflections of the geometry, with unreflected UVs.
pub(crate) fn wind(right: bool, t: f64) -> Vec<Layer> {
    if t >= 0.82 {
        return vec![Layer::full(Source::New, 1.)];
    }
    let mut layers = vec![Layer::full(Source::New, 1.)];
    let [travel, lift, shear, amplitude] = track(
        t,
        &[
            (0., [0., 0., 0., 0.]),
            (0.125, [0., 0.005, 0., 0.008]),
            (0.25, [0.007, 0.032, 0.013, 0.028]),
            (0.3125, [0.038, 0.058, 0.02, 0.043]),
            (0.375, [0.106, 0.094, 0.04, 0.068]),
            (0.46, [0.238, 0.17, 0.072, 0.081]),
            (0.50, [0.335, 0.216, 0.10, 0.096]),
            (0.625, [0.63, 0.35, 0.11, 0.067]),
            (0.75, [0.934, 0.49, 0.065, 0.055]),
            (0.82, [1.16, 0.61, 0.05, 0.04]),
        ],
    );
    texture_grid(&mut layers, Source::Old, [0., 1.], 9, 4, |[u, v]| {
        let across = if right { 1. - u } else { u };
        let phase = PI * (4.2 * across - 1.2 * v - 2.5 * t);
        let ripple = amplitude * phase.sin();
        let x = across - travel - shear * (v - 0.5) + 0.65 * ripple;
        let y = v - lift + ripple * (0.55 + 0.45 * v);
        let shadow = amplitude * (0.35 + 0.65 * phase.cos()).max(0.);
        ([if right { 1. - x } else { x }, y], shadow)
    });
    layers
}

/// A central pull lifts the fabric before its free corners follow. Gathering
/// the hem narrows the outgoing texture as the whole slide rises offscreen.
pub(crate) fn prestige(t: f64) -> Vec<Layer> {
    if t <= 0.35 {
        return vec![Layer::full(Source::Old, 1.)];
    }
    let mut layers = vec![Layer::full(Source::New, 1.)];
    if t >= 0.82 {
        return layers;
    }
    let [lift, pinch, gather, twist, folds] = track(
        t,
        &[
            (0.35, [0., 0., 0., 0., 0.]),
            (0.42, [-0.01, 0.006, 0., 0., 0.]),
            (0.50, [-0.044, 0.10, 0.012, 0.020, 0.015]),
            (0.56, [-0.025, 0.22, 0.057, 0.043, 0.04]),
            (0.5833, [0.01, 0.29, 0.099, 0.064, 0.055]),
            (0.625, [0.15, 0.38, 0.17, 0.10, 0.085]),
            (0.6875, [0.56, 0.36, 0.31, 0.16, 0.10]),
            (0.75, [0.96, 0.19, 0.53, 0.13, 0.08]),
            (0.82, [1.36, 0.12, 0.66, 0.10, 0.04]),
        ],
    );
    texture_grid(&mut layers, Source::Old, [0., 1.], 12, 4, |[u, v]| {
        let across = u - 0.5;
        let center = (PI * u).sin().powi(2);
        let pleat = (4. * PI * u + 0.3 * v).cos();
        let x = 0.5 + across * (1. + gather * (0.25 - 1.85 * v));
        let y = v - lift - pinch * center + twist * across - folds * pleat * v;
        let shadow = folds * (0.5 + 0.5 * (6. * PI * u).cos()) * (0.5 + 0.5 * v);
        ([x, y], shadow)
    });
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falling_edge_and_texture_keep_their_handedness() {
        let left = fall_over(false, 0.75)[1];
        let right = fall_over(true, 0.75)[1];
        assert_eq!(left.uv, right.uv);
        for (a, b) in left
            .quad
            .into_iter()
            .zip([1, 0, 3, 2].map(|i| right.quad[i]))
        {
            assert!((a[0] + b[0] - 1.).abs() < 1e-12);
            assert_eq!(a[1], b[1]);
        }
        assert!(left.quad[0][1] > left.quad[1][1]);
        assert_eq!(fall_over(false, 0.91), [Layer::full(Source::New, 1.)]);
    }

    #[test]
    fn fabric_remains_attached_at_the_top_and_parts_at_the_middle() {
        let frame = drape(false, 0.375);
        assert_eq!(frame[0].source, Source::Old);
        let incoming: Vec<_> = frame
            .iter()
            .filter(|layer| layer.source == Source::New)
            .collect();
        assert!(
            incoming
                .last()
                .is_some_and(|tile| (0.25..0.30).contains(&tile.quad[2][1]))
        );
        for tile in incoming.iter().filter(|tile| tile.src[1] == 0.) {
            assert_eq!(tile.quad[0][1], 0.);
            assert_eq!(tile.quad[1][1], 0.);
        }
        for tile in curtains(0.375)
            .iter()
            .filter(|tile| tile.source == Source::Old)
        {
            assert!(tile.quad.iter().all(|p| p[0] < 0.4) || tile.quad.iter().all(|p| p[0] > 0.6));
        }
    }

    #[test]
    fn gusts_keep_texture_orientation_and_the_lift_starts_at_the_center() {
        let tiles = |right| {
            wind(right, 0.5)
                .into_iter()
                .filter(|l| l.source == Source::Old)
                .collect::<Vec<_>>()
        };
        let left = tiles(false);
        let right = tiles(true);
        assert_eq!(left.len(), 36);
        assert_eq!(right.len(), left.len());
        for (row, reflected) in left.chunks(9).zip(right.chunks(9)) {
            for (a, b) in row.iter().zip(reflected.iter().rev()) {
                for (i, j) in [1, 0, 3, 2].into_iter().enumerate() {
                    assert!((a.quad[i][0] + b.quad[j][0] - 1.).abs() < 1e-12);
                    assert!((a.quad[i][1] - b.quad[j][1]).abs() < 1e-12);
                    assert!((a.uv[i][0] + b.uv[j][0] - 1.).abs() < 1e-12);
                }
            }
        }
        let hem: Vec<_> = prestige(0.625)
            .into_iter()
            .filter(|l| l.source == Source::Old && l.src[3] == 1.)
            .collect();
        assert_eq!(hem.len(), 12);
        assert!(hem[6].quad[3][1] < hem[0].quad[3][1] - 0.20);
        assert_eq!(prestige(0.2), [Layer::full(Source::Old, 1.)]);
        assert_eq!(wind(true, 0.83), prestige(0.83));
        assert_eq!(prestige(0.83), [Layer::full(Source::New, 1.)]);
    }
}
