//! EMF+ pen records and expansion of a stroked path into a brush-filled outline.

use kurbo::{
    Affine, BezPath, Cap, Join, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Point, Stroke,
    StrokeOpts, Vec2,
};

use crate::bytes::Bytes;
use crate::geom::Op;
use crate::plus_caps::{self, CustomCap};
use crate::plus_paint::{self, Brush};
use crate::plus_shapes::{floats, matrix};

const MAX_PATTERN: usize = 64;
const MAX_OUTLINE: usize = 16_384;
pub(super) const TOLERANCE: f64 = 0.08;
const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub struct Pen {
    pub brush: Brush,
    pub width: f64,
    pub unit: u32,
    pub simple: bool,
    transform: Affine,
    start: u32,
    end: u32,
    join: Join,
    miter: f64,
    dash: Vec<f64>,
    dash_offset: f64,
    dash_cap: u32,
    alignment: u32,
    bands: Vec<f64>,
    custom: [Option<CustomCap>; 2],
}

pub fn read(bytes: Bytes<'_>) -> Option<Pen> {
    let flags = bytes.u32(8)?;
    if flags & !0x1fff != 0 {
        return None;
    }
    let [width] = floats(bytes, 16)?;
    let unit = bytes.u32(12)?;
    let mut fields = Fields { bytes, offset: 20 };
    let transform = if flags & 1 != 0 {
        let m = matrix(bytes, fields.offset)?;
        fields.offset += 24;
        m
    } else {
        IDENTITY
    };
    let start = fields.integer(flags & 2, 0)?;
    let end = fields.integer(flags & 4, 0)?;
    let join = join(fields.integer(flags & 8, 0)?);
    let miter = fields.number(flags & 0x10, 10.0)?.clamp(1.0, 100.0);
    let line = fields.integer(flags & 0x20, 0)?;
    let dash_cap = fields.integer(flags & 0x40, 0)?;
    let dash_offset = fields.number(flags & 0x80, 0.0)?;
    let dash = if flags & 0x100 != 0 {
        let pattern = fields.array()?;
        if pattern.iter().any(|value| *value <= 0.0) {
            return None;
        }
        if pattern.len() % 2 == 0 {
            pattern
        } else {
            pattern
                .iter()
                .cycle()
                .take(pattern.len() * 2)
                .copied()
                .collect()
        }
    } else {
        let pattern: &[f64] = match line {
            1 => &[3.0, 1.0],
            2 => &[1.0, 1.0],
            3 => &[3.0, 1.0, 1.0, 1.0],
            4 => &[3.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            _ => &[],
        };
        pattern.to_vec()
    };
    let alignment = fields.integer(flags & 0x200, 0)?;
    let bands = if flags & 0x400 != 0 {
        let values = fields.array()?;
        if values.len() % 2 != 0
            || values.len() < 2
            || values.windows(2).any(|pair| pair[0] >= pair[1])
            || values.iter().any(|v| !(0.0..=1.0).contains(v))
        {
            return None;
        }
        values
    } else {
        vec![0.0, 1.0]
    };
    let custom = [
        fields.custom_cap(flags & 0x800)?,
        fields.custom_cap(flags & 0x1000)?,
    ];
    let brush = plus_paint::brush(bytes.tail(fields.offset)?)?;
    let simple = matches!(brush, Brush::Solid(_)) && flags & !0x80 == 0 && unit <= 2;
    Some(Pen {
        brush,
        width: width.clamp(0.0, 100_000.0),
        unit,
        simple,
        transform: Affine::new(transform),
        start,
        end,
        join,
        miter,
        dash,
        dash_offset,
        dash_cap,
        alignment,
        bands,
        custom,
    })
}

struct Fields<'a> {
    bytes: Bytes<'a>,
    offset: usize,
}

