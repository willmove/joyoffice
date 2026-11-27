//! Brush and pen records and brush-space color evaluation for EMF+.

use std::sync::Arc;

use crate::bitmap::Rgba;
use crate::bytes::Bytes;
use crate::cliprect::{ops_bbox, pts_bbox};
use crate::device::Xform;
use crate::geom::P;
use crate::plus_image::{ImageData, image};
use crate::plus_shapes::{MAX_POINTS, floats, matrix, path, points};

const STOP_LIMIT: usize = 256;
const IDENTITY: Xform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
type Color = [u8; 4];
type Ramp = Vec<(f64, Color)>;

#[derive(Clone)]
pub enum Brush {
    Solid(Color),
    Hatch {
        style: u32,
        fg: Color,
        bg: Color,
    },
    Linear {
        rect: [f64; 4],
        stops: Ramp,
        wrap: u32,
        xf: Xform,
    },
    Radial {
        center: P,
        radius: P,
        stops: Ramp,
        xf: Xform,
    },
    Texture {
        img: Arc<Rgba>,
        wrap: u32,
        xf: Xform,
    },
}

impl Brush {
    pub fn color(&self) -> Color {
        match self {
            Self::Solid(color) | Self::Hatch { fg: color, .. } => *color,
            Self::Linear { stops, .. } | Self::Radial { stops, .. } => evaluate(stops, 0.5),
            Self::Texture { img, .. } => img.px.first_chunk().copied().unwrap_or([0, 0, 0, 255]),
        }
    }

    pub fn sample(&self, (x, y): P) -> Color {
        match self {
            Self::Solid(color) | Self::Hatch { fg: color, .. } => *color,
            Self::Linear {
                rect, stops, wrap, ..
            } => {
                let position = if rect[2] == 0.0 {
                    0.0
                } else {
                    (x - rect[0]) / rect[2]
                };
                evaluate(stops, wrap_gradient(position, *wrap))
            }
            Self::Radial {
                center,
                radius,
                stops,
                ..
            } => {
                let dx = (x - center.0) / radius.0.max(1e-9);
                let dy = (y - center.1) / radius.1.max(1e-9);
                evaluate(stops, dx.hypot(dy).min(1.0))
            }
            Self::Texture { img, wrap, .. } => {
                let (w, h) = (img.w as f64, img.h as f64);
                if img.w == 0
                    || img.h == 0
                    || (*wrap == 4 && !((0.0..w).contains(&x) && (0.0..h).contains(&y)))
                {
                    return [0; 4];
                }
                let column = (x.rem_euclid(w) as usize).min(img.w - 1);
                let row = (y.rem_euclid(h) as usize).min(img.h - 1);
                img.px
                    .as_chunks::<4>()
                    .0
                    .get(row * img.w + column)
                    .copied()
                    .unwrap_or([0; 4])
            }
        }
    }

    pub fn xf(&self) -> Xform {
        match self {
            Self::Linear { xf, .. } | Self::Radial { xf, .. } | Self::Texture { xf, .. } => *xf,
            _ => IDENTITY,
        }
    }
}

pub fn argb(value: u32) -> Color {
    let [a, r, g, b] = value.to_be_bytes();
    [r, g, b, a]
}

pub fn blend(start: Color, end: Color, weight: f64) -> Color {
    std::array::from_fn(|channel| {
        let a = f64::from(start[channel]);
        (a + (f64::from(end[channel]) - a) * weight)
            .round()
            .clamp(0.0, 255.0) as u8
    })
}

fn evaluate(stops: &[(f64, Color)], position: f64) -> Color {
    let Some(&(start, first)) = stops.first() else {
        return [0; 4];
    };
    if position <= start {
        return first;
    }
    let index = stops.partition_point(|&(offset, _)| offset < position);
    match (stops.get(index.saturating_sub(1)), stops.get(index)) {
        (Some(&(a, left)), Some(&(b, right))) if index != 0 => blend(
            left,
            right,
            if b > a { (position - a) / (b - a) } else { 0.0 },
        ),
        _ => stops.last().map_or(first, |stop| stop.1),
    }
}

