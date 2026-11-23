//! 3-D staged slide transitions: Switch, Flip, Gallery, Doors and Zoom/Newsflash.
use crate::transition::{FULL, Layer, R, Source, corners};
use crate::transition_surface::{
    Camera3D, Vec3, quad_ease, rotate_2d, smoothstep, span, transform_rect,
};
use std::f64::consts::PI;

/// Switching cards: both slides swing out to opposite sides and exchange depth at `t = 0.5`.
pub(crate) fn switch(right: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let cam = Camera3D {
        aspect,
        distance: 2.5,
    };
    let sign = if right { -1. } else { 1. };
    let u = 0.5 * (t + smoothstep(t));
    let arc = (u * PI).sin();
    let shift = 0.50 * arc * aspect;
    let origin = Vec3::new(0., 0., 0.);

    let old_pts = cam.rect_corners(FULL).map(|p| {
        let q = p.yaw_about(origin, sign * 14. * arc);
        Vec3::new(q.x - sign * shift, q.y, q.z + 0.12 * arc)
    });
    let new_pts = cam.rect_corners(FULL).map(|p| {
        let q = p.yaw_about(origin, -sign * 14. * arc);
        Vec3::new(q.x + sign * shift, q.y, q.z + 0.12 * arc)
    });

    let mut out = vec![Layer::full(Source::Black, 1.)];
    let old_shade = Camera3D::obliquity_shade(old_pts, 0.30);
    let new_shade = Camera3D::obliquity_shade(new_pts, 0.30);
    if u < 0.5 {
        cam.push_card(&mut out, Source::New, FULL, new_pts, 1., new_shade);
        cam.push_card(&mut out, Source::Old, FULL, old_pts, 1., old_shade);
    } else {
        cam.push_card(&mut out, Source::Old, FULL, old_pts, 1., old_shade);
        cam.push_card(&mut out, Source::New, FULL, new_pts, 1., new_shade);
    }
    out
}

/// 3-D card flip about the vertical center axis, revealing the un-mirrored new slide on back.
pub(crate) fn flip(right: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let cam = Camera3D {
        aspect,
        distance: 2.5,
    };
    let s = quad_ease(t);
    let sign = if right { 1. } else { -1. };
    let deg = 180. * s * sign;
    let retreat = 0.58 * (s * PI).sin();
    let origin = Vec3::new(0., 0., 0.);
    let pts = cam.rect_corners(FULL).map(|p| {
        let q = p.yaw_about(origin, deg);
        Vec3::new(q.x, q.y, q.z + retreat)
    });
    let mut out = vec![Layer::full(Source::Black, 1.)];
    if s < 0.5 {
        let shade = Camera3D::obliquity_shade(pts, 0.50);
        cam.push_card(&mut out, Source::Old, FULL, pts, 1., shade);
    } else {
        let back = [pts[1], pts[0], pts[3], pts[2]];
        let shade = Camera3D::obliquity_shade(back, 0.50);
        cam.push_card(&mut out, Source::New, FULL, back, 1., shade);
    }
    out
}

/// Slides mounted on an angled gallery wall with soft bottom floor reflections.
pub(crate) fn gallery(right: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let cam = Camera3D {
        aspect,
        distance: 2.5,
    };
    let sign = if right { 1. } else { -1. };
    let tilt = smoothstep(span(t, 0., 0.35)) * (1. - smoothstep(span(t, 0.70, 1.)));
    let travel = quad_ease(span(t, 0.30, 0.80));
    let retreat = 0.70 * tilt;
    let yaw = -sign * 22. * tilt;
    let gap = 0.15;
    let pivot = Vec3::new(0., 0., retreat);

    let mut out = vec![Layer::full(Source::Black, 1.)];
    for (source, offset_slides) in [
        (Source::Old, sign * travel),
        (Source::New, sign * (travel - 1.)),
    ] {
        let shift_x = offset_slides * (aspect + gap);
        let pts = cam
            .rect_corners(FULL)
            .map(|p| Vec3::new(p.x + shift_x, p.y, p.z + retreat).yaw_about(pivot, yaw));
        let refl_pts = [
            pts[3],
            pts[2],
            Vec3::new(pts[2].x, pts[2].y + 0.25, pts[2].z),
            Vec3::new(pts[3].x, pts[3].y + 0.25, pts[3].z),
        ];
        if tilt > 1e-4 {
            out.push(Layer {
                source,
                src: [0., 0.75, 1., 1.],
                quad: cam.project_quad(refl_pts),
                uv: [[0., 1.], [1., 1.], [1., 0.75], [0., 0.75]],
                alpha: 0.30 * tilt,
                fade: [1., 1., 0., 0.],
            });
        }
        cam.push_card(&mut out, source, FULL, pts, 1., 0.);
    }
    out
}

