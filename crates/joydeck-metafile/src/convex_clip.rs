//! Intersections with oriented convex device polygons.

use crate::geom::P;

const WORK_LIMIT: usize = 1_000_000;

#[derive(Clone, Copy)]
struct Edge {
    origin: P,
    direction: P,
}

impl Edge {
    fn between(a: P, b: P) -> Self {
        Self {
            origin: a,
            direction: delta(a, b),
        }
    }

    fn side(self, point: P) -> f64 {
        determinant(self.direction, delta(self.origin, point))
    }
}

fn delta(a: P, b: P) -> P {
    (b.0 - a.0, b.1 - a.1)
}
fn determinant(a: P, b: P) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
fn lerp(a: P, direction: P, t: f64) -> P {
    (a.0 + direction.0 * t, a.1 + direction.1 * t)
}

fn edges(points: &[P]) -> impl Iterator<Item = Edge> + '_ {
    points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
        .map(|(a, b)| Edge::between(a, b))
}

pub(crate) fn convex(points: &[P]) -> Option<Vec<P>> {
    if points
        .iter()
        .any(|&(x, y)| !(x.is_finite() && y.is_finite()))
    {
        return None;
    }
    let mut ring = points.to_vec();
    ring.dedup_by(|b, a| (b.0 - a.0).abs() <= 1e-9 && (b.1 - a.1).abs() <= 1e-9);
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    let area: f64 = ring
        .iter()
        .copied()
        .zip(ring.iter().copied().cycle().skip(1))
        .map(|(a, b)| determinant(a, b))
        .sum();
    if ring.len() < 3 || area.abs() < 1e-12 || !area.is_finite() {
        return None;
    }
    if area < 0.0 {
        ring.reverse();
    }
    let n = ring.len();
    let mut result = Vec::with_capacity(n);
    for (i, &b) in ring.iter().enumerate() {
        let a = ring[(i + n - 1) % n];
        let c = ring[(i + 1) % n];
        let turn = Edge::between(a, b).side(c);
        if turn < -1e-9 * (1.0 + area.abs()) {
            return None;
        }
        if turn.abs() > 1e-12 {
            result.push(b);
        }
    }
    (result.len() >= 3).then_some(result)
}

pub(crate) fn contains(clip: &[P], point: P) -> bool {
    edges(clip).all(|edge| edge.side(point) >= -1e-9)
}

pub(crate) fn clip_polygon(subject: &[P], clip: &[P]) -> Vec<P> {
    if subject.len().saturating_mul(clip.len()) > WORK_LIMIT {
        return Vec::new();
    }
    let mut input = subject.to_vec();
    let mut output = Vec::new();
    for (index, edge) in edges(clip).enumerate() {
        let Some(&last) = input.last() else { break };
        if input.len().saturating_mul(clip.len() - index) > WORK_LIMIT {
            return Vec::new();
        }
        let mut previous = last;
        let mut distance = edge.side(previous);
        for &point in &input {
            let next = edge.side(point);
            if (distance >= 0.0) != (next >= 0.0) {
                output.push(lerp(
                    previous,
                    delta(previous, point),
                    distance / (distance - next),
                ));
            }
            if next >= 0.0 {
                output.push(point);
            }
            previous = point;
            distance = next;
        }
        std::mem::swap(&mut input, &mut output);
        output.clear();
    }
    input
}

pub(crate) fn clip_segment(a: P, b: P, clip: &[P]) -> Option<(P, P)> {
    let direction = delta(a, b);
    let (mut enter, mut exit) = (0.0_f64, 1.0_f64);
    for edge in edges(clip) {
        let start = edge.side(a);
        let slope = determinant(edge.direction, direction);
        if slope > 0.0 {
            enter = enter.max(-start / slope);
        } else if slope < 0.0 {
            exit = exit.min(-start / slope);
        } else if start < 0.0 {
            return None;
        }
        if enter > exit {
            return None;
        }
    }
    Some((lerp(a, direction, enter), lerp(a, direction, exit)))
}
