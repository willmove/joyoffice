use super::builder::Dir;
use super::catalog::{Geometry, adjustment_defaults};
use super::{LineCallout, action_buttons, flowchart, office_shapes, shapes};

/// Build an Office preset at the frame's point dimensions.
pub fn build(name: &str, w: f64, h: f64, adj: &[f64]) -> Option<Geometry> {
    if !(w.is_finite() && h.is_finite()) {
        return None;
    }
    let (w, h) = (w.max(0.), h.max(0.));
    let values = adjustment_defaults(name);
    let read = |i: usize| {
        adj.get(i)
            .or_else(|| values.get(i))
            .copied()
            .filter(|v| v.is_finite())
            .unwrap_or(0.)
    };
    match name {
        "rightArrow" | "leftArrow" | "upArrow" | "downArrow" => {
            let dir = match name {
                "leftArrow" => Dir::Left,
                "upArrow" => Dir::Up,
                "downArrow" => Dir::Down,
                _ => Dir::Right,
            };
            Some(office_shapes::straight_arrow(dir, w, h, read(0), read(1)))
        }
        "leftRightArrow" | "upDownArrow" => Some(office_shapes::opposed_arrows(
            name == "upDownArrow",
            w,
            h,
            read(0),
            read(1),
        )),
        _ => flowchart::build(name, w, h)
            .or_else(|| LineCallout::named(name).map(|c| c.geometry(w, h, adj)))
            .or_else(|| action_buttons::build(name, w, h))
            .or_else(|| office_shapes::rectangle(name, w, h, read(0), read(1)))
            .or_else(|| office_shapes::connector(name, w, h, adj))
            .or_else(|| shapes::build(name, w, h, adj)),
    }
}
