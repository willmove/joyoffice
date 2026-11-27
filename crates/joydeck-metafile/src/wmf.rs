//! WMF reader (MS-WMF). A file is an optional 22-byte placeable header, a 9-word standard header, then
//! records of (size in 16-bit words, function, parameters). Parameters are read at byte 6 of each record.

use std::sync::Arc;

use crate::bitmap;
use crate::bytes::Bytes;
use crate::canvas::{BS_HATCHED, Brush, Canvas, LogFont, Obj, Pen, colorref};
use crate::records::{self, Format};
use crate::text::{Font, decode_bytes};
use crate::text_records;
use crate::vector_records::{play_wmf, wmf_point as xy16, wmf_rect as rect16};
use crate::{Error, Frame};

const PLACEABLE_KEY: u32 = 0x9AC6_CDD7;
const DEFAULT_INCH: f64 = 1440.0;
/// A placeable WMF plays into a target sized in points; brushes are laid out in 96-dpi pixels.
const PT_PER_PX: f64 = 72.0 / 96.0;

pub(crate) fn is_wmf(b: Bytes) -> bool {
    if b.u32(0) == Some(PLACEABLE_KEY) {
        return true;
    }
    matches!(b.u16(0), Some(1 | 2))
        && b.u16(2) == Some(9)
        && matches!(b.u16(4), Some(0x100 | 0x300))
}

/// Reads all records. Returns the drawing state and the picture frame, or an error when no record
/// could be read at all.
pub(crate) fn parse(input: Bytes) -> Result<(Canvas, Option<Frame>), Error> {
    let placeable = input.u32(0) == Some(PLACEABLE_KEY);
    // Placeable bounds: logical corners (left, top, right, bottom) and their size in points.
    let (hdr, bounds) = if placeable {
        let [l, t, r, b] = [6, 8, 10, 12].map(|o| input.i16(o).map(f64::from));
        let (Some(l), Some(t), Some(r), Some(b)) = (l, t, r, b) else {
            return Err(Error::BadHeader);
        };
        let inch = input
            .u16(14)
            .filter(|v| *v > 0)
            .map_or(DEFAULT_INCH, f64::from);
        let (w, h) = (r - l, b - t);
        let bounds =
            (w > 0.0 && h > 0.0).then_some(([l, t, r, b], (w * 72.0 / inch, h * 72.0 / inch)));
        (22, bounds)
    } else {
        (0, None)
    };
    let hsize = input.u16(hdr + 2).ok_or(Error::BadHeader)? as usize;
    if hsize < 9 {
        return Err(Error::BadHeader);
    }
    let pos = hdr
        .checked_add(hsize.checked_mul(2).ok_or(Error::BadHeader)?)
        .ok_or(Error::BadHeader)?;
    // Played into a target like Word does: the placeable bounds are the first window, the target (the
    // picture size) the viewport. A plain WMF's target is its first window.
    let mut cv = Canvas::new();
    if let Some((c, size)) = bounds {
        cv.play_into(Some([c[0], c[1], c[2] - c[0], c[3] - c[1]]), size);
        cv.set_clip_surface([0.0, 0.0, size.0, size.1]);
        cv.pattern_px = PT_PER_PX;
    }
    let mut player_viewport = bounds.is_some();
    let mut parsed = 0usize;
    for entry in records::read(input, pos, Format::Wmf) {
        if cv.full() || entry.kind == 0 {
            break;
        }
        record(&mut cv, entry.kind as u16, entry.data);
        if entry.kind == 0x020C && cv.window_set && !player_viewport && !cv.viewport_set {
            let we = cv.map.window_size;
            cv.play_into(None, (we.0.abs(), we.1.abs()));
            player_viewport = true;
        }
        parsed += 1;
    }
    if parsed == 0 {
        return Err(Error::NoRecords);
    }
    let frame = match bounds {
        // Played into its target: the frame is the target.
        Some((_, size)) if !cv.viewport_set && cv.map.uses_extents() => Some(Frame {
            rect: [0.0, 0.0, size.0, size.1],
            size: Some(size),
        }),
        // The file set its own viewport, so the drawing is in its device units and the logical bounds
        // go through the final mapping.
        Some((c, size)) => {
            let (a, b) = (cv.map.device((c[0], c[1])), cv.map.device((c[2], c[3])));
            let rect = [
                a.0.min(b.0),
                a.1.min(b.1),
                (b.0 - a.0).abs(),
                (b.1 - a.1).abs(),
            ];
            (rect[2] > 0.0 && rect[3] > 0.0).then_some(Frame {
                rect,
                size: Some(size),
            })
        }
        None if player_viewport && !cv.viewport_set && cv.map.uses_extents() => {
            let ve = cv.map.viewport_size;
            Some(Frame {
                rect: [0.0, 0.0, ve.0, ve.1],
                size: None,
            })
        }
        None if cv.window_set => Some(Frame {
            rect: cv.map.window_rect(),
            size: None,
        }),
        None => None,
    };
    Ok((cv, frame))
}