impl Fields<'_> {
    fn integer(&mut self, flag: u32, fallback: u32) -> Option<u32> {
        if flag == 0 {
            return Some(fallback);
        }
        let value = self.bytes.u32(self.offset)?;
        self.offset += 4;
        Some(value)
    }

    fn number(&mut self, flag: u32, fallback: f64) -> Option<f64> {
        if flag == 0 {
            return Some(fallback);
        }
        let [value] = floats(self.bytes, self.offset)?;
        self.offset += 4;
        Some(value)
    }

    fn array(&mut self) -> Option<Vec<f64>> {
        let length = self.integer(1, 0)? as usize;
        if length > MAX_PATTERN {
            return None;
        }
        (0..length).map(|_| self.number(1, 0.0)).collect()
    }

    fn custom_cap(&mut self, flag: u32) -> Option<Option<CustomCap>> {
        if flag == 0 {
            return Some(None);
        }
        let size = self.integer(1, 0)? as usize;
        let cap = Bytes(self.bytes.slice(self.offset, size)?);
        self.offset += size;
        Some(Some(CustomCap::read(cap)?))
    }
}

impl Pen {
    pub fn outline(&self, ops: &[Op], width: f64) -> Option<Vec<Op>> {
        if ops.len() > 4096 || !width.is_finite() {
            return None;
        }
        let inv = self.transform.inverse();
        if !inv.is_finite() {
            return None;
        }
        let input = inv * bez(ops);
        if input.elements().len() < 2 || input.control_box().area() > 1e14 {
            return None;
        }
        let width = width.max(0.5);
        let pattern: Vec<_> = self.dash.iter().map(|d| (width * d).max(0.05)).collect();
        let length_bound: f64 = input
            .segments()
            .map(|segment| match segment {
                PathSeg::Line(line) => line.p0.distance(line.p1),
                PathSeg::Quad(quad) => quad.p0.distance(quad.p1) + quad.p1.distance(quad.p2),
                PathSeg::Cubic(cubic) => {
                    cubic.p0.distance(cubic.p1)
                        + cubic.p1.distance(cubic.p2)
                        + cubic.p2.distance(cubic.p3)
                }
            })
            .sum();
        if !length_bound.is_finite()
            || length_bound > 1e7
            || pattern.iter().any(|length| !length.is_finite())
            || !pattern.is_empty()
                && length_bound * pattern.len() as f64 > pattern.iter().sum::<f64>() * 8192.0
        {
            return None;
        }
        let needs_bands = self.alignment != 0 || self.bands.len() > 2;
        let custom = self.custom.each_ref().map(Option::as_ref);
        let style = Stroke::new(width)
            .with_join(self.join)
            .with_miter_limit(self.miter)
            .with_start_cap(cap(custom[0].map_or(self.start, CustomCap::base)))
            .with_end_cap(cap(custom[1].map_or(self.end, CustomCap::base)));
        let insets = custom.map(|cap| cap.map_or(0.0, |cap| cap.inset(width)));
        let trimmed;
        let stem = if insets.iter().any(|inset| *inset > 0.0) {
            trimmed = trim_open_subpaths(&input, insets);
            &trimmed
        } else {
            &input
        };
        let mut outline = if needs_bands {
            self.stroke_bands(stem, &style, &pattern)?
        } else {
            self.stroke_band(stem, &style, &pattern)
        };
        add_caps(&mut outline, &input, width, [self.start, self.end], custom);
        if outline.elements().len() > MAX_OUTLINE {
            return None;
        }
        Some(ops_from_bez(&(self.transform * outline)))
    }

    fn stroke_band(&self, path: &BezPath, style: &Stroke, pattern: &[f64]) -> BezPath {
        if pattern.is_empty() {
            return kurbo::stroke(path.iter(), style, &StrokeOpts::default(), TOLERANCE);
        }
        let cycle: f64 = pattern.iter().sum();
        let phase = (self.dash_offset * self.width.max(0.5)).rem_euclid(cycle);
        let dashes: BezPath = kurbo::dash(path.iter(), phase, pattern).collect();
        let mut dashed = style.clone().with_caps(if self.dash_cap == 2 {
            Cap::Round
        } else {
            Cap::Butt
        });
        if self.dash_cap == 3 {
            dashed = dashed.with_caps(Cap::Butt);
        }
        let mut outline = kurbo::stroke(dashes.iter(), &dashed, &StrokeOpts::default(), TOLERANCE);
        if self.dash_cap == 3 {
            add_caps(&mut outline, &dashes, style.width, [3, 3], [None, None]);
        }
        outline
    }

