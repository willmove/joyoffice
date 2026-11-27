//! Image objects carried by EMF+ drawing and texture brush records.

use std::{cell::Cell, io::Cursor, sync::Arc};

use crate::{MAX_PIXELS, Picture, bitmap::Rgba, bytes::Bytes};

pub(crate) enum ImageData {
    Bitmap(Rgba),
    Meta {
        picture: Arc<Picture>,
        device_size: [f64; 2],
        bytes: usize,
    },
}

thread_local! {
    static LEVEL: Cell<u8> = const { Cell::new(0) };
}

struct Nesting;

impl Nesting {
    fn enter() -> Option<Self> {
        LEVEL.with(|level| {
            let depth = level.get();
            if depth >= 3 {
                return None;
            }
            level.set(depth + 1);
            Some(Self)
        })
    }
}

impl Drop for Nesting {
    fn drop(&mut self) {
        LEVEL.with(|level| level.set(level.get() - 1));
    }
}

pub(crate) fn image(record: Bytes<'_>) -> Option<ImageData> {
    match record.u32(4)? {
        1 => bitmap(record.tail(8)?).map(ImageData::Bitmap),
        2 => {
            let declared = record.u32(12)? as usize;
            let payload = record.tail(16)?.0;
            let _guard = Nesting::enter()?;
            let (picture, device, bytes) = crate::parse_picture(payload.get(..declared)?).ok()?;
            if !picture.has_size() {
                return None;
            }
            let device_size = device
                .filter(|size| size.iter().all(|value| value.is_finite() && *value > 0.0))
                .unwrap_or([f64::from(picture.width), f64::from(picture.height)]);
            Some(ImageData::Meta {
                picture: Arc::new(picture),
                device_size,
                bytes,
            })
        }
        _ => None,
    }
}

enum Pixels {
    Indexed { bits: usize, palette: Vec<[u8; 4]> },
    Bgr,
    Bgrx,
    Bgra,
    Premultiplied,
    Rgb555,
    Rgb565,
}

impl Pixels {
    fn read(format: u32, data: Bytes<'_>) -> Option<(Self, Bytes<'_>)> {
        let bits = ((format >> 8) & 0xff) as usize;
        if format & 0x0001_0000 != 0 {
            let count = (data.u32(4)? as usize).min(256);
            let colors = data.slice(8, 4 * count)?;
            let palette = colors
                .as_chunks::<4>()
                .0
                .iter()
                .map(|&[b, g, r, a]| [r, g, b, a])
                .collect();
            if matches!(bits, 1 | 4 | 8) && count > 0 {
                return Some((
                    Self::Indexed { bits, palette },
                    data.tail(8 + colors.len())?,
                ));
            }
            return Self::read(format & !0x0001_0000, data.tail(8 + colors.len())?);
        }
        let codec = match (bits, format) {
            (16, 0x0002_1006) => Self::Rgb565,
            (16, _) => Self::Rgb555,
            (24, _) => Self::Bgr,
            (32, 0x0002_2009) => Self::Bgrx,
            (32, 0x000E_200B) => Self::Premultiplied,
            (32, _) => Self::Bgra,
            _ => return None,
        };
        Some((codec, data))
    }

    fn pixel(&self, row: &[u8], x: usize) -> Option<[u8; 4]> {
        let scale = |value: u16, max: u16| (value * 255 / max) as u8;
        Some(match self {
            Self::Indexed { bits, palette } => {
                let bit = x * bits;
                let index = usize::from(*row.get(bit / 8)?) >> (8 - bits - bit % 8);
                palette
                    .get(index & ((1 << bits) - 1))
                    .copied()
                    .unwrap_or([0, 0, 0, 255])
            }
            Self::Bgr => {
                let &[b, g, r] = row.get(3 * x..)?.first_chunk()?;
                [r, g, b, 255]
            }
            Self::Bgrx | Self::Bgra | Self::Premultiplied => {
                let &[b, g, r, a] = row.get(4 * x..)?.first_chunk()?;
                match self {
                    Self::Bgrx => [r, g, b, 255],
                    Self::Premultiplied => crate::bitmap::straight([r, g, b, a]),
                    _ => [r, g, b, a],
                }
            }
            Self::Rgb555 | Self::Rgb565 => {
                let word = u16::from_le_bytes(*row.get(2 * x..)?.first_chunk()?);
                let (r, g, max) = if matches!(self, Self::Rgb565) {
                    (word >> 11, (word >> 5) & 63, 63)
                } else {
                    ((word >> 10) & 31, (word >> 5) & 31, 31)
                };
                [scale(r, 31), scale(g, max), scale(word & 31, 31), 255]
            }
        })
    }
}

fn bitmap(record: Bytes<'_>) -> Option<Rgba> {
    let payload = record.tail(20)?;
    if record.u32(16)? == 1 {
        return compressed(payload.0);
    }
    let w = usize::try_from(record.i32(0)?).ok()?;
    let h = usize::try_from(record.i32(4)?).ok()?;
    let stride = record.i32(8)?;
    let format = record.u32(12)?;
    if w == 0 || h == 0 || w.checked_mul(h)? > MAX_PIXELS {
        return None;
    }
    let (codec, pixels) = Pixels::read(format, payload)?;
    let row = w.checked_mul(((format >> 8) & 0xff) as usize)?.div_ceil(8);
    let step = (stride.unsigned_abs() as usize).max(row);
    pixels.slice(step.checked_mul(h - 1)?, row)?;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let source = if stride < 0 { h - 1 - y } else { y };
        let scan = pixels.slice(source * step, row)?;
        for x in 0..w {
            rgba.extend_from_slice(&codec.pixel(scan, x)?);
        }
    }
    Some(Rgba {
        w,
        h,
        bottom_up: false,
        px: rgba,
    })
}

fn compressed(bytes: &[u8]) -> Option<Rgba> {
    let reader = || {
        let mut source = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(1 << 15);
        limits.max_image_height = Some(1 << 15);
        limits.max_alloc = Some((MAX_PIXELS * 4) as u64);
        source.limits(limits);
        Some(source)
    };
    let (w, h) = reader()?.into_dimensions().ok()?;
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || w.checked_mul(h)? > MAX_PIXELS {
        return None;
    }
    let px = reader()?.decode().ok()?.into_rgba8().into_raw();
    Some(Rgba {
        w,
        h,
        bottom_up: false,
        px,
    })
}
