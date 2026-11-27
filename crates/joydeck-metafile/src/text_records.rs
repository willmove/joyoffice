//! Text payloads in META_TEXTOUT, META_EXTTEXTOUT and the EMR text records.

use crate::bytes::Bytes;
use crate::canvas::Canvas;
use crate::device::{Mapping, Xform};
use crate::geom::P;
use crate::text::{
    ETO_CLIPPED, ETO_GLYPH_INDEX, ETO_OPAQUE, ETO_PDY, MAX_CHARS, decode_bytes, decode_utf16,
};
use crate::vector_records::{emf_point, emf_rect, wmf_point};
use crate::{HAlign, VAlign};

const TA_UPDATECP: u32 = 1;

const NO_RECT: u32 = 0x0100;
const SMALL_CHARS: u32 = 0x0200;

pub struct Placement {
    pub origin: P,
    pub horizontal: HAlign,
    pub vertical: VAlign,
    pub updates_position: bool,
    axes: (P, P, P),
    angle: f64,
}

impl Placement {
    pub fn new(map: &Mapping, flags: u32, at: P, current: P, escapement: f64) -> Self {
        let updates_position = flags & TA_UPDATECP != 0;
        let origin = if updates_position { current } else { at };
        Self {
            origin,
            horizontal: match flags & 6 {
                2 => HAlign::Right,
                6 => HAlign::Center,
                _ => HAlign::Left,
            },
            vertical: match flags & 24 {
                8 => VAlign::Bottom,
                24 => VAlign::Baseline,
                _ => VAlign::Top,
            },
            updates_position,
            axes: map.basis(origin),
            angle: (escapement / 10.0).to_radians(),
        }
    }

    pub fn after(&self, text: &str, advances: &[f32], height: f32) -> Option<P> {
        let direction = match self.horizontal {
            HAlign::Left => 1.0,
            HAlign::Right => -1.0,
            HAlign::Center => return Some(self.origin),
        };
        let length = if advances.is_empty() {
            f64::from(height.abs().max(1.0)) * 0.5 * text.chars().count() as f64
        } else {
            advances.iter().copied().map(f64::from).sum()
        };
        let advance = direction * length;
        let y_direction = if self.axes.2.1 < 0.0 { 1.0 } else { -1.0 };
        let x = self.origin.0 + advance * self.angle.cos();
        let y = self.origin.1 + advance * self.angle.sin() * y_direction;
        (x.is_finite() && y.is_finite()).then_some((x, y))
    }

    pub fn transform(&self) -> Option<Xform> {
        let ((x, y), (xx, xy), (yx, yy)) = self.axes;
        let (sx, sy) = (xx.hypot(xy), yx.hypot(yy));
        if ![sx, sy, x, y].iter().all(|value| value.is_finite()) || sx <= 0.0 || sy <= 0.0 {
            return None;
        }
        // Keep the baseline direction without reflecting glyph outlines.
        let axis = if xx * yy - xy * yx > 0.0 {
            xx
        } else {
            xx.abs()
        };
        let rotation = xy.atan2(axis) - self.angle;
        Some([
            sx * rotation.cos(),
            sx * rotation.sin(),
            -sy * rotation.sin(),
            sy * rotation.cos(),
            x,
            y,
        ])
    }
}

struct Run<'a> {
    origin: P,
    raw: &'a [u8],
    wide: bool,
    flags: u32,
    bounds: Option<[f64; 4]>,
    advances: Vec<f32>,
}

impl Run<'_> {
    fn draw(self, canvas: &mut Canvas) {
        let (text, advances) = if self.wide {
            let units: Vec<_> = self
                .raw
                .as_chunks::<2>()
                .0
                .iter()
                .copied()
                .map(u16::from_le_bytes)
                .collect();
            decode_utf16(&units, &self.advances)
        } else {
            decode_bytes(self.raw, canvas.font_charset(), &self.advances)
        };
        canvas.text(self.origin, text, advances, self.flags, self.bounds);
    }
}

