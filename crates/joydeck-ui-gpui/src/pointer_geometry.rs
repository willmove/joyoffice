use joydeck_engine::{Command, WorkspaceAction};
use joydeck_geom::{Affine, KurboShape as _, Point, Rect, Vec2, Xfrm};
use joydeck_model::{Shape, ShapeKind, resolve, style::Fill};

pub(super) const GRIPS: [(i8, i8); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];
pub(super) const ROTATION_OFFSET: f64 = 24.;

#[derive(Clone, Copy)]
pub(super) enum Transform {
    Move,
    Resize(i8, i8),
    Rotate { start: Vec2 },
}

#[derive(Clone, Copy)]
pub(super) struct Frame {
    pub bounds: Xfrm,
    min_scale: Vec2,
    single: bool,
    uniform: bool,
}

impl Frame {
    pub fn new(transforms: impl IntoIterator<Item = Xfrm>) -> Option<Self> {
        let mut iter = transforms.into_iter();
        let first = iter.next()?;
        let mut bounds = first.bounds();
        let mut min_size = Vec2::new(first.w, first.h);
        let mut single = true;
        let mut rotated = first.rot.rem_euclid(180.) != 0.;
        for shape in iter {
            bounds = bounds.union(shape.bounds());
            min_size.x = min_size.x.min(shape.w);
            min_size.y = min_size.y.min(shape.h);
            rotated |= shape.rot.rem_euclid(180.) != 0.;
            single = false;
        }
        Some(Self {
            bounds: if single {
                Xfrm {
                    flip_h: false,
                    flip_v: false,
                    ..first
                }
            } else {
                Xfrm::new(bounds.x0, bounds.y0, bounds.width(), bounds.height())
            },
            min_scale: Vec2::new(min_size.x.recip().max(0.001), min_size.y.recip().max(0.001)),
            single,
            // A slide-space stretch on differently rotated objects would introduce shear.
            uniform: !single && rotated,
        })
    }

    pub fn handle(self, point: Point, scale: f64) -> Option<Transform> {
        if (point - grip(self.bounds, 0, -1, ROTATION_OFFSET / scale)).hypot() <= 8. / scale {
            return Some(Transform::Rotate {
                start: point - self.bounds.center(),
            });
        }
        GRIPS.iter().copied().find_map(|(x, y)| {
            ((point - grip(self.bounds, x, y, 0.)).hypot() <= 7. / scale)
                .then_some(Transform::Resize(x, y))
        })
    }

    pub fn preview(self, transform: Transform, delta: Vec2, snap: bool) -> Xfrm {
        let mut next = self.bounds;
        match transform {
            Transform::Move => {
                next.x += delta.x;
                next.y += delta.y;
            }
            Transform::Rotate { start } => {
                let end = start + delta;
                if end.hypot2() > 1e-12 {
                    let angle = end.y.atan2(end.x) - start.y.atan2(start.x);
                    let angle = (angle.to_degrees() + 180.).rem_euclid(360.) - 180.;
                    next.rot += joydeck_geom::snap(angle, if snap { 15. } else { 0. });
                }
            }
            Transform::Resize(x, y) => {
                let rotation = self.bounds.rot.to_radians();
                let local = Affine::rotate(-rotation) * delta.to_point();
                let direction = Vec2::new(f64::from(x), f64::from(y));
                let span = Vec2::new(self.bounds.w, self.bounds.h);
                let mut sx = 1. + local.x * direction.x / span.x.max(1e-12);
                let mut sy = 1. + local.y * direction.y / span.y.max(1e-12);
                if self.uniform {
                    let diagonal = Vec2::new(direction.x * span.x, direction.y * span.y);
                    let scale = (1. + local.to_vec2().dot(diagonal) / diagonal.hypot2().max(1e-12))
                        .clamp(self.min_scale.x.max(self.min_scale.y).min(1000.), 1000.);
                    sx = scale;
                    sy = scale;
                } else {
                    if x != 0 {
                        sx = sx.clamp(self.min_scale.x.min(1000.), 1000.);
                    }
                    if y != 0 {
                        sy = sy.clamp(self.min_scale.y.min(1000.), 1000.);
                    }
                }
                next.w = span.x * sx;
                next.h = span.y * sy;
                let offset = Point::new(
                    direction.x * (next.w - span.x) / 2.,
                    direction.y * (next.h - span.y) / 2.,
                );
                let center = self.bounds.center() + (Affine::rotate(rotation) * offset).to_vec2();
                next.x = center.x - next.w / 2.;
                next.y = center.y - next.h / 2.;
            }
        }
        next
    }

