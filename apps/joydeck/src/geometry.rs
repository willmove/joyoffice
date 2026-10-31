use crate::document::{Object, Result};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.w && y <= self.y + self.h
    }
}
fn rotate(x: f32, y: f32, a: f32) -> (f32, f32) {
    let (s, c) = a.to_radians().sin_cos();
    (x * c - y * s, x * s + y * c)
}
pub fn bounds(o: &Object) -> Rect {
    let (s, c) = o.rotation.to_radians().sin_cos();
    let w = o.width * c.abs() + o.height * s.abs();
    let h = o.width * s.abs() + o.height * c.abs();
    Rect {
        x: o.x + (o.width - w) / 2.,
        y: o.y + (o.height - h) / 2.,
        w,
        h,
    }
}
pub fn union(objects: &[Object]) -> Rect {
    let mut r = Rect {
        x: f32::MAX,
        y: f32::MAX,
        w: 0.,
        h: 0.,
    };
    let mut right = f32::MIN;
    let mut bottom = f32::MIN;
    for o in objects {
        let b = bounds(o);
        r.x = r.x.min(b.x);
        r.y = r.y.min(b.y);
        right = right.max(b.x + b.w);
        bottom = bottom.max(b.y + b.h);
    }
    r.w = (right - r.x).max(1.);
    r.h = (bottom - r.y).max(1.);
    r
}
pub fn hit(o: &Object, x: f32, y: f32) -> bool {
    let (x, y) = rotate(x - o.x - o.width / 2., y - o.y - o.height / 2., -o.rotation);
    let (x, y) = (x + o.width / 2., y + o.height / 2.);
    if o.kind == "ellipse" {
        return ((x - o.width / 2.) / (o.width / 2.).max(0.1)).powi(2)
            + ((y - o.height / 2.) / (o.height / 2.).max(0.1)).powi(2)
            <= 1.;
    }
    if o.kind == "arrow" {
        let u = x / o.width.max(0.1);
        let v = y / o.height.max(0.1);
        return (0. ..=0.65).contains(&u) && (0.25..=0.75).contains(&v)
            || (0.65..=1.).contains(&u) && (v - 0.5).abs() <= (1. - u) / 0.7;
    }
    if o.kind == "roundRect" {
        let r = 12f32.min(o.width / 2.).min(o.height / 2.).max(0.);
        let cx = x.clamp(r, (o.width - r).max(r));
        let cy = y.clamp(r, (o.height - r).max(r));
        return (x - cx).hypot(y - cy) <= r;
    }
    if o.kind == "line" {
        let den = (o.width * o.width + o.height * o.height).max(0.1);
        let t = ((x * o.width + y * o.height) / den).clamp(0., 1.);
        return (x - t * o.width).hypot(y - t * o.height) <= 4. + o.stroke_width / 2.;
    }
    Rect {
        x: 0.,
        y: 0.,
        w: o.width,
        h: o.height,
    }
    .contains(x, y)
}
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Move,
    Scale,
    Rotate,
}
/// Every update is calculated from the gesture's original snapshot.
pub fn transform(
    objects: &[Object],
    mode: Mode,
    start: (f32, f32),
    end: (f32, f32),
) -> Result<Vec<Object>> {
    if objects.is_empty()
        || ![start.0, start.1, end.0, end.1]
            .iter()
            .all(|v| v.is_finite())
    {
        return Err("无效手势".into());
    }
    let r = union(objects);
    let (cx, cy) = (r.x + r.w / 2., r.y + r.h / 2.);
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let angle = ((end.1 - cy).atan2(end.0 - cx) - (start.1 - cy).atan2(start.0 - cx)).to_degrees();
    let mut sx = ((r.w + dx) / r.w).clamp(0.01, 100.);
    let mut sy = ((r.h + dy) / r.h).clamp(0.01, 100.);
    if objects.iter().any(|o| o.rotation != 0.) {
        sx = if (sx - 1.).abs() > (sy - 1.).abs() {
            sx
        } else {
            sy
        };
        sy = sx;
    }
    Ok(objects
        .iter()
        .map(|v| {
            let mut o = v.clone();
            match mode {
                Mode::Move => {
                    o.x += dx;
                    o.y += dy;
                }
                Mode::Scale => {
                    o.x = r.x + (o.x - r.x) * sx;
                    o.y = r.y + (o.y - r.y) * sy;
                    o.width = (o.width * sx).max(1.);
                    o.height = (o.height * sy).max(1.);
                }
                Mode::Rotate => {
                    let (x, y) = rotate(o.x + o.width / 2. - cx, o.y + o.height / 2. - cy, angle);
                    o.x = cx + x - o.width / 2.;
                    o.y = cy + y - o.height / 2.;
                    o.rotation = (o.rotation + angle).rem_euclid(360.);
                }
            }
            o
        })
        .collect())
}