    fn stroke_bands(&self, path: &BezPath, style: &Stroke, pattern: &[f64]) -> Option<BezPath> {
        let figures = polylines(path)?;
        let mut result = BezPath::new();
        for (points, closed) in &figures {
            let ccw = signed_area(points) > 0.0;
            let shift = match self.alignment {
                1 if *closed => {
                    if ccw {
                        0.5
                    } else {
                        -0.5
                    }
                }
                3 if *closed => {
                    if ccw {
                        -0.5
                    } else {
                        0.5
                    }
                }
                2 => 0.5,
                4 => -0.5,
                _ => 0.0,
            };
            for pair in self.bands.as_chunks::<2>().0 {
                let distance = style.width * (shift + (pair[0] + pair[1]) * 0.5 - 0.5);
                let shifted = parallel(points, *closed, distance, self.miter);
                let mut band = style.clone();
                band.width *= pair[1] - pair[0];
                result.extend(self.stroke_band(&shifted, &band, pattern));
                if result.elements().len() > MAX_OUTLINE {
                    return None;
                }
            }
        }
        Some(result)
    }
}

pub(super) fn cap(value: u32) -> Cap {
    match value {
        1 => Cap::Square,
        2 | 0x12 => Cap::Round,
        _ => Cap::Butt,
    }
}

pub(super) fn join(value: u32) -> Join {
    match value {
        1 => Join::Bevel,
        2 => Join::Round,
        _ => Join::Miter,
    }
}

pub(super) fn bez(ops: &[Op]) -> BezPath {
    let mut path = BezPath::new();
    for &op in ops {
        match op {
            Op::M(p) => path.move_to(p),
            Op::L(p) => path.line_to(p),
            Op::C(a, b, c) => path.curve_to(a, b, c),
            Op::Z => path.close_path(),
        }
    }
    path
}

fn ops_from_bez(path: &BezPath) -> Vec<Op> {
    let xy = |p: Point| (p.x, p.y);
    let mut first = Point::ORIGIN;
    let mut at = first;
    path.iter()
        .map(|el| match el {
            PathEl::MoveTo(p) => {
                first = p;
                at = p;
                Op::M(xy(p))
            }
            PathEl::LineTo(p) => {
                at = p;
                Op::L(xy(p))
            }
            PathEl::QuadTo(b, c) => {
                let a = at;
                at = c;
                Op::C(xy(a.lerp(b, 2.0 / 3.0)), xy(c.lerp(b, 2.0 / 3.0)), xy(c))
            }
            PathEl::CurveTo(a, b, c) => {
                at = c;
                Op::C(xy(a), xy(b), xy(c))
            }
            PathEl::ClosePath => {
                at = first;
                Op::Z
            }
        })
        .collect()
}

fn polylines(path: &BezPath) -> Option<Vec<(Vec<Point>, bool)>> {
    let mut current = Vec::new();
    let mut output = Vec::new();
    let mut remaining = MAX_OUTLINE;
    let mut flush = |points: &mut Vec<Point>, closed| {
        if points.len() > 1 {
            output.push((std::mem::take(points), closed));
        } else {
            points.clear();
        }
    };
    kurbo::flatten(path.iter(), TOLERANCE, |el| {
        if remaining == 0 {
            return;
        }
        remaining -= 1;
        match el {
            PathEl::MoveTo(p) => {
                flush(&mut current, false);
                current.push(p);
            }
            PathEl::LineTo(p) if current.last() != Some(&p) => current.push(p),
            PathEl::ClosePath => {
                if current.first() == current.last() {
                    current.pop();
                }
                flush(&mut current, true);
            }
            _ => {}
        }
    });
    flush(&mut current, false);
    (remaining > 0).then_some(output)
}

fn signed_area(points: &[Point]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.to_vec2().cross(b.to_vec2()))
        .sum()
}

