//! Bitwise GDI transfer operations and independent source, mask and brush sampling.

use std::sync::Arc;

use kurbo::{Affine, Point, Rect};

use crate::{ImagePlacement, bitmap::Rgba, bytes::Bytes, canvas::RawTile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Logic {
    pub foreground: u8,
    pub background: u8,
}

impl Logic {
    pub const SOURCE: u8 = 0xcc;
    pub const DESTINATION: u8 = 0xaa;

    pub fn single(value: u32) -> Self {
        let index = (value >> 16) as u8;
        Self {
            foreground: index,
            background: index,
        }
    }
    pub fn masked(value: u32) -> Self {
        Self {
            foreground: (value >> 16) as u8,
            background: (value >> 24) as u8,
        }
    }

    pub fn uses_source(self) -> bool {
        [self.foreground, self.background]
            .iter()
            .any(|&code| code & 0x33 != (code >> 2) & 0x33)
    }

    pub fn uses_destination(self) -> bool {
        [self.foreground, self.background]
            .iter()
            .any(|&code| code != Self::DESTINATION && code & 0x55 != (code >> 1) & 0x55)
    }

    pub fn uses_pattern(self) -> bool {
        [self.foreground, self.background]
            .iter()
            .any(|&code| code & 0x0f != code >> 4)
    }
}

/// Truth-table order is pattern, source, destination, with destination the low bit.
pub fn ternary(code: u8, pattern: u8, source: u8, destination: u8) -> u8 {
    let from_pair = |pair: u8| {
        (destination & (0u8.wrapping_sub((pair >> 1) & 1)))
            | (!destination & (0u8.wrapping_sub(pair & 1)))
    };
    let p0 = (from_pair(code) & !source) | (from_pair(code >> 2) & source);
    let p1 = (from_pair(code >> 4) & !source) | (from_pair(code >> 6) & source);
    (p0 & !pattern) | (p1 & pattern)
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransferSource {
    width: usize,
    height: usize,
    rgba: Arc<[u8]>,
    /// Maps transfer coordinates (unit square) into the top-down bitmap.
    transform: Affine,
}

impl TransferSource {
    pub(crate) fn new(pixels: Rgba, transform: Affine) -> Option<Self> {
        if !transform.is_finite() || pixels.w == 0 || pixels.h == 0 {
            return None;
        }
        let transform = if pixels.bottom_up {
            let [a, b, c, d, e, f] = transform.as_coeffs();
            Affine::new([a, -b, c, d, e, pixels.h as f64 - f - d])
        } else {
            transform
        };
        Some(Self {
            width: pixels.w,
            height: pixels.h,
            rgba: pixels.px.into(),
            transform,
        })
    }

    pub(crate) fn dib(pixels: Rgba, mut source: [f64; 4]) -> Option<Self> {
        if source[2] == 0.0 || source[3] == 0.0 {
            source = [0.0, 0.0, pixels.w as f64, pixels.h as f64];
        }
        Self::new(pixels, crate::image_placement::rectangle(source))
    }

    fn sample(&self, point: Point) -> Option<[u8; 3]> {
        let p = self.transform * point;
        if !(p.x >= 0.0 && p.x < self.width as f64 && p.y >= 0.0 && p.y < self.height as f64) {
            return None;
        }
        let [r, g, b, _] = *self
            .rgba
            .as_chunks::<4>()
            .0
            .get(p.y as usize * self.width + p.x as usize)?;
        Some([r, g, b])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonoMask {
    width: usize,
    height: usize,
    stride: usize,
    bits: Arc<[u8]>,
    bottom_up: bool,
    origin: [f64; 2],
    span: [f64; 2],
}

impl MonoMask {
    pub(crate) fn read(
        info: Bytes<'_>,
        bits: Bytes<'_>,
        origin: [f64; 2],
        span: [f64; 2],
    ) -> Option<Self> {
        if info.u32(0)? < 40 || info.u16(12)? != 1 || info.u16(14)? != 1 || info.u32(16)? != 0 {
            return None;
        }
        let width = usize::try_from(info.i32(4)?).ok()?;
        let signed_height = info.i32(8)?;
        let height = signed_height.unsigned_abs() as usize;
        if width == 0 || height == 0 || width.checked_mul(height)? > crate::MAX_PIXELS {
            return None;
        }
        let stride = width.div_ceil(32) * 4;
        let bits = bits.slice(0, stride.checked_mul(height)?)?.into();
        Some(Self {
            width,
            height,
            stride,
            bits,
            bottom_up: signed_height > 0,
            origin,
            span,
        })
    }

    pub fn covers(&self, unit: Point) -> bool {
        let x = (self.origin[0] + self.span[0] * unit.x)
            .floor()
            .rem_euclid(self.width as f64) as usize;
        let y = (self.origin[1] + self.span[1] * unit.y)
            .floor()
            .rem_euclid(self.height as f64) as usize;
        let y = if self.bottom_up {
            self.height - 1 - y
        } else {
            y
        };
        self.bits[y * self.stride + x / 8] & (0x80 >> (x % 8)) != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TransferBrush {
    Solid([u8; 3]),
    Tile {
        pixels_to_target: Affine,
        tile: Arc<RawTile>,
    },
}

impl TransferBrush {
    fn sample(&self, target: Point) -> [u8; 3] {
        match self {
            Self::Solid(rgb) => *rgb,
            Self::Tile {
                pixels_to_target,
                tile,
            } => {
                let p = pixels_to_target.inverse() * target;
                if !p.is_finite() || tile.w == 0 || tile.h == 0 {
                    return [0; 3];
                }
                let u = p.x.floor().rem_euclid(tile.w as f64) as usize;
                let v = p.y.floor().rem_euclid(tile.h as f64) as usize;
                match tile.px.as_chunks::<4>().0.get(v * tile.w + u) {
                    Some(&[r, g, b, a]) if a != 0 => [r, g, b],
                    _ => [0; 3],
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RasterTransfer {
    pub placement: ImagePlacement,
    pub logic: Logic,
    source: Option<TransferSource>,
    mask: Option<MonoMask>,
    pub(crate) brush: TransferBrush,
    color_key: Option<[u8; 3]>,
}

impl RasterTransfer {
    pub(crate) fn new(destination: Affine, logic: Logic, source: Option<TransferSource>) -> Self {
        Self {
            placement: ImagePlacement {
                transform: destination,
                source: Rect::new(0.0, 0.0, 1.0, 1.0),
            },
            logic,
            source,
            mask: None,
            brush: TransferBrush::Solid([0; 3]),
            color_key: None,
        }
    }

    pub(crate) fn with_mask(mut self, mask: Option<MonoMask>) -> Self {
        self.mask = mask;
        self
    }

    pub(crate) fn with_color_key(mut self, color: [u8; 3]) -> Self {
        self.color_key = Some(color);
        self
    }

    pub(crate) fn footprint(&self) -> usize {
        128 + self.source.as_ref().map_or(0, |source| source.rgba.len())
            + self.mask.as_ref().map_or(0, |mask| mask.bits.len())
            + match &self.brush {
                TransferBrush::Tile { tile, .. } => tile.px.len(),
                _ => 0,
            }
    }

    pub fn map(&mut self, transform: Affine) {
        self.placement = self.placement.map(transform);
        if let TransferBrush::Tile {
            pixels_to_target, ..
        } = &mut self.brush
        {
            *pixels_to_target = transform * *pixels_to_target;
        }
    }

    /// Compute an opaque write, or leave the output untouched for an identity branch.
    pub fn paint(&self, unit: Point, target: Point, mut destination: [u8; 4]) -> Option<[u8; 4]> {
        if !(0.0..1.0).contains(&unit.x) || !(0.0..1.0).contains(&unit.y) {
            return None;
        }
        let fg = self.mask.as_ref().is_none_or(|mask| mask.covers(unit));
        let code = if fg {
            self.logic.foreground
        } else {
            self.logic.background
        };
        let source = if code & 0x33 != (code >> 2) & 0x33 {
            self.source.as_ref()?.sample(unit)?
        } else {
            [0; 3]
        };
        if self.color_key == Some(source) {
            return None;
        }
        let pattern = self.brush.sample(target);
        if (0..3).all(|c| {
            ternary(code, pattern[c], source[c], 0) == 0
                && ternary(code, pattern[c], source[c], 255) == 255
        }) {
            return None;
        }
        // A transparent clear pixel has no device color. Preserve the historical empty-paper
        // behavior of AND masks; XOR and OR use a zero destination until a layer paints it.
        if destination[3] == 0 {
            destination = if code == 0x88 {
                [255, 255, 255, 0]
            } else {
                [0; 4]
            };
        }
        let out = std::array::from_fn::<_, 3, _>(|i| {
            ternary(code, pattern[i], source[i], destination[i])
        });
        Some([out[0], out[1], out[2], 255])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rop_truth_table_uses_pattern_source_destination_order() {
        for code in 0_u8..=255 {
            for p in [0, 255] {
                for s in [0, 255] {
                    for d in [0, 255] {
                        let bit =
                            usize::from(p != 0) * 4 + usize::from(s != 0) * 2 + usize::from(d != 0);
                        assert_eq!(
                            ternary(code, p, s, d),
                            if (code >> bit) & 1 == 1 { 255 } else { 0 }
                        );
                    }
                }
            }
        }
    }
}
