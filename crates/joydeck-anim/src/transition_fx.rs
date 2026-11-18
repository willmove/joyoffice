//! The transition effects, as measured on PowerPoint (see [`crate::transition`]).
//!
//! `t` is linear progress (0..1) through the transition's duration; each effect applies its own
//! curve. Directions follow the file format's option: for Push, Wipe, Cover, Uncover, Pan, Vortex,
//! Cube… `dir` is where things move (`l`: toward the left); for Glitter it is the side the effect
//! starts from; Ripple's corner is where the wave heads.

use std::f64::consts::PI;

use crate::easing::ease;
use crate::transition::{FULL, Layer, R, Source};
use crate::transition_geom::{
    Cam, card, center, dir, ein, eout, facing_shade, hash64, moved, offset, q2, rnd, rot_quad,
    scale_rect, scaled, ss, v3, win,
};

use Source::{Black, New, NewBack, NewContent, Old, OldBack, OldContent, White};

const RANDOM_KINDS: &[&str] = &[
    "fade",
    "push",
    "wipe",
    "split",
    "reveal",
    "randomBar",
    "shape",
    "uncover",
    "cover",
    "fallOver",
    "drape",
    "curtains",
    "wind",
    "prestige",
    "fracture",
    "crush",
    "peelOff",
    "pageCurlDouble",
    "airplane",
    "origami",
    "dissolve",
    "checker",
    "blinds",
    "clock",
    "ripple",
    "honeycomb",
    "glitter",
    "vortex",
    "shred",
    "switch",
    "flip",
    "gallery",
    "cube",
    "doors",
    "box",
    "comb",
    "zoom",
    "pan",
    "ferris",
    "conveyor",
    "rotate",
    "window",
    "orbit",
    "flythrough",
];

pub(crate) fn layers(kind: &str, o: &str, t: f64, a: f64, depth: u32) -> Vec<Layer> {
    if let Some(layers) = crate::slide_motion::layers(kind, o, t) {
        return layers;
    }
    match kind {
        "wipe" | "split" => crate::transition_mask::layers(kind == "split", o, t),
        "reveal" => reveal(o, t),
        "randomBar" => crate::transition_pattern::bars(o == "vert", t),
        "shape" => crate::transition_pattern::shape(o, t, a),
        "flash" => flash(t),
        "dissolve" => crate::transition_pattern::dissolve(t),
        "checker" => crate::transition_panels::checker(o == "vert", t, a),
        "blinds" => crate::transition_panels::blinds(o == "vert", t),
        "clock" => crate::transition_pattern::clock(o, t),
        "ripple" => crate::transition_panels::ripple(o, t, a),
        "comb" => crate::transition_panels::comb(o == "vert", t),
        "zoom" => zoom(o, t, a),
        "pan" => pan(o, t),
        "fallOver" => crate::transition_fabric::fall_over(o == "r", t),
        "drape" => crate::transition_fabric::drape(o == "r", t),
        "curtains" => crate::transition_fabric::curtains(t),
        "wind" => crate::transition_fabric::wind(o == "r", t),
        "prestige" => crate::transition_fabric::prestige(t),
        "fracture" => crate::transition_paper::fracture(t, a),
        "crush" => crate::transition_paper::crush(t),
        "peelOff" => crate::transition_paper::peel(o == "r", t, a),
        "pageCurlDouble" => crate::transition_paper::turn(o, t),
        "airplane" => crate::transition_fold::airplane(o == "r", t),
        "origami" => crate::transition_fold::origami(o == "r", t),
        "honeycomb" => honeycomb(t, a),
        "glitter" => glitter(o, t, a),
        "vortex" => vortex(o, t, a),
        "shred" => shred(o, t, a),
        "switch" => switch(o == "r", t, a),
        "flip" => flip(o == "r", t, a),
        "gallery" => gallery(o == "r", t, a),
        "conveyor" => conveyor(o == "r", t, a),
        "cube" | "box" | "rotate" | "orbit" => crate::transition_prism::frame(kind, o, t, a),
        "doors" | "window" => doors(kind == "window", o == "horz", t, a),
        "ferris" => ferris(o == "r", t, a),
        "flythrough" => flythrough(o, t),
        "random" if depth == 0 => {
            let h = o.bytes().fold(0x51ED_u64, |h, b| hash64(h ^ b as u64));
            let k = RANDOM_KINDS
                .get((h % RANDOM_KINDS.len() as u64) as usize)
                .copied()
                .unwrap_or("fade");
            let opt = joydeck_model::anim::TRANSITIONS
                .iter()
                .find(|x| x.0 == k)
                .and_then(|x| x.4.first().copied())
                .unwrap_or("");
            layers(k, opt, t, a, depth + 1)
        }
        _ => vec![Layer::full(Old, 1.0), Layer::full(New, ss(t))],
    }
}