fn parallel(points: &[Point], closed: bool, distance: f64, miter: f64) -> BezPath {
    let mut path = BezPath::new();
    let last = points.len() - 1;
    let unit_normal = |a: Point, b: Point| {
        let v = b - a;
        let length = v.hypot();
        if length > 1e-10 {
            Vec2::new(-v.y, v.x) / length
        } else {
            Vec2::ZERO
        }
    };
    for (index, &point) in points.iter().enumerate() {
        let prev = if index > 0 {
            Some(points[index - 1])
        } else {
            closed.then_some(points[last])
        };
        let next = if index < last {
            Some(points[index + 1])
        } else {
            closed.then_some(points[0])
        };
        let normal = match (prev, next) {
            (Some(a), Some(b)) => {
                let u = unit_normal(a, point);
                let v = unit_normal(point, b);
                let sum = u + v;
                if sum.hypot() > 1e-8 {
                    sum / (1.0 + u.dot(v)).max(1.0 / miter)
                } else {
                    v
                }
            }
            (Some(a), None) => unit_normal(a, point),
            (None, Some(b)) => unit_normal(point, b),
            (None, None) => Vec2::ZERO,
        };
        let p = point + normal * distance;
        if index == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    if closed {
        path.close_path();
    }
    path
}

fn add_caps(
    outline: &mut BezPath,
    path: &BezPath,
    width: f64,
    kinds: [u32; 2],
    custom: [Option<&CustomCap>; 2],
) {
    if kinds.iter().all(|&kind| kind <= 2) && custom.iter().all(Option::is_none) {
        return;
    }
    let Some(lines) = polylines(path) else { return };
    for (line, closed) in lines {
        if closed {
            continue;
        }
        for ((anchor, next), (kind, datum)) in [
            (line[0], line[1]),
            (line[line.len() - 1], line[line.len() - 2]),
        ]
        .into_iter()
        .zip(kinds.into_iter().zip(custom))
        {
            let v = anchor - next;
            let direction = v / v.hypot().max(1e-9);
            let normal = Vec2::new(-direction.y, direction.x);
            let point = |x, y| anchor + direction * (x * width) + normal * (y * width);
            let mut cap = BezPath::new();
            let polygon = |cap: &mut BezPath, points: &[(f64, f64)]| {
                cap.move_to(point(points[0].0, points[0].1));
                for &(x, y) in &points[1..] {
                    cap.line_to(point(x, y));
                }
                cap.close_path();
            };
            match (kind, datum) {
                (_, Some(custom)) => cap = custom.paint(anchor, direction, width),
                (3, None) => polygon(&mut cap, &[(0.0, -0.5), (0.5, 0.0), (0.0, 0.5)]),
                (0x11, None) => polygon(
                    &mut cap,
                    &[(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)],
                ),
                (0x12, None) => {
                    use kurbo::Shape;
                    cap.extend(kurbo::Circle::new(anchor, width).path_elements(TOLERANCE));
                }
                (0x13, None) => polygon(
                    &mut cap,
                    &[(-1.0, 0.0), (0.0, -1.0), (1.0, 0.0), (0.0, 1.0)],
                ),
                (0x14, None) => polygon(&mut cap, &[(0.75, 0.0), (-1.0, 0.9), (-1.0, -0.9)]),
                _ => {}
            }
            if outline.elements().len() + cap.elements().len() <= MAX_OUTLINE {
                if datum.is_some() {
                    plus_caps::extend(outline, cap);
                } else {
                    outline.extend(cap);
                }
            }
        }
    }
}

fn trim_open_subpaths(path: &BezPath, [start, end]: [f64; 2]) -> BezPath {
    let elements = path.elements();
    let starts = elements
        .iter()
        .enumerate()
        .filter_map(|(i, el)| matches!(el, PathEl::MoveTo(_)).then_some(i));
    let mut result = BezPath::new();
    for (from, to) in starts
        .clone()
        .zip(starts.skip(1).chain(std::iter::once(elements.len())))
    {
        let subpath = &elements[from..to];
        if matches!(subpath.last(), Some(PathEl::ClosePath)) {
            result.extend(subpath.iter().copied());
            continue;
        }
        const ACCURACY: f64 = 0.002;
        let segments: Vec<_> = kurbo::segments(subpath.iter().copied())
            .map(|segment| (segment, segment.arclen(ACCURACY)))
            .collect();
        let stop = segments.iter().map(|segment| segment.1).sum::<f64>() - end;
        let mut distance = 0.0;
        let mut open = false;
        for (segment, length) in segments {
            let lo = (start - distance).max(0.0);
            let hi = (stop - distance).min(length);
            distance += length;
            if hi <= lo {
                continue;
            }
            let piece = segment
                .subsegment(segment.inv_arclen(lo, ACCURACY)..segment.inv_arclen(hi, ACCURACY));
            if !open {
                result.move_to(piece.start());
                open = true;
            }
            result.push(piece.as_path_el());
        }
    }
    result
}
