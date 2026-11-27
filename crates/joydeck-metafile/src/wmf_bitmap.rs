//! Windows Metafile Bitmap16 data and device-dependent raster transfers.

use crate::{
    MAX_PIXELS,
    bitmap::Rgba,
    bytes::Bytes,
    canvas::{Brush, Canvas, MAX_TILE_SIDE},
    image_placement::rectangle,
    raster_transfer::{Logic, RasterTransfer, TransferSource},
    wmf_palette::{self, Entry},
};

pub(crate) fn pattern(record: Bytes<'_>) -> Option<Brush> {
    // META_CREATEPATTERNBRUSH includes a 32-bit pointer placeholder and 18 reserved bytes.
    let data = record.tail(6)?;
    let bitmap = Bitmap16::read(data, 32)?;
    if bitmap.w > MAX_TILE_SIDE || bitmap.h > MAX_TILE_SIDE {
        return None;
    }
    if matches!(bitmap.depth, 4 | 8) {
        let indices = bitmap
            .rows()
            .flat_map(|row| {
                let bitmap = &bitmap;
                (0..bitmap.w).map(move |x| bitmap.index(row, x))
            })
            .collect();
        return Brush::indexed_pattern(bitmap.w, bitmap.h, bitmap.depth, indices);
    }
    Brush::pattern(bitmap.rgba(&[]), bitmap.depth == 1)
}

pub(crate) fn blit(canvas: &mut Canvas, function: u16, record: Bytes<'_>) -> Option<()> {
    let stretch = function as u8 == 0x23;
    let rop = record.u32(6)?;
    let embedded = record.0.len() / 2 != usize::from(function >> 8) + 3;
    let dest_at = if stretch { 18 } else { 14 } + 2 * usize::from(!embedded);
    let field = |offset| record.i16(offset).map(f64::from);
    let destination = [
        field(dest_at + 6)?,
        field(dest_at + 4)?,
        field(dest_at + 2)?,
        field(dest_at)?,
    ];
    if Canvas::sourceless(rop) {
        canvas.fill_rop(destination, rop);
        return Some(());
    }
    // In the bitmap-less forms MS-WMF rejects operations that require a source.
    if !embedded {
        return None;
    }
    let source = if stretch {
        [field(16)?, field(14)?, field(12)?, field(10)?]
    } else {
        [field(12)?, field(10)?, destination[2], destination[3]]
    };
    if source[2] == 0.0 || source[3] == 0.0 {
        return None;
    }
    let bitmap = Bitmap16::read(record.tail(dest_at + 8)?, 10)?;
    let mut pixels = bitmap.rgba(canvas.selected_palette());
    if bitmap.depth == 1 {
        let (text, background) = canvas.monochrome_colors();
        for pixel in pixels.px.as_chunks_mut::<4>().0 {
            *pixel = if pixel[0] == 0 { text } else { background };
        }
    }
    let source = TransferSource::dib(pixels, source)?;
    canvas.raster_transfer(RasterTransfer::new(
        rectangle(destination),
        Logic::single(rop),
        Some(source),
    ));
    Some(())
}

struct Bitmap16<'a> {
    w: usize,
    h: usize,
    stride: usize,
    depth: u8,
    bytes: &'a [u8],
}

impl<'a> Bitmap16<'a> {
    fn read(header: Bytes<'a>, at: usize) -> Option<Self> {
        let (w, h) = (
            usize::try_from(header.i16(2)?).ok()?,
            usize::try_from(header.i16(4)?).ok()?,
        );
        let stride = usize::try_from(header.i16(6)?).ok()?;
        let [planes, depth] = *header.0.get(8..10)?.first_chunk()?;
        if !(1..=MAX_PIXELS).contains(&w.checked_mul(h)?)
            || header.u16(0)? != 0
            || planes != 1
            || !matches!(depth, 1 | 4 | 8 | 16 | 24 | 32)
        {
            return None;
        }
        let minimum = w.checked_mul(usize::from(depth))?.div_ceil(16) * 2;
        if stride < minimum || stride % 2 != 0 {
            return None;
        }
        let bytes = header.slice(at, stride.checked_mul(h)?)?;
        Some(Self {
            w,
            h,
            stride,
            depth,
            bytes,
        })
    }

    fn rows(&self) -> impl Iterator<Item = &'a [u8]> {
        self.bytes.chunks_exact(self.stride)
    }

    fn index(&self, row: &[u8], x: usize) -> u8 {
        if self.depth == 4 {
            (row[x / 2] >> (4 * (1 - x % 2))) & 15
        } else {
            row[x]
        }
    }

    fn rgba(&self, palette: &[Entry]) -> Rgba {
        let mut px = Vec::with_capacity(self.w * self.h * 4);
        for row in self.rows() {
            for x in 0..self.w {
                px.extend_from_slice(&self.pixel(row, x, palette));
            }
        }
        Rgba {
            w: self.w,
            h: self.h,
            px,
            bottom_up: false,
        }
    }

    fn pixel(&self, row: &[u8], x: usize, palette: &[Entry]) -> [u8; 4] {
        let [r, g, b] = match self.depth {
            1 => [255 * ((row[x / 8] >> (7 - x % 8)) & 1); 3],
            4 | 8 => return wmf_palette::color(palette, self.depth, self.index(row, x)),
            16 => {
                let word = u16::from_le_bytes([row[x * 2], row[x * 2 + 1]]);
                [10, 5, 0].map(|shift| (((word >> shift) & 31) * 255 / 31) as u8)
            }
            24 | 32 => {
                let start = x * usize::from(self.depth / 8);
                [row[start + 2], row[start + 1], row[start]]
            }
            _ => [0; 3],
        };
        [r, g, b, 255]
    }
}
