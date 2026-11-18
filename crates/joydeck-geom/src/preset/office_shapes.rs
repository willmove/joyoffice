//! ECMA-376 Part 1 §20.1.9.18 and the DrawingML geometry attachment, 5th edition.
//! Use the head length and shaft thickness in shape coordinates before applying orientation.
use super::builder::{Corner, Dir, Pb, corner_rect, fill, len_thick, simple};
use super::catalog::{Geometry, Handle};
use kurbo::{Point, Rect, Vec2};

pub(super) fn rounded_rectangle(w: f64, h: f64, adjustment: f64) -> Geometry {
    let unit = w.min(h) / 100_000.;
    let amount = adjustment.clamp(0., 50_000.);
    let radius = unit * amount;
    let inner_margin = radius * 29_289. / 100_000.;
    let text = Rect::new(
        inner_margin,
        inner_margin,
        w - inner_margin,
        h - inner_margin,
    );
    let mut shape = simple(corner_rect(w, h, [Corner::Round(radius); 4]), text, w, h);
    shape.handles.push(Handle::new(
        0,
        Point::ORIGIN,
        Vec2::new(unit, 0.),
        amount,
        0.,
        50_000.,
    ));
    shape
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
    use kurbo::Shape;

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