fn record(cv: &mut Canvas, func: u16, r: Bytes) {
    if cv.full() {
        return;
    }
    let _ = match func {
        0x0103 => r.u16(6).map(|m| cv.set_map_mode(u32::from(m))),
        0x0106 => r.u16(6).map(|m| cv.set_polyfill(u32::from(m))),
        0x0104 => r.u16(6).map(|m| cv.set_rop2(u32::from(m))),
        // Y is stored before X in these records.
        0x020B => xy16(r, 6).map(|p| cv.set_window_org(p)),
        0x020C => xy16(r, 6).map(|p| cv.set_window_ext(p)),
        0x020D => xy16(r, 6).map(|p| cv.set_viewport_org(p)),
        0x020E => xy16(r, 6).map(|p| cv.set_viewport_ext(p)),
        0x001E => {
            cv.save_dc();
            Some(())
        }
        0x0127 => r.i16(6).map(|n| cv.restore_dc(i32::from(n))),
        0x0415 => rect16(r, 6).map(|rect| cv.exclude_clip_rect(rect)),
        0x0416 => rect16(r, 6).map(|rect| cv.intersect_clip_rect(rect)),
        0x02FA => create_pen(cv, r),
        0x02FC => create_brush(cv, r),
        0x02FB => {
            let f = font(r).unwrap_or_default();
            cv.add_object(None, Obj::Font(Arc::new(f)));
            Some(())
        }
        0x0142 => {
            // A pattern the reader cannot decode still takes its handle slot.
            let b = dib_pattern_brush(r).map_or(Obj::Other, Obj::Brush);
            cv.add_object(None, b);
            Some(())
        }
        _ if func as u8 == 0xf9 => {
            let brush = crate::wmf_bitmap::pattern(r).map_or(Obj::Other, Obj::Brush);
            cv.add_object(None, brush);
            Some(())
        }
        _ if matches!(func as u8, 0xf7 | 0x34..=0x37 | 0x39) => r
            .tail(6)
            .and_then(|bytes| crate::wmf_palette::play(cv, func as u8, bytes)),
        // Unhandled region objects take a slot so later handles still line up.
        0x06FF => {
            cv.add_object(None, Obj::Other);
            Some(())
        }
        0x0209 => r.u32(6).map(|c| cv.set_text_color(c)),
        0x0201 => r.u32(6).map(|c| cv.set_bk_color(c)),
        0x0102 => r.u16(6).map(|m| cv.set_bk_mode(u32::from(m))),
        0x012E => r.u16(6).map(|a| cv.set_text_align(u32::from(a))),
        0x0A32 | 0x0521 => text_records::play_wmf(cv, r, func == 0x0A32),
        // PATBLT: rop(4), height, width, y, x.
        0x061D => r
            .u32(6)
            .map(|rop| cv.fill_rop([i16s(r, 16), i16s(r, 14), i16s(r, 12), i16s(r, 10)], rop)),
        0x012D => r.u16(6).map(|h| cv.select(wmf_handle(h))),
        0x01F0 => r.u16(6).map(|h| cv.delete_object(u32::from(h))),
        // STRETCHDIB: rop(4) usage(2) srcH srcW ySrc xSrc destH destW yDest xDest, then the DIB.
        0x0F43 => {
            let rop = r.u32(6).unwrap_or(0);
            dib_blit(
                cv,
                r,
                28,
                rop,
                [i16s(r, 26), i16s(r, 24), i16s(r, 22), i16s(r, 20)],
                [i16s(r, 18), i16s(r, 16), i16s(r, 14), i16s(r, 12)],
            )
        }
        0x0B41 => dib_stretchblt(cv, r),
        0x0940 => dib_bitblt(cv, r),
        _ if matches!(func as u8, 0x22 | 0x23) => crate::wmf_bitmap::blit(cv, func, r),
        _ => play_wmf(cv, func, r),
    };
}

