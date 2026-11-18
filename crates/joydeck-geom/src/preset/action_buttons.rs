//! DrawingML action buttons (ECMA-376 Part 1, DrawingML preset geometry attachment).
//! Each icon occupies the middle three quarters of the shorter side of the button.
//! Keep the base silhouette, shaded faces and pen paths separate for transparent
//! brushes and dashed outlines. The glue sites are ordered clockwise from the right.
use super::builder::{Pb, ellipse_arc, fill, fill_only, stroke_only};
use super::catalog::{FillMode, Geometry};
use kurbo::{Affine, BezPath, Point, Rect};
use std::sync::LazyLock;

struct Icon {
    mask: BezPath,
    faces: Vec<(FillMode, BezPath)>,
    pen: BezPath,
}

impl Icon {
    fn dark(mask: BezPath, pen: BezPath) -> Self {
        Self {
            faces: vec![(FillMode::Darken, mask.clone())],
            mask,
            pen,
        }
    }
}

fn polygon(points: &[(f64, f64)]) -> BezPath {
    Pb::new().poly(points).take()
}

// Start the dashed pen at the first vertex even when the fill winds the other way.
fn reverse_polygon(points: &[(f64, f64)]) -> BezPath {
    let mut pen = Pb::new();
    let Some((&(x, y), rest)) = points.split_first() else {
        return pen.take();
    };
    pen.m(x, y);
    for &(x, y) in rest.iter().rev() {
        pen.l(x, y);
    }
    pen.z().take()
}

fn navigation(forward: bool, bar: bool) -> Icon {
    let x = |value: f64| if forward { 32. - value } else { value };
    let head = [
        (x(if bar { 10. } else { 4. }), 16.),
        (x(28.), 4.),
        (x(28.), 28.),
    ];
    let mut mask = polygon(&head);
    let mut pen = if forward {
        reverse_polygon(&head)
    } else {
        mask.clone()
    };
    if bar {
        let bar = [(x(7.), 4.), (x(4.), 4.), (x(4.), 28.), (x(7.), 28.)];
        let face = polygon(&bar);
        let stroke = if forward {
            face.clone()
        } else {
            reverse_polygon(&bar)
        };
        mask.extend(face);
        pen.extend(stroke);
    }
    Icon::dark(mask, pen)
}

fn paper() -> Icon {
    let mask = polygon(&[(7., 4.), (19., 4.), (25., 10.), (25., 28.), (7., 28.)]);
    let sheet = polygon(&[
        (7., 4.),
        (19., 4.),
        (19., 10.),
        (25., 10.),
        (25., 28.),
        (7., 28.),
    ]);
    let fold = polygon(&[(19., 4.), (19., 10.), (25., 10.)]);
    let mut pen = mask.clone();
    pen.extend(Pb::new().m(25., 10.).l(19., 10.).l(19., 4.).take());
    Icon {
        mask,
        faces: vec![(FillMode::DarkenLess, sheet), (FillMode::Darken, fold)],
        pen,
    }
}

fn home() -> Icon {
    let silhouette = [
        (16., 4.),
        (4., 16.),
        (7., 16.),
        (7., 28.),
        (25., 28.),
        (25., 16.),
        (28., 16.),
        (23.5, 11.5),
        (23.5, 5.5),
        (20.5, 5.5),
        (20.5, 8.5),
    ];
    let mut walls = polygon(&[(23.5, 11.5), (23.5, 5.5), (20.5, 5.5), (20.5, 8.5)]);
    walls.extend(polygon(&[
        (7., 16.),
        (7., 28.),
        (14.5, 28.),
        (14.5, 22.),
        (17.5, 22.),
        (17.5, 28.),
        (25., 28.),
        (25., 16.),
    ]));
    let mut roof_and_door = polygon(&[(16., 4.), (4., 16.), (28., 16.)]);
    roof_and_door.extend(polygon(&[
        (14.5, 22.),
        (17.5, 22.),
        (17.5, 28.),
        (14.5, 28.),
    ]));
    let mut pen = reverse_polygon(&silhouette);
    pen.extend(
        Pb::new()
            .m(20.5, 8.5)
            .l(23.5, 11.5)
            .m(25., 16.)
            .l(7., 16.)
            .m(14.5, 28.)
            .l(14.5, 22.)
            .l(17.5, 22.)
            .l(17.5, 28.)
            .take(),
    );
    Icon {
        mask: polygon(&silhouette),
        faces: vec![
            (FillMode::DarkenLess, walls),
            (FillMode::Darken, roof_and_door),
        ],
        pen,
    }
}

