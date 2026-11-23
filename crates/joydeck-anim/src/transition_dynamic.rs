//! Dynamic Content slide transitions: Pan, Ferris, Conveyor, Window and Flythrough.
//! Every layer animates slide backgrounds and foreground contents independently.
use crate::easing::ease;
use crate::transition::{FULL, Layer, Source, corners};
use crate::transition_stage::bifold;
use crate::transition_surface::{
    Camera3D, Vec3, direction_vec, quad_ease, rotate_2d, smoothstep, span, transform_rect,
};
use std::f64::consts::PI;

/// Foreground contents glide together across the slide while backgrounds cross-fade.
pub(crate) fn pan(option: &str, t: f64) -> Vec<Layer> {
    let [dx, dy] = direction_vec(option, [-1., 0.]);
    let s = ease(t, 0.2, 0.8, 0.);
    vec![
        Layer::full(Source::OldBack, 1.),
        Layer::full(Source::NewBack, s),
        Layer::rect(
            Source::OldContent,
            FULL,
            transform_rect(FULL, 1., [0.5, 0.5], [dx * s, dy * s]),
            1.,
        ),
        Layer::rect(
            Source::NewContent,
            FULL,
            transform_rect(FULL, 1., [0.5, 0.5], [dx * (s - 1.), dy * (s - 1.)]),
            1.,
        ),
    ]
}

/// Foreground contents turn on a wide Ferris wheel arc over cross-fading backgrounds.
pub(crate) fn ferris(right: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let sign = if right { -1. } else { 1. };
    let mut out = vec![
        Layer::full(Source::OldBack, 1.),
        Layer::full(Source::NewBack, smoothstep(t)),
    ];
    let depart_u = span(t, 0., 0.50);
    if depart_u < 1. {
        let drop = depart_u.powf(2.15);
        let dx = -sign * 0.45 * depart_u.powf(3.0);
        let dy = 0.90 * drop;
        let rect = transform_rect(FULL, 1., [0.5, 0.5], [dx, dy]);
        let pivot = [0.5 + dx, 0.5 + dy];
        let quad = corners(rect).map(|pt| rotate_2d(pt, pivot, -sign * 15. * drop, aspect));
        out.push(Layer::quad(Source::OldContent, FULL, quad, 1.));
    }
    let arrive = quad_ease(span(t, 0.50, 1.));
    if arrive > 0. {
        let k = 1. - arrive;
        let dx = -sign * 0.50 * k.powf(1.3);
        let dy = -0.75 * k;
        let rect = transform_rect(FULL, 1. - 0.25 * k, [0.5, 0.5], [dx, dy]);
        let pivot = [0.5 * (rect[0] + rect[2]), 0.5 * (rect[1] + rect[3])];
        let quad = corners(rect).map(|pt| rotate_2d(pt, pivot, -sign * 18. * k, aspect));
        out.push(Layer::quad(Source::NewContent, FULL, quad, 1.));
    }
    out
}

/// Foreground contents ride an angled 3-D conveyor belt while backgrounds cross-fade.
pub(crate) fn conveyor(right: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let cam = Camera3D {
        aspect,
        distance: 2.5,
    };
    let sign = if right { 1. } else { -1. };
    let tilt = smoothstep(span(t, 0., 0.35)) * (1. - smoothstep(span(t, 0.65, 1.)));
    let travel = quad_ease(span(t, 0.33, 0.75));
    let retreat = 0.45 * tilt;
    let yaw = -sign * 18. * tilt;
    let pivot = Vec3::new(0., 0., retreat);

    let mut out = vec![
        Layer::full(Source::OldBack, 1.),
        Layer::full(Source::NewBack, smoothstep(t)),
    ];
    for (source, offset_slides) in [
        (Source::OldContent, sign * travel),
        (Source::NewContent, sign * (travel - 1.)),
    ] {
        let shift_x = offset_slides * aspect * 1.05;
        let pts = cam
            .rect_corners(FULL)
            .map(|p| Vec3::new(p.x + shift_x, p.y, p.z + retreat).yaw_about(pivot, yaw));
        cam.push_card(&mut out, source, FULL, pts, 1., 0.);
    }
    out
}

/// Foreground contents part like bi-fold window panes over cross-fading backgrounds.
pub(crate) fn window(horz: bool, t: f64, aspect: f64) -> Vec<Layer> {
    bifold(true, horz, t, aspect)
}

/// Foreground contents fly past (`in`) or recede (`out`), optionally with bounce overshoot.
pub(crate) fn flythrough(option: &str, t: f64) -> Vec<Layer> {
    let bounce = option.ends_with("Bounce");
    let outward = option.starts_with("out");
    let s = quad_ease(t);
    let overshoot = if bounce {
        0.08 * (span(t, 0.58, 1.) * PI).sin()
    } else {
        0.
    };
    let old_alpha = 1. - smoothstep(span(t, 0., 0.60));
    let new_alpha = smoothstep(span(t, 0.30, 0.95));
    let (old_scale, new_scale) = if outward {
        (1. - 0.50 * s, 1.80 - 0.80 * s - overshoot)
    } else {
        (1. + 1.50 * s, 0.40 + 0.60 * s + overshoot)
    };
    vec![
        Layer::full(Source::OldBack, 1.),
        Layer::full(Source::NewBack, smoothstep(t)),
        Layer::rect(
            Source::NewContent,
            FULL,
            transform_rect(FULL, new_scale, [0.5, 0.5], [0., 0.]),
            new_alpha,
        ),
        Layer::rect(
            Source::OldContent,
            FULL,
            transform_rect(FULL, old_scale, [0.5, 0.5], [0., 0.]),
            old_alpha,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_transitions_emit_part_sources_only() {
        for layers in [
            pan("l", 0.5),
            ferris(false, 0.35, 16. / 9.),
            conveyor(true, 0.5, 16. / 9.),
            window(false, 0.4, 16. / 9.),
            flythrough("inBounce", 0.75),
        ] {
            assert!(!layers.is_empty());
            assert!(layers.iter().all(|l| l.source.is_part()));
        }
    }

    #[test]
    fn flythrough_bounce_differs_from_plain_late_in_transition() {
        assert_ne!(flythrough("in", 0.78), flythrough("inBounce", 0.78));
        assert_ne!(flythrough("out", 0.78), flythrough("outBounce", 0.78));
    }
}
