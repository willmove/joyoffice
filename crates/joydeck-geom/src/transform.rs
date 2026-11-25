//! Shape and group coordinates from ECMA-376 Part 1 §20.1.7 and §20.1.10.3.

use kurbo::{Affine, Point, Rect, Size};
use serde::{Deserialize, Serialize};

/// A frame in its parent's coordinate space. Rotation is clockwise in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Xfrm {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub rot: f64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_h: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_v: bool,
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

impl Xfrm {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self {
            x,
            y,
            w,
            h,
            ..Self::default()
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.x + self.w, self.y + self.h)
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }

    /// Map a local point into the parent, reflecting before rotating about the frame center.
    pub fn affine(&self) -> Affine {
        let (sin, cos) = self.rot.to_radians().sin_cos();
        let horizontal = if self.flip_h { -1.0 } else { 1.0 };
        let vertical = if self.flip_v { -1.0 } else { 1.0 };
        let (a, b) = (cos * horizontal, sin * horizontal);
        let (c, d) = (-sin * vertical, cos * vertical);
        let (half_w, half_h) = (self.w * 0.5, self.h * 0.5);
        let tx = self.x + (half_w * (1.0 - a) - c * half_h);
        let ty = self.y + (half_h * (1.0 - d) - b * half_w);
        Affine::new([a, b, c, d, tx, ty])
    }

    /// Map group child coordinates into the group's parent, including the group's orientation.
    pub fn child_affine(&self, child: &Self) -> Affine {
        let axis_scale = |outer: f64, inner: f64| {
            if inner.abs() > 1e-9 {
                outer / inner
            } else {
                1.0
            }
        };
        let sx = axis_scale(self.w, child.w);
        let sy = axis_scale(self.h, child.h);
        self.affine() * Affine::new([sx, 0.0, 0.0, sy, -child.x * sx, -child.y * sy])
    }

    pub fn bounds(&self) -> Rect {
        self.affine()
            .transform_rect_bbox(Rect::new(0.0, 0.0, self.w, self.h))
    }

    pub fn is_finite(&self) -> bool {
        [self.x, self.y, self.w, self.h, self.rot]
            .into_iter()
            .all(f64::is_finite)
    }

    /// Rotation in the range 0..360 degrees.
    pub fn rot_norm(&self) -> f64 {
        self.rot.rem_euclid(360.0)
    }

    /// Retain a preset line's orientation angle while fitting its directed diagonal.
    pub fn fit_diagonal(self, start: Point, end: Point, rising: bool) -> Self {
        let local = Affine::rotate(-self.rot.to_radians()) * (end - start).to_point();
        let (w, h) = (local.x.abs(), local.y.abs());
        let center = start.midpoint(end);
        Self {
            x: center.x - w * 0.5,
            y: center.y - h * 0.5,
            w,
            h,
            flip_h: local.x < 0.0,
            flip_v: (local.y < 0.0) != rising,
            ..self
        }
    }
}

/// Map a group's child rectangle into its parent's coordinates.
pub fn group_child_affine(group: &Xfrm, child: Rect) -> Affine {
    group.child_affine(&Xfrm::new(
        child.x0,
        child.y0,
        child.width(),
        child.height(),
    ))
}

/// Measure each local axis in slide points, keeping the orientation in the output map.
pub fn point_sized_box(output: Affine, to_slide: Affine, size: Size) -> (Size, Affine) {
    let [a, b, c, d, ..] = to_slide.as_coeffs();
    let (sx, sy) = (a.hypot(b), c.hypot(d));
    let scaled = Size::new(size.width * sx, size.height * sy);
    if sx <= 1e-9
        || sy <= 1e-9
        || !scaled.is_finite()
        || ((sx - 1.0).abs() < 1e-9 && (sy - 1.0).abs() < 1e-9)
    {
        return (size, output);
    }
    (
        scaled,
        output * Affine::scale_non_uniform(1.0 / sx, 1.0 / sy),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_child_offsets_scales_reflections_and_quarter_turns() {
        let outer = Xfrm {
            rot: 90.0,
            flip_h: true,
            ..Xfrm::new(100.0, 50.0, 80.0, 60.0)
        };
        let inner = Xfrm {
            rot: 90.0,
            flip_v: true,
            ..Xfrm::new(20.0, 30.0, 10.0, 15.0)
        };
        let map = outer.child_affine(&Xfrm::new(10.0, 20.0, 40.0, 20.0))
            * inner.child_affine(&Xfrm::new(2.0, 4.0, 5.0, 5.0));
        assert!((map * Point::new(2.0, 4.0)).distance(Point::new(132.5, 105.0)) < 1e-9);
        assert!((map * Point::new(7.0, 9.0)).distance(Point::new(102.5, 75.0)) < 1e-9);

        let degenerate =
            Xfrm::new(100.0, 50.0, 80.0, 60.0).child_affine(&Xfrm::new(10.0, 20.0, 0.0, 20.0));
        assert_eq!(degenerate * Point::new(13.0, 25.0), Point::new(103.0, 65.0));
    }

    #[test]
    fn fits_the_directed_diagonal_at_the_existing_rotation() {
        let start = Point::new(35.0, 50.0);
        let end = Point::new(12.0, 80.0);
        for rising in [false, true] {
            let frame = Xfrm {
                rot: 32.0,
                ..Xfrm::default()
            }
            .fit_diagonal(start, end, rising);
            let y = if rising { frame.h } else { 0.0 };
            assert!((frame.affine() * Point::new(0.0, y)).distance(start) < 1e-9);
            assert!((frame.affine() * Point::new(frame.w, frame.h - y)).distance(end) < 1e-9);
        }
    }
}