// ---------- Subtle ----------

fn reveal(o: &str, t: f64) -> Vec<Layer> {
    let dx = if o.ends_with("Right") { 1.0 } else { -1.0 };
    if o.starts_with("black") {
        // Old slides away fading to black, a pause on black, new slides in from black.
        let mut v = vec![Layer::full(Black, 1.0)];
        if t < 0.4 {
            let u = win(t, 0.0, 0.38);
            v.push(moved(Old, dx * 0.12 * ein(u), 0.0, 1.0 - ss(u)));
        }
        if t > 0.55 {
            let u = win(t, 0.55, 1.0);
            v.push(moved(New, -dx * 0.12 * (1.0 - eout(u)), 0.0, ss(u)));
        }
        return v;
    }
    // Smoothly: the backgrounds cross-fade while the old content slides off fading and the new
    // content follows.
    let mut v = vec![Layer::full(OldBack, 1.0), Layer::full(NewBack, ss(t))];
    if t < 0.5 {
        let u = win(t, 0.0, 0.45);
        v.push(moved(OldContent, dx * 0.12 * ein(u), 0.0, 1.0 - ss(u)));
    }
    if t > 0.45 {
        let u = win(t, 0.47, 0.95);
        v.push(moved(NewContent, -dx * 0.12 * (1.0 - eout(u)), 0.0, ss(u)));
    }
    v
}

fn flash(t: f64) -> Vec<Layer> {
    // A flash bulb: the old slide burns out to white fast, the white holds, then fades off the new
    // slide.
    let white = if t < 0.1 {
        t / 0.1
    } else if t < 0.47 {
        1.0
    } else {
        1.0 - win(t, 0.47, 0.88)
    };
    let base = if t < 0.2 { Old } else { New };
    vec![Layer::full(base, 1.0), Layer::full(White, white)]
}

// ---------- Exciting: patterns ----------

/// One tile flipping in 3-D about its own vertical (or horizontal) axis: old face for the first
/// half of `k`, new face for the second.
fn flip_tile(cam: &Cam, cell: R, k: f64, vertical_axis: bool, out: &mut Vec<Layer>) {
    if k <= 0.0 {
        out.push(Layer::inplace(Old, cell, 1.0));
        return;
    }
    if k >= 1.0 {
        out.push(Layer::inplace(New, cell, 1.0));
        return;
    }
    let ang = 180.0 * k;
    let cc = center(cell);
    let c3 = cam.unit(cc[0], cc[1]);
    let (src, deg) = if k < 0.5 {
        (Old, ang)
    } else {
        (New, ang - 180.0)
    };
    let pts = cam.rect(cell).map(|p| {
        if vertical_axis {
            p.yaw(c3, deg)
        } else {
            p.pitch(c3, deg)
        }
    });
    let sh = facing_shade(pts, 0.6);
    out.extend(card(cam, src, cell, pts, 1.0, sh, true));
}

