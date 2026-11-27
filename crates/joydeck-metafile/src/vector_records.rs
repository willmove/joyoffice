//! WMF and EMF drawing records decoded before touching the device context.

use crate::MAX_PATH_OPS;
use crate::bytes::Bytes;
use crate::canvas::Canvas;
use crate::geom::{self, ArcKind, P};

type Bounds = [f64; 4];

#[derive(Clone, Copy)]
enum Figure {
    Lines,
    Polygon,
    LinesTo,
    Bezier,
    BezierTo,
}

enum Drawing {
    Move(P),
    Line(P),
    Rectangle(Bounds),
    Ellipse(Bounds),
    RoundRect(Bounds, P),
    Arc(Bounds, P, P, ArcKind),
    Figure(Figure, Vec<P>),
    Subpaths(Vec<usize>, Vec<P>, bool),
    Begin,
    End,
    Close,
    Paint { fill: bool, stroke: bool },
}

impl Drawing {
    fn play(self, canvas: &mut Canvas) {
        match self {
            Self::Move(point) => canvas.move_to(point),
            Self::Line(point) => canvas.line_to(point),
            Self::Rectangle([l, t, r, b]) => canvas.shape(&geom::rect(l, t, r, b), true, true),
            Self::Ellipse([l, t, r, b]) => canvas.shape(&geom::ellipse(l, t, r, b), true, true),
            Self::RoundRect([l, t, r, b], (w, h)) => {
                canvas.shape(&geom::round_rect(l, t, r, b, w, h), true, true);
            }
            Self::Arc([l, t, r, b], start, end, kind) => canvas.shape(
                &geom::arc_shape(l, t, r, b, start, end, kind),
                kind != ArcKind::Arc,
                true,
            ),
            Self::Figure(figure, points) => match figure {
                Figure::Lines => canvas.poly(&points, false, false, true),
                Figure::Polygon => canvas.poly(&points, true, true, true),
                Figure::LinesTo => canvas.line_chain(&points, false, true),
                Figure::Bezier => {
                    if let Some((&first, rest)) = points.split_first() {
                        canvas.bezier(first, rest);
                    }
                }
                Figure::BezierTo => canvas.bezier_to(&points),
            },
            Self::Subpaths(counts, points, closed) => {
                canvas.shape(&geom::subpaths(&points, &counts, closed), closed, true);
            }
            Self::Begin => canvas.begin_path(),
            Self::End => canvas.end_path(),
            Self::Close => canvas.close_figure(),
            Self::Paint { fill, stroke } => canvas.paint_path(fill, stroke),
        }
    }
}

pub fn play_emf(canvas: &mut Canvas, opcode: u32, record: Bytes<'_>) -> Option<()> {
    decode_emf(opcode, record)?.play(canvas);
    Some(())
}

pub fn play_wmf(canvas: &mut Canvas, opcode: u16, record: Bytes<'_>) -> Option<()> {
    decode_wmf(opcode, record)?.play(canvas);
    Some(())
}

fn decode_emf(opcode: u32, record: Bytes<'_>) -> Option<Drawing> {
    let point = |offset| emf_point(record, offset);
    let bounds = || emf_rect(record, 8);
    let figure = match opcode {
        2 | 85 => Figure::Bezier,
        3 | 86 => Figure::Polygon,
        4 | 87 => Figure::Lines,
        5 | 88 => Figure::BezierTo,
        6 | 89 => Figure::LinesTo,
        7 | 8 | 90 | 91 => {
            let wide = opcode < 85;
            let closed = matches!(opcode, 8 | 91);
            let groups = record.u32(24)? as usize;
            return grouped(record, 32, groups, true, wide, closed);
        }
        27 => return Some(Drawing::Move(point(8)?)),
        54 => return Some(Drawing::Line(point(8)?)),
        42 => return Some(Drawing::Ellipse(bounds()?)),
        43 => return Some(Drawing::Rectangle(bounds()?)),
        44 => return Some(Drawing::RoundRect(bounds()?, point(24)?)),
        45..=47 => {
            let kind = [ArcKind::Arc, ArcKind::Chord, ArcKind::Pie][(opcode - 45) as usize];
            return Some(Drawing::Arc(bounds()?, point(24)?, point(32)?, kind));
        }
        59 => return Some(Drawing::Begin),
        60 => return Some(Drawing::End),
        61 => return Some(Drawing::Close),
        62..=64 => {
            return Some(Drawing::Paint {
                fill: opcode != 64,
                stroke: opcode != 62,
            });
        }
        _ => return None,
    };
    let count = record.u32(24)? as usize;
    if count > MAX_PATH_OPS {
        return None;
    }
    let points = record.points(28, count, opcode < 85)?;
    Some(Drawing::Figure(figure, points))
}

