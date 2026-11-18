//! PresentationML shape and wheel transitions. Textures stay in slide coordinates
//! for reveals. A zoom independently scales the full texture into its new rectangle.
use crate::transition::{FULL, Layer, P, Source, corners};
use std::f64::consts::TAU;

const CENTRE: P = [0.5, 0.5];

fn opacity(d: f64, leading: f64, feather: f64) -> f64 {
    let u = ((leading - d) / feather).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

#[derive(Clone, Copy)]
struct Vertex {
    point: P,
    distance: f64,
}

/// Clip one triangle by two level sets of its interpolated distance. There
/// are at most five vertices between parallel cuts, even at a slide corner.
fn clip(vertices: &[Vertex], level: f64, above: bool, out: &mut [Vertex; 6]) -> usize {
    let Some(mut prev) = vertices.last().copied() else {
        return 0;
    };

    let mut count = 0;
    for &next in vertices {
        let inside = |v: Vertex| {
            if above {
                v.distance >= level
            } else {
                v.distance <= level
            }
        };
        if inside(prev) != inside(next) && prev.distance != level && next.distance != level {
            let t = (level - prev.distance) / (next.distance - prev.distance);
            out[count] = Vertex {
                point: std::array::from_fn(|i| prev.point[i] + t * (next.point[i] - prev.point[i])),
                distance: level,
            };
            count += 1;
        }
        if inside(next) {
            out[count] = next;
            count += 1;
        }
        prev = next;
    }
    count
}

fn band(triangle: &[Vertex; 3], low: f64, high: f64, leading: f64, out: &mut Vec<Layer>) {
    let mut first = [triangle[0]; 6];
    let mut strip = first;
    let n = clip(triangle, low, true, &mut first);
    let n = clip(&first[..n], high, false, &mut strip);
    for i in 2..n {
        let corners = [strip[0], strip[i - 1], strip[i]];
        let points = corners.map(|v| v.point);
        let fade = corners.map(|v| opacity(v.distance, leading, 0.328));
        out.push(Layer::tri_uv(Source::New, points, points, fade, 1.0));
    }
}

fn reveal(option: &str, t: f64, aspect: f64) -> Vec<Layer> {
    // Measured on the circular 1600 ms PowerPoint transition. The same
    // transfer function describes the diamond, plus and wedge variants.
    let leading = 1.32 * t + 0.025;
    let distance = |p: P| {
        let (x, y) = ((p[0] - 0.5).abs(), (p[1] - 0.5).abs());
        match option {
            "diamond" => x + y,
            "plus" => 2.0 * x.min(y),
            _ => (x * aspect).hypot(y) / (0.5 * aspect).hypot(0.5),
        }
    };
    let sides = if matches!(option, "diamond" | "plus") {
        2
    } else {
        16
    };
    let perimeter = corners(FULL);
    let mut out = vec![Layer::full(Source::Old, 1.)];
    let center = Vertex {
        point: CENTRE,
        distance: 0.,
    };
    for k in 0..4 {
        let a = perimeter[k];
        let b = perimeter[(k + 1) % 4];
        let at = |step: usize| {
            let point = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * step as f64 / sides as f64);
            Vertex {
                point,
                distance: distance(point),
            }
        };
        let mut from = at(0);
        for step in 1..=sides {
            let to = at(step);
            let max = from.distance.max(to.distance);
            let triangle = [center, from, to];
            let mut low = 0.;
            for j in 0..=8 {
                let high = (leading - 0.328 * (1.0 - f64::from(j) / 8.0)).clamp(0., max);
                if high > low {
                    band(&triangle, low, high, leading, &mut out);
                    low = high;
                }
            }
            from = to;
        }
    }
    out
}