/// A 16-bit WMF handle as a canvas object index: stock objects are `0x8000 | index`, which the
/// canvas reads with the 32-bit stock flag.
fn wmf_handle(h: u16) -> u32 {
    let h = u32::from(h);
    if h & 0x8000 != 0 {
        (h & 0x7FFF) | 0x8000_0000
    } else {
        h
    }
}

/// An i16 field as f64; 0 past the end.
fn i16s(r: Bytes, off: usize) -> f64 {
    r.i16(off).map_or(0.0, f64::from)
}

/// CREATEPENINDIRECT: style, width (x, y), colour. Only the width's x component is used.
fn create_pen(cv: &mut Canvas, r: Bytes) -> Option<()> {
    let style = r.u16(6)?;
    let width = r.i16(8)?;
    let color = r.u32(12)?;
    cv.add_object(
        None,
        Obj::Pen(Pen {
            style: u32::from(style),
            width: f64::from(width),
            color: colorref(color),
        }),
    );
    Some(())
}

/// CREATEBRUSHINDIRECT: style, colour, hatch.
fn create_brush(cv: &mut Canvas, r: Bytes) -> Option<()> {
    let style = u32::from(r.u16(6)?);
    let color = r.u32(8)?;
    let mut b = Brush::new(style, colorref(color));
    if style == BS_HATCHED {
        b.hatch = u32::from(r.u16(12).unwrap_or(0));
    }
    cv.add_object(None, Obj::Brush(b));
    Some(())
}

/// DIBCREATEPATTERNBRUSH: style, colour usage, then a DIB (MS-WMF 2.3.4.8). The DIB's own palette
/// gives the colours; with DIB_PAL_COLORS (indices into a palette the reader does not keep) a
/// monochrome DIB takes the text and background colours.
fn dib_pattern_brush(r: Bytes) -> Option<Brush> {
    let usage = r.u16(8)?;
    let dib = r.tail(10)?;
    let n = bitmap::info_len(dib)?;
    let img = bitmap::decode(Bytes(dib.slice(0, n)?), dib.tail(n)?, false)?;
    Brush::pattern(img, usage == 1)
}

/// Font object (MS-WMF 2.2.1.2): height, width, escapement, orientation, weight (i16 each), italic,
/// underline, strike-out and charset bytes, three precision/quality bytes, pitch and family, then the
/// face name (up to 32 bytes, NUL-terminated, in the font's character set).
fn font(r: Bytes) -> Option<LogFont> {
    let height = r.i16(6)?;
    let width = r.i16(8).unwrap_or(0);
    let escapement = r.i16(10).unwrap_or(0);
    let weight = r.i16(14).unwrap_or(0);
    let byte = |o: usize| r.0.get(o).copied().unwrap_or(0);
    let charset = byte(19);
    let name = r.0.get(24..).unwrap_or(&[]);
    let name = name.get(..32).unwrap_or(name);
    let name = name.split(|b| *b == 0).next().unwrap_or(&[]);
    // Face names are plain ASCII in practice; other bytes go through the Western code page.
    let (family, _) = decode_bytes(name, 0, &[]);
    Some(LogFont {
        font: Font {
            family: family.trim().to_string(),
            height: f32::from(height),
            width: f32::from(width),
            weight: weight.clamp(0, 1000) as u16,
            italic: byte(16) != 0,
            underline: byte(17) != 0,
            strike: byte(18) != 0,
            charset,
        },
        escapement: f64::from(escapement),
    })
}

