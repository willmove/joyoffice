//! EMF+ brushes and sampling in brush space.

use std::sync::Arc;

use crate::bitmap::Rgba;
use crate::bytes::Bytes;
use crate::cliprect::{flatten, pts_bbox};
use crate::device::Xform;
use crate::geom::P;
use crate::plus_image::{ImageData, image};
use crate::plus_shapes::{floats, matrix, path, points};

const STOP_LIMIT: usize = 256;
const BOUNDARY_LIMIT: usize = 256;
const DIRECTIONS: usize = 512;
const IDENTITY: Xform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
type Color = [u8; 4];
type Ramp = Vec<(f64, Color)>;
type Factors = Vec<(f64, f64)>;

#[derive(Clone)]
pub enum Brush {
    Solid(Color),
    Hatch {
        style: u32,
        fg: Color,
        bg: Color,
    },
    Linear(Arc<Linear>),
    Radial(Arc<Radial>),
    Texture {
        img: Arc<Rgba>,
        wrap: u32,
        xf: Xform,
        device: bool,
    },
}

#[derive(Clone)]
enum Gradient {
    Colors(Ramp),
    Factors { x: Factors, y: Factors },
}

pub struct Linear {
    rect: [f64; 4],
    ends: [Color; 2],
    gradient: Gradient,
    wrap: u32,
    xf: Xform,
    gamma: bool,
}

pub struct Radial {
    center: P,
    bounds: [f64; 4],
    outer: Vec<(f64, Color)>,
    color: Color,
    gradient: Gradient,
    focus: Option<[f64; 2]>,
    wrap: u32,
    xf: Xform,
    gamma: bool,
}

impl Brush {
    pub fn color(&self) -> Color {
        match self {
            Self::Solid(color) | Self::Hatch { fg: color, .. } => *color,
            Self::Linear(linear) => {
                gradient_color(&linear.gradient, linear.ends, 0.5, 0.5, linear.gamma)
            }
            Self::Radial(radial) => radial.color,
            Self::Texture { img, .. } => img.px.first_chunk().copied().unwrap_or([0, 0, 0, 255]),
        }
    }

    pub fn sample(&self, (x, y): P) -> Color {
        match self {
            Self::Solid(color) | Self::Hatch { fg: color, .. } => *color,
            Self::Linear(linear) => {
                let [l, t, w, h] = linear.rect;
                let x = wrap_gradient((x - l) / w, linear.wrap, false);
                let y = wrap_gradient((y - t) / h, linear.wrap, true);
                gradient_color(&linear.gradient, linear.ends, x, y, linear.gamma)
            }
            Self::Radial(radial) => radial.sample((x, y)),
            Self::Texture { img, wrap, .. } => {
                let column = texture_index(x, img.w, *wrap, false);
                let row = texture_index(y, img.h, *wrap, true);
                match (row, column) {
                    (Some(row), Some(col)) => {
                        let row = if img.bottom_up { img.h - row - 1 } else { row };
                        img.px
                            .as_chunks::<4>()
                            .0
                            .get(row * img.w + col)
                            .copied()
                            .unwrap_or([0; 4])
                    }
                    _ => [0; 4],
                }
            }
        }
    }

    pub fn xf(&self) -> Xform {
        match self {
            Self::Linear(brush) => brush.xf,
            Self::Radial(brush) => brush.xf,
            Self::Texture { xf, .. } => *xf,
            _ => IDENTITY,
        }
    }

    pub fn device_space(&self) -> bool {
        matches!(self, Self::Texture { device: true, .. })
    }

    pub fn cost(&self) -> usize {
        match self {
            Self::Texture { img, .. } => img.px.len(),
            Self::Linear(_) => 8192,
            Self::Radial(_) => 16384,
            _ => 32,
        }
    }
}

impl Radial {
    fn sample(&self, (x, y): P) -> Color {
        let [l, t, r, b] = self.bounds;
        let wrap = |value, lo, hi, vertical| {
            if self.wrap != 4 && (value < lo || value > hi) && hi > lo {
                lo + (hi - lo) * wrap_gradient((value - lo) / (hi - lo), self.wrap, vertical)
            } else {
                value
            }
        };
        let ray = (
            wrap(x, l, r, false) - self.center.0,
            wrap(y, t, b, true) - self.center.1,
        );
        let length = ray.0.hypot(ray.1);
        if !length.is_finite() || length <= 1e-10 {
            return self.color;
        }
        let angular = ray.1.atan2(ray.0).rem_euclid(std::f64::consts::TAU)
            * (DIRECTIONS as f64 / std::f64::consts::TAU);
        let base = angular.floor() as usize;
        let fraction = angular - base as f64;
        let (r0, c0) = self.outer[base % DIRECTIONS];
        let (r1, c1) = self.outer[(base + 1) % DIRECTIONS];
        let radius = r0 + (r1 - r0) * fraction;
        let outer = mix(c0, c1, fraction, self.gamma);
        let focus = self
            .focus
            .map_or(0.0, |[sx, sy]| length / (ray.0 / sx).hypot(ray.1 / sy));
        let t = ((length / radius.max(1e-9) - focus) / (1.0 - focus).max(1e-9)).clamp(0.0, 1.0);
        gradient_color(&self.gradient, [self.color, outer], t, 0.0, self.gamma)
    }
}

