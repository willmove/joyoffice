//! Particle, honeycomb, glitter and shred slide transitions.
//! All meshes keep bounded triangle counts across extreme aspect ratios.
use crate::transition::{Layer, P, Source, corners};
use crate::transition_surface::{
    Camera3D, Vec3, direction_vec, quad_ease, rotate_2d, smoothstep, span, transform_rect,
    unit_hash,
};
use std::f64::consts::{PI, TAU};

const SQRT3: f64 = 1.732_050_807_568_877_2;

/// Hexagonal lattice breakup: outgoing cells fly toward the viewer along dark seams,
/// then incoming cells pop in and close the lattice cleanly at every aspect ratio.
pub(crate) fn honeycomb(t: f64, aspect: f64) -> Vec<Layer> {
    let cols = 10_usize;
    let cell_w = 1. / cols as f64;
    let rx = cell_w / SQRT3;
    let natural_ry = rx * aspect;
    // Clamp row count for pathological narrow aspects while guaranteeing complete [0, 1] coverage.
    let rows = (((1. / (1.5 * natural_ry)).ceil() as usize) + 2).clamp(3, 24);
    let ry = if rows == 24 {
        (1. / (1.5 * (rows - 2) as f64)).max(natural_ry)
    } else {
        natural_ry
    };
    let seam = smoothstep(span(t, 0., 0.13)) * (1. - smoothstep(span(t, 0.84, 1.)));
    let gap = 1. - 0.11 * seam;

    let hex_ring = |cx: f64, cy: f64, scale: f64| -> [P; 6] {
        std::array::from_fn(|k| {
            let angle = TAU * k as f64 / 6. + PI / 6.;
            let (sin, cos) = angle.sin_cos();
            [cx + cos * rx * scale, cy + sin * ry * scale]
        })
    };

    let mut out = vec![Layer::full(Source::Black, 1.)];
    for row in 0..rows {
        let cy = row as f64 * 1.5 * ry;
        let stagger = if row % 2 == 1 { 0.5 } else { 0. };
        for col in 0..=cols {
            let cx = (col as f64 + stagger) * cell_w;
            let id = (row * 32 + col) as u64;
            let src_ring = hex_ring(cx, cy, 1.);
            let out_begin = 0.09 + 0.22 * unit_hash(id + 7);
            let out_u = span(t, out_begin, out_begin + 0.14);
            let out_k = out_u * out_u;
            let in_begin = 0.36 + 0.42 * unit_hash(id + 19);
            let in_u = span(t, in_begin, in_begin + 0.12);
            let in_k = 1. - (1. - in_u) * (1. - in_u);

            let emit_hex = |layers: &mut Vec<Layer>, source: Source, scale: f64, alpha: f64| {
                let dst_ring = hex_ring(cx, cy, scale);
                for k in 0..6 {
                    let next = (k + 1) % 6;
                    layers.push(Layer::tri_uv(
                        source,
                        [[cx, cy], dst_ring[k], dst_ring[next]],
                        [[cx, cy], src_ring[k], src_ring[next]],
                        [1.; 3],
                        alpha,
                    ));
                }
            };

            if t < 0.52 && out_k < 1. {
                emit_hex(
                    &mut out,
                    Source::Old,
                    gap * (1. + 2.8 * out_k),
                    1. - out_k * out_k,
                );
            }
            if in_k > 0. {
                emit_hex(&mut out, Source::New, gap * (0.28 + 0.72 * in_k), 1.);
            }
        }
    }
    out
}