fn zoom(inward: bool, t: f64, aspect: f64) -> Vec<Layer> {
    let (base, texture, size) = if inward {
        (
            Source::New,
            Source::Old,
            (1.0 - 2.0 * t - 0.03).clamp(0., 1.),
        )
    } else {
        (Source::Old, Source::New, (2.0 * t - 0.975).clamp(0., 1.))
    };
    let mut out = vec![Layer::full(base, 1.)];
    if size == 0. {
        return out;
    }
    let a = (1.0 - size) * 0.5;
    let b = 1. - a;
    let edges = |feather| [a - feather, a, b, b + feather];
    let (x, y) = (edges(0.012), edges(0.012 * aspect));
    let uv = [0., 0., 1., 1.];
    for row in 0..3 {
        for col in 0..3 {
            let dst = [x[col], y[row], x[col + 1], y[row + 1]];
            let src = [uv[col], uv[row], uv[col + 1], uv[row + 1]];
            let alpha = |i, j| {
                if (1..=2).contains(&i) && (1..=2).contains(&j) {
                    1.
                } else {
                    0.
                }
            };
            let fade = [
                alpha(col, row),
                alpha(col + 1, row),
                alpha(col + 1, row + 1),
                alpha(col, row + 1),
            ];
            out.push(Layer::rect(texture, src, dst, 1.).faded(fade));
        }
    }
    out
}

pub(crate) fn shape(option: &str, t: f64, aspect: f64) -> Vec<Layer> {
    match option {
        "in" | "out" => zoom(option == "in", t, aspect),
        _ => reveal(option, t, aspect),
    }
}

/// A wheel's angle is measured in the slide's own normalized coordinates.
/// Split at corners and along the moving feather. A uniform-opacity sector
/// follows its ray all the way to the centre at any aspect ratio.
pub(crate) fn clock(option: &str, t: f64) -> Vec<Layer> {
    let wedge = option == "wedge";
    let reverse = option == "counterClockwise";
    let (leading, feather) = if wedge {
        (1.32 * t + 0.025, 0.328)
    } else {
        (1.099 * t + 0.023, 0.109)
    };
    let mut angles = vec![0., 0.125, 0.375, 0.5, 0.625, 0.875, 1.];
    for i in 0..=24 {
        let progress = (leading - feather * f64::from(i) / 24.).clamp(0., 1.);
        if wedge {
            angles.extend([0.5 * progress, 1. - 0.5 * progress]);
        } else {
            angles.push(if reverse { 1. - progress } else { progress });
        }
    }
    angles.sort_by(f64::total_cmp);
    angles.dedup();
    let edge = |turn: f64| {
        let (x, y) = (turn * TAU).sin_cos();
        let reach = 0.5 / x.abs().max(y.abs());
        [0.5 + reach * x, 0.5 - reach * y]
    };
    let mut out = vec![Layer::full(Source::Old, 1.)];
    for pair in angles.windows(2) {
        let middle = 0.5 * (pair[0] + pair[1]);
        let distance = if wedge {
            2. * middle.min(1. - middle)
        } else if reverse {
            1. - middle
        } else {
            middle
        };
        let alpha = opacity(distance, leading, feather);
        if alpha > 0. {
            let triangle = [CENTRE, edge(pair[0]), edge(pair[1])];
            out.push(Layer::tri_uv(
                Source::New,
                triangle,
                triangle,
                [alpha; 3],
                1.,
            ));
        }
    }
    out
}

/// Assign each location a different threshold, once. The order is stable
/// when seeking, rerunning a show, or switching between CPU and GPU.
fn shuffled_ranks<const N: usize>(seed: u64) -> [u16; N] {
    let mut ranks = std::array::from_fn(|i| i as u16);
    let mut state = seed;
    for i in (1..N).rev() {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ranks.swap(i, (((state >> 32) * (i as u64 + 1)) >> 32) as usize);
    }
    ranks
}

