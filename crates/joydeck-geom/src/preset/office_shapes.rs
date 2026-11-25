//! ECMA-376 Part 1 §20.1.9.18 and the DrawingML geometry attachment, 5th edition.
//! Use the head length and shaft thickness in shape coordinates before applying orientation.
use super::builder::{Corner, Dir, Pb, corner_rect, fill, len_thick, simple, stroke_only};
use super::catalog::{Geometry, Handle};
use kurbo::{BezPath, Point, Rect, Vec2};

fn connector_legs(name: &str) -> Option<usize> {
    Some(match name {
        "line" | "lineInv" | "straightConnector1" => 1,
        "bentConnector2" | "curvedConnector2" => 2,
        "bentConnector3" | "curvedConnector3" => 3,
        "bentConnector4" | "curvedConnector4" => 4,
        "bentConnector5" | "curvedConnector5" => 5,
        _ => return None,
    })
}

/// The connector's directed local diagonal, shared with edits that follow an attached shape.
pub fn connector_endpoints(name: &str, w: f64, h: f64) -> Option<[Point; 2]> {
    connector_legs(name)?;
    Some(diagonal(name, w, h))
}

fn diagonal(name: &str, w: f64, h: f64) -> [Point; 2] {
    if name == "lineInv" {
        [Point::new(0., h), Point::new(w, 0.)]
    } else {
        [Point::ORIGIN, Point::new(w, h)]
    }
}

/// DrawingML §20.1.9.18. The curved presets round the corners of the orthogonal
/// route. Each two adjacent corners share their midpoint as the end of one cubic
/// and the start of the next, including when an adjustment lies outside the frame.
pub(super) fn connector(name: &str, w: f64, h: f64, adjustments: &[f64]) -> Option<Geometry> {
    let legs = connector_legs(name)?;
    let value = |index: usize| match adjustments.get(index) {
        Some(v) if v.is_finite() => *v,
        Some(_) => 0.,
        None => 50_000.,
    };
    let [start, end] = diagonal(name, w, h);
    let x = if legs >= 3 {
        w * value(0) / 100_000.
    } else {
        w
    };
    let y = if legs >= 4 {
        h * value(1) / 100_000.
    } else {
        h
    };
    let last_x = if legs == 5 {
        w * value(2) / 100_000.
    } else {
        w
    };
    let corners = [
        Point::new(x, 0.),
        Point::new(x, y),
        Point::new(last_x, y),
        Point::new(last_x, h),
    ];
    let corners = &corners[..legs - 1];
    let mut outline = BezPath::new();
    outline.move_to(start);
    if name.starts_with("curved") {
        let mut from = start;
        for (i, &corner) in corners.iter().enumerate() {
            let to = corners
                .get(i + 1)
                .map_or(end, |&next| corner.midpoint(next));
            outline.curve_to(from.midpoint(corner), corner.midpoint(to), to);
            from = to;
        }
    } else {
        for &corner in corners {
            outline.line_to(corner);
        }
        outline.line_to(end);
    }
    let mut handles = Vec::new();
    for (i, pair) in corners.windows(2).enumerate() {
        let center = pair[0].midpoint(pair[1]);
        let (origin, dir) = if i == 1 {
            (Point::new(center.x, 0.), Vec2::new(0., h / 100_000.))
        } else {
            (Point::new(0., center.y), Vec2::new(w / 100_000., 0.))
        };
        handles.push(Handle::new(i, origin, dir, value(i), -1e6, 1e6));
    }
    Some(Geometry {
        paths: vec![stroke_only(outline)],
        text_rect: Rect::new(0., 0., w, h),
        handles,
        sites: vec![start, end],
    })
}

/// ECMA's corner guides are measured against the shorter shape side. The document keeps
/// its original preset and values; text placement and connector indices retain the editor's
/// established layout, including its padding for mixed corners.
pub(super) fn rectangle(name: &str, w: f64, h: f64, first: f64, second: f64) -> Option<Geometry> {
    use Corner::{Round, Snip, Square};
    let unit = w.min(h) / 100_000.;
    let first = first.clamp(0., 50_000.);
    let second = second.clamp(0., 50_000.);
    let (a, b) = (unit * first, unit * second);
    let full = Rect::new(0., 0., w, h);
    let tl = Point::ORIGIN;
    let tr = Point::new(w, 0.);
    let bl = Point::new(0., h);
    let right = Vec2::new(unit, 0.);
    let left = -right;
    let guide = |index, origin, direction, adjustment| {
        Handle::new(index, origin, direction, adjustment, 0., 50_000.)
    };
    let round_inset = |radius: f64| radius * 29_289. / 100_000.;
    let (corners, text, controls) = match name {
        "rect" | "textBox" => ([Square; 4], full, vec![]),
        "roundRect" => (
            [Round(a); 4],
            full.inset(-round_inset(a)),
            vec![guide(0, tl, right, first)],
        ),
        "round1Rect" => (
            [Square, Round(a), Square, Square],
            full.inset(-round_inset(a)),
            vec![guide(0, tr, left, first)],
        ),
        "snip1Rect" => (
            [Square, Snip(a), Square, Square],
            Rect::new(0., a / 2., w - a / 2., h),
            vec![guide(0, tr, left, first)],
        ),
        "snipRoundRect" => (
            [Round(a), Snip(b), Square, Square],
            full.inset(-a.max(b) / 2.),
            vec![guide(0, tl, right, first), guide(1, tr, left, second)],
        ),
        "snip2SameRect" | "round2SameRect" | "snip2DiagRect" | "round2DiagRect" => {
            let (first_corner, second_corner, inset) = if name.starts_with("round") {
                (Round(a), Round(b), round_inset(a.max(b)))
            } else {
                (Snip(a), Snip(b), a.max(b) / 2.)
            };
            if name.ends_with("SameRect") {
                (
                    [first_corner, first_corner, second_corner, second_corner],
                    full.inset(-inset),
                    vec![guide(0, tr, left, first), guide(1, bl, right, second)],
                )
            } else {
                (
                    [first_corner, second_corner, first_corner, second_corner],
                    full.inset(-inset),
                    vec![guide(0, tl, right, first), guide(1, tr, left, second)],
                )
            }
        }
        _ => return None,
    };
    let mut geometry = simple(corner_rect(w, h, corners), text, w, h);
    geometry.handles = controls;
    Some(geometry)
}

