//! GDI bitmap input and placement for MS-WMF, MS-EMF and MS-EMFPLUS.

use crate::{MAX_PIXELS, bytes::Bytes};

pub(crate) struct Rgba {
    pub w: usize,
    pub h: usize,
    pub bottom_up: bool,
    pub px: Vec<u8>,
}

#[derive(Clone, Copy)]
pub(crate) enum Blend {
    Rop(u32),
    Alpha { constant: u8, per_pixel: bool },
}

impl Blend {
    pub fn per_pixel(self) -> bool {
        matches!(
            self,
            Self::Alpha {
                per_pixel: true,
                ..
            }
        )
    }
}

/// Describes both the format and the layout of the variable-sized info block.
struct BitmapInfo {
    width: usize,
    rows: usize,
    bottom_up: bool,
    depth: usize,
    channels: [u32; 3],
    palette_start: usize,
    palette_bytes: usize,
}

impl BitmapInfo {
    fn parse(info: Bytes) -> Option<Self> {
        let header = info.u32(0)? as usize;
        let width = usize::try_from(info.i32(4)?).ok()?;
        let height = info.i32(8)?;
        let depth = usize::from(info.u16(14)?);
        let compression = info.u32(16)?;
        if header < 40 || width == 0 || height == 0 || !matches!(depth, 1 | 4 | 8 | 16 | 24 | 32) {
            return None;
        }
        let (channels, extra) = match compression {
            0 if depth == 16 => ([0x7c00, 0x03e0, 0x001f], 0),
            0 => ([0xff_00_00, 0x00_ff_00, 0x00_00_ff], 0),
            3 => (
                [info.u32(40)?, info.u32(44)?, info.u32(48)?],
                if header == 40 { 12 } else { 0 },
            ),
            _ => return None,
        };
        let declared = info.u32(32)? as usize;
        let entries = if depth <= 8 {
            let maximum = 1 << depth;
            if declared == 0 {
                maximum
            } else {
                declared.min(maximum)
            }
        } else {
            declared.min(256)
        };
        Some(Self {
            width,
            rows: height.unsigned_abs() as usize,
            bottom_up: height > 0,
            depth,
            channels,
            palette_start: header.checked_add(extra)?,
            palette_bytes: entries * 4,
        })
    }

    fn stride(&self) -> Option<usize> {
        self.width
            .checked_mul(self.depth)?
            .div_ceil(32)
            .checked_mul(4)
    }
}

pub(crate) fn info_len(dib: Bytes) -> Option<usize> {
    let info = BitmapInfo::parse(dib)?;
    info.palette_start.checked_add(info.palette_bytes)
}

pub(crate) fn decode(info: Bytes, bits: Bytes, keep_alpha: bool) -> Option<Rgba> {
    let format = BitmapInfo::parse(info)?;
    let count = format.width.checked_mul(format.rows)?;
    if count > MAX_PIXELS {
        return None;
    }
    let stride = format.stride()?;
    let source = bits.slice(0, stride.checked_mul(format.rows)?)?;
    let palette = info
        .slice(format.palette_start, format.palette_bytes)?
        .as_chunks::<4>()
        .0;
    let rows = source.chunks_exact(stride);
    let mut pixels = Vec::with_capacity(count * 4);
    if format.bottom_up {
        for row in rows.rev() {
            decode_row(row, &format, palette, keep_alpha, &mut pixels);
        }
    } else {
        for row in rows {
            decode_row(row, &format, palette, keep_alpha, &mut pixels);
        }
    }
    Some(Rgba {
        w: format.width,
        h: format.rows,
        bottom_up: format.bottom_up,
        px: pixels,
    })
}

fn decode_row(
    row: &[u8],
    info: &BitmapInfo,
    palette: &[[u8; 4]],
    keep_alpha: bool,
    out: &mut Vec<u8>,
) {
    if info.depth <= 8 {
        // Indexed DIBs pack the first pixel into the high bits of each byte.
        let group = 8 / info.depth;
        let indices = row.iter().flat_map(|&byte| {
            (0..group).rev().map(move |slot| {
                let mask = (1u16 << info.depth) - 1;
                ((u16::from(byte) >> (slot * info.depth)) & mask) as usize
            })
        });
        for index in indices.take(info.width) {
            let [b, g, r, _] = palette.get(index).copied().unwrap_or_default();
            out.extend([r, g, b, 255]);
        }
        return;
    }
    for color in row.chunks_exact(info.depth / 8).take(info.width) {
        let (rgb, alpha) = match *color {
            [low, high] => {
                let value = u32::from(u16::from_le_bytes([low, high]));
                (info.channels.map(|mask| channel(value, mask)), 255)
            }
            [b, g, r] => ([r, g, b], 255),
            [b, g, r, a] => {
                let value = u32::from_le_bytes([b, g, r, a]);
                (
                    info.channels.map(|mask| channel(value, mask)),
                    if keep_alpha { a } else { 255 },
                )
            }
            _ => return,
        };
        out.extend(rgb);
        out.push(alpha);
    }
}

