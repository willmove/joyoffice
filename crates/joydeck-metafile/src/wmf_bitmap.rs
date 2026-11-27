//! Windows Metafile Bitmap16 data and device-dependent raster transfers.

use crate::{
    MAX_PIXELS,
    bitmap::Rgba,
    bytes::Bytes,
    canvas::{Brush, Canvas},
    image_placement::rectangle,
    raster_transfer::{Logic, RasterTransfer, TransferSource},
};

pub(crate) fn pattern(record: Bytes<'_>) -> Option<Brush> {
    // META_CREATEPATTERNBRUSH includes a 32-bit pointer placeholder and 18 reserved bytes.
    let data = record.tail(6)?;
    let (pixels, mono) = bitmap16(data, 32)?;
    Brush::pattern(pixels, mono)
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
    let (mut pixels, mono) = bitmap16(record.tail(dest_at + 8)?, 10)?;
    if mono {
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

fn bitmap16(header: Bytes<'_>, at: usize) -> Option<(Rgba, bool)> {
    let (w, h) = (
        usize::try_from(header.i16(2)?).ok()?,
        usize::try_from(header.i16(4)?).ok()?,
    );
    let stride = usize::try_from(header.i16(6)?).ok()?;
    let [planes, depth] = *header.0.get(8..10)?.first_chunk()?;
    let pixels = w.checked_mul(h)?;
    if pixels == 0
        || pixels > MAX_PIXELS
        || header.u16(0)? != 0
        || planes != 1
        || !matches!(depth, 1 | 16 | 24 | 32)
    {
        return None;
    }
    let minimum = w.checked_mul(usize::from(depth))?.div_ceil(16) * 2;
    if stride < minimum || stride % 2 != 0 {
        return None;
    }
    let bytes = header.slice(at, stride.checked_mul(h)?)?;
    let mut px = Vec::with_capacity(pixels * 4);
    for row in bytes.chunks_exact(stride) {
        for x in 0..w {
            let [r, g, b] = match depth {
                1 => [255 * ((row[x / 8] >> (7 - x % 8)) & 1); 3],
                16 => {
                    let word = u16::from_le_bytes([row[x * 2], row[x * 2 + 1]]);
                    [10, 5, 0].map(|shift| (((word >> shift) & 31) * 255 / 31) as u8)
                }
                24 | 32 => {
                    let start = x * usize::from(depth / 8);
                    [row[start + 2], row[start + 1], row[start]]
                }
                _ => return None,
            };
            px.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Some((
        Rgba {
            w,
            h,
            px,
            bottom_up: false,
        },
        depth == 1,
    ))
}
