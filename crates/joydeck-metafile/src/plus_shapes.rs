//! Bounded geometry operands for EMF+ paths and drawing records.

use crate::bytes::Bytes;
use crate::device::Xform;
use crate::geom::{self, Op, P};
use crate::image_placement;
use kurbo::Affine;

pub const MAX_POINTS: usize = 100_000;
pub type Rect = [f64; 4];

pub enum Paint {
    Fill(u32),
    Stroke(usize),
}

pub enum Geometry {
    Path(Vec<Op>),
    Object(usize),
    Rectangles(Vec<Rect>),
}

pub struct Drawing {
    pub paint: Paint,
    pub geometry: Geometry,
}

pub enum Command {
    Clear(u32),
    Region { slot: usize, brush: u32 },
    Image { slot: usize, destination: Affine },
    Text,
    DriverText,
}

pub fn command(kind: u16, flags: u16, record: Bytes<'_>) -> Option<Command> {
    let slot = usize::from(flags & 0xff);
    let short = flags & 0x4000 != 0;
    Some(match kind {
        0x4009 => Command::Clear(record.u32(0)?),
        0x4013 => Command::Region {
            slot,
            brush: record.u32(0)?,
        },
        0x401a => {
            let [l, t, r, b] = rectangle(record, 24, short)?;
            Command::Image {
                slot,
                destination: image_placement::rectangle([l, t, r - l, b - t]),
            }
        }
        0x401b => {
            if record.u32(24)? != 3 {
                return None;
            }
            let vertices = if flags & 0x0800 != 0 {
                relative_image_points(record.tail(28)?)?
            } else {
                points(record, 28, 3, short)?.try_into().ok()?
            };
            Command::Image {
                slot,
                destination: image_placement::parallelogram(vertices),
            }
        }
        0x401c => Command::Text,
        0x4036 => Command::DriverText,
        _ => return None,
    })
}

fn relative_image_points(data: Bytes<'_>) -> Option<[P; 3]> {
    let mut offset = 0;
    let mut next = || {
        let first = *data.0.get(offset)?;
        offset += 1;
        if first & 0x80 == 0 {
            Some(i32::from(((first << 1) as i8) >> 1))
        } else {
            let last = *data.0.get(offset)?;
            offset += 1;
            let bits = u16::from_be_bytes([first & 0x7f, last]);
            Some(i32::from(((bits << 1) as i16) >> 1))
        }
    };
    let mut previous = (0, 0);
    let mut points = [(0.0, 0.0); 3];
    for point in &mut points {
        previous.0 += next()?;
        previous.1 += next()?;
        *point = (f64::from(previous.0), f64::from(previous.1));
    }
    Some(points)
}

pub fn drawing(kind: u16, flags: u16, r: Bytes<'_>) -> Option<Drawing> {
    let short = flags & 0x4000 != 0;
    let slot = usize::from(flags & 0xff);
    let fill = matches!(kind, 0x400a | 0x400c | 0x400e | 0x4010 | 0x4014 | 0x4016);
    let at = usize::from(fill) * 4;
    let geometry = match kind {
        0x400a | 0x400b => Geometry::Rectangles(rects(r, at + 4, r.u32(at)? as usize, short)?),
        0x400c | 0x400d => {
            let p = points(r, at + 4, r.u32(at)? as usize, short)?;
            Geometry::Path(geom::polygon(&p, fill || flags & 0x2000 != 0))
        }
        0x400e | 0x400f => Geometry::Path(geom::plus_ellipse(rectangle(r, at, short)?)),
        0x4010..=0x4012 => {
            let [start, sweep] = floats::<2>(r, at)?;
            Geometry::Path(geom::plus_arc(
                rectangle(r, at + 8, short)?,
                start,
                sweep,
                kind != 0x4012,
            ))
        }
        0x4014 | 0x4015 => Geometry::Object(slot),
        0x4016..=0x4018 => {
            let [tension] = floats::<1>(r, at)?;
            let (first, count, count_at) = if kind == 0x4018 {
                (r.u32(4)? as usize, r.u32(8)? as usize, 12)
            } else {
                (0, usize::MAX, at + 4)
            };
            let p = points(r, count_at + 4, r.u32(count_at)? as usize, short)?;
            Geometry::Path(geom::cardinal(&p, tension, kind != 0x4018, first, count))
        }
        0x4019 => {
            let p = points(r, 4, r.u32(0)? as usize, short)?;
            let (&first, rest) = p.split_first()?;
            let mut ops = vec![Op::M(first)];
            ops.extend(
                rest.as_chunks::<3>()
                    .0
                    .iter()
                    .map(|&[a, b, c]| Op::C(a, b, c)),
            );
            Geometry::Path(ops)
        }
        _ => return None,
    };
    let paint = if fill {
        Paint::Fill(r.u32(0)?)
    } else {
        Paint::Stroke(if kind == 0x4015 {
            r.u32(0)? as usize
        } else {
            slot
        })
    };
    Some(Drawing { paint, geometry })
}

pub fn path(record: Bytes<'_>) -> Option<Vec<Op>> {
    let count = (record.u32(4)? as usize).min(MAX_POINTS);
    let flags = record.u32(8)?;
    if flags & 0x1800 != 0 {
        return None;
    }
    let short = flags & 0x4000 != 0;
    let points = points(record, 12, count, short)?;
    let tags = record.slice(12 + count * if short { 4 } else { 8 }, count)?;
    let mut path = Vec::with_capacity(count);
    let mut rest = points.iter().copied().zip(tags.iter().copied());
    while let Some((point, tag)) = rest.next() {
        let mut end = tag;
        match tag & 7 {
            0 => path.push(Op::M(point)),
            3 if rest.len() >= 2 => {
                let (second, _) = rest.next()?;
                let (third, tag) = rest.next()?;
                path.push(Op::C(point, second, third));
                end = tag;
            }
            _ => path.push(Op::L(point)),
        }
        if end & 0x80 != 0 {
            path.push(Op::Z);
        }
    }
    Some(path)
}

pub fn points(input: Bytes<'_>, offset: usize, count: usize, short: bool) -> Option<Vec<P>> {
    let count = count.min(MAX_POINTS);
    if short {
        return input.points(offset, count, false);
    }
    let data = input.slice(offset, count * 8)?;
    data.as_chunks::<8>()
        .0
        .iter()
        .map(|point| {
            let [x, y] = floats::<2>(Bytes(point), 0)?;
            Some((x, y))
        })
        .collect()
}

fn rectangle(input: Bytes<'_>, offset: usize, short: bool) -> Option<Rect> {
    let p = points(input, offset, 2, short)?;
    let [(x, y), (w, h)] = p.as_slice() else {
        return None;
    };
    Some([*x, *y, x + w, y + h])
}

pub fn rects(input: Bytes<'_>, offset: usize, count: usize, short: bool) -> Option<Vec<Rect>> {
    let points = points(input, offset, count.min(MAX_POINTS / 2) * 2, short)?;
    Some(
        points
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[(x, y), (w, h)]| [x, y, x + w, y + h])
            .collect(),
    )
}

pub fn floats<const N: usize>(input: Bytes<'_>, offset: usize) -> Option<[f64; N]> {
    let mut out = [0.0; N];
    for (index, field) in out.iter_mut().enumerate() {
        let value = f64::from(input.f32(offset + index * 4)?);
        if !value.is_finite() {
            return None;
        }
        *field = value;
    }
    Some(out)
}

pub fn matrix(input: Bytes<'_>, offset: usize) -> Option<Xform> {
    floats(input, offset)
}