/// Axis-aligned group handles. Rotated selections scale uniformly to avoid shear.
pub fn resize(
    objects: &[Object],
    handle: (i8, i8),
    delta: (f32, f32),
    constrain: bool,
    center: bool,
) -> Result<Vec<Object>> {
    if objects.is_empty()
        || ![delta.0, delta.1].iter().all(|v| v.is_finite())
        || ![-1, 0, 1].contains(&handle.0)
        || ![-1, 0, 1].contains(&handle.1)
        || handle == (0, 0)
    {
        return Err("缩放手柄无效".into());
    }
    let r = union(objects);
    let factor = if center { 2. } else { 1. };
    let mut sx = if handle.0 == 0 {
        1.
    } else {
        ((r.w + delta.0 * f32::from(handle.0) * factor) / r.w).clamp(0.01, 100.)
    };
    let mut sy = if handle.1 == 0 {
        1.
    } else {
        ((r.h + delta.1 * f32::from(handle.1) * factor) / r.h).clamp(0.01, 100.)
    };
    if constrain || objects.iter().any(|o| o.rotation != 0.) {
        let uniform = if (sx - 1.).abs() > (sy - 1.).abs() {
            sx
        } else {
            sy
        };
        sx = uniform;
        sy = uniform;
    }
    let ax = if center || handle.0 == 0 {
        r.x + r.w / 2.
    } else if handle.0 < 0 {
        r.x + r.w
    } else {
        r.x
    };
    let ay = if center || handle.1 == 0 {
        r.y + r.h / 2.
    } else if handle.1 < 0 {
        r.y + r.h
    } else {
        r.y
    };
    Ok(objects
        .iter()
        .map(|v| {
            let mut o = v.clone();
            o.x = ax + (o.x - ax) * sx;
            o.y = ay + (o.y - ay) * sy;
            o.width = (o.width * sx).max(1.);
            o.height = (o.height * sy).max(1.);
            o
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{self, Command, Session};
    #[test]
    fn rotated_hit_and_gesture_single_history() {
        let mut s = Session::new();
        s.execute(s.id, Command::AddObject(0, edit::object("rect")))
            .unwrap();
        let old = s.deck.clone();
        let objects = &old.slides[0].objects;
        let moved = transform(objects, Mode::Move, (0., 0.), (100., 50.)).unwrap();
        assert_eq!(moved[0].x, 180.);
        s.execute(s.id, Command::ReplaceMany(0, moved)).unwrap();
        s.undo();
        assert!(std::sync::Arc::ptr_eq(&old, &s.deck));
        let o = &objects[0];
        let mut rot = o.clone();
        rot.rotation = 90.;
        assert!(hit(&rot, o.x + o.width / 2., o.y + o.height / 2.));
        assert!(!hit(&rot, o.x, o.y));
    }
    #[test]
    fn composite_failure_preserves_snapshot_and_reorder_identity() {
        let mut s = Session::new();
        let old = s.deck.clone();
        assert!(
            s.execute(
                s.id,
                Command::Batch(vec![Command::AddPage, Command::DeletePage(100)])
            )
            .is_err()
        );
        assert!(std::sync::Arc::ptr_eq(&old, &s.deck));
        s.execute(
            s.id,
            Command::Batch(vec![Command::AddPage, Command::AddPage]),
        )
        .unwrap();
        let ids: Vec<_> = s.deck.slides.iter().map(|s| s.id.clone()).collect();
        s.execute(s.id, Command::ReorderPages(vec![0, 1], 3))
            .unwrap();
        assert_eq!(
            s.deck
                .slides
                .iter()
                .map(|s| s.id.clone())
                .collect::<Vec<_>>(),
            vec![ids[2].clone(), ids[0].clone(), ids[1].clone()]
        );
    }
    #[test]
    fn all_handles_center_and_aspect_constraints() {
        let o = edit::object("rect");
        for handle in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            let r = resize(std::slice::from_ref(&o), handle, (20., 15.), false, false).unwrap();
            assert!(r[0].width > 0. && r[0].height > 0.);
            if handle.0 < 0 {
                assert!((r[0].x + r[0].width - o.x - o.width).abs() < 0.01);
            }
            if handle.1 < 0 {
                assert!((r[0].y + r[0].height - o.y - o.height).abs() < 0.01);
            }
        }
        let r = resize(std::slice::from_ref(&o), (1, 1), (20., 10.), true, true).unwrap();
        assert!((r[0].x + r[0].width / 2. - o.x - o.width / 2.).abs() < 0.01);
        assert!((r[0].width / r[0].height - o.width / o.height).abs() < 0.01);
    }
}

#[derive(Clone, Copy)]
pub enum Arrange {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
    Horizontal,
    Vertical,
}
/// Stable ordering and rotated visual bounds; commits through ReplaceMany once.
pub fn arrange(objects: &[Object], mode: Arrange) -> Result<Vec<Object>> {
    if objects.len() < 2 {
        return Err("对齐至少选择两个对象".into());
    }
    let mut out = objects.to_vec();
    let area = union(objects);
    let horizontal = matches!(
        mode,
        Arrange::Left | Arrange::Center | Arrange::Right | Arrange::Horizontal
    );
    if matches!(mode, Arrange::Horizontal | Arrange::Vertical) {
        if objects.len() < 3 {
            return Err("分布至少选择三个对象".into());
        }
        let mut order: Vec<_> = (0..objects.len()).collect();
        order.sort_by(|a, b| {
            let a = bounds(&objects[*a]);
            let b = bounds(&objects[*b]);
            if horizontal {
                a.x.total_cmp(&b.x)
            } else {
                a.y.total_cmp(&b.y)
            }
        });
        let sizes: f32 = objects
            .iter()
            .map(|o| {
                let b = bounds(o);
                if horizontal { b.w } else { b.h }
            })
            .sum();
        let gap = (if horizontal { area.w } else { area.h } - sizes) / (objects.len() - 1) as f32;
        let mut cursor = if horizontal { area.x } else { area.y };
        for index in order {
            let b = bounds(&objects[index]);
            if horizontal {
                out[index].x += cursor - b.x;
                cursor += b.w + gap;
            } else {
                out[index].y += cursor - b.y;
                cursor += b.h + gap;
            }
        }
    } else {
        for o in &mut out {
            let b = bounds(o);
            let delta = match mode {
                Arrange::Left => area.x - b.x,
                Arrange::Center => area.x + area.w / 2. - b.x - b.w / 2.,
                Arrange::Right => area.x + area.w - b.x - b.w,
                Arrange::Top => area.y - b.y,
                Arrange::Middle => area.y + area.h / 2. - b.y - b.h / 2.,
                Arrange::Bottom => area.y + area.h - b.y - b.h,
                _ => unreachable!(),
            };
            if horizontal {
                o.x += delta;
            } else {
                o.y += delta;
            }
        }
    }
    Ok(out)
}
