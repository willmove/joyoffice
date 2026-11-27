//! DrawString layout and explicit DrawDriverString glyph origins.

use crate::{
    bytes::Bytes,
    canvas::LogFont,
    device::Xform,
    geom::P,
    plus_shapes::{floats, matrix, points},
    text::{Font, decode_utf16},
};

const MAX_CHARS: usize = 32_768;
pub const IDENTITY: Xform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

#[derive(Clone)]
pub struct FontSpec {
    em: f64,
    unit: u32,
    style: u32,
    pub family: String,
}

impl FontSpec {
    pub fn read(data: Bytes<'_>) -> Option<Self> {
        let [em] = floats::<1>(data, 4)?;
        let family = utf16(data, 24, (data.u32(20)? as usize).min(64))?;
        Some(Self {
            em,
            unit: data.u32(8)?,
            style: data.u32(12)?,
            family: String::from_utf16_lossy(&family)
                .trim_end_matches('\0')
                .into(),
        })
    }

    pub fn logical(&self, dpi: f64, page: f64) -> LogFont {
        let em = if self.unit == 0 {
            self.em
        } else {
            self.em * unit_px(self.unit, dpi) / page
        };
        LogFont {
            font: Font {
                family: self.family.clone(),
                height: -(em.clamp(0.0, 1e6) as f32),
                weight: if self.style & 1 != 0 { 700 } else { 400 },
                italic: self.style & 2 != 0,
                underline: self.style & 4 != 0,
                strike: self.style & 8 != 0,
                ..Default::default()
            },
            escapement: 0.0,
        }
    }
}

pub fn unit_px(unit: u32, dpi: f64) -> f64 {
    match unit {
        3 => dpi / 72.0,
        4 => dpi,
        5 => dpi / 300.0,
        6 => dpi / 25.4,
        _ => 1.0,
    }
}

#[derive(Default, Clone, Copy)]
pub struct Format {
    flags: u32,
    align: u32,
    line_align: u32,
}

impl Format {
    pub fn read(data: Bytes<'_>) -> Option<Self> {
        Some(Self {
            flags: data.u32(4)?,
            align: data.u32(12)?,
            line_align: data.u32(16)?,
        })
    }
}

pub struct Run {
    pub text: String,
    pub advances: Vec<f32>,
    pub at: P,
    pub align: u32,
}

pub fn layout(data: Bytes<'_>, format: Format, em: f64) -> Option<(Vec<Run>, Option<[f64; 4]>)> {
    let count = (data.u32(8)? as usize).min(MAX_CHARS);
    let [x, y, w, h] = floats(data, 12)?;
    let units = utf16(data, 28, count)?;
    let wrap = w > 0.0 && format.flags & 0x1000 == 0;
    let mut lines = Vec::new();
    for line in units.split(|&unit| unit == 10) {
        let line: Vec<_> = line.iter().copied().filter(|&unit| unit != 13).collect();
        if wrap {
            lines.extend(wrap_line(&line, w, em));
        } else {
            lines.push(line);
        }
    }
    let line_height = em * 1.2;
    let (left, align) = match format.align {
        1 => (x + w / 2.0, 6),
        2 => (x + w, 2),
        _ => (x, 0),
    };
    let gap = h - line_height * lines.len() as f64;
    let top = match format.line_align {
        1 => y + gap / 2.0,
        2 => y + gap,
        _ => y,
    };
    let bottom = format.line_align == 2 && lines.len() == 1;
    let runs = lines
        .into_iter()
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .map(|(row, line)| Run {
            text: String::from_utf16_lossy(&line),
            advances: Vec::new(),
            at: (
                left,
                if bottom {
                    y + h
                } else {
                    top + line_height * row as f64
                },
            ),
            align: align | if bottom { 8 } else { 0 },
        })
        .collect();
    let clip = (w > 0.0 && h > 0.0 && format.flags & 0x4000 == 0).then_some([x, y, x + w, y + h]);
    Some((runs, clip))
}

pub fn driver(data: Bytes<'_>, family: &str, em: f32) -> Option<(Vec<Run>, Xform)> {
    let options = data.u32(4)?;
    let has_matrix = data.u32(8)? != 0;
    let count = (data.u32(12)? as usize).min(MAX_CHARS);
    if count == 0 {
        return None;
    }
    let mut units = utf16(data, 16, count)?;
    if options & 1 == 0 {
        map_core_font(family, &mut units)?;
    }
    let offsets = 16 + 2 * count;
    let realized = options & 4 != 0;
    let trailing = usize::from(has_matrix) * 24;
    let long = data.0.len() >= offsets + 8 * count + trailing;
    let position_count = if realized && !long { 1 } else { count };
    let locations = points(data, offsets, position_count, false)?;
    let transform = if has_matrix {
        matrix(data, offsets + 8 * position_count)?
    } else {
        IDENTITY
    };
    let first = *locations.first()?;
    if realized {
        return Some((
            vec![Run {
                text: String::from_utf16_lossy(&units),
                advances: Vec::new(),
                at: first,
                align: 24,
            }],
            transform,
        ));
    }

    // Keep the supplied baselines, and group consecutive glyphs on the same baseline into one run.
    let mut advances: Vec<_> = locations
        .windows(2)
        .map(|pair| (pair[1].0 - pair[0].0) as f32)
        .collect();
    advances.push(advances.last().copied().unwrap_or(em * 0.5));
    let mut runs = Vec::new();
    let mut at = 0;
    while at < units.len() {
        let origin = locations[at];
        let mut end = at + 1;
        while end < units.len() && (locations[end].1 == origin.1 || surrogate_boundary(&units, end))
        {
            end += 1;
        }
        let (text, advances) = decode_utf16(&units[at..end], &advances[at..end]);
        runs.push(Run {
            text,
            advances,
            at: origin,
            align: 24,
        });
        at = end;
    }
    Some((runs, transform))
}