fn wrap_gradient(value: f64, wrap: u32) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    match wrap {
        1 | 3 => {
            let position = value.rem_euclid(2.0);
            position.min(2.0 - position)
        }
        4 => value.clamp(0.0, 1.0),
        _ => value.rem_euclid(1.0),
    }
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
            let wrap = data.u32(4)?;
            let (xf, offset) = transform(data, 8)?;
            let ImageData::Bitmap(pixels) = image(data.tail(offset)?)? else {
                return None;
            };
            Some(Brush::Texture {
                img: Arc::new(pixels),
                wrap,
                xf,
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

fn linear(data: Bytes<'_>) -> Option<Brush> {
    let flags = data.u32(0)?;
    let endpoints = (argb(data.u32(24)?), argb(data.u32(28)?));
    let (xf, offset) = transform(data, 40)?;
    let mut stops = Vec::new();
    if flags & 0xc == 0 {
        stops.extend([(0.0, endpoints.0), (1.0, endpoints.1)]);
    } else {
        let count = data.u32(offset)? as usize;
        let rows = data.slice(offset + 4, count.checked_mul(8)?)?;
        let (positions, values) = rows.split_at(count * 4);
        for (position, color) in positions
            .as_chunks::<4>()
            .0
            .iter()
            .zip(values.as_chunks::<4>().0)
            .take(STOP_LIMIT)
        {
            let position = f64::from(f32::from_le_bytes(*position));
            if !position.is_finite() {
                continue;
            }
            let color = if flags & 4 != 0 {
                argb(u32::from_le_bytes(*color))
            } else {
                blend(
                    endpoints.0,
                    endpoints.1,
                    f64::from(f32::from_le_bytes(*color)).clamp(0.0, 1.0),
                )
            };
            stops.push((position, color));
        }
        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let rect = floats(data, 8)?;
    Some(Brush::Linear {
        rect,
        stops,
        wrap: data.u32(4)?,
        xf,
    })
    .filter(|brush| matches!(brush, Brush::Linear { stops, .. } if !stops.is_empty()))
}

fn radial(data: Bytes<'_>) -> Option<Brush> {
    let first = argb(data.u32(8)?);
    let [x, y] = floats::<2>(data, 12)?;
    let count = data.u32(20)? as usize;
    if count > STOP_LIMIT {
        return None;
    }
    let outer = if count == 0 {
        first
    } else {
        argb(data.u32(24)?)
    };
    let boundary = 24 + count * 4;
    let length = data.u32(boundary)? as usize;
    let start = boundary + 4;
    let (bounds, end) = if data.u32(0)? & 1 != 0 {
        let path = path(Bytes(data.slice(start, length)?))?;
        (ops_bbox(&path)?, start.checked_add(length)?)
    } else {
        let points = points(data, start, length, false)?;
        (pts_bbox(&points)?, start.checked_add(points.len() * 8)?)
    };
    Some(Brush::Radial {
        center: (x, y),
        radius: ((bounds[2] - bounds[0]) / 2.0, (bounds[3] - bounds[1]) / 2.0),
        stops: vec![(0.0, first), (1.0, outer)],
        xf: transform(data, end)?.0,
    })
}

pub fn pen(record: Bytes<'_>) -> Option<(Color, f64)> {
    let flags = record.u32(8)?;
    let width = f64::from(record.f32(16)?);
    let fixed = usize::from(flags & 1 != 0) * 24 + ((flags >> 1) & 127).count_ones() as usize * 4;
    let mut offset = 20 + fixed;
    for (flag, units, maximum) in [
        (0x100, 4, MAX_POINTS),
        (0x200, 0, 0),
        (0x400, 4, MAX_POINTS),
        (0x800, 1, 1 << 20),
        (0x1000, 1, 1 << 20),
    ] {
        if flags & flag != 0 {
            let count = if units == 0 {
                0
            } else {
                (record.u32(offset)? as usize).min(maximum)
            };
            offset += 4 + units * count;
        }
    }
    let brush = brush(record.tail(offset)?)?;
    Some((
        brush.color(),
        if width.is_finite() {
            width.clamp(0.0, 1e5)
        } else {
            0.0
        },
    ))
}
