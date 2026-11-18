//! DrawingML preset geometry from Ecma's machine-readable definitions.
//! The compiled program has no XML parsing, name lookup or recursive evaluation.

use super::{FillMode, Geometry, Handle, SubPath};
use joyoffice_drawingml::guide::{Builtin, GuideSpace, Operation as Op};
use kurbo::{BezPath, Point, Rect, Vec2};
use std::sync::Arc;

#[derive(Clone, Copy)]
enum V {
    Number(f64),
    Builtin(Builtin),
    Slot(usize),
}
struct Rule(Op, [V; 3], Option<usize>);
type Pair = [V; 2];

enum Step {
    Move(Pair),
    Line(Pair),
    Quad(Pair, Pair),
    Cubic(Pair, Pair, Pair),
    Arc([V; 4]),
    Close,
}
struct Path {
    size: [f64; 2],
    fill: FillMode,
    stroke: bool,
    steps: &'static [Step],
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    X,
    Y,
    Radius,
    Angle,
}
struct Control {
    index: usize,
    kind: Axis,
    pos: Pair,
    bounds: Pair,
}
impl Control {
    fn range(&self, frame: &Frame, preset: &str) -> [f64; 2] {
        // The two star definitions give the handle's extent in slide distance
        // (ssd2). Their inner radius guide uses a 0..50000 ratio at every size.
        if matches!(preset, "star24" | "star32") && self.index == 0 {
            return [0., 50_000.];
        }
        self.bounds.map(|v| frame.number(v))
    }
}

struct Program {
    name: &'static str,
    rules: &'static [Rule],
    paths: &'static [Path],
    rect: [V; 4],
    sites: &'static [Pair],
    handles: &'static [Control],
}

include!(concat!(env!("OUT_DIR"), "/standard_presets.rs"));

/// A flattened preset arc still visits the extrema of its surviving axis.
/// A collapsed can lid, for example, travels all the way across the cylinder.
fn flat_arc(
    at: Point,
    rx: f64,
    ry: f64,
    first: f64,
    sweep: f64,
    mut line: impl FnMut(Point),
) -> Point {
    const QUARTER: f64 = 5_400_000.;
    let start = first.rem_euclid(4. * QUARTER);
    let end = start + sweep.clamp(-4. * QUARTER, 4. * QUARTER);
    let offset = |angle: f64| match angle.rem_euclid(4. * QUARTER) {
        0. => Vec2::new(rx, 0.),
        QUARTER => Vec2::new(0., ry),
        10_800_000. => Vec2::new(-rx, 0.),
        16_200_000. => Vec2::new(0., -ry),
        _ => Vec2::ZERO,
    };
    let centre = at - offset(start);
    let mut point = at;
    let mut visit = |angle| {
        let next = centre + offset(angle);
        if point != next {
            line(next);
            point = next;
        }
    };
    let step = QUARTER.copysign(sweep);
    let mut angle = if sweep < 0. {
        (start / QUARTER).ceil() * QUARTER - QUARTER
    } else {
        (start / QUARTER).floor() * QUARTER + QUARTER
    };
    while (sweep > 0. && angle < end) || (sweep < 0. && angle > end) {
        visit(angle);
        angle += step;
    }
    visit(end);
    point
}

struct Frame {
    space: GuideSpace,
    slots: Vec<f64>,
}