fn zoom(o: &str, t: f64, a: f64) -> Vec<Layer> {
    match o {
        // Zoom and Rotate: the new slide spins in from the centre, a full clockwise turn, fading
        // in as it grows.
        "rotate" => {
            let u = win(t, 0.1, 0.98);
            let s = 0.1 + 0.9 * u;
            let ang = 360.0 * (1.0 - u);
            let r = scale_rect(FULL, s);
            let q = rot_quad(crate::transition::corners(r), [0.5, 0.5], ang, a);
            vec![
                Layer::full(Old, 1.0),
                Layer::quad(New, FULL, q, 0.35 + 0.65 * ss(win(t, 0.0, 0.45))),
            ]
        }
        // Zoom Out: the new slide comes down from large underneath while the old one shrinks away
        // fading.
        "out" => {
            let s = eout(win(t, 0.0, 0.8));
            vec![
                scaled(New, 1.9 - 0.9 * s, 0.5, 0.5, 0.0, 0.0, 1.0),
                scaled(
                    Old,
                    1.0 - 0.5 * eout(t),
                    0.5,
                    0.5,
                    0.0,
                    0.0,
                    1.0 - ss(win(t, 0.0, 0.95)),
                ),
            ]
        }
        // Zoom In: the old slide grows toward the viewer fading, the new one grows into place
        // from small.
        _ => vec![
            Layer::full(Black, 1.0),
            scaled(
                Old,
                1.0 + 0.3 * ss(t),
                0.5,
                0.5,
                0.0,
                0.0,
                1.0 - ss(win(t, 0.0, 0.9)),
            ),
            scaled(
                New,
                0.5 + 0.5 * ss(win(t, 0.15, 0.95)),
                0.5,
                0.5,
                0.0,
                0.0,
                ss(win(t, 0.0, 0.85)),
            ),
        ],
    }
}

fn honeycomb(t: f64, a: f64) -> Vec<Layer> {
    // The old slide splits into hexagonal cells along dark seams; the cells fly at the viewer and
    // out of sight, leaving black; cells of the new slide pop in one by one and close the seams.
    let cols = 11usize;
    let w = 1.0 / cols as f64;
    let rh = w * a * 0.5;
    // Rows sit 0.866*rh apart; a settled hex reaches only 0.577*rh from its centre at the side
    // vertices, and the flat bottom zone 0.289*rh. Count the rows whose FLAT zone still covers
    // the slide's bottom: counting the vertex reach instead left V-notches of black above the
    // bottom edge on about a third of the aspect ratios (#420).
    let rows = ((1.0 / (rh * 0.866) - 1.0 / 3.0).ceil().max(1.0)) as usize + 1;
    let mut v = vec![Layer::full(Black, 1.0)];
    let seam = ss(win(t, 0.0, 0.12)) * (1.0 - ss(win(t, 0.85, 1.0)));
    let hex = |cx: f64, cy: f64, s: f64| -> Vec<[f64; 2]> {
        (0..6)
            .map(|k| {
                let ang = PI / 3.0 * k as f64 + PI / 6.0;
                [
                    cx + ang.cos() * s * w * 0.577,
                    cy + ang.sin() * s * rh * 0.577 / 0.5 * 0.5,
                ]
            })
            .collect()
    };
    for r in 0..rows {
        for c in 0..=cols {
            let cx = (c as f64 + if r % 2 == 1 { 0.5 } else { 0.0 }) * w;
            let cy = r as f64 * rh * 0.866;
            let id = (r * 64 + c) as u64;
            let src_hex = hex(cx, cy, 1.0);
            let out_start = 0.1 + 0.2 * rnd(id + 1);
            let out_k = ein(win(t, out_start, out_start + 0.12));
            let in_start = 0.38 + 0.42 * rnd(id + 3);
            let in_k = eout(win(t, in_start, in_start + 0.1));
            let gap = 1.0 - 0.1 * seam;
            let mut cell = |src: Source, s: f64, alpha: f64| {
                let dst = hex(cx, cy, s);
                for k in 0..6 {
                    let (p0, p1) = (dst[k], dst[(k + 1) % 6]);
                    let (u0, u1) = (src_hex[k], src_hex[(k + 1) % 6]);
                    v.push(Layer::tri_uv(
                        src,
                        [[cx, cy], p0, p1],
                        [[cx, cy], u0, u1],
                        [1.0; 3],
                        alpha,
                    ));
                }
            };
            if t < 0.5 && out_k < 1.0 {
                cell(Old, gap * (1.0 + 3.0 * out_k), 1.0 - out_k * out_k);
            }
            if in_k > 0.0 {
                cell(New, gap * (0.3 + 0.7 * in_k), 1.0);
            }
        }
    }
    v
}

