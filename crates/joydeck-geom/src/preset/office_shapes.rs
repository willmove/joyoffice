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

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

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