pub fn argb(value: u32) -> Color {
    let [a, r, g, b] = value.to_be_bytes();
    [r, g, b, a]
}

pub fn blend(start: Color, end: Color, weight: f64) -> Color {
    mix(start, end, weight, false)
}

fn mix(start: Color, end: Color, weight: f64, gamma: bool) -> Color {
    std::array::from_fn(|channel| {
        let a = f64::from(start[channel]);
        let b = f64::from(end[channel]);
        let value = if gamma && channel < 3 {
            let linear = |sample: f64| {
                if sample <= 10.31475 {
                    sample / (255.0 * 12.92)
                } else {
                    ((sample / 255.0 + 0.055) / 1.055).powf(2.4)
                }
            };
            let light = linear(a) * (1.0 - weight) + linear(b) * weight;
            255.0
                * if light <= 0.0031308 {
                    12.92 * light
                } else {
                    1.055 * light.powf(1.0 / 2.4) - 0.055
                }
        } else {
            a + (b - a) * weight
        };
        value.round().clamp(0.0, 255.0) as u8
    })
}

fn interval<T: Copy>(stops: &[(f64, T)], t: f64) -> Option<(T, T, f64)> {
    let &(first, a) = stops.first()?;
    if t <= first {
        return Some((a, a, 0.0));
    }
    let index = stops.partition_point(|&(offset, _)| offset < t);
    let &(x, left) = stops.get(index.saturating_sub(1))?;
    Some(match stops.get(index) {
        Some(&(y, right)) if y > x => (left, right, ((t - x) / (y - x)).clamp(0.0, 1.0)),
        _ => (left, left, 0.0),
    })
}

fn gradient_color(ramp: &Gradient, ends: [Color; 2], x: f64, y: f64, gamma: bool) -> Color {
    let factor = |curve: &Factors, t| interval(curve, t).map(|(a, b, w)| a + (b - a) * w);
    match ramp {
        Gradient::Colors(stops) => {
            interval(stops, x).map_or(ends[0], |(a, b, t)| mix(a, b, t, gamma))
        }
        Gradient::Factors {
            x: horizontal,
            y: vertical,
        } => {
            let t = match (factor(horizontal, x), factor(vertical, y)) {
                (Some(a), Some(b)) => a + b - a * b,
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => x,
            };
            mix(ends[0], ends[1], t, gamma)
        }
    }
}

fn wraps_flipped(wrap: u32, vertical: bool) -> bool {
    wrap == 3 || wrap == if vertical { 2 } else { 1 }
}

fn wrap_gradient(value: f64, wrap: u32, vertical: bool) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    if wraps_flipped(wrap, vertical) {
        let position = value.rem_euclid(2.0);
        position.min(2.0 - position)
    } else if wrap == 4 {
        value.clamp(0.0, 1.0)
    } else {
        value.rem_euclid(1.0)
    }
}

fn texture_index(value: f64, count: usize, wrap: u32, vertical: bool) -> Option<usize> {
    let n = count as f64;
    if count == 0 || !value.is_finite() || (wrap == 4 && !(0.0..n).contains(&value)) {
        return None;
    }
    let value = wrap_gradient(value / n, wrap, vertical) * n;
    Some((value as usize).min(count - 1))
}

pub fn brush(record: Bytes<'_>) -> Option<Brush> {
    let data = record.tail(8)?;
    match record.u32(4)? {
        0 => Some(Brush::Solid(argb(data.u32(0)?))),
        1 => Some(Brush::Hatch {
            style: data.u32(0)?,
            fg: argb(data.u32(4)?),
            bg: argb(data.u32(8)?),
        }),
        2 => {
            let flags = data.u32(0)?;
            let wrap = data.u32(4)?;
            let (xf, offset) = transform(data, 8)?;
            let ImageData::Bitmap(pixels) = image(data.tail(offset)?)? else {
                return None;
            };
            Some(Brush::Texture {
                img: Arc::new(pixels),
                wrap,
                xf,
                device: flags & 0x100 != 0,
            })
        }
        3 => radial(data),
        4 => linear(data),
        _ => None,
    }
}

fn transform(record: Bytes<'_>, offset: usize) -> Option<(Xform, usize)> {
    if record.u32(0)? & 2 == 0 {
        Some((IDENTITY, offset))
    } else {
        Some((matrix(record, offset)?, offset.checked_add(24)?))
    }
}