fn glitter(o: &str, t: f64, a: f64) -> Vec<Layer> {
    // Tiles flip from the old slide to the new in a sparkling sweep starting on the option's side.
    let hexes = o.starts_with("hex");
    let d = o.trim_start_matches("hex").to_ascii_lowercase();
    let (n, m) = if hexes {
        (28usize, 16usize)
    } else {
        (40usize, 22usize)
    };
    let cam = Cam { a, d: 3.0 };
    let mut v = vec![Layer::full(Old, 1.0)];
    for r in 0..m {
        for c in 0..n {
            let cell = [
                c as f64 / n as f64,
                r as f64 / m as f64,
                (c + 1) as f64 / n as f64,
                (r + 1) as f64 / m as f64,
            ];
            let cc = center(cell);
            // Distance from the side the sweep starts on ("r": from the right).
            let grad = match d.as_str() {
                "r" => 1.0 - cc[0],
                "u" => cc[1],
                "d" => 1.0 - cc[1],
                _ => cc[0],
            };
            let start = 0.75 * grad * 0.85 + rnd((r * n + c) as u64 + 77) * 0.12;
            let k = win(t, start, start + 0.25);
            if k <= 0.0 {
                continue;
            }
            v.push(Layer::inplace(Black, cell, 1.0));
            flip_tile(&cam, cell, k, true, &mut v);
        }
    }
    v
}

fn vortex(o: &str, t: f64, a: f64) -> Vec<Layer> {
    // The old slide breaks into fine particles that whirl off toward the option's side, the
    // trailing side first; the new slide's particles whirl in and settle, the leading side first.
    let (dx, dy) = dir(o, (-1.0, 0.0));
    let (n, m) = (48usize, 27usize);
    let mut v = vec![Layer::full(Black, 1.0)];
    for r in 0..m {
        for c in 0..n {
            let id = (r * n + c) as u64;
            let cell = [
                c as f64 / n as f64,
                r as f64 / m as f64,
                (c + 1) as f64 / n as f64,
                (r + 1) as f64 / m as f64,
            ];
            let cc = center(cell);
            // 1 on the side things move toward, 0 on the other.
            let lead = (0.5 + (cc[0] - 0.5) * dx + (cc[1] - 0.5) * dy).clamp(0.0, 1.0);
            let go = 0.4 * lead + 0.08 * rnd(id + 31);
            let ko = win(t, go, go + 0.35);
            let end = 0.62 + 0.3 * (1.0 - lead) + 0.06 * rnd(id + 41);
            let kn = 1.0 - win(t, end - 0.35, end);
            let mut piece = |src: Source, k: f64| {
                let swirl = 2.0 * PI * (rnd(id + 5) - 0.5) * k;
                let off = [
                    dx * k * 0.8 + swirl.sin() * 0.15 * k,
                    dy * k * 0.8 - swirl.cos().abs() * 0.12 * k,
                ];
                let s = 0.8 - 0.5 * k;
                let r2 = offset(scale_rect(cell, s), off[0], off[1]);
                let q = rot_quad(
                    crate::transition::corners(r2),
                    center(r2),
                    rnd(id) * 360.0 * k,
                    a,
                );
                v.push(Layer::quad(src, cell, q, 1.0 - k * k));
            };
            if kn < 1.0 {
                piece(New, kn);
            }
            if ko > 0.0 && ko < 1.0 && kn >= 1.0 {
                piece(Old, ko);
            } else if ko <= 0.0 {
                v.push(Layer::inplace(Old, cell, 1.0));
            }
        }
    }
    v
}