/// Shared bi-fold swinging panel opening used by `doors` (whole slide) and `window` (content only).
pub(crate) fn bifold(content_only: bool, horz: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let cam = Camera3D {
        aspect,
        distance: 2.2,
    };
    let open_u = span(t, 0.05, 0.68);
    let open = open_u * open_u;
    let approach = quad_ease(span(t, 0., 0.72));
    let (old_src, new_src) = if content_only {
        (Source::OldContent, Source::NewContent)
    } else {
        (Source::Old, Source::New)
    };
    let mut out = if content_only {
        vec![
            Layer::full(Source::OldBack, 1.),
            Layer::full(Source::NewBack, smoothstep(t)),
        ]
    } else {
        vec![Layer::full(Source::Black, 1.)]
    };

    let leaves: [(R, f64); 2] = if horz {
        [([0., 0., 1., 0.5], -1.), ([0., 0.5, 1., 1.], 1.)]
    } else {
        [([0., 0., 0.5, 1.], -1.), ([0.5, 0., 1., 1.], 1.)]
    };
    if open < 1. {
        let leaf_alpha = if content_only {
            1. - smoothstep(span(t, 0.20, 0.60))
        } else {
            1.
        };
        let leaf_shade = if content_only { 0. } else { 0.68 * open };
        let angle = 32. * open;
        let slide = 0.55 * open;
        for (rect, sgn) in leaves {
            let hinge = if horz {
                cam.on_plane(0.5, if sgn < 0. { 0. } else { 1. })
            } else {
                cam.on_plane(if sgn < 0. { 0. } else { 1. }, 0.5)
            };
            let pts = cam.rect_corners(rect).map(|p| {
                if horz {
                    let q = p.pitch_about(hinge, sgn * angle);
                    Vec3::new(q.x, q.y + sgn * slide, q.z)
                } else {
                    let q = p.yaw_about(hinge, -sgn * angle);
                    Vec3::new(q.x + sgn * slide * aspect, q.y, q.z)
                }
            });
            cam.push_card(&mut out, old_src, rect, pts, leaf_alpha, leaf_shade);
        }
    }

    let scale = 0.50 + 0.50 * approach;
    let fade_in = if content_only {
        smoothstep(t)
    } else {
        smoothstep(span(t, 0.05, 0.62))
    };
    out.push(Layer::rect(
        new_src,
        FULL,
        transform_rect(FULL, scale, [0.5, 0.5], [0., 0.]),
        fade_in,
    ));
    out
}

/// Whole-slide `doors` transition (`vert` / `horz`).
pub(crate) fn doors(horz: bool, t: f64, aspect: f64) -> Vec<Layer> {
    bifold(false, horz, t, aspect)
}

/// Whole-slide zoom (`in`, `out`) and clockwise spin (`rotate` / `p:newsflash`).
pub(crate) fn zoom(option: &str, t: f64, aspect: f64) -> Vec<Layer> {
    match option {
        "rotate" => {
            let u = span(t, 0.08, 0.98);
            let scale = 0.08 + 0.92 * u;
            let degrees = 360. * (1. - u);
            let rect = transform_rect(FULL, scale, [0.5, 0.5], [0., 0.]);
            let quad = corners(rect).map(|pt| rotate_2d(pt, [0.5, 0.5], degrees, aspect));
            vec![
                Layer::full(Source::Old, 1.),
                Layer::quad(
                    Source::New,
                    FULL,
                    quad,
                    0.35 + 0.65 * smoothstep(span(t, 0., 0.45)),
                ),
            ]
        }
        "out" => {
            let u = span(t, 0., 0.80);
            let settle = 1. - (1. - u) * (1. - u);
            let shrink = 1. - (1. - t) * (1. - t);
            vec![
                Layer::rect(
                    Source::New,
                    FULL,
                    transform_rect(FULL, 1.90 - 0.90 * settle, [0.5, 0.5], [0., 0.]),
                    1.,
                ),
                Layer::rect(
                    Source::Old,
                    FULL,
                    transform_rect(FULL, 1. - 0.50 * shrink, [0.5, 0.5], [0., 0.]),
                    1. - smoothstep(span(t, 0., 0.95)),
                ),
            ]
        }
        _ => vec![
            Layer::full(Source::Black, 1.),
            Layer::rect(
                Source::Old,
                FULL,
                transform_rect(FULL, 1. + 0.30 * smoothstep(t), [0.5, 0.5], [0., 0.]),
                1. - smoothstep(span(t, 0., 0.90)),
            ),
            Layer::rect(
                Source::New,
                FULL,
                transform_rect(
                    FULL,
                    0.50 + 0.50 * smoothstep(span(t, 0.15, 0.95)),
                    [0.5, 0.5],
                    [0., 0.],
                ),
                smoothstep(span(t, 0., 0.85)),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_switches_to_unmirrored_new_face_after_midpoint() {
        let early = flip(false, 0.25, 16. / 9.);
        let late = flip(false, 0.75, 16. / 9.);
        assert!(early.iter().any(|l| l.source == Source::Old));
        assert!(late.iter().any(|l| l.source == Source::New));
        let new_card = late.iter().find(|l| l.source == Source::New);
        assert!(new_card.is_some_and(|l| l.quad[1][0] > l.quad[0][0]));
    }

    #[test]
    fn gallery_reflection_stays_bounded_and_subtle() {
        let frame = gallery(false, 0.5, 16. / 9.);
        let refls: Vec<_> = frame
            .iter()
            .filter(|l| l.fade == [1., 1., 0., 0.])
            .collect();
        assert_eq!(refls.len(), 2);
        assert!(refls.iter().all(|l| l.alpha > 0. && l.alpha <= 0.31));
    }
}