fn information() -> Icon {
    let mut disc = Pb::new();
    ellipse_arc(disc.m(16., 4.), 12., 12., 270., 360.).z();
    let mask = disc.take();
    let mut dot = Pb::new();
    ellipse_arc(dot.m(16., 4.75), 2.25, 2.25, 270., 360.);
    let letter = [
        (11.5, 11.5),
        (18.25, 11.5),
        (18.25, 23.5),
        (20.5, 23.5),
        (20.5, 25.),
        (11.5, 25.),
        (11.5, 23.5),
        (13.75, 23.5),
        (13.75, 13.),
        (11.5, 13.),
    ];
    let mut dark = mask.clone();
    let mut light = dot.take();
    dark.extend(light.iter());
    dark.extend(reverse_polygon(&letter));
    light.extend(polygon(&letter));
    let mut pen = mask.clone();
    pen.extend(light.iter());
    Icon {
        mask,
        faces: vec![(FillMode::Darken, dark), (FillMode::Lighten, light)],
        pen,
    }
}

fn help() -> Icon {
    // The question mark's seven equal vertical divisions avoid rounded join positions.
    let mut p = Pb::new();
    p.m(64., 76.);
    ellipse_arc(&mut p, 48., 48., 180., 180.);
    ellipse_arc(&mut p, 24., 36., 0., 90.);
    ellipse_arc(&mut p, 12., 18., 270., -90.);
    p.l(124., 154.).l(100., 154.).l(100., 130.);
    ellipse_arc(&mut p, 24., 36., 180., 90.);
    ellipse_arc(&mut p, 12., 18., 90., -90.);
    ellipse_arc(&mut p, 24., 24., 0., -180.).z().m(112., 160.);
    ellipse_arc(&mut p, 18., 18., 270., 360.).z();
    let shape = Affine::scale(1. / 7.) * p.take();
    Icon::dark(shape.clone(), shape)
}

fn return_arrow() -> Icon {
    let mut p = Pb::new();
    p.m(28., 10.).l(22., 4.).l(16., 10.).l(19., 10.).l(19., 19.);
    ellipse_arc(&mut p, 3., 3., 0., 90.).l(13., 22.);
    ellipse_arc(&mut p, 3., 3., 90., 90.)
        .l(10., 10.)
        .l(4., 10.)
        .l(4., 19.);
    ellipse_arc(&mut p, 9., 9., 180., -90.).l(16., 28.);
    ellipse_arc(&mut p, 9., 9., 90., -90.).l(25., 10.).z();
    let mask = p.take();
    p.m(28., 10.).l(25., 10.).l(25., 19.);
    ellipse_arc(&mut p, 9., 9., 0., 90.).l(13., 28.);
    ellipse_arc(&mut p, 9., 9., 90., 90.)
        .l(4., 10.)
        .l(10., 10.)
        .l(10., 19.);
    ellipse_arc(&mut p, 3., 3., 180., -90.).l(16., 22.);
    ellipse_arc(&mut p, 3., 3., 90., -90.)
        .l(19., 10.)
        .l(16., 10.)
        .l(22., 4.)
        .z();
    Icon::dark(mask, p.take())
}

fn movie() -> Icon {
    // Coordinates within the camera's 21600-unit square from the DrawingML attachment.
    let camera = [
        (0., 5280.),
        (0., 9555.),
        (1455., 9555.),
        (1905., 9067.),
        (2325., 9067.),
        (2325., 15592.),
        (17010., 15592.),
        (17010., 13342.),
        (19335., 13342.),
        (20595., 14580.),
        (21600., 14580.),
        (21600., 6630.),
        (20595., 6630.),
        (19725., 7492.),
        (17010., 7492.),
        (17010., 6630.),
        (16155., 5730.),
        (1905., 5730.),
        (1455., 5280.),
    ];
    let place = Affine::translate((4., 4.)) * Affine::scale(24. / 21600.);
    Icon::dark(place * polygon(&camera), place * reverse_polygon(&camera))
}

fn sound() -> Icon {
    let speaker = [
        (4., 11.5),
        (4., 20.5),
        (11.5, 20.5),
        (19., 28.),
        (19., 4.),
        (11.5, 11.5),
    ];
    let mut pen = reverse_polygon(&speaker);
    pen.extend(
        Pb::new()
            .m(22., 11.5)
            .l(28., 7.)
            .m(22., 16.)
            .l(28., 16.)
            .m(22., 20.5)
            .l(28., 25.)
            .take(),
    );
    Icon::dark(polygon(&speaker), pen)
}

