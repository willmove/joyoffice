//! Source and mask fields from MS-EMF bitmap transfers.

use kurbo::Affine;

use crate::bitmap;
use crate::bytes::Bytes;
use crate::canvas::{Canvas, colorref};
use crate::image_placement::{parallelogram, rectangle};
use crate::raster_transfer::{Logic, MonoMask, RasterTransfer, TransferSource};

pub(crate) fn play(canvas: &mut Canvas, kind: u32, record: Bytes<'_>) -> Option<()> {
    let (destination, source, mut logic, source_info, mask_origin) = match kind {
        78 => {
            let [x, y, w, h] = fields::<4>(record, 24)?;
            let [sx, sy] = fields::<2>(record, 44)?;
            (
                rectangle([x, y, w, h]),
                [sx, sy, w, h],
                Logic::masked(record.u32(40)?),
                84,
                Some(100),
            )
        }
        79 => {
            let [x, y, ux, uy, vx, vy] = fields::<6>(record, 24)?;
            (
                parallelogram([(x, y), (ux, uy), (vx, vy)]),
                fields::<4>(record, 48)?,
                Logic {
                    foreground: Logic::SOURCE,
                    background: Logic::DESTINATION,
                },
                96,
                Some(112),
            )
        }
        116 => {
            let [sx, sy] = fields::<2>(record, 44)?;
            let [w, h] = fields::<2>(record, 100)?;
            (
                rectangle(fields::<4>(record, 24)?),
                [sx, sy, w, h],
                Logic::single(u32::from(Logic::SOURCE) << 16),
                84,
                None,
            )
        }
        _ => return None,
    };
    let mask = if let Some(offset) = mask_origin {
        let info = offset + 12;
        if record.u32(info + 4)? != 0 || record.u32(info + 12)? != 0 {
            let (header, bits) = planes(record, info)?;
            Some(MonoMask::read(
                header,
                bits,
                fields::<2>(record, offset)?,
                [source[2], source[3]],
            )?)
        } else {
            None
        }
    } else {
        None
    };
    if mask.is_none() {
        logic.background = logic.foreground;
    }
    let source = if logic.uses_source() {
        Some(dc_source(record, source, source_info)?)
    } else {
        None
    };
    let mut transfer = RasterTransfer::new(destination, logic, source).with_mask(mask);
    if kind == 116 {
        let [r, g, b, _] = colorref(record.u32(40)?);
        transfer = transfer.with_color_key([r, g, b]);
    }
    canvas.raster_transfer(transfer);
    Some(())
}

pub(crate) fn dc_source(record: Bytes<'_>, source: [f64; 4], at: usize) -> Option<TransferSource> {
    let (info, bits) = planes(record, at)?;
    let mut image = bitmap::decode(info, bits, false)?;
    // A DC's origin is at the top left. The DIB height sign controls storage, not this origin.
    image.bottom_up = false;
    let mut matrix = [0.0; 6];
    for (i, value) in matrix.iter_mut().enumerate() {
        *value = f64::from(record.f32(at - 32 + i * 4)?);
    }
    let base = if matrix == [0.0; 6] {
        Affine::IDENTITY
    } else {
        Affine::new(matrix)
    };
    TransferSource::new(image, base * rectangle(source))
}

fn planes(record: Bytes<'_>, at: usize) -> Option<(Bytes<'_>, Bytes<'_>)> {
    let pair = |offset| {
        let start = record.u32(offset)? as usize;
        let len = record.u32(offset + 4)? as usize;
        (start >= 8 && len != 0).then_some(Bytes(record.slice(start, len)?))
    };
    Some((pair(at)?, pair(at + 8)?))
}

fn fields<const N: usize>(record: Bytes<'_>, at: usize) -> Option<[f64; N]> {
    let mut values = [0.0; N];
    for (i, value) in values.iter_mut().enumerate() {
        *value = f64::from(record.i32(at + i * 4)?);
    }
    Some(values)
}