    pub fn command(self, next: Xfrm) -> Option<Command> {
        if next == self.bounds {
            return None;
        }
        let rotation = next.rot - self.bounds.rot;
        Some(if self.single {
            Command::Transform {
                dx: next.x - self.bounds.x,
                dy: next.y - self.bounds.y,
                dw: next.w - self.bounds.w,
                dh: next.h - self.bounds.h,
                rotation,
            }
        } else {
            let delta = next.center() - self.bounds.center();
            Command::Workspace {
                action: WorkspaceAction::TransformSelection {
                    dx: delta.x,
                    dy: delta.y,
                    scale_x: next.w / self.bounds.w.max(1e-12),
                    scale_y: next.h / self.bounds.h.max(1e-12),
                    rotation,
                },
            }
        })
    }
}

pub(super) fn grip(frame: Xfrm, x: i8, y: i8, offset: f64) -> Point {
    frame.affine()
        * Point::new(
            frame.w * (f64::from(x) + 1.) / 2.,
            frame.h * (f64::from(y) + 1.) / 2. - offset,
        )
}

pub(super) fn hits(ctx: &resolve::Ctx, shape: &Shape, point: Point, tolerance: f64) -> bool {
    let transform = resolve::xfrm(ctx, shape);
    let local = transform.affine().inverse() * point;
    let bounds = Rect::new(0., 0., transform.w, transform.h);
    if !matches!(shape.kind, ShapeKind::Shape | ShapeKind::Connector { .. }) {
        return bounds.contains(local);
    }
    if (shape.ph.is_some() || shape.text_box || shape.text.as_ref().is_some_and(|t| !t.is_empty()))
        && bounds.contains(local)
    {
        return true;
    }
    let (line, _) = resolve::line(ctx, shape);
    let has_stroke = line
        .fill
        .as_ref()
        .is_some_and(|fill| !matches!(fill, Fill::None));
    let stroke_tolerance = line.width.unwrap_or(0.75).max(0.) / 2. + tolerance;
    if !bounds
        .inflate(stroke_tolerance, stroke_tolerance)
        .contains(local)
    {
        return false;
    }
    let (fill, _) = resolve::fill(ctx, shape);
    let has_fill = fill
        .as_ref()
        .is_some_and(|fill| !matches!(fill, Fill::None));
    joydeck_render::shape_geometry(shape, transform.w, transform.h)
        .paths
        .iter()
        .any(|path| {
            let filled = has_fill && path.fill != joydeck_geom::preset::FillMode::None;
            (filled && {
                let winding = path.path.winding(local);
                if path.even_odd {
                    winding % 2 != 0
                } else {
                    winding != 0
                }
            }) || (has_stroke
                && path.stroke
                && joydeck_geom::near_path(&path.path, local, stroke_tolerance))
        })
}

pub(super) fn intersects(frame: Xfrm, marquee: Rect) -> bool {
    frame.bounds().intersect(marquee).area() > 0.
        && frame
            .affine()
            .inverse()
            .transform_rect_bbox(marquee)
            .intersect(Rect::new(0., 0., frame.w, frame.h))
            .area()
            > 0.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_resize_keeps_opposite_grip_fixed() -> Result<(), &'static str> {
        let original = Xfrm {
            rot: 45.,
            ..Xfrm::new(100., 120., 80., 40.)
        };
        let frame = Frame::new([original]).ok_or("missing frame")?;
        let opposite = grip(original, -1, -1, 0.);
        let end = grip(original, 1, 1, 0.);
        for scale in [0.5, 1., 2., 4.] {
            let handle = frame.handle(end, scale).ok_or("missing grip")?;
            let delta = Affine::rotate(45f64.to_radians()) * Point::new(40., -20.);
            let next = frame.preview(handle, delta.to_vec2(), false);
            assert!((next.w - 120.).abs() < 1e-9 && (next.h - 20.).abs() < 1e-9);
            assert!((grip(next, -1, -1, 0.) - opposite).hypot() < 1e-9);
            assert!((grip(next, 1, 1, 0.) - end - delta.to_vec2()).hypot() < 1e-9);
        }
        Ok(())
    }

    #[test]
    fn marquee_ignores_empty_corner_of_rotated_box() {
        let original = Xfrm {
            rot: 45.,
            ..Xfrm::new(100., 120., 80., 40.)
        };
        let b = original.bounds();
        let corner = Rect::new(b.x0, b.y0, b.x0 + 4., b.y0 + 4.);
        assert!(b.intersect(corner).area() > 0.);
        assert!(!intersects(original, corner));
        assert!(intersects(original, Rect::new(130., 130., 145., 145.)));
    }
}