// Build each immutable glyph once. A resize only maps the glyph into the new frame.
static ICONS: LazyLock<[Icon; 11]> = LazyLock::new(|| {
    [
        navigation(false, false),
        navigation(true, false),
        navigation(false, true),
        navigation(true, true),
        paper(),
        home(),
        information(),
        help(),
        return_arrow(),
        movie(),
        sound(),
    ]
});

pub(super) fn build(name: &str, w: f64, h: f64) -> Option<Geometry> {
    let glyph = match name.strip_prefix("actionButton")? {
        "Blank" => None,
        "BackPrevious" => Some(0),
        "ForwardNext" => Some(1),
        "Beginning" => Some(2),
        "End" => Some(3),
        "Document" => Some(4),
        "Home" => Some(5),
        "Information" => Some(6),
        "Help" => Some(7),
        "Return" => Some(8),
        "Movie" => Some(9),
        "Sound" => Some(10),
        _ => return None,
    };
    let border = polygon(&[(0., 0.), (w, 0.), (w, h), (0., h)]);
    let paths = if let Some(index) = glyph {
        let icon = &ICONS[index];
        let side = w.min(h);
        let place =
            Affine::translate(((w - side) * 0.5, (h - side) * 0.5)) * Affine::scale(side / 32.);
        let mut base = border.clone();
        base.extend(place * &icon.mask);
        // The glyph cuts a hole in the body even when a mirrored contour
        // winds in the same direction. Otherwise an alpha brush paints it twice.
        let mut body = fill_only(base);
        body.even_odd = true;
        let mut paths = vec![body];
        for (color, face) in &icon.faces {
            let mut layer = fill_only(place * face);
            layer.fill = *color;
            layer.even_odd = true;
            paths.push(layer);
        }
        paths.push(stroke_only(place * &icon.pen));
        paths.push(stroke_only(border));
        paths
    } else {
        vec![fill(border)]
    };
    Some(Geometry {
        paths,
        text_rect: Rect::new(0., 0., w, h),
        handles: vec![],
        sites: vec![
            Point::new(w, h / 2.),
            Point::new(w / 2., h),
            Point::new(0., h / 2.),
            Point::new(w / 2., 0.),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::{CATALOG, Category};
    use FillMode::{Darken, DarkenLess, Lighten, None as NoFill, Norm};

    #[test]
    fn button_layers_and_cardinal_sites_match_the_published_order() -> Result<(), &'static str> {
        let names: Vec<_> = CATALOG
            .iter()
            .filter(|shape| shape.category == Category::ActionButtons)
            .collect();
        assert_eq!(names.len(), 12);
        for shape in names {
            for (w, h) in [
                (143., 57.),
                (46., 122.),
                (80., 80.),
                (0., 74.),
                (92., 0.),
                (0., 0.),
            ] {
                let g =
                    crate::preset::build(shape.name, w, h, &[]).ok_or("missing action button")?;
                let expected: &[_] = match shape.name {
                    "actionButtonBlank" => &[(Norm, true)],
                    "actionButtonHome" | "actionButtonDocument" => &[
                        (Norm, false),
                        (DarkenLess, false),
                        (Darken, false),
                        (NoFill, true),
                        (NoFill, true),
                    ],
                    "actionButtonInformation" => &[
                        (Norm, false),
                        (Darken, false),
                        (Lighten, false),
                        (NoFill, true),
                        (NoFill, true),
                    ],
                    _ => &[
                        (Norm, false),
                        (Darken, false),
                        (NoFill, true),
                        (NoFill, true),
                    ],
                };
                let actual: Vec<_> = g.paths.iter().map(|p| (p.fill, p.stroke)).collect();
                assert_eq!(actual, expected, "{}", shape.name);
                assert!(
                    g.paths
                        .iter()
                        .all(|p| p.path.is_finite() && !p.path.is_empty())
                );
                assert_eq!(
                    g.sites,
                    [(w, h / 2.), (w / 2., h), (0., h / 2.), (w / 2., 0.)].map(Point::from)
                );
                assert_eq!(g.text_rect, Rect::new(0., 0., w, h));
                assert!(g.handles.is_empty());
            }
        }
        Ok(())
    }
}