pub(super) fn straight_arrow(
    direction: Dir,
    w: f64,
    h: f64,
    shaft_adj: f64,
    head_adj: f64,
) -> Geometry {
    let (length, width) = len_thick(direction, w, h);
    let unit = length.min(width) / 100_000.;
    let max_head = if unit > 0. { length / unit } else { 0. };
    let head_adj = head_adj.clamp(0., max_head);
    let shaft_adj = shaft_adj.clamp(0., 100_000.);
    let head = (unit * head_adj).min(length);
    let neck = length - head;
    let fraction = shaft_adj / 100_000.;
    let top = width * (1. - fraction) / 2.;
    let bottom = width - top;
    let project = |along: f64, across: f64| match direction {
        Dir::Right => Point::new(along, across),
        Dir::Left => Point::new(w - along, across),
        Dir::Up => Point::new(across, h - along),
        Dir::Down => Point::new(across, along),
    };
    let corners = [
        (0., top),
        (neck, top),
        (neck, 0.),
        (length, width / 2.),
        (neck, width),
        (neck, bottom),
        (0., bottom),
    ]
    .map(|(x, y)| {
        let p = project(x, y);
        (p.x, p.y)
    });
    let shaft_origin = project(0., width / 2.);
    let head_origin = project(length, 0.);
    let shaft_dir = project(0., 0.) - shaft_origin;
    let head_dir = match direction {
        Dir::Right => Vec2::new(-unit, 0.),
        Dir::Left => Vec2::new(unit, 0.),
        Dir::Down => Vec2::new(0., -unit),
        Dir::Up => Vec2::new(0., unit),
    };
    let shoulders = [project(neck, 0.), project(neck, width)];
    let tail = project(0., width / 2.);
    let tip = project(length, width / 2.);
    // DrawingML connection indices run top, left, bottom, right for these presets.
    let sites = match direction {
        Dir::Right => [shoulders[0], tail, shoulders[1], tip],
        Dir::Left => [shoulders[0], tip, shoulders[1], tail],
        Dir::Down => [tail, shoulders[0], tip, shoulders[1]],
        Dir::Up => [tip, shoulders[0], tail, shoulders[1]],
    };
    Geometry {
        paths: vec![fill(Pb::new().poly(&corners).take())],
        text_rect: Rect::from_points(project(0., top), project(length - head * fraction, bottom)),
        handles: vec![
            Handle::new(
                0,
                shaft_origin,
                shaft_dir / 100_000.,
                shaft_adj,
                0.,
                100_000.,
            ),
            Handle::new(1, head_origin, head_dir, head_adj, 0., max_head),
        ],
        sites: sites.to_vec(),
    }
}