pub(crate) fn bars(vertical: bool, t: f64) -> Vec<Layer> {
    const COUNT: usize = 240;
    static RANKS: std::sync::OnceLock<[u16; COUNT]> = std::sync::OnceLock::new();
    let ranks = RANKS.get_or_init(|| shuffled_ranks(8729));
    let x = (t + 0.032).min(1.0);
    let coverage = x * x * x * (10. + x * (6. * x - 15.));
    let feather = 1.4 * coverage.min(1. - coverage);
    let mut out = vec![Layer::full(Source::Old, 1.)];
    if feather <= f64::EPSILON {
        if coverage > 0.5 {
            out.push(Layer::full(Source::New, 1.))
        }
        return out;
    }
    for (i, &rank) in ranks.iter().enumerate() {
        let threshold = (f64::from(rank) + 0.5) / COUNT as f64;
        let sweep = (f64::from(ranks[(i + 1) % COUNT]) / COUNT as f64 - 0.5) * 0.5 * feather;
        let start = opacity(threshold - sweep, coverage + 0.5 * feather, feather);
        let end = opacity(threshold + sweep, coverage + 0.5 * feather, feather);
        if start.max(end) > 0. {
            let lo = i as f64 / COUNT as f64;
            let hi = (i + 1) as f64 / COUNT as f64;
            let (rect, fade) = if vertical {
                ([lo, 0., hi, 1.], [start, start, end, end])
            } else {
                ([0., lo, 1., hi], [start, end, end, start])
            };
            out.push(Layer::inplace(Source::New, rect, 1.).faded(fade));
        }
    }
    out
}

pub(crate) fn dissolve(t: f64) -> Vec<Layer> {
    const WIDTH: usize = 64;
    const HEIGHT: usize = 36;
    const CELLS: usize = WIDTH * HEIGHT;
    static RANKS: std::sync::OnceLock<[u16; CELLS]> = std::sync::OnceLock::new();
    let ranks = RANKS.get_or_init(|| shuffled_ranks(347));
    let covered = (((1.14 * t - 0.044).clamp(0., 1.)) * CELLS as f64).round() as u16;
    let show_new = usize::from(covered) <= CELLS / 2;
    let (base, over) = if show_new {
        (Source::Old, Source::New)
    } else {
        (Source::New, Source::Old)
    };
    let mut out = vec![Layer::full(base, 1.)];
    for (row, cells) in ranks.as_chunks::<WIDTH>().0.iter().enumerate() {
        let mut start = None;
        for (col, active) in cells
            .iter()
            .map(|&rank| (rank < covered) == show_new)
            .chain([false])
            .enumerate()
        {
            if active {
                start.get_or_insert(col);
            } else if let Some(first) = start.take() {
                out.push(Layer::inplace(
                    over,
                    [
                        first as f64 / WIDTH as f64,
                        row as f64 / HEIGHT as f64,
                        col as f64 / WIDTH as f64,
                        (row + 1) as f64 / HEIGHT as f64,
                    ],
                    1.,
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dissolved_cells_change_texture_once_even_when_the_base_switches() {
        let mut previous = [false; 64 * 36];
        for frame in 1..40 {
            let layers = dissolve(f64::from(frame) / 40.);
            let mut now = [layers[0].source == Source::New; 64 * 36];
            for overlay in &layers[1..] {
                let [left, top, right, _] = overlay.src;
                let row = (top * 36.).round() as usize;
                now[row * 64 + (left * 64.).round() as usize
                    ..row * 64 + (right * 64.).round() as usize]
                    .fill(overlay.source == Source::New);
            }
            assert!(previous.iter().zip(now).all(|(&was, is)| !was || is));
            previous = now;
        }
        assert!(previous.iter().all(|&v| v));
    }

    #[test]
    fn wheel_progress_is_measured_on_the_slide_and_respects_reversing() {
        for t in [0.1, 0.5, 0.9] {
            let forward = clock("clockwise", t);
            let reverse = clock("counterClockwise", t);
            let area = |layers: &[Layer]| {
                layers
                    .iter()
                    .skip(1)
                    .map(|layer| {
                        let [a, b, c, _] = layer.quad;
                        0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs()
                            * layer.fade[0]
                    })
                    .sum::<f64>()
            };
            assert!((area(&forward) - area(&reverse)).abs() < 1e-10);
            assert!(forward.len() < 36 && reverse.len() < 36);
        }
    }
}
