//! DrawingML arcTo (§20.1.9.1). Angles describe rays from the ellipse centre.
//! Supply cubic segments to the caller in its own coordinate space.

use std::f64::consts::{FRAC_PI_2, TAU};

pub fn cubics(
    start: [f64; 2],
    radii: [f64; 2],
    angles: [f64; 2],
    mut emit: impl FnMut([[f64; 2]; 3]),
) -> [f64; 2] {
    let [rx, ry] = radii;
    if start
        .into_iter()
        .chain(radii)
        .chain(angles)
        .any(|v| !v.is_finite())
        || rx.abs() < 1e-9
        || ry.abs() < 1e-9
        || angles[1] == 0.0
    {
        return start;
    }
    let ray = (angles[0] / 60_000.).to_radians();
    let sweep = (angles[1] / 60_000.)
        .to_radians()
        .clamp(-2. * TAU, 2. * TAU);
    let parameter = |theta: f64| (rx * theta.sin()).atan2(ry * theta.cos());
    let mut at = parameter(ray);
    let turn = if sweep.abs() >= TAU - 1e-9 {
        TAU.copysign(sweep)
    } else {
        let diff = parameter(ray + sweep) - at;
        if sweep > 0. && diff < 0. {
            diff + TAU
        } else if sweep < 0. && diff > 0. {
            diff - TAU
        } else {
            diff
        }
    };
    let centre = [start[0] - rx * at.cos(), start[1] - ry * at.sin()];
    let steps = (turn.abs() / FRAC_PI_2).ceil().clamp(1., 4.) as u32;
    let step = turn / f64::from(steps);
    let k = 4. / 3. * (step / 4.).tan();
    let mut p = start;
    for _ in 0..steps {
        let next = at + step;
        let (sa, ca) = at.sin_cos();
        let (sb, cb) = next.sin_cos();
        let end = [centre[0] + rx * cb, centre[1] + ry * sb];
        let control1 = [p[0] - k * rx * sa, p[1] + k * ry * ca];
        let control2 = [end[0] + k * rx * sb, end[1] - k * ry * cb];
        emit([control1, control2, end]);
        p = end;
        at = next;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_ellipse_controls_and_signed_sweeps() {
        for sign in [1., -1.] {
            let mut segments = Vec::new();
            let end = cubics([40., 3.], [30., 10.], [0., sign * 5_400_000.], |s| {
                segments.push(s)
            });
            assert_eq!(segments.len(), 1);
            let k = 4. * (2_f64.sqrt() - 1.) / 3.;
            let expected = [
                [40., 3. + sign * k * 10.],
                [10. + k * 30., 3. + sign * 10.],
                [10., 3. + sign * 10.],
            ];
            for (a, b) in segments[0].iter().zip(expected) {
                assert!(a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9));
            }
            assert_eq!(end, segments[0][2]);
        }
    }

    #[test]
    fn ray_angles_and_degenerate_arcs_obey_bounds() {
        let offset = 30. / 10_f64.sqrt();
        let from = [offset, offset];
        let end = cubics(from, [30., 10.], [2_700_000., 5_400_000.], |_| {});
        assert!((end[0] + offset).abs() < 1e-9 && (end[1] - offset).abs() < 1e-9);
        for sweep in [-43_200_000., 21_600_000.] {
            let mut count = 0;
            let end = cubics(from, [30., 10.], [2_700_000., sweep], |_| count += 1);
            assert_eq!(count, 4);
            assert!(from.iter().zip(end).all(|(a, b)| (a - b).abs() < 1e-9));
        }
        for (r, a) in [
            ([2., 0.], [0., 1.]),
            ([2., 3.], [0., 0.]),
            ([f64::NAN, 3.], [0., 1.]),
        ] {
            let mut count = 0;
            assert_eq!(cubics(from, r, a, |_| count += 1), from);
            assert_eq!(count, 0);
        }
    }
}