/// leftRightArrow and upDownArrow use a common polygon, with distinct connector ordering.
pub(super) fn opposed_arrows(vertical: bool, w: f64, h: f64, shaft: f64, head: f64) -> Geometry {
    let (length, width) = if vertical { (h, w) } else { (w, h) };
    let at = |along, across| {
        if vertical {
            Point::new(across, along)
        } else {
            Point::new(along, across)
        }
    };
    let unit = w.min(h) / 100_000.;
    let max_head = if unit > 0. { length / (2. * unit) } else { 0. };
    let head = head.clamp(0., max_head);
    let shaft = shaft.clamp(0., 100_000.);
    let depth = head * unit;
    let far = length - depth;
    let mid = width / 2.;
    let fraction = shaft / 100_000.;
    let inset = mid * (1. - fraction);
    let outline = [
        at(0., mid),
        at(depth, 0.),
        at(depth, inset),
        at(far, inset),
        at(far, 0.),
        at(length, mid),
        at(far, width),
        at(far, width - inset),
        at(depth, width - inset),
        at(depth, width),
    ];
    let sites = if vertical {
        vec![
            at(0., mid),
            at(depth, 0.),
            at(length / 2., inset),
            at(far, 0.),
            at(length, mid),
            at(far, width),
            at(length / 2., width - inset),
            at(depth, width),
        ]
    } else {
        vec![
            at(length, mid),
            at(far, width),
            at(depth, width),
            at(0., mid),
            at(depth, 0.),
            at(far, 0.),
        ]
    };
    let shaft_origin = at(far, mid);
    let text_inset = depth * fraction;
    Geometry {
        paths: vec![fill(Pb::new().poly(&outline.map(|p| (p.x, p.y))).take())],
        text_rect: Rect::from_points(
            at(text_inset, inset),
            at(length - text_inset, width - inset),
        ),
        handles: vec![
            Handle::new(
                0,
                shaft_origin,
                (at(far, 0.) - shaft_origin) / 100_000.,
                shaft,
                0.,
                100_000.,
            ),
            Handle::new(1, Point::ORIGIN, at(unit, 0.).to_vec2(), head, 0., max_head),
        ],
        sites,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_guides_route_every_leg_and_keep_draggable_adjustments() -> Result<(), &'static str>
    {
        use kurbo::PathEl;
        for curved in [false, true] {
            for legs in 2..=5 {
                let name = format!("{}Connector{legs}", if curved { "curved" } else { "bent" });
                let g = connector(&name, 200., 80., &[25_000., 75_000., 60_000.])
                    .ok_or("missing connector")?;
                let path = g.paths[0].path.elements();
                assert_eq!(g.sites, [Point::ORIGIN, Point::new(200., 80.)]);
                assert_eq!(g.handles.len(), legs - 2);
                assert_eq!(path.len(), if curved { legs } else { legs + 1 });
                if legs == 5 {
                    assert_eq!(g.handles[0].pos, Point::new(50., 30.));
                    assert_eq!(g.handles[1].pos, Point::new(85., 60.));
                    assert_eq!(g.handles[2].pos, Point::new(120., 70.));
                    if curved {
                        assert_eq!(
                            path[2],
                            PathEl::CurveTo(
                                Point::new(50., 45.),
                                Point::new(67.5, 60.),
                                Point::new(85., 60.)
                            )
                        );
                    } else {
                        assert_eq!(path[3], PathEl::LineTo(Point::new(120., 60.)));
                    }
                }
            }
        }
        Ok(())
    }

    use kurbo::Shape;

    #[test]
    fn every_rectangle_adj_handle_round_trips_from_its_declared_edge() -> Result<(), String> {
        let w = 200.;
        let h = 100.;
        for entry in super::super::CATALOG
            .iter()
            .filter(|p| p.category == super::super::Category::Rectangles)
        {
            let g = super::super::build(entry.name, w, h, &[20_000., 35_000.])
                .ok_or_else(|| format!("missing rectangle {}", entry.name))?;
            assert_eq!(g.handles.len(), entry.defaults.len(), "{}", entry.name);
            assert_eq!(g.sites.len(), 4);
            assert!(g.paths[0].path.contains(Point::new(100., 50.)));
            for (i, handle) in g.handles.iter().enumerate() {
                let value = [20_000., 35_000.][i];
                assert_eq!(handle.index, i);
                assert_eq!(handle.value_at(handle.pos), value, "{}", entry.name);
                assert_eq!(
                    handle.value_at(handle.origin + handle.dir * 90_000.),
                    50_000.
                );
                assert_eq!(handle.value_at(handle.origin - handle.dir * 10_000.), 0.);
            }
        }
        Ok(())
    }

    #[test]
    fn opposite_heads_have_independent_connectors_and_draggable_handles() {
        for vertical in [false, true] {
            let g = opposed_arrows(vertical, 200., 100., 50_000., 50_000.);
            assert_eq!(g.sites.len(), if vertical { 8 } else { 6 });
            assert_eq!(
                g.text_rect,
                if vertical {
                    Rect::new(50., 25., 150., 75.)
                } else {
                    Rect::new(25., 25., 175., 75.)
                }
            );
            assert!(g.paths[0].path.contains(Point::new(100., 50.)));
            assert!(!g.paths[0].path.contains(Point::new(1., 1.)));
            for handle in &g.handles {
                assert!((handle.value_at(handle.pos) - 50_000.).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn right_arrow_has_specified_sites_handles_text_and_hit_boundary() {
        let g = straight_arrow(Dir::Right, 200., 100., 50_000., 50_000.);
        assert_eq!(g.text_rect, Rect::new(0., 25., 175., 75.));
        assert_eq!(
            g.sites,
            [(150., 0.), (0., 50.), (150., 100.), (200., 50.)].map(Point::from)
        );
        assert_eq!(g.handles[0].pos, Point::new(0., 25.));
        assert_eq!(g.handles[1].pos, Point::new(150., 0.));
        assert!(g.paths[0].path.contains(Point::new(170., 50.)));
        assert!(!g.paths[0].path.contains(Point::new(195., 5.)));
        let empty_head = straight_arrow(Dir::Right, 200., 100., -100., 0.);
        assert_eq!(
            empty_head.handles[1].value_at(Point::new(100., 0.)),
            100_000.
        );
    }
}