fn surrogate_boundary(units: &[u16], at: usize) -> bool {
    (0xd800..=0xdbff).contains(&units[at - 1]) && (0xdc00..=0xdfff).contains(&units[at])
}

fn utf16(bytes: Bytes<'_>, at: usize, count: usize) -> Option<Vec<u16>> {
    Some(
        bytes
            .slice(at, count * 2)?
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect(),
    )
}

fn map_core_font(family: &str, units: &mut [u16]) -> Option<()> {
    let family = family.trim().to_ascii_lowercase();
    if !matches!(
        family.as_str(),
        "arial"
            | "arial black"
            | "times new roman"
            | "courier new"
            | "verdana"
            | "georgia"
            | "tahoma"
            | "trebuchet ms"
            | "comic sans ms"
            | "impact"
    ) {
        return None;
    }
    for glyph in units {
        if !(3..=97).contains(glyph) {
            return None;
        }
        *glyph += 29;
    }
    Some(())
}

fn glyph_width(unit: u16) -> f64 {
    match char::from_u32(u32::from(unit)).unwrap_or_default() {
        ' ' | 'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '\'' | '!' | '|' | 'I' | 'f' | 't'
        | 'r' => 0.3,
        'm' | 'w' | 'M' | 'W' => 0.85,
        'A'..='Z' | '0'..='9' => 0.62,
        ch if ch >= '\u{2e80}' => 1.0,
        _ => 0.52,
    }
}

fn wrap_line(line: &[u16], width: f64, em: f64) -> Vec<Vec<u16>> {
    if em <= 0.0 || !width.is_finite() {
        return vec![line.to_vec()];
    }
    let strip = |text: &[u16]| {
        text.iter()
            .rposition(|&unit| unit != 32)
            .map_or(0, |i| i + 1)
    };
    let amount = |text: &[u16]| text.iter().map(|&unit| glyph_width(unit) * em).sum::<f64>();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut used = 0.0;
    let end_row = |rows: &mut Vec<Vec<u16>>, row: &mut Vec<u16>| {
        row.truncate(strip(row));
        rows.push(std::mem::take(row));
    };
    for word in line.split_inclusive(|&unit| unit == 32) {
        let visible = amount(&word[..strip(word)]);
        if !row.is_empty() && used + visible > width {
            end_row(&mut rows, &mut row);
            used = 0.0;
        }
        if visible <= width {
            row.extend_from_slice(word);
            used += amount(word);
        } else {
            for (index, &unit) in word.iter().enumerate() {
                let advance = glyph_width(unit) * em;
                if !row.is_empty()
                    && used + advance > width
                    && (index == 0 || !surrogate_boundary(word, index))
                {
                    end_row(&mut rows, &mut row);
                    used = 0.0;
                }
                row.push(unit);
                used += advance;
            }
        }
    }
    end_row(&mut rows, &mut row);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_keeps_separate_baselines_and_accepts_realized_advance_with_one_origin() {
        let record = |flags: u32, positions: &[[f32; 2]]| {
            let mut bytes = [0_u32, flags, 0, 2]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            bytes.extend("Ax".encode_utf16().flat_map(u16::to_le_bytes));
            bytes.extend(
                positions
                    .iter()
                    .flatten()
                    .flat_map(|number| number.to_le_bytes()),
            );
            bytes
        };
        let data = record(1, &[[5.0, 20.0], [12.0, 30.0]]);
        let placements = driver(Bytes(&data), "Arial", 10.0).map(|(runs, _)| {
            runs.into_iter()
                .map(|run| (run.text, run.at))
                .collect::<Vec<_>>()
        });
        assert_eq!(
            placements,
            Some(vec![("A".into(), (5.0, 20.0)), ("x".into(), (12.0, 30.0))])
        );
        let data = record(5, &[[5.0, 20.0]]);
        let realized = driver(Bytes(&data), "Arial", 10.0).map(|(runs, _)| {
            runs.into_iter()
                .map(|run| (run.text, run.at, run.advances))
                .collect::<Vec<_>>()
        });
        assert_eq!(realized, Some(vec![("Ax".into(), (5.0, 20.0), vec![])]));
    }
}