/// Sparkling sweep of flipping tiles (`l`/`r`/`u`/`d` square/diamond tiles;
/// `hexL`/`hexR`/`hexU`/`hexD` true hexagonal tiles).
pub(crate) fn glitter(option: &str, t: f64, aspect: f64) -> Vec<Layer> {
    let hex_mode = option.starts_with("hex");
    let dir_code = option.trim_start_matches("hex").to_ascii_lowercase();
    let sweep = |cx: f64, cy: f64| -> f64 {
        match dir_code.as_str() {
            "r" => 1. - cx,
            "u" => cy,
            "d" => 1. - cy,
            _ => cx,
        }
    };
    let vertical_axis = matches!(dir_code.as_str(), "l" | "r" | "");
    let cam = Camera3D {
        aspect,
        distance: 3.0,
    };
    let mut out = vec![Layer::full(Source::Old, 1.)];

    if hex_mode {
        let cols = 18_usize;
        let cell_w = 1. / cols as f64;
        let rx = cell_w / SQRT3;
        let natural_ry = rx * aspect;
        let rows = (((1. / (1.5 * natural_ry)).ceil() as usize) + 2).clamp(4, 18);
        let ry = if rows == 18 {
            (1. / (1.5 * (rows - 2) as f64)).max(natural_ry)
        } else {
            natural_ry
        };
        for row in 0..rows {
            let cy = row as f64 * 1.5 * ry;
            let stagger = if row % 2 == 1 { 0.5 } else { 0. };
            for col in 0..=cols {
                let cx = (col as f64 + stagger) * cell_w;
                let id = (row * 64 + col) as u64;
                let start = 0.64 * sweep(cx, cy).clamp(0., 1.) + 0.13 * unit_hash(id + 101);
                let k = span(t, start, start + 0.23);
                if k <= 0. {
                    continue;
                }
                let src_ring: [P; 6] = std::array::from_fn(|i| {
                    let ang = TAU * i as f64 / 6. + PI / 6.;
                    let (s, c) = ang.sin_cos();
                    [cx + c * rx, cy + s * ry]
                });
                if k >= 1. {
                    for i in 0..6 {
                        let next = (i + 1) % 6;
                        out.push(Layer::tri_uv(
                            Source::New,
                            [[cx, cy], src_ring[i], src_ring[next]],
                            [[cx, cy], src_ring[i], src_ring[next]],
                            [1.; 3],
                            1.,
                        ));
                    }
                    continue;
                }
                let (source, deg) = if k < 0.5 {
                    (Source::Old, 180. * k)
                } else {
                    (Source::New, 180. * (k - 1.))
                };
                let pivot = cam.on_plane(cx, cy);
                let center_p = cam.project(pivot);
                let dst_ring: [P; 6] = src_ring.map(|pt| {
                    let v = cam.on_plane(pt[0], pt[1]);
                    let turned = if vertical_axis {
                        v.yaw_about(pivot, deg)
                    } else {
                        v.pitch_about(pivot, deg)
                    };
                    cam.project(turned)
                });
                let glint = (1. - (2. * k - 1.).abs()).powi(3) * 0.55;
                for i in 0..6 {
                    let next = (i + 1) % 6;
                    out.push(Layer::tri_uv(
                        Source::Black,
                        [[cx, cy], src_ring[i], src_ring[next]],
                        [[cx, cy], src_ring[i], src_ring[next]],
                        [1.; 3],
                        1.,
                    ));
                    let face = Layer::tri_uv(
                        source,
                        [center_p, dst_ring[i], dst_ring[next]],
                        [[cx, cy], src_ring[i], src_ring[next]],
                        [1.; 3],
                        1.,
                    );
                    out.push(face);
                    if glint > 0.02 {
                        out.push(Layer::tri_uv(
                            Source::White,
                            [center_p, dst_ring[i], dst_ring[next]],
                            [[cx, cy], src_ring[i], src_ring[next]],
                            [1.; 3],
                            glint,
                        ));
                    }
                }
            }
        }
        return out;
    }

    let (cols, rows) = (32_usize, 18_usize);
    for row in 0..rows {
        for col in 0..cols {
            let cell = [
                col as f64 / cols as f64,
                row as f64 / rows as f64,
                (col + 1) as f64 / cols as f64,
                (row + 1) as f64 / rows as f64,
            ];
            let cx = 0.5 * (cell[0] + cell[2]);
            let cy = 0.5 * (cell[1] + cell[3]);
            let id = (row * cols + col) as u64;
            let start = 0.64 * sweep(cx, cy) + 0.13 * unit_hash(id + 77);
            let k = span(t, start, start + 0.23);
            if k <= 0. {
                continue;
            }
            if k >= 1. {
                out.push(Layer::inplace(Source::New, cell, 1.));
                continue;
            }
            out.push(Layer::inplace(Source::Black, cell, 1.));
            let (source, deg) = if k < 0.5 {
                (Source::Old, 180. * k)
            } else {
                (Source::New, 180. * (k - 1.))
            };
            let pivot = cam.on_plane(cx, cy);
            let pts = cam.rect_corners(cell).map(|p| {
                if vertical_axis {
                    p.yaw_about(pivot, deg)
                } else {
                    p.pitch_about(pivot, deg)
                }
            });
            let shade = Camera3D::obliquity_shade(pts, 0.55);
            cam.push_card(&mut out, source, cell, pts, 1., shade);
            let glint = (1. - (2. * k - 1.).abs()).powi(3) * 0.45;
            if glint > 0.02 {
                out.push(Layer::quad(
                    Source::White,
                    cell,
                    cam.project_quad(pts),
                    glint,
                ));
            }
        }
    }
    out
}