fn channel(pixel: u32, mask: u32) -> u8 {
    // A zero mask does not contribute a channel.
    if mask == 0 {
        return 0;
    }
    let offset = mask.trailing_zeros();
    let range = u64::from(mask >> offset);
    let value = u64::from((pixel & mask) >> offset);
    ((255 * value + range / 2) / range) as u8
}

pub(crate) fn clip_blit(
    image: &Rgba,
    source: [i64; 4],
    target: [f64; 4],
) -> Option<(Rgba, [f64; 4])> {
    let bounds = (i64::try_from(image.w).ok()?, i64::try_from(image.h).ok()?);
    let [mut x, mut y, mut w, mut h] = source;
    if w <= 0 || h <= 0 {
        [x, y, w, h] = [0, 0, bounds.0, bounds.1];
    }
    // Locate the source rectangle in the decoded, top-down image.
    if image.bottom_up {
        y = bounds.1.saturating_sub(y).saturating_sub(h);
    }
    let left = x.max(0);
    let top = y.max(0);
    let right = x.saturating_add(w).min(bounds.0);
    let bottom = y.saturating_add(h).min(bounds.1);
    if left >= right || top >= bottom {
        return None;
    }
    let width = (right - left) as usize;
    let height = (bottom - top) as usize;
    let stride = image.w.checked_mul(4)?;
    let start = (left as usize).checked_mul(4)?;
    let row_bytes = width.checked_mul(4)?;
    let mut px = Vec::with_capacity(row_bytes.checked_mul(height)?);
    for line in top as usize..bottom as usize {
        let offset = line.checked_mul(stride)?.checked_add(start)?;
        px.extend_from_slice(image.px.get(offset..offset.checked_add(row_bytes)?)?);
    }
    let sx = target[2] / w as f64;
    let sy = target[3] / h as f64;
    let destination = [
        target[0] + left.saturating_sub(x) as f64 * sx,
        target[1] + top.saturating_sub(y) as f64 * sy,
        width as f64 * sx,
        height as f64 * sy,
    ];
    Some((
        Rgba {
            w: width,
            h: height,
            bottom_up: false,
            px,
        },
        destination,
    ))
}

pub(crate) fn prepare(image: &mut Rgba, blend: Blend) {
    for [r, g, b, a] in image.px.as_chunks_mut::<4>().0 {
        match blend {
            Blend::Rop(0x0088_00c6) if [*r, *g, *b] == [255; 3] => *a = 0,
            Blend::Rop(0x00ee_0086 | 0x0066_0046) if [*r, *g, *b] == [0; 3] => *a = 0,
            Blend::Alpha {
                constant,
                per_pixel,
            } => {
                let alpha = u32::from(*a);
                if per_pixel && (1..255).contains(&alpha) {
                    for component in [r, g, b] {
                        *component =
                            ((u32::from(*component) * 255 + alpha / 2) / alpha).min(255) as u8;
                    }
                }
                *a = (alpha * u32::from(constant) / 255) as u8;
            }
            Blend::Rop(_) => (),
        }
    }
}

pub(crate) fn flip(image: &mut Rgba, horizontal: bool, vertical: bool) {
    let Some(stride) = image.w.checked_mul(4).filter(|&n| n > 0) else {
        return;
    };
    if image.h == 0 {
        return;
    }
    if horizontal {
        for row in image.px.chunks_exact_mut(stride) {
            row.as_chunks_mut::<4>().0.reverse();
        }
    }
    if vertical {
        let count = image.h.min(image.px.len() / stride);
        let (head, tail) = image.px[..count * stride].split_at_mut(count.div_ceil(2) * stride);
        for (upper, lower) in head
            .chunks_exact_mut(stride)
            .zip(tail.chunks_exact_mut(stride).rev())
        {
            upper.swap_with_slice(lower);
        }
    }
}
