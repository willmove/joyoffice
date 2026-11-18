use kurbo::{BezPath, CubicBez, ParamCurve, PathEl, Point, QuadBez};

fn line_crossing(a: Point, b: Point, p: Point) -> i32 {
    let direction = i32::from(b.y > p.y) - i32::from(a.y > p.y);
    if direction == 0 || p.x >= a.x.max(b.x) {
        return 0;
    }
    if p.x < a.x.min(b.x) || p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
        direction
    } else {
        0
    }
}

// Take a horizontal ray from the pointer to the right. If the entire Bézier
// hull is beyond the ray origin, signed crossings inside it cancel in pairs
// and only the two endpoints matter. Subdivision retains both endpoints and
// gives its children the very same midpoint, including at horizontal joins.
fn cubic_crossings(curve: CubicBez, p: Point, depth: u8) -> i32 {
    let CubicBez { p0, p1, p2, p3 } = curve;
    if p.y < p0.y.min(p1.y).min(p2.y).min(p3.y)
        || p.y >= p0.y.max(p1.y).max(p2.y).max(p3.y)
        || p.x >= p0.x.max(p1.x).max(p2.x).max(p3.x)
    {
        return 0;
    }
    if p.x < p0.x.min(p1.x).min(p2.x).min(p3.x) {
        return i32::from(p3.y > p.y) - i32::from(p0.y > p.y);
    }
    if depth == 0 {
        return line_crossing(p0, p3, p);
    }
    let (left, right) = curve.subdivide();
    cubic_crossings(left, p, depth - 1) + cubic_crossings(right, p, depth - 1)
}

/// Signed winding for a filled path, including the implicit closing edge of
/// an open contour. The bounded subdivision applies only near the pointer.
pub fn fill_winding(path: &BezPath, point: Point) -> i32 {
    let mut start = Point::ZERO;
    let mut last = start;
    let mut winding = 0;
    for element in path.iter() {
        match element {
            PathEl::MoveTo(next) => {
                winding += line_crossing(last, start, point);
                start = next;
                last = next;
            }
            PathEl::LineTo(next) => {
                winding += line_crossing(last, next, point);
                last = next;
            }
            PathEl::QuadTo(control, next) => {
                let curve = QuadBez::new(last, control, next).raise();
                winding += cubic_crossings(curve, point, 26);
                last = next;
            }
            PathEl::CurveTo(a, b, next) => {
                winding += cubic_crossings(CubicBez::new(last, a, b, next), point, 26);
                last = next;
            }
            PathEl::ClosePath => {
                winding += line_crossing(last, start, point);
                last = start;
            }
        }
    }
    winding + line_crossing(last, start, point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_quadratic_and_oppositely_wound_inner_contour() {
        let mut path = BezPath::new();
        path.move_to((0., 0.));
        path.quad_to((10., -10.), (20., 0.));
        path.line_to((20., 20.));
        path.line_to((0., 20.));
        path.move_to((5., 5.)); // closes the outer shape
        path.line_to((5., 15.));
        path.line_to((15., 15.));
        path.line_to((15., 5.)); // fill implicitly closes this hole
        for (p, expected) in [
            ((10., -4.), 1),
            ((10., -6.), 0),
            ((10., 3.), 1),
            ((10., 10.), 0),
            ((5., 18.), 1),
            ((-1., 18.), 0),
        ] {
            assert_eq!(fill_winding(&path, p.into()), expected, "{p:?}");
        }
    }
}