fn shred(o: &str, t: f64, a: f64) -> Vec<Layer> {
    let particles = o.starts_with("particles") || o.starts_with("rect");
    let out = o.ends_with("Out");
    let cam = Cam { a, d: 2.5 };
    let mut v = vec![Layer::full(Black, 1.0)];
    // First half: the old slide comes apart and recedes; second half: the new one assembles.
    let (src, k) = if t < 0.55 {
        (Old, q2(win(t, 0.05, 0.55)))
    } else {
        (New, 1.0 - q2(win(t, 0.55, 0.95)))
    };
    if particles {
        let (n, m) = (64usize, 36usize);
        for r in 0..m {
            for c in 0..n {
                let id = (r * n + c) as u64;
                let cell = [
                    c as f64 / n as f64,
                    r as f64 / m as f64,
                    (c + 1) as f64 / n as f64,
                    (r + 1) as f64 / m as f64,
                ];
                let cc = center(cell);
                let z = if out { -0.6 } else { 1.5 } * k * (0.5 + rnd(id));
                let jitter = [(rnd(id + 1) - 0.5) * 0.4 * k, (rnd(id + 2) - 0.5) * 0.4 * k];
                let p3 = cam.unit(cc[0] + jitter[0], cc[1] + jitter[1]);
                let pc = cam.p(v3(p3.x, p3.y, z));
                let s = 0.8 * (1.0 - 0.3 * k) * cam.d / (cam.d + z).max(0.1);
                let r2 = offset(scale_rect(cell, s), pc[0] - cc[0], pc[1] - cc[1]);
                let l = Layer::rect(src, cell, r2, 1.0 - 0.3 * k * rnd(id + 5));
                v.push(l);
                v.push(l.shade(0.4 * k));
            }
        }
    } else {
        let n = 30usize;
        for i in 0..n {
            let id = i as u64;
            let (x0, x1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
            let cell = [x0, 0.0, x1, 1.0];
            let z = if out { -0.3 } else { 1.0 } * k * k * (0.6 + rnd(id + 11));
            let tilt = (rnd(id + 12) - 0.5) * 30.0 * k;
            let drop = (rnd(id + 13) - 0.3) * 0.25 * k;
            let inset = 0.004 * k;
            let w3 = cam.rect([x0 + inset, 0.0, x1 - inset, 1.0]);
            let c3 = cam.unit((x0 + x1) / 2.0, 0.5);
            let pts = w3.map(|p| {
                let q = p.roll(c3, tilt * 0.2).yaw(c3, tilt);
                v3(q.x, q.y + drop, q.z + z)
            });
            v.extend(card(&cam, src, cell, pts, 1.0, 0.3 * k, true));
        }
    }
    v
}

fn switch(right: bool, t: f64, a: f64) -> Vec<Layer> {
    // Switching cards: the old slide is pulled out to one side ("l": left) and slipped behind
    // while the new one, pulled out to the other side from behind it, comes to the front. Both
    // turn a little away at their outer edges.
    let cam = Cam { a, d: 2.5 };
    let side = if right { -1.0 } else { 1.0 };
    let u = (t + ss(t)) / 2.0;
    let k = (u * PI).sin();
    let shift = 0.5 * k * a;
    let old = cam.rect(FULL).map(|p| {
        let q = p.yaw(v3(0.0, 0.0, 0.0), side * 14.0 * k);
        v3(q.x - side * shift, q.y, q.z + 0.12 * k)
    });
    let new = cam.rect(FULL).map(|p| {
        let q = p.yaw(v3(0.0, 0.0, 0.0), -side * 14.0 * k);
        v3(q.x + side * shift, q.y, q.z + 0.12 * k)
    });
    let mut v = vec![Layer::full(Black, 1.0)];
    let o = card(&cam, Old, FULL, old, 1.0, facing_shade(old, 0.3), true);
    let n = card(&cam, New, FULL, new, 1.0, facing_shade(new, 0.3), true);
    if u < 0.5 {
        v.extend(n);
        v.extend(o);
    } else {
        v.extend(o);
        v.extend(n);
    }
    v
}

fn flip(right: bool, t: f64, a: f64) -> Vec<Layer> {
    // The slide turns over like a card: the old face, then the new one on its back.
    let cam = Cam { a, d: 2.5 };
    let s = q2(t);
    let side = if right { 1.0 } else { -1.0 };
    let deg = 180.0 * s * side;
    let back = 0.6 * (s * PI).sin();
    let c = v3(0.0, 0.0, 0.0);
    let pts = cam.rect(FULL).map(|p| {
        let q = p.yaw(c, deg);
        v3(q.x, q.y, q.z + back)
    });
    let mut v = vec![Layer::full(Black, 1.0)];
    if s < 0.5 {
        v.extend(card(
            &cam,
            Old,
            FULL,
            pts,
            1.0,
            facing_shade(pts, 0.5),
            true,
        ));
    } else {
        // The back face: the new slide, mirrored so it reads right way round.
        let m = [pts[1], pts[0], pts[3], pts[2]];
        v.extend(card(&cam, New, FULL, m, 1.0, facing_shade(m, 0.5), true));
    }
    v
}

fn gallery(right: bool, t: f64, a: f64) -> Vec<Layer> {
    // The slides hang on a wall: it turns away and recedes, the slides slide along it, then it
    // turns back. A faint reflection shows below.
    let cam = Cam { a, d: 2.5 };
    let side = if right { 1.0 } else { -1.0 };
    let k = ss(win(t, 0.0, 0.35)) * (1.0 - ss(win(t, 0.7, 1.0)));
    let s = q2(win(t, 0.3, 0.8));
    let gap = 0.15;
    let back = 0.7 * k;
    let yaw = -side * 22.0 * k;
    let mut v = vec![Layer::full(Black, 1.0)];
    for (src, shift) in [(Old, side * s), (New, side * (s - 1.0))] {
        let x0 = shift * (a + gap);
        let pts = cam
            .rect(FULL)
            .map(|p| v3(p.x + x0, p.y, p.z + back))
            .map(|p| p.yaw(v3(0.0, 0.0, back), yaw));
        // Reflection: the slide flipped below its bottom edge, faint.
        let refl = [
            pts[3],
            pts[2],
            v3(pts[2].x, pts[2].y + 0.25, pts[2].z),
            v3(pts[3].x, pts[3].y + 0.25, pts[3].z),
        ];
        let rq = cam.quad(refl);
        v.push(Layer {
            source: src,
            src: [0.0, 0.75, 1.0, 1.0],
            quad: rq,
            uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.75], [0.0, 0.75]],
            alpha: 0.3 * k,
            fade: [1.0, 1.0, 0.0, 0.0],
        });
        v.extend(card(&cam, src, FULL, pts, 1.0, 0.0, true));
    }
    v
}