/// Swirling particle vortex moving in the chosen direction (`l`, `r`, `u`, `d`).
pub(crate) fn vortex(option: &str, t: f64, aspect: f64) -> Vec<Layer> {
    let [dx, dy] = direction_vec(option, [-1., 0.]);
    let (cols, rows) = (36_usize, 20_usize);
    let mut out = vec![Layer::full(Source::Black, 1.)];

    for row in 0..rows {
        for col in 0..cols {
            let id = (row * cols + col) as u64;
            let cell = [
                col as f64 / cols as f64,
                row as f64 / rows as f64,
                (col + 1) as f64 / cols as f64,
                (row + 1) as f64 / rows as f64,
            ];
            let cx = 0.5 * (cell[0] + cell[2]);
            let cy = 0.5 * (cell[1] + cell[3]);
            let lead = (0.5 + (cx - 0.5) * dx + (cy - 0.5) * dy).clamp(0., 1.);
            let depart = 0.38 * lead + 0.09 * unit_hash(id + 31);
            let ko = span(t, depart, depart + 0.36);
            let settle = 0.62 + 0.30 * (1. - lead) + 0.06 * unit_hash(id + 43);
            let kn = 1. - span(t, settle - 0.36, settle);

            let emit_particle = |layers: &mut Vec<Layer>, source: Source, k: f64| {
                let swirl = TAU * (unit_hash(id + 13) - 0.5) * k;
                let shift = [
                    dx * 0.82 * k + swirl.sin() * 0.14 * k,
                    dy * 0.82 * k - swirl.cos().abs() * 0.11 * k,
                ];
                let scale = 0.85 - 0.52 * k;
                let rect = transform_rect(cell, scale, [cx, cy], shift);
                let pivot = [0.5 * (rect[0] + rect[2]), 0.5 * (rect[1] + rect[3])];
                let spin = (unit_hash(id + 5) * 340. + 20.) * k;
                let quad = corners(rect).map(|pt| rotate_2d(pt, pivot, spin, aspect));
                layers.push(Layer::quad(source, cell, quad, 1. - k * k));
            };

            if kn <= 0. {
                out.push(Layer::inplace(Source::New, cell, 1.));
            } else if kn < 1. {
                emit_particle(&mut out, Source::New, kn);
            }
            if ko <= 0. {
                out.push(Layer::inplace(Source::Old, cell, 1.));
            } else if ko < 1. && kn >= 1. {
                emit_particle(&mut out, Source::Old, ko);
            }
        }
    }
    out
}

