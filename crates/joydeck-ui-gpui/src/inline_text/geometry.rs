//! The editor uses logical window pixels. GPUI maps these to the current backing scale.
use gpui_kit::{Background, Bounds, PathBuilder, Pixels, Point, Window, point, px, size};
use joydeck_geom::{Affine, Point as LayoutPoint, Rect};

pub(super) struct Geometry {
    transform: Affine,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Motion {
    Along { forward: bool },
    Across { down: bool },
}

impl Geometry {
    pub(super) fn new(placement: Affine, canvas: Bounds<Pixels>, scale: f32) -> Option<Self> {
        let transform = Affine::translate((f64::from(canvas.left()), f64::from(canvas.top())))
            * Affine::scale(f64::from(scale))
            * placement;
        (transform.is_finite() && transform.determinant().abs() > f64::EPSILON)
            .then_some(Self { transform })
    }

    pub(super) fn local(&self, p: Point<Pixels>) -> LayoutPoint {
        self.transform.inverse() * LayoutPoint::new(f64::from(p.x), f64::from(p.y))
    }

    pub(super) fn bounds(&self, rect: Rect) -> Bounds<Pixels> {
        let b = self.transform.transform_rect_bbox(rect);
        Bounds::new(
            point(px(b.x0 as f32), px(b.y0 as f32)),
            size(px(b.width() as f32), px(b.height() as f32)),
        )
    }

    pub(super) fn paint(&self, rect: Rect, color: impl Into<Background>, w: &mut Window) {
        let screen = |x, y| {
            let p = self.transform * LayoutPoint::new(x, y);
            point(px(p.x as f32), px(p.y as f32))
        };
        let mut quad = PathBuilder::fill();
        quad.move_to(screen(rect.x0, rect.y0));
        quad.line_to(screen(rect.x1, rect.y0));
        quad.line_to(screen(rect.x1, rect.y1));
        quad.line_to(screen(rect.x0, rect.y1));
        quad.close();
        if let Ok(path) = quad.build() {
            w.paint_path(path, color);
        }
    }

    /// Map a screen arrow to its dominant layout axis and direction.
    pub(super) fn arrow(&self, x: f64, y: f64) -> Motion {
        let inverse = self.transform.inverse();
        let [a, b, c, d, ..] = inverse.as_coeffs();
        let dx = a * x + c * y;
        let dy = b * x + d * y;
        if dx.abs() >= dy.abs() {
            Motion::Along { forward: dx > 0. }
        } else {
            Motion::Across { down: dy > 0. }
        }
    }
}

pub(super) fn reveal_offset(
    caret: Bounds<Pixels>,
    viewport: Bounds<Pixels>,
    offset: Point<Pixels>,
    limit: Point<Pixels>,
) -> Point<Pixels> {
    fn shift(start: Pixels, end: Pixels, min: Pixels, max: Pixels) -> Pixels {
        // A tall caret can straddle both edges at high zoom. Reveal its center in that case.
        if end - start > max - min {
            (min + max - start - end) / 2.
        } else if start < min {
            min - start
        } else {
            (max - end).min(px(0.))
        }
    }
    let margin = px(8.).min(viewport.size.width.min(viewport.size.height) / 4.);
    point(
        (offset.x
            + shift(
                caret.left(),
                caret.right(),
                viewport.left() + margin,
                viewport.right() - margin,
            ))
        .clamp(-limit.x, px(0.)),
        (offset.y
            + shift(
                caret.top(),
                caret.bottom(),
                viewport.top() + margin,
                viewport.bottom() - margin,
            ))
        .clamp(-limit.y, px(0.)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_and_hit_round_trip_at_zoom_and_backing_scales()
    -> Result<(), Box<dyn std::error::Error>> {
        let layout = Rect::new(40., 20., 41., 44.);
        for angle in [0_f64, 33., 90., 180., 270.] {
            for zoom in [0.65, 1., 2.5] {
                for backing in [1., 2.] {
                    let placement =
                        Affine::translate((200., 160.)) * Affine::rotate(angle.to_radians());
                    let origin = point(px(-175.), px(52.));
                    let canvas = Bounds::new(origin, size(px(800.), px(500.)));
                    let geo = Geometry::new(placement, canvas, zoom).ok_or("invalid placement")?;
                    let projected = geo.bounds(layout);
                    let middle = projected.center();
                    let physical = point(middle.x * backing, middle.y * backing);
                    let picked = geo.local(point(physical.x / backing, physical.y / backing));
                    assert!((picked - layout.center()).hypot() < 0.001);
                    assert!(projected.size.width > px(0.) && projected.size.height > px(0.));
                }
            }
        }
        let clockwise = Geometry::new(
            Affine::rotate(std::f64::consts::FRAC_PI_2),
            Bounds::default(),
            1.,
        )
        .ok_or("invalid quarter turn")?;
        assert_eq!(clockwise.arrow(0., 1.), Motion::Along { forward: true });
        assert_eq!(clockwise.arrow(-1., 0.), Motion::Across { down: true });
        Ok(())
    }

    #[test]
    fn panning_reveals_the_caret_within_the_scroll_limits() {
        let viewport = Bounds::new(point(px(200.), px(140.)), size(px(400.), px(340.)));
        let caret = Bounds::new(point(px(686.), px(730.)), size(px(2.), px(36.)));
        let offset = point(px(-20.), px(-100.));
        let limit = point(px(200.), px(370.));
        assert_eq!(
            reveal_offset(caret, viewport, offset, limit),
            point(px(-116.), px(-370.))
        );
    }
}