pub fn play_wmf(canvas: &mut Canvas, record: Bytes<'_>, extended: bool) -> Option<()> {
    let run = if extended {
        wmf_extended(record)?
    } else {
        wmf_plain(record)?
    };
    run.draw(canvas);
    Some(())
}

fn wmf_plain(record: Bytes<'_>) -> Option<Run<'_>> {
    let count = usize::from(record.u16(6)?).min(MAX_CHARS);
    Some(Run {
        origin: wmf_point(record, 8 + count.next_multiple_of(2))?,
        raw: record.slice(8, count)?,
        wide: false,
        flags: 0,
        bounds: None,
        advances: Vec::new(),
    })
}

fn wmf_extended(record: Bytes<'_>) -> Option<Run<'_>> {
    let origin = wmf_point(record, 6)?;
    let flags = u32::from(record.u16(12)?);
    let mut offset = 14;
    let bounds = if flags & (ETO_OPAQUE | ETO_CLIPPED) != 0 {
        offset += 8;
        Some(std::array::from_fn(|i| {
            f64::from(record.i16(14 + 2 * i).unwrap_or_default())
        }))
    } else {
        None
    };
    let tail = record.tail(offset)?.0;
    let count = usize::from(record.u16(10)?).min(MAX_CHARS).min(tail.len());
    let raw = tail.get(..count)?;
    let advances = advances(record, offset + count.next_multiple_of(2), count, 2);
    Some(Run {
        origin,
        raw,
        wide: false,
        flags,
        bounds,
        advances,
    })
}

pub fn play_emf(canvas: &mut Canvas, opcode: u32, record: Bytes<'_>) -> Option<()> {
    let run = match opcode {
        83 => emf_extended(record, false)?,
        84 => emf_extended(record, true)?,
        108 => emf_small(record)?,
        _ => return None,
    };
    run.draw(canvas);
    Some(())
}

fn emf_extended(record: Bytes<'_>, wide: bool) -> Option<Run<'_>> {
    let flags = record.u32(52)?;
    if flags & ETO_GLYPH_INDEX != 0 {
        return None;
    }
    let offset = record.u32(48)? as usize;
    let count = (record.u32(44)? as usize).min(MAX_CHARS);
    let (bounds, dx_field) = if flags & NO_RECT == 0 {
        (emf_rect(record, 56), 72)
    } else {
        (None, 56)
    };
    let dx_offset = record.u32(dx_field)? as usize;
    let pitch = if flags & ETO_PDY == 0 { 4 } else { 8 };
    Some(Run {
        origin: emf_point(record, 36)?,
        raw: record.slice(offset, count * if wide { 2 } else { 1 })?,
        wide,
        flags,
        bounds,
        advances: if dx_offset == 0 {
            Vec::new()
        } else {
            advances(record, dx_offset, count, pitch)
        },
    })
}

fn emf_small(record: Bytes<'_>) -> Option<Run<'_>> {
    let flags = record.u32(20)?;
    if flags & ETO_GLYPH_INDEX != 0 {
        return None;
    }
    let wide = flags & SMALL_CHARS == 0;
    let (bounds, offset) = if flags & NO_RECT == 0 {
        (emf_rect(record, 36), 52)
    } else {
        (None, 36)
    };
    let count = (record.u32(16)? as usize).min(MAX_CHARS);
    Some(Run {
        origin: emf_point(record, 8)?,
        raw: record.slice(offset, count * if wide { 2 } else { 1 })?,
        wide,
        flags,
        bounds,
        advances: Vec::new(),
    })
}

fn advances(record: Bytes<'_>, offset: usize, count: usize, pitch: usize) -> Vec<f32> {
    let values: Option<Vec<_>> = (0..count)
        .map(|index| {
            let position = offset.checked_add(index * pitch)?;
            if pitch == 2 {
                record.i16(position).map(f32::from)
            } else {
                record.i32(position).map(|value| value as f32)
            }
        })
        .collect();
    values.unwrap_or_default()
}