/// Shred into strips or diced particles that recede (`*In`) or advance (`*Out`).
pub(crate) fn shred(option: &str, t: f64, aspect: f64) -> Vec<Layer> {
    let particles = option.starts_with("particles") || option.starts_with("rect");
    let outward = option.ends_with("Out");
    let cam = Camera3D {
        aspect,
        distance: 2.5,
    };
    let mut out = vec![Layer::full(Source::Black, 1.)];

    let (source, k) = if t < 0.54 {
        (Source::Old, quad_ease(span(t, 0.04, 0.54)))
    } else {
        (Source::New, 1. - quad_ease(span(t, 0.54, 0.96)))
    };

    if particles {
        let (cols, rows) = (40_usize, 22_usize);
        for row in 0..rows {
            for col in 0..cols {
                let id = (row * cols + col) as u64;
                let cell = [
                    col as f64 / cols as f64,
                    row as f64 / rows as f64,
                    (col + 1) as f64 / cols as f64,
                    (row + 1) as f64 / rows as f64,
                ];
                let cx = 0.5 * (cell[0] + cell[2]);
                let cy = 0.5 * (cell[1] + cell[3]);
                let z = if outward { -0.58 } else { 1.45 } * k * (0.45 + unit_hash(id + 3));
                let jx = (unit_hash(id + 11) - 0.5) * 0.36 * k;
                let jy = (unit_hash(id + 17) - 0.5) * 0.36 * k;
                let base = cam.on_plane(cx + jx, cy + jy);
                let projected = cam.project(Vec3::new(base.x, base.y, z));
                let scale = (0.86 - 0.28 * k) * cam.distance / (cam.distance + z).max(0.15);
                let rect = transform_rect(
                    cell,
                    scale,
                    [cx, cy],
                    [projected[0] - cx, projected[1] - cy],
                );
                let alpha = (1. - 0.32 * k * unit_hash(id + 29)).clamp(0., 1.);
                let tile = Layer::rect(source, cell, rect, alpha);
                out.push(tile);
                if k > 0.02 {
                    out.push(tile.shade(0.38 * k));
                }
            }
        }
        return out;
    }

    let strips = 28_usize;
    for i in 0..strips {
        let id = i as u64;
        let x0 = i as f64 / strips as f64;
        let x1 = (i + 1) as f64 / strips as f64;
        let cell = [x0, 0., x1, 1.];
        let inset = 0.0035 * k;
        let z = if outward { -0.32 } else { 1.05 } * k * k * (0.55 + unit_hash(id + 53));
        let tilt = (unit_hash(id + 67) - 0.5) * 32. * k;
        let drop = (unit_hash(id + 79) - 0.3) * 0.24 * k;
        let pivot = cam.on_plane(0.5 * (x0 + x1), 0.5);
        let pts = cam.rect_corners([x0 + inset, 0., x1 - inset, 1.]).map(|p| {
            let q = p.roll_about(pivot, tilt * 0.22).yaw_about(pivot, tilt);
            Vec3::new(q.x, q.y + drop, q.z + z)
        });
        cam.push_card(&mut out, source, cell, pts, 1., 0.30 * k);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glitter_hex_uses_triangles_and_differs_from_square_grid() {
        let sq = glitter("l", 0.5, 16. / 9.);
        let hx = glitter("hexL", 0.5, 16. / 9.);
        assert_ne!(sq, hx);
        assert!(hx.iter().any(|l| l.quad[2] == l.quad[3]));
    }

    #[test]
    fn honeycomb_stays_bounded_at_extreme_aspects() {
        for aspect in [0.01, 0.5, 16. / 9., 100.] {
            let layers = honeycomb(0.45, aspect);
            assert!(
                layers.len() < 4000,
                "aspect={aspect}: {} layers",
                layers.len()
            );
            assert!(
                layers
                    .iter()
                    .all(|l| l.quad.iter().flatten().all(|v| v.is_finite()))
            );
        }
    }

    #[test]
    fn shred_distinguishes_all_four_options() {
        let frames: Vec<_> = ["stripsIn", "stripsOut", "particlesIn", "particlesOut"]
            .into_iter()
            .map(|opt| shred(opt, 0.35, 16. / 9.))
            .collect();
        for i in 0..frames.len() {
            for j in (i + 1)..frames.len() {
                assert_ne!(frames[i], frames[j]);
            }
        }
    }
}