fn conveyor(right: bool, t: f64, a: f64) -> Vec<Layer> {
    // The contents ride a belt: it turns away, the contents slide along, it turns back; the
    // backgrounds cross-fade underneath.
    let cam = Cam { a, d: 2.5 };
    let side = if right { 1.0 } else { -1.0 };
    let k = ss(win(t, 0.0, 0.35)) * (1.0 - ss(win(t, 0.65, 1.0)));
    let s = q2(win(t, 0.33, 0.75));
    let back = 0.45 * k;
    let yaw = -side * 18.0 * k;
    let mut v = vec![Layer::full(OldBack, 1.0), Layer::full(NewBack, ss(t))];
    for (src, shift) in [(OldContent, side * s), (NewContent, side * (s - 1.0))] {
        let x0 = shift * a * 1.05;
        let pts = cam
            .rect(FULL)
            .map(|p| v3(p.x + x0, p.y, p.z + back))
            .map(|p| p.yaw(v3(0.0, 0.0, back), yaw));
        v.extend(card(&cam, src, FULL, pts, 1.0, 0.0, true));
    }
    v
}

fn doors(window: bool, horz: bool, t: f64, a: f64) -> Vec<Layer> {
    // The old slide parts in two halves that swing open toward the viewer and darken; the new one
    // comes forward from the middle, fading in over them. Window does it with the contents only,
    // over cross-fading backgrounds.
    let cam = Cam { a, d: 2.2 };
    let open = ein(win(t, 0.05, 0.68));
    let come = q2(win(t, 0.0, 0.72));
    let (os, ns) = if window {
        (OldContent, NewContent)
    } else {
        (Old, New)
    };
    let mut v = if window {
        vec![Layer::full(OldBack, 1.0), Layer::full(NewBack, ss(t))]
    } else {
        vec![Layer::full(Black, 1.0)]
    };
    let halves: [(R, f64); 2] = if horz {
        [([0.0, 0.0, 1.0, 0.5], -1.0), ([0.0, 0.5, 1.0, 1.0], 1.0)]
    } else {
        [([0.0, 0.0, 0.5, 1.0], -1.0), ([0.5, 0.0, 1.0, 1.0], 1.0)]
    };
    if open < 1.0 {
        for (r, sgn) in halves {
            let hinge = if horz {
                cam.unit(0.5, if sgn < 0.0 { 0.0 } else { 1.0 })
            } else {
                cam.unit(if sgn < 0.0 { 0.0 } else { 1.0 }, 0.5)
            };
            let ang = 30.0 * open;
            let slide = 0.55 * open;
            let pts = cam.rect(r).map(|p| {
                let q = if horz {
                    p.pitch(hinge, sgn * ang)
                } else {
                    p.yaw(hinge, -sgn * ang)
                };
                if horz {
                    v3(q.x, q.y + sgn * slide, q.z)
                } else {
                    v3(q.x + sgn * slide * a, q.y, q.z)
                }
            });
            let dark = if window { 0.0 } else { 0.7 * open };
            let alpha = if window {
                1.0 - ss(win(t, 0.2, 0.6))
            } else {
                1.0
            };
            v.extend(card(&cam, os, r, pts, alpha, dark, true));
        }
    }
    let sc = 0.5 + 0.5 * come;
    let fade_in = if window {
        ss(t)
    } else {
        ss(win(t, 0.05, 0.62))
    };
    v.push(scaled(ns, sc, 0.5, 0.5, 0.0, 0.0, fade_in));
    v
}