impl Frame {
    fn new(w: f64, h: f64, program: &Program, values: &[f64]) -> Self {
        let mut f = Self {
            space: GuideSpace::new(w, h),
            slots: Vec::with_capacity(program.rules.len()),
        };
        f.evaluate(program, values);
        f
    }
    fn number(&self, v: V) -> f64 {
        match v {
            V::Number(n) => n,
            V::Builtin(b) => b.evaluate(self.space),
            V::Slot(slot) => self.slots[slot],
        }
    }
    fn point(&self, pair: Pair) -> Point {
        Point::new(self.number(pair[0]), self.number(pair[1]))
    }
    fn evaluate(&mut self, program: &Program, values: &[f64]) {
        self.slots.clear();
        for Rule(op, values_in, adjust) in program.rules {
            let value = match adjust.and_then(|i| values.get(i)) {
                Some(&n) if n.is_finite() => n,
                Some(_) => 0.,
                None => op.apply(values_in.map(|v| self.number(v))),
            };
            self.slots.push(value);
        }
    }
    fn geometry_path(&self, path: &Path, bounded: bool) -> SubPath {
        let GuideSpace {
            width: w,
            height: h,
        } = self.space;
        let [pw, ph] = path.size;
        let sx = if pw > 0. { w / pw } else { 1. };
        let sy = if ph > 0. { h / ph } else { 1. };
        let placed = |p: Point| {
            let q = Point::new(p.x * sx, p.y * sy);
            if bounded {
                Point::new(q.x.clamp(0., w), q.y.clamp(0., h))
            } else {
                q
            }
        };
        let mut outline = BezPath::new();
        let mut at = Point::ZERO;
        let mut start = Point::ZERO;
        for step in path.steps {
            match *step {
                Step::Move(p) => {
                    at = self.point(p);
                    start = at;
                    outline.move_to(placed(at));
                }
                Step::Line(p) => {
                    at = self.point(p);
                    outline.line_to(placed(at));
                }
                Step::Quad(p, q) => {
                    at = self.point(q);
                    outline.quad_to(placed(self.point(p)), placed(at));
                }
                Step::Cubic(p, q, r) => {
                    at = self.point(r);
                    outline.curve_to(placed(self.point(p)), placed(self.point(q)), placed(at));
                }
                Step::Arc(params) => {
                    let [rx, ry, first, sweep] = params.map(|v| self.number(v));
                    if rx == 0. || ry == 0. {
                        at = flat_arc(at, rx, ry, first, sweep, |p| outline.line_to(placed(p)));
                        continue;
                    }
                    let end = joyoffice_drawingml::arc::cubics(
                        [at.x, at.y],
                        [rx, ry],
                        [first, sweep],
                        |points| {
                            let [a, b, c] = points.map(|[x, y]| placed(Point::new(x, y)));
                            outline.curve_to(a, b, c);
                        },
                    );
                    at = Point::new(end[0], end[1]);
                }
                Step::Close => {
                    outline.close_path();
                    at = start;
                }
            }
        }
        SubPath {
            path: outline,
            fill: path.fill,
            stroke: path.stroke,
            even_odd: false,
        }
    }
}

#[derive(Debug, PartialEq)]
struct Adjustments {
    preset: &'static str,
    size: [f64; 2],
    values: Vec<f64>,
}

/// Invert a standard handle against the same guide program that placed it.
/// Modifying one independent adjustment leaves the other guides at their current values.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Drag {
    state: Arc<Adjustments>,
    control: usize,
}

impl Drag {
    pub(super) fn value_at(&self, target: Point, min: f64, max: f64) -> f64 {
        let Some(program) = program(self.state.preset) else {
            return min;
        };
        let control = &program.handles[self.control];
        let [w, h] = self.state.size;
        let mut values = self.state.values.clone();
        let current = values[control.index].clamp(min, max);
        if !target.is_finite() || min == max {
            return current;
        }
        let mut frame = Frame::new(w, h, program, &values);
        if (target - frame.point(control.pos)).hypot2() < 1e-14 {
            return current;
        }
        let mut position = |v: f64| {
            values[control.index] = v;
            frame.evaluate(program, &values);
            frame.point(control.pos)
        };
        if matches!(control.kind, Axis::X | Axis::Y) {
            let axis = |p: Point| if control.kind == Axis::X { p.x } else { p.y };
            let (a, b) = (axis(position(min)), axis(position(max)));
            // Standard XY handles are monotone along their named axis.
            if (a - b).abs() > 1e-10 {
                let desired = axis(target).clamp(a.min(b), a.max(b));
                let (mut lo, mut hi) = (min, max);
                for _ in 0..42 {
                    let m = (lo + hi) * 0.5;
                    if (axis(position(m)) < desired) == (a < b) {
                        lo = m;
                    } else {
                        hi = m;
                    }
                }
                return ((lo + hi) * 0.5).round().clamp(min, max);
            }
        }
        // Polar or coupled tracks can turn back in X or Y. Locate the nearest
        // part of the track, then refine it to a single DrawingML unit.
        let error = |p: Point| (p - target).hypot2();
        let stride = (max - min) / 32.;
        let mut best = current;
        let mut distance = error(position(current));
        for i in 0..=32 {
            let v = min + stride * f64::from(i);
            let d = error(position(v));
            if d < distance {
                distance = d;
                best = v;
            }
        }
        let (mut lo, mut hi) = ((best - stride).max(min), (best + stride).min(max));
        for _ in 0..64 {
            if hi - lo < 0.5 {
                break;
            }
            let third = (hi - lo) / 3.;
            if error(position(lo + third)) < error(position(hi - third)) {
                hi -= third;
            } else {
                lo += third;
            }
        }
        ((lo + hi) * 0.5).round().clamp(min, max)
    }
}

