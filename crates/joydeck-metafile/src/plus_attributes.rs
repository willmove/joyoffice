//! EMF+ image attributes describe how outward samples are produced.

use crate::{bitmap::Rgba, bytes::Bytes, plus_paint::argb};

const MAX_EXTENDED_PIXELS: usize = 8_000_000;

#[derive(Clone, Copy)]
pub struct Attributes {
    wrap: u32,
    clamp: [u8; 4],
}

impl Attributes {
    pub fn read(bytes: Bytes<'_>) -> Option<Self> {
        let wrap = bytes.u32(8)?;
        let object_clamp = bytes.u32(16)?;
        bytes.u32(20)?;
        (wrap <= 4 && object_clamp <= 1).then_some(Self {
            wrap,
            clamp: argb(bytes.u32(12)?),
        })
    }

    pub fn extend(
        self,
        image: &Rgba,
        source: &mut [f64; 4],
        work: &mut usize,
    ) -> Option<Option<Rgba>> {
        let [x, y, w, h] = *source;
        let left = x.min(x + w).floor();
        let top = y.min(y + h).floor();
        let right = x.max(x + w).ceil();
        let bottom = y.max(y + h).ceil();
        if left >= 0.0 && top >= 0.0 && right <= image.w as f64 && bottom <= image.h as f64 {
            return Some(None);
        }
        if left.abs().max(right.abs()).max(top.abs()).max(bottom.abs()) > 1e9
            || image.w == 0
            || image.h == 0
        {
            return None;
        }
        let width = (right - left) as usize;
        let height = (bottom - top) as usize;
        let area = width.checked_mul(height)?;
        if area > MAX_EXTENDED_PIXELS || area == 0 {
            return None;
        }
        *work = work.checked_sub(area)?;
        let mut px = Vec::with_capacity(area * 4);
        for row in 0..height {
            let sy = self.coordinate(top as i64 + row as i64, image.h, true);
            for col in 0..width {
                let sx = self.coordinate(left as i64 + col as i64, image.w, false);
                let color = match (sx, sy) {
                    (Some(u), Some(v)) => {
                        image.px.as_chunks::<4>().0.get(v * image.w + u).copied()?
                    }
                    _ => self.clamp,
                };
                px.extend_from_slice(&color);
            }
        }
        source[0] -= left;
        source[1] -= top;
        Some(Some(Rgba {
            w: width,
            h: height,
            bottom_up: false,
            px,
        }))
    }

    fn coordinate(self, position: i64, len: usize, vertical: bool) -> Option<usize> {
        let len = i64::try_from(len).ok()?;
        if self.wrap == 4 {
            // Native playback paints ClampColor for out-of-image source samples with either
            // ObjectClamp value. The image sampler handles the filtered boundary.
            return (0..len).contains(&position).then_some(position as usize);
        }
        let flip = self.wrap == 3 || self.wrap == if vertical { 2 } else { 1 };
        let bit = position.rem_euclid(if flip { len * 2 } else { len });
        Some(if bit >= len {
            (len * 2 - 1 - bit) as usize
        } else {
            bit as usize
        })
    }
}
