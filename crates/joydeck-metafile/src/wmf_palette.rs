//! Logical WMF palettes and the portable GDI+ indexed color defaults.

use crate::{
    bytes::Bytes,
    canvas::{Canvas, Obj},
};

pub(crate) type Entry = [u8; 4];
const MAX_COLORS: usize = 256;
const RESERVED: u8 = 1;
const EXPLICIT: u8 = 2;

pub(crate) fn play(canvas: &mut Canvas, function: u8, data: Bytes<'_>) -> Option<()> {
    match function {
        0xf7 => {
            let palette = entries(data).and_then(|(start, entries)| {
                (start == 0x0300)
                    .then(|| Obj::Palette(entries[..entries.len().min(MAX_COLORS)].to_vec()))
            });
            // A damaged object still consumes its implicit handle.
            canvas.add_object(None, palette.unwrap_or(Obj::Other));
        }
        0x34 => canvas.select_palette(data.u16(0)?),
        // In the true-color scene colors are resolved when a record draws.
        0x35 => {}
        0x36 | 0x37 => {
            let (start, colors) = entries(data)?;
            let palette = canvas.selected_palette_mut()?;
            if let Some(destination) = palette.get_mut(start..) {
                for (target, color) in destination.iter_mut().zip(colors) {
                    if function == 0x37 {
                        *target = *color;
                    } else if target[3] & RESERVED != 0 {
                        target[..3].copy_from_slice(&color[..3]);
                    }
                }
            }
        }
        0x39 => canvas
            .selected_palette_mut()?
            .resize(usize::from(data.u16(0)?).min(MAX_COLORS), [0; 4]),
        _ => return None,
    }
    Some(())
}

fn entries(data: Bytes<'_>) -> Option<(usize, &[Entry])> {
    let first = usize::from(data.u16(0)?);
    let count = usize::from(data.u16(2)?);
    Some((first, data.slice(4, count * 4)?.as_chunks::<4>().0))
}

pub(crate) fn color(palette: &[Entry], depth: u8, index: u8) -> [u8; 4] {
    if let Some(&[r, g, b, flags]) = palette.get(usize::from(index)) {
        if flags & EXPLICIT != 0 {
            // A hardware palette is unavailable for an archived DDB. Use the same indexed
            // default as an untagged 8-bit bitmap for this physical index.
            return default_color(8, u8::try_from(u16::from_le_bytes([r, g])).unwrap_or(0));
        }
        return [r, g, b, 255];
    }
    default_color(depth, index)
}

fn default_color(depth: u8, index: u8) -> [u8; 4] {
    // Observed defaults for Windows Format4bppIndexed and Format8bppIndexed.
    const BASIC: [[u8; 3]; 16] = [
        [0, 0, 0],
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
        [128, 128, 128],
        [192, 192, 192],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
    ];
    let [r, g, b] = if let Some(color) = BASIC.get(usize::from(index)) {
        *color
    } else if depth == 8 && index >= 40 {
        let step = index - 40;
        [step / 36 * 51, (step / 6) % 6 * 51, step % 6 * 51]
    } else {
        [0; 3]
    };
    [r, g, b, 255]
}