pub(super) fn build(name: &str, w: f64, h: f64, adj: &[f64]) -> Option<Geometry> {
    let program = program(name)?;
    let adj = &adj[..adj.len().min(8)];
    let mut frame = Frame::new(w, h, program, adj);
    // Imported adjust values can lie outside the domain shown by the standard's
    // handles. Some definitions (notably curvedUpArrow) also use the raw adjust
    // later in their path. Resolve dependent bounds before drawing that path.
    let mut clamped = None;
    for _ in 0..=program.handles.len() {
        let mut changed = false;
        for control in program.handles {
            let Some(&raw) = clamped.as_deref().unwrap_or(adj).get(control.index) else {
                continue;
            };
            let [a, b] = control.range(&frame, name);
            let v = if raw.is_finite() {
                raw.clamp(a.min(b), a.max(b))
            } else {
                0.
            };
            if v != raw {
                clamped.get_or_insert_with(|| adj.to_vec())[control.index] = v;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        frame.evaluate(program, clamped.as_deref().unwrap_or(adj));
    }
    if name == "swooshArrow" && w > 0. && h > 0. {
        // The arrowhead leans by 1/14 of a right angle. Leave room for
        // its widest corner even when a file requests a zero-length head.
        let side = w.min(h);
        let thickness = frame.slots[0].clamp(1., 75000.) * h / 100000.;
        let minimum =
            (side / 8. + thickness) * (std::f64::consts::FRAC_PI_2 / 14.).tan() * 100000. / side;
        if frame.slots[1] < minimum {
            let values = [frame.slots[0], minimum.min(w / side * 70000.)];
            frame.evaluate(program, &values);
        }
    }
    let handles = if program.handles.is_empty() {
        Vec::new()
    } else {
        let count = program
            .handles
            .iter()
            .map(|c| c.index + 1)
            .max()
            .unwrap_or(0);
        let mut values = vec![0.; count];
        for (rule, &v) in program.rules.iter().zip(&frame.slots) {
            if let Some(i) = rule.2
                && let Some(dest) = values.get_mut(i)
            {
                *dest = v;
            }
        }
        let state = Arc::new(Adjustments {
            preset: program.name,
            size: [w, h],
            values,
        });
        let center = Point::new(w * 0.5, h * 0.5);
        let mut handles: Vec<_> = program
            .handles
            .iter()
            .enumerate()
            .map(|(i, control)| {
                let value = state.values[control.index];
                if control.kind == Axis::Angle
                    && matches!(name, "arc" | "pie" | "chord" | "blockArc")
                {
                    let inset = if name == "blockArc" {
                        w.min(h) * state.values.get(2).copied().unwrap_or(0.).clamp(0., 50000.)
                            / 200000.
                    } else {
                        0.
                    };
                    return Handle::angle(
                        control.index,
                        center,
                        w * 0.5 - inset,
                        h * 0.5 - inset,
                        value,
                    );
                }
                let [a, b] = control.range(&frame, name);
                Handle {
                    index: control.index,
                    pos: frame.point(control.pos),
                    origin: center,
                    dir: Vec2::ZERO,
                    min: a.min(b),
                    max: a.max(b),
                    angle: control.kind == Axis::Angle,
                    standard: Some(Drag {
                        state: state.clone(),
                        control: i,
                    }),
                }
            })
            .collect();
        handles.sort_by_key(|h| h.index);
        handles
    };
    let [l, t, r, b] = program.rect.map(|v| frame.number(v));
    let mut paths: Vec<_> = program
        .paths
        .iter()
        .map(|p| {
            frame.geometry_path(
                p,
                w == 0.
                    || h == 0.
                    || matches!(
                        name,
                        "swooshArrow" | "cornerTabs" | "squareTabs" | "plaqueTabs"
                    ),
            )
        })
        .collect();
    // Desktop chart markers put the unstroked background under their lines.
    // Curved arrows paint the darker rear face before the front and outline.
    if matches!(
        name,
        "chartX"
            | "chartStar"
            | "chartPlus"
            | "curvedRightArrow"
            | "curvedLeftArrow"
            | "curvedUpArrow"
            | "curvedDownArrow"
    ) && paths.len() >= 2
    {
        paths.swap(0, 1);
    }
    // These paths have nested contours with opposite winding. The equivalent
    // even-odd representation also lets Morph fade a hole into a solid face.
    if matches!(name, "donut" | "frame" | "funnel" | "noSmoking") {
        for p in &mut paths {
            p.even_odd = p
                .path
                .elements()
                .iter()
                .filter(|e| matches!(e, kurbo::PathEl::ClosePath))
                .count()
                > 1;
        }
    }
    Some(Geometry {
        paths,
        text_rect: Rect::new(l, t, r, b),
        sites: program.sites.iter().map(|&p| frame.point(p)).collect(),
        handles,
    })
}