fn ferris(right: bool, t: f64, a: f64) -> Vec<Layer> {
    // The contents ride a big wheel turning about a hub far off the lower corner: the old ones
    // drop away down and sideways, the new ones come down round from the top, over cross-fading
    // backgrounds.
    let side = if right { -1.0 } else { 1.0 };
    let mut v = vec![Layer::full(OldBack, 1.0), Layer::full(NewBack, ss(t))];
    let out = ein(win(t, 0.0, 0.5));
    if out < 1.0 {
        let (dx, dy) = (-side * 0.45 * out.powf(1.5), 0.9 * out);
        let q = rot_quad(
            crate::transition::corners(offset(FULL, dx, dy)),
            [0.5 + dx, 0.5 + dy],
            side * -15.0 * out,
            a,
        );
        v.push(Layer::quad(OldContent, FULL, q, 1.0));
    }
    let inn = q2(win(t, 0.5, 1.0));
    if inn > 0.0 {
        let k = 1.0 - inn;
        let (dx, dy) = (-side * 0.5 * k.powf(1.3), -0.75 * k);
        let r = offset(scale_rect(FULL, 1.0 - 0.25 * k), dx, dy);
        let q = rot_quad(
            crate::transition::corners(r),
            center(r),
            side * -18.0 * k,
            a,
        );
        v.push(Layer::quad(NewContent, FULL, q, 1.0));
    }
    v
}

fn pan(o: &str, t: f64) -> Vec<Layer> {
    // The contents slide along together while the backgrounds cross-fade.
    let (dx, dy) = dir(o, (-1.0, 0.0));
    // Quick start, long glide.
    let s = ease(t, 0.2, 0.8, 0.0);
    vec![
        Layer::full(OldBack, 1.0),
        Layer::full(NewBack, s),
        moved(OldContent, dx * s, dy * s, 1.0),
        moved(NewContent, dx * (s - 1.0), dy * (s - 1.0), 1.0),
    ]
}

fn flythrough(o: &str, t: f64) -> Vec<Layer> {
    // The contents fly past the viewer (in) or recede (out) while the backgrounds cross-fade.
    let bounce = o.ends_with("Bounce");
    let out = o.starts_with("out");
    let s = q2(t);
    let b = if bounce {
        0.08 * (win(t, 0.6, 1.0) * PI).sin()
    } else {
        0.0
    };
    let old_a = 1.0 - ss(win(t, 0.0, 0.6));
    let new_a = ss(win(t, 0.3, 0.95));
    let (os, ns) = if out {
        (1.0 - 0.5 * s, 1.8 - 0.8 * s - b)
    } else {
        (1.0 + 1.5 * s, 0.4 + 0.6 * s + b)
    };
    vec![
        Layer::full(OldBack, 1.0),
        Layer::full(NewBack, ss(t)),
        scaled(NewContent, ns, 0.5, 0.5, 0.0, 0.0, new_a),
        scaled(OldContent, os, 0.5, 0.5, 0.0, 0.0, old_a),
    ]
}
