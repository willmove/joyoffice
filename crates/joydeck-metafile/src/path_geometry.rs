//! Windows metafile path construction in logical coordinates.

use std::f64::consts::{FRAC_PI_2, TAU};

pub(crate) type P = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Op {
    M(P),
    L(P),
    C(P, P, P),
    Z,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArcKind {
    Arc,
    Chord,
    Pie,
}

pub(crate) fn rect(l: f64, t: f64, r: f64, b: f64) -> Vec<Op> {
    polygon(&[(l, t), (r, t), (r, b), (l, b)], true)
}

pub(crate) fn polygon(points: &[P], closed: bool) -> Vec<Op> {
    let mut ops = Vec::with_capacity(points.len() + usize::from(closed));
    if let Some((&first, rest)) = points.split_first() {
        ops.push(Op::M(first));
        ops.extend(rest.iter().copied().map(Op::L));
    }
    ops.extend(closed.then_some(Op::Z));
    ops
}

pub(crate) fn subpaths(points: &[P], counts: &[usize], closed: bool) -> Vec<Op> {
    let mut out = Vec::with_capacity(points.len() + counts.len());
    let mut remaining = points;
    for &count in counts {
        let (figure, rest) = remaining.split_at(count.min(remaining.len()));
        if let Some((&first, lines)) = figure.split_first() {
            out.push(Op::M(first));
            out.extend(lines.iter().copied().map(Op::L));
            out.extend(closed.then_some(Op::Z));
        }
        remaining = rest;
    }
    out
}

#[derive(Clone, Copy)]
struct Ellipse {
    center: P,
    radius: P,
}

impl Ellipse {
    fn from_bounds([l, t, r, b]: [f64; 4]) -> Self {
        Self {
            center: ((l + r) / 2.0, (t + b) / 2.0),
            radius: ((r - l) / 2.0, (b - t) / 2.0),
        }
    }

    fn point(self, angle: f64) -> P {
        let (x, y) = self.center;
        let (rx, ry) = self.radius;
        (x + rx * angle.cos(), y + ry * angle.sin())
    }

    fn toward(self, (x, y): P) -> f64 {
        let (cx, cy) = self.center;
        let (rx, ry) = self.radius;
        ((y - cy) / ry.max(f64::MIN_POSITIVE)).atan2((x - cx) / rx.max(f64::MIN_POSITIVE))
    }

    fn curve(self, a: f64, b: f64, weight: f64) -> Op {
        let (rx, ry) = self.radius;
        let (x, y) = self.point(a);
        let (u, v) = self.point(b);
        Op::C(
            (x - weight * rx * a.sin(), y + weight * ry * a.cos()),
            (u + weight * rx * b.sin(), v - weight * ry * b.cos()),
            (u, v),
        )
    }

    fn append(self, path: &mut Vec<Op>, start: f64, sweep: f64, arc_limit: usize, gdi_plus: bool) {
        let pieces = ((sweep.abs() / FRAC_PI_2).ceil() as usize).clamp(1, arc_limit);
        let delta = sweep / pieces as f64;
        let weight = (4.0 / 3.0) * (delta / 4.0).tan();
        path.extend((0..pieces).map(|index| {
            let a = start + index as f64 * delta;
            let b = if gdi_plus {
                start + (index + 1) as f64 * delta
            } else {
                a + delta
            };
            self.curve(a, b, weight)
        }));
    }
}

pub(crate) fn ellipse(l: f64, t: f64, r: f64, b: f64) -> Vec<Op> {
    let mut ellipse = Ellipse::from_bounds([l, t, r, b]);
    ellipse.radius = (ellipse.radius.0.abs(), ellipse.radius.1.abs());
    let mut path = vec![Op::M(ellipse.point(0.0))];
    ellipse.append(&mut path, 0.0, TAU, 8, false);
    path.push(Op::Z);
    path
}

pub(crate) fn round_rect(l: f64, t: f64, r: f64, b: f64, w: f64, h: f64) -> Vec<Op> {
    let (l, r) = (l.min(r), l.max(r));
    let (t, b) = (t.min(b), t.max(b));
    let rx = (w.abs() / 2.0).min((r - l) / 2.0);
    let ry = (h.abs() / 2.0).min((b - t) / 2.0);
    let mut path = vec![Op::M((l + rx, t))];
    for (quarter, (edge, center)) in [
        ((r - rx, t), (r - rx, t + ry)),
        ((r, b - ry), (r - rx, b - ry)),
        ((l + rx, b), (l + rx, b - ry)),
        ((l, t + ry), (l + rx, t + ry)),
    ]
    .into_iter()
    .enumerate()
    {
        path.push(Op::L(edge));
        Ellipse {
            center,
            radius: (rx, ry),
        }
        .append(
            &mut path,
            (quarter as f64 - 1.0) * FRAC_PI_2,
            FRAC_PI_2,
            1,
            false,
        );
    }
    path.push(Op::Z);
    path
}

pub(crate) fn arc_shape(
    l: f64,
    t: f64,
    r: f64,
    b: f64,
    start: P,
    end: P,
    kind: ArcKind,
) -> Vec<Op> {
    let mut ellipse = Ellipse::from_bounds([l, t, r, b]);
    ellipse.radius = (ellipse.radius.0.abs(), ellipse.radius.1.abs());
    let start = ellipse.toward(start);
    let sweep = (start - ellipse.toward(end)).rem_euclid(TAU);
    let mut path = Vec::with_capacity(8);
    if kind == ArcKind::Pie {
        path.extend([Op::M(ellipse.center), Op::L(ellipse.point(start))]);
    } else {
        path.push(Op::M(ellipse.point(start)));
    }
    ellipse.append(
        &mut path,
        start,
        if sweep < 1e-12 { -TAU } else { -sweep },
        8,
        false,
    );
    if kind == ArcKind::Pie {
        path.push(Op::L(ellipse.center));
    }
    path.extend((kind != ArcKind::Arc).then_some(Op::Z));
    path
}

pub(crate) fn plus_arc(bounds: [f64; 4], start: f64, sweep: f64, pie: bool) -> Vec<Op> {
    let ellipse = Ellipse::from_bounds(bounds);
    let (rx, ry) = ellipse.radius;
    let angle = |degrees: f64| {
        let ray = degrees.to_radians();
        (rx * ray.sin()).atan2(ry * ray.cos())
    };
    let sweep = sweep.clamp(-360.0, 360.0);
    let a = angle(start);
    let mut delta = angle(start + sweep) - a;
    if sweep.abs() == 360.0 {
        delta = TAU.copysign(sweep);
    } else if sweep > 0.0 && delta < 0.0 {
        delta += TAU;
    } else if sweep < 0.0 && delta > 0.0 {
        delta -= TAU;
    }
    let mut path = if pie {
        vec![Op::M(ellipse.center), Op::L(ellipse.point(a))]
    } else {
        vec![Op::M(ellipse.point(a))]
    };
    ellipse.append(&mut path, a, delta, 4, true);
    path.extend(pie.then_some(Op::Z));
    path
}

pub(crate) fn plus_ellipse(bounds: [f64; 4]) -> Vec<Op> {
    let Ellipse {
        center: (x, y),
        radius: (rx, ry),
    } = Ellipse::from_bounds(bounds);
    const TANGENT: f64 = 0.552_284_75;
    let (dx, dy) = (rx * TANGENT, ry * TANGENT);
    let arcs = [
        ((x + rx, y + dy), (x + dx, y + ry), (x, y + ry)),
        ((x - dx, y + ry), (x - rx, y + dy), (x - rx, y)),
        ((x - rx, y - dy), (x - dx, y - ry), (x, y - ry)),
        ((x + dx, y - ry), (x + rx, y - dy), (x + rx, y)),
    ];
    let mut path = vec![Op::M((x + rx, y))];
    path.extend(arcs.map(|(a, b, c)| Op::C(a, b, c)));
    path.push(Op::Z);
    path
}

pub(crate) fn cardinal(
    points: &[P],
    tension: f64,
    closed: bool,
    start: usize,
    count: usize,
) -> Vec<Op> {
    let n = points.len();
    if n < 2 {
        return Vec::new();
    }
    let weight = tension.clamp(0.0, 10.0) / 3.0;
    let at = |index: isize| {
        let index = if closed {
            index.rem_euclid(n as isize)
        } else {
            index.clamp(0, n as isize - 1)
        };
        points[index as usize]
    };
    let first = if closed { 0 } else { start.min(n - 1) };
    let count = if closed { n } else { count.min(n - first - 1) };
    let mut path = vec![Op::M(at(first as isize))];
    path.extend((first..first + count).map(|index| {
        let i = index as isize;
        let (a, b, c, d) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        Op::C(
            (b.0 + (c.0 - a.0) * weight, b.1 + (c.1 - a.1) * weight),
            (c.0 - (d.0 - b.0) * weight, c.1 - (d.1 - b.1) * weight),
            c,
        )
    }));
    path.extend(closed.then_some(Op::Z));
    path
}
