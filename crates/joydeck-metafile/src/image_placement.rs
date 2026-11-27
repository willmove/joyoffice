//! Source-window to destination transforms shared by bitmap and nested picture draws.

use kurbo::{Affine, Rect};

use crate::geom::P;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageComposition {
    #[default]
    SourceOver,
    SourceCopy,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImagePlacement {
    /// Maps source coordinates to the drawing space.
    pub transform: Affine,
    /// The visible part of the source, in source coordinates.
    pub source: Rect,
}

impl ImagePlacement {
    pub(crate) fn new(
        unit_destination: Affine,
        [x, y, w, h]: [f64; 4],
        bounds: Rect,
    ) -> Option<Self> {
        if ![x, y, w, h].iter().all(|coordinate| coordinate.is_finite())
            || w.abs() < 1e-9
            || h.abs() < 1e-9
        {
            return None;
        }
        let requested = Rect::from_points((x, y), (x + w, y + h));
        let source = requested.intersect(bounds);
        let transform = unit_destination
            * Affine::scale_non_uniform(1.0 / w, 1.0 / h)
            * Affine::translate((-x, -y));
        let result = Self { transform, source };
        result.valid().then_some(result)
    }

    pub(crate) fn map(self, transform: Affine) -> Self {
        Self {
            transform: transform * self.transform,
            ..self
        }
    }

    pub(crate) fn valid(self) -> bool {
        let determinant = self.transform.determinant();
        let [a, b, c, d, ..] = self.transform.as_coeffs();
        self.transform.is_finite()
            && determinant.is_finite()
            && determinant.abs() > a.hypot(b) * c.hypot(d) * 1e-12
            && self.source.is_finite()
            && self.source.width() > 0.0
            && self.source.height() > 0.0
            && self.bounds().is_finite()
    }

    pub fn bounds(self) -> Rect {
        self.transform.transform_rect_bbox(self.source)
    }
}

pub(crate) fn parallelogram([origin, right, bottom]: [P; 3]) -> Affine {
    Affine::new([
        right.0 - origin.0,
        right.1 - origin.1,
        bottom.0 - origin.0,
        bottom.1 - origin.1,
        origin.0,
        origin.1,
    ])
}

pub(crate) fn rectangle([x, y, w, h]: [f64; 4]) -> Affine {
    Affine::new([w, 0.0, 0.0, h, x, y])
}

pub(crate) fn axis_aligned(transform: Affine) -> bool {
    let [a, b, c, d, ..] = transform.as_coeffs();
    b.abs() <= a.abs() * 1e-12 && c.abs() <= d.abs() * 1e-12
}