fn table<T: Copy>(
    bytes: Bytes<'_>,
    offset: &mut usize,
    value: impl Fn(u32) -> Option<T>,
) -> Option<Vec<(f64, T)>> {
    let count = bytes.u32(*offset)? as usize;
    if !(1..=STOP_LIMIT).contains(&count) {
        return None;
    }
    let start = *offset + 4;
    let end = start + count * 8;
    bytes.slice(start, count * 8)?;
    let mut stops = Vec::with_capacity(count);
    for index in 0..count {
        let [position] = floats(bytes, start + index * 4)?;
        let sample = value(bytes.u32(start + (index + count) * 4)?)?;
        stops.push((position.clamp(0.0, 1.0), sample));
    }
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    *offset = end;
    Some(stops)
}

fn gradient(data: Bytes<'_>, flags: u32, offset: &mut usize) -> Option<Gradient> {
    if flags & 4 != 0 {
        return Some(Gradient::Colors(table(data, offset, |value| {
            Some(argb(value))
        })?));
    }
    let factors = |at: &mut usize| {
        table(data, at, |value| {
            let factor = f64::from(f32::from_bits(value));
            factor.is_finite().then_some(factor.clamp(0.0, 1.0))
        })
    };
    // When both axes are present, the vertical table precedes the horizontal table.
    let y = if flags & 16 != 0 {
        factors(offset)?
    } else {
        Vec::new()
    };
    let x = if flags & 8 != 0 {
        factors(offset)?
    } else {
        Vec::new()
    };
    Some(Gradient::Factors { x, y })
}

fn linear(data: Bytes<'_>) -> Option<Brush> {
    let flags = data.u32(0)?;
    let (xf, mut offset) = transform(data, 40)?;
    Some(Brush::Linear(Arc::new(Linear {
        rect: floats(data, 8)?,
        ends: [argb(data.u32(24)?), argb(data.u32(28)?)],
        gradient: gradient(data, flags, &mut offset)?,
        wrap: data.u32(4)?,
        xf,
        gamma: flags & 0x80 != 0,
    })))
}

fn radial(data: Bytes<'_>) -> Option<Brush> {
    let flags = data.u32(0)?;
    let color = argb(data.u32(8)?);
    let [x, y] = floats(data, 12)?;
    let count = data.u32(20)? as usize;
    if count > STOP_LIMIT {
        return None;
    }
    let colors: Vec<_> = (0..count)
        .map(|index| data.u32(24 + index * 4).map(argb))
        .collect::<Option<_>>()?;
    let boundary = 24 + count * 4;
    let length = data.u32(boundary)? as usize;
    let start = boundary + 4;
    let (ring, end) = if flags & 1 != 0 {
        let payload = Bytes(data.slice(start, length)?);
        if payload.u32(4)? as usize > BOUNDARY_LIMIT {
            return None;
        }
        let figures = flatten(&path(payload)?);
        let [(one, _)] = figures.as_slice() else {
            return None;
        };
        if one.len() > BOUNDARY_LIMIT {
            return None;
        }
        (one.clone(), start.checked_add(length)?)
    } else {
        if length > BOUNDARY_LIMIT {
            return None;
        }
        (points(data, start, length, false)?, start + length * 8)
    };
    if ring.len() < 3 {
        return None;
    }
    let (xf, mut offset) = transform(data, end)?;
    let ramp = gradient(data, flags, &mut offset)?;
    let focus = if flags & 0x40 != 0 && data.u32(offset)? == 2 {
        let scales = floats::<2>(data, offset + 4)?;
        Some(scales.map(|scale| scale.clamp(1e-6, 1.0 - 1e-6)))
    } else {
        None
    };
    let bounds = pts_bbox(&ring)?;
    let gamma = flags & 0x80 != 0;
    let colored: Vec<_> = ring
        .iter()
        .enumerate()
        .map(|(index, &point)| {
            let color = colors
                .get(index)
                .or_else(|| colors.last())
                .copied()
                .unwrap_or(color);
            ((point.0 - x, point.1 - y), color)
        })
        .collect();
    let outer = (0..DIRECTIONS)
        .map(|index| {
            let angle = index as f64 * std::f64::consts::TAU / DIRECTIONS as f64;
            let (sin, cos) = angle.sin_cos();
            let direction = (cos, sin);
            let cross = |a: P, b: P| a.0 * b.1 - a.1 * b.0;
            colored
                .iter()
                .zip(colored.iter().cycle().skip(1))
                .filter_map(|(&(a, left), &(b, right))| {
                    let side = (b.0 - a.0, b.1 - a.1);
                    let denominator = cross(direction, side);
                    if denominator.abs() <= 1e-10 {
                        return None;
                    }
                    let distance = cross(a, side) / denominator;
                    let t = cross(a, direction) / denominator;
                    (distance > 0.0 && (0.0..=1.0).contains(&t))
                        .then(|| (distance, mix(left, right, t, gamma)))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .unwrap_or((
                    (bounds[2] - bounds[0]).hypot(bounds[3] - bounds[1]),
                    colors.first().copied().unwrap_or(color),
                ))
        })
        .collect();
    Some(Brush::Radial(Arc::new(Radial {
        center: (x, y),
        bounds,
        outer,
        color,
        gradient: ramp,
        focus,
        wrap: data.u32(4)?,
        xf,
        gamma,
    })))
}