fn decode_wmf(opcode: u16, record: Bytes<'_>) -> Option<Drawing> {
    let point = |offset| wmf_point(record, offset);
    let bounds = |offset| wmf_rect(record, offset);
    Some(match opcode {
        0x0214 => Drawing::Move(point(6)?),
        0x0213 => Drawing::Line(point(6)?),
        0x0418 => Drawing::Ellipse(bounds(6)?),
        0x041B => Drawing::Rectangle(bounds(6)?),
        0x061C => Drawing::RoundRect(bounds(10)?, point(6)?),
        0x0817 | 0x081A | 0x0830 => {
            let kind = match opcode {
                0x0817 => ArcKind::Arc,
                0x081A => ArcKind::Pie,
                _ => ArcKind::Chord,
            };
            Drawing::Arc(bounds(14)?, point(10)?, point(6)?, kind)
        }
        0x0324 | 0x0325 => {
            let figure = if opcode == 0x0324 {
                Figure::Polygon
            } else {
                Figure::Lines
            };
            Drawing::Figure(
                figure,
                record.points(8, usize::from(record.u16(6)?), false)?,
            )
        }
        0x0538 => return grouped(record, 8, usize::from(record.u16(6)?), false, false, true),
        _ => return None,
    })
}

fn grouped(
    record: Bytes<'_>,
    start: usize,
    groups: usize,
    wide_counts: bool,
    wide_points: bool,
    closed: bool,
) -> Option<Drawing> {
    if groups > MAX_PATH_OPS {
        return None;
    }
    let width = if wide_counts { 4 } else { 2 };
    let data = record.slice(start, groups.checked_mul(width)?)?;
    let counts: Vec<usize> = if wide_counts {
        data.as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes) as usize)
            .collect()
    } else {
        data.as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| usize::from(u16::from_le_bytes(*bytes)))
            .collect()
    };
    let count = counts
        .iter()
        .try_fold(0usize, |total, &next| total.checked_add(next))?;
    if count > MAX_PATH_OPS {
        return None;
    }
    Some(Drawing::Subpaths(
        counts,
        record.points(start + data.len(), count, wide_points)?,
        closed,
    ))
}

pub fn emf_point(record: Bytes<'_>, offset: usize) -> Option<P> {
    Some((record.i32(offset)?.into(), record.i32(offset + 4)?.into()))
}

pub fn emf_rect(record: Bytes<'_>, offset: usize) -> Option<Bounds> {
    let (left, top) = emf_point(record, offset)?;
    let (right, bottom) = emf_point(record, offset + 8)?;
    Some([left, top, right, bottom])
}

pub fn wmf_point(record: Bytes<'_>, offset: usize) -> Option<P> {
    Some((record.i16(offset + 2)?.into(), record.i16(offset)?.into()))
}

pub fn wmf_rect(record: Bytes<'_>, offset: usize) -> Option<Bounds> {
    let (right, bottom) = wmf_point(record, offset)?;
    let (left, top) = wmf_point(record, offset + 4)?;
    Some([left, top, right, bottom])
}
