//! EMF+ brush placement independent of glyph origins and the chosen font.

use std::sync::Arc;

use kurbo::{Affine, Point};

use crate::{
    device::{Xform, mul},
    geom::P,
    plus_paint::Brush,
};

#[derive(Debug, Clone, PartialEq)]
pub struct TextBrush {
    paint: Paint,
    space: Affine,
}

#[derive(Debug, Clone, PartialEq)]
enum Paint {
    Hatch(Arc<[u8]>),
    Sample(Brush),
}

impl TextBrush {
    pub(crate) fn new(brush: &Brush, device: Xform, origin: P, cell: f64) -> Option<Self> {
        match brush {
            Brush::Solid(_) => None,
            Brush::Hatch { style, fg, bg } => {
                let tile = crate::plus_hatch::tile(*style, *fg, *bg)?;
                Some(Self {
                    paint: Paint::Hatch(tile.into()),
                    space: Affine::translate(origin) * Affine::scale(cell),
                })
            }
            _ => {
                let space = Affine::new(if brush.device_space() {
                    brush.xf()
                } else {
                    mul(brush.xf(), device)
                });
                (space.is_finite() && space.determinant().abs() > 1e-12).then(|| Self {
                    paint: Paint::Sample(brush.clone()),
                    space,
                })
            }
        }
    }

    pub(crate) fn map(&mut self, device_to_picture: Affine) {
        self.space = device_to_picture * self.space;
    }

    pub(crate) fn cost(&self) -> usize {
        match &self.paint {
            Paint::Hatch(tile) => tile.len(),
            Paint::Sample(brush) => brush.cost(),
        }
    }

    /// Repeating 8×8 hatch and its source-pixel to picture transform.
    pub fn hatch(&self) -> Option<(&[u8], Affine)> {
        match &self.paint {
            Paint::Hatch(tile) => Some((tile, self.space)),
            Paint::Sample(_) => None,
        }
    }

    pub fn picture_to_brush(&self) -> Affine {
        self.space.inverse()
    }

    /// Samples a non-hatch brush in its own coordinates.
    pub fn sample(&self, point: Point) -> [u8; 4] {
        match &self.paint {
            Paint::Sample(brush) => brush.sample((point.x, point.y)),
            Paint::Hatch(_) => [0; 4],
        }
    }
}