/// Whether a DIBBITBLT or DIBSTRETCHBLT record carries a bitmap: without one it is exactly
/// `(function >> 8) + 3` words long (MS-WMF 2.3.1.2, 2.3.1.3).
fn has_bitmap(func: u16, r: Bytes) -> bool {
    r.0.len() / 2 > usize::from(func >> 8) + 3
}

/// META_DIBBITBLT: rop(4), ySrc, xSrc, then height, width, yDest, xDest and the DIB at 22. The
/// bitmap-less form has a reserved word before the height, so its fields sit 2 bytes later; it, or
/// a source-less raster operation, fills the destination.
fn dib_bitblt(cv: &mut Canvas, r: Bytes) -> Option<()> {
    let rop = r.u32(6)?;
    if !has_bitmap(0x0940, r) {
        cv.fill_rop([i16s(r, 22), i16s(r, 20), i16s(r, 18), i16s(r, 16)], rop);
        return Some(());
    }
    let dst = [i16s(r, 20), i16s(r, 18), i16s(r, 16), i16s(r, 14)];
    if Canvas::sourceless(rop) {
        cv.fill_rop(dst, rop);
        return Some(());
    }
    let src = [i16s(r, 12), i16s(r, 10), dst[2], dst[3]];
    dib_blit(cv, r, 22, rop, dst, src)
}

/// META_DIBSTRETCHBLT: rop(4), srcH, srcW, ySrc, xSrc, then destH, destW, yDest, xDest and the DIB at
/// 26. The bitmap-less form has a reserved word after xSrc, so the destination sits 2 bytes later;
/// it, or a source-less raster operation, fills the destination.
fn dib_stretchblt(cv: &mut Canvas, r: Bytes) -> Option<()> {
    let rop = r.u32(6)?;
    if !has_bitmap(0x0B41, r) {
        cv.fill_rop([i16s(r, 26), i16s(r, 24), i16s(r, 22), i16s(r, 20)], rop);
        return Some(());
    }
    let dst = [i16s(r, 24), i16s(r, 22), i16s(r, 20), i16s(r, 18)];
    if Canvas::sourceless(rop) {
        cv.fill_rop(dst, rop);
        return Some(());
    }
    let src = [i16s(r, 16), i16s(r, 14), i16s(r, 12), i16s(r, 10)];
    dib_blit(cv, r, 26, rop, dst, src)
}

/// A DIB at `at`, stretched from source rectangle `src` into `dst` (both in logical units, pixels for `src`).
/// A ROP other than source copy is evaluated with the selected brush and the current destination.
fn dib_blit(
    cv: &mut Canvas,
    r: Bytes,
    at: usize,
    rop: u32,
    dst: [f64; 4],
    src: [f64; 4],
) -> Option<()> {
    use crate::image_placement::rectangle;
    use crate::raster_transfer::{Logic, RasterTransfer, TransferSource};

    if Canvas::sourceless(rop) {
        cv.fill_rop(dst, rop);
        return Some(());
    }
    let dib = r.tail(at)?;
    let n = bitmap::info_len(dib)?;
    let info = dib.slice(0, n)?;
    let bits = dib.tail(n)?;
    let img = bitmap::decode(Bytes(info), bits, false)?;
    let logic = Logic::single(rop);
    if logic.foreground != Logic::SOURCE {
        let source = TransferSource::dib(img, src)?;
        cv.raster_transfer(RasterTransfer::new(rectangle(dst), logic, Some(source)));
        return Some(());
    }
    let (img, dst) = bitmap::clip_blit(&img, src.map(|v| v as i64), dst)?;
    cv.bitmap(dst, img);
    Some(())
}
