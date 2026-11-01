use super::super::{CATALOG, FillMode, build, info};
use kurbo::{BezPath, PathEl, Point, Shape};

const NEW: [&str; 38] = [
    "lineInv",
    "nonIsoscelesTrapezoid",
    "pieWedge",
    "leftRightArrowCallout",
    "upDownArrowCallout",
    "quadArrowCallout",
    "circularArrow",
    "leftCircularArrow",
    "leftRightCircularArrow",
    "curvedRightArrow",
    "curvedLeftArrow",
    "curvedUpArrow",
    "curvedDownArrow",
    "swooshArrow",
    "callout1",
    "callout2",
    "callout3",
    "accentCallout1",
    "accentCallout2",
    "accentCallout3",
    "borderCallout2",
    "borderCallout3",
    "accentBorderCallout1",
    "accentBorderCallout2",
    "accentBorderCallout3",
    "ellipseRibbon",
    "ellipseRibbon2",
    "leftRightRibbon",
    "flowChartOfflineStorage",
    "gear6",
    "gear9",
    "funnel",
    "cornerTabs",
    "squareTabs",
    "plaqueTabs",
    "chartX",
    "chartStar",
    "chartPlus",
];

fn outline(name: &str, w: f64, h: f64, adj: &[f64]) -> BezPath {
    build(name, w, h, adj)
        .map(|g| g.outline())
        .unwrap_or_default()
}

/// Distance from `p` to the nearest point of the (flattened) outline.
fn dist(path: &BezPath, p: Point) -> f64 {
    let mut best = f64::MAX;
    let mut prev = Point::ZERO;
    let mut start = Point::ZERO;
    kurbo::flatten(path.iter(), 0.01, |el| {
        let (a, b) = match el {
            PathEl::MoveTo(q) => {
                prev = q;
                start = q;
                return;
            }
            PathEl::LineTo(q) => (prev, q),
            PathEl::ClosePath => (prev, start),
            _ => return,
        };
        let d = b - a;
        let t = if d.hypot2() > 0.0 {
            ((p - a).dot(d) / d.hypot2()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        best = best.min((a + d * t - p).hypot());
        prev = b;
    });
    best
}

/// Asserts every point lies within 1pt of the outline.
fn on(name: &str, w: f64, h: f64, adj: &[f64], pts: &[(f64, f64)]) {
    let path = outline(name, w, h, adj);
    for &(x, y) in pts {
        let d = dist(&path, Point::new(x, y));
        assert!(
            d < 1.0,
            "{name} {w}x{h} {adj:?}: ({x}, {y}) is {d:.2} from the outline"
        );
    }
}

#[test]
fn new_presets_are_in_the_catalog() {
    for n in NEW {
        let i = info(n).unwrap_or_else(|| panic!("{n} missing from the catalog"));
        assert!(!i.label.is_empty());
    }
    assert!(CATALOG.len() >= 183);
}

#[test]
fn new_presets_build_within_bounds_at_many_sizes_and_adjusts() {
    let sizes = [
        (480.0, 300.0),
        (300.0, 300.0),
        (200.0, 460.0),
        (100.0, 460.0),
        (1.0, 1.0),
        (0.0, 0.0),
        (0.0, 50.0),
        (60.0, 0.0),
    ];
    let adjusts: [&[f64]; 8] = [
        &[],
        &[0.0; 8],
        &[100000.0; 8],
        &[-50000.0; 8],
        &[1e12, -1e12, 1e12, -1e12, 1e12, -1e12, 1e12, -1e12],
        &[
            f64::MAX,
            f64::MIN,
            f64::MAX,
            f64::MIN,
            f64::MAX,
            f64::MIN,
            f64::MAX,
            f64::MIN,
        ],
        &[
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            5.0,
            6.0,
            7.0,
            8.0,
        ],
        &[
            30000.0, 60000.0, 20000.0, 9000000.0, 20000.0, 40000.0, 120000.0, -30000.0,
        ],
    ];
    for n in NEW {
        // Line callouts' leaders may reach anywhere, by design.
        let callout = n.contains("allout") && !n.contains("Arrow");
        for (w, h) in sizes {
            for adj in adjusts {
                let g = build(n, w, h, adj).unwrap_or_else(|| panic!("{n} did not build"));
                assert!(!g.paths.is_empty(), "{n} has no paths");
                for s in &g.paths {
                    let bb = s.path.bounding_box();
                    assert!(
                        bb.x0.is_finite()
                            && bb.y0.is_finite()
                            && bb.x1.is_finite()
                            && bb.y1.is_finite(),
                        "{n} {w}x{h} {adj:?}: {bb:?}"
                    );
                    if !callout {
                        // Circular arrows with a head longer than their ring can hold carry the
                        // inner edge on from the head, as the reference does, so may spill.
                        let spill = if n.contains("ircular") { 0.5 } else { 0.01 };
                        let tol = 0.5 + w.max(h) * spill;
                        assert!(
                            bb.x0 >= -tol && bb.y0 >= -tol && bb.x1 <= w + tol && bb.y1 <= h + tol,
                            "{n} {w}x{h} {adj:?} out of bounds: {bb:?}"
                        );
                    }
                }
                let t = g.text_rect;
                assert!(
                    t.x0.is_finite() && t.y0.is_finite() && t.x1.is_finite() && t.y1.is_finite(),
                    "{n} text rect {t:?}"
                );
            }
        }
        let g = build(n, 480.0, 300.0, &[]).unwrap_or_else(|| panic!("{n}"));
        assert!(
            g.outline().elements().len() >= 2,
            "{n} default outline is empty"
        );
    }
}

// The spot checks below are outline points measured on reference renders.

#[test]
fn simple_new_presets_match_reference_points() {
    on("lineInv", 480.0, 300.0, &[], &[(0.0, 300.0), (480.0, 0.0)]);
    on(
        "nonIsoscelesTrapezoid",
        480.0,
        300.0,
        &[],
        &[(75.0, 0.0), (405.0, 0.0), (0.0, 300.0)],
    );
    on(
        "nonIsoscelesTrapezoid",
        200.0,
        460.0,
        &[80000.0, 20000.0],
        &[(100.0, 0.0), (160.0, 0.0)],
    );
    on(
        "pieWedge",
        480.0,
        300.0,
        &[],
        &[(64.3, 150.0), (0.0, 300.0), (480.0, 0.0)],
    );
    on(
        "flowChartOfflineStorage",
        480.0,
        300.0,
        &[],
        &[(240.0, 300.0), (192.0, 240.0), (288.0, 240.0)],
    );
    on(
        "squareTabs",
        480.0,
        300.0,
        &[],
        &[(28.3, 28.3), (451.7, 271.7)],
    );
    on("cornerTabs", 480.0, 300.0, &[], &[(23.3, 5.0)]);
    on("plaqueTabs", 480.0, 300.0, &[], &[(27.85, 5.0)]);
    on(
        "chartStar",
        480.0,
        300.0,
        &[],
        &[(240.0, 0.0), (240.0, 300.0), (48.0, 30.0)],
    );
    on(
        "chartPlus",
        480.0,
        300.0,
        &[],
        &[(0.0, 150.0), (240.0, 0.0)],
    );
    // Chart markers and plain callouts leave the box unstroked.
    for n in ["chartX", "callout1", "accentCallout2"] {
        let g = build(n, 100.0, 100.0, &[]).unwrap_or_else(|| panic!("{n}"));
        assert!(!g.paths[0].stroke, "{n}");
    }
}

#[test]
fn line_callouts_follow_their_adjusts() {
    on(
        "callout3",
        480.0,
        300.0,
        &[],
        &[
            (-40.0, 56.25),
            (-80.0, 56.25),
            (-80.0, 300.0),
            (-40.0, 338.9),
        ],
    );
    let adj = [
        50000.0, 110000.0, 20000.0, 120000.0, -20000.0, 80000.0, 30000.0, 40000.0,
    ];
    let g = build("accentBorderCallout3", 480.0, 300.0, &adj)
        .unwrap_or_else(|| panic!("accentBorderCallout3"));
    assert_eq!(g.paths.len(), 3);
    assert!(g.paths[0].stroke);
    let accent = g.paths[1].path.bounding_box();
    assert!(
        (accent.x0 - 528.0).abs() < 1e-6 && accent.y0 == 0.0 && accent.y1 == 300.0,
        "{accent:?}"
    );
    on(
        "accentBorderCallout3",
        480.0,
        300.0,
        &adj,
        &[(576.0, 60.0), (384.0, -60.0), (192.0, 90.0)],
    );
}

#[test]
fn arrow_callouts_match_reference_points() {
    on(
        "leftRightArrowCallout",
        480.0,
        300.0,
        &[],
        &[
            (0.0, 150.0),
            (75.0, 75.0),
            (124.5, 0.0),
            (355.5, 300.0),
            (480.0, 150.0),
        ],
    );
    on(
        "leftRightArrowCallout",
        200.0,
        460.0,
        &[],
        &[(50.0, 180.0), (52.0, 0.0), (148.0, 460.0)],
    );
    on(
        "upDownArrowCallout",
        480.0,
        300.0,
        &[],
        &[(240.0, 0.0), (0.0, 77.8), (480.0, 222.2), (240.0, 300.0)],
    );
    on(
        "quadArrowCallout",
        480.0,
        300.0,
        &[],
        &[
            (0.0, 150.0),
            (124.5, 77.8),
            (240.0, 0.0),
            (480.0, 150.0),
            (240.0, 300.0),
        ],
    );
}

#[test]
fn curved_arrows_match_reference_points() {
    on(
        "curvedRightArrow",
        480.0,
        300.0,
        &[],
        &[
            (480.0, 0.0),
            (480.0, 75.0),
            (480.0, 225.0),
            (405.0, 149.0),
            (405.0, 299.0),
            (100.0, 151.5),
            (300.0, 6.7),
        ],
    );
    on(
        "curvedRightArrow",
        200.0,
        460.0,
        &[30000.0, 60000.0, 40000.0],
        &[
            (200.0, 400.0),
            (120.0, 324.5),
            (120.0, 443.5),
            (200.0, 60.0),
        ],
    );
    // The band never outgrows the head; the head never outgrows half the frame.
    on(
        "curvedRightArrow",
        200.0,
        460.0,
        &[60000.0, 30000.0, 80000.0],
        &[(200.0, 60.0)],
    );
    on(
        "curvedLeftArrow",
        480.0,
        300.0,
        &[],
        &[(0.0, 225.0), (0.0, 0.0)],
    );
    on(
        "curvedUpArrow",
        480.0,
        300.0,
        &[],
        &[(405.0, 0.0), (0.0, 0.0), (75.0, 0.0)],
    );
    on(
        "curvedDownArrow",
        480.0,
        300.0,
        &[],
        &[(405.0, 300.0), (0.0, 300.0)],
    );
    let g =
        build("curvedRightArrow", 480.0, 300.0, &[]).unwrap_or_else(|| panic!("curvedRightArrow"));
    assert_eq!(g.paths[0].fill, FillMode::DarkenLess);
}

#[test]
fn swoosh_matches_reference_points() {
    on(
        "swooshArrow",
        480.0,
        300.0,
        &[],
        &[
            (0.0, 300.0),
            (425.8, 0.0),
            (480.0, 59.7),
            (442.8, 150.0),
            (202.0, 105.5),
            (202.0, 159.0),
        ],
    );
    on(
        "swooshArrow",
        480.0,
        300.0,
        &[40000.0, 16667.0],
        &[(480.0, 82.2), (447.8, 195.0)],
    );
    on(
        "swooshArrow",
        200.0,
        460.0,
        &[],
        &[(163.9, 0.0), (200.0, 59.4)],
    );
}

#[test]
fn ribbons_match_reference_points() {
    on(
        "ellipseRibbon",
        480.0,
        300.0,
        &[],
        &[
            (0.0, 0.0),
            (120.0, 28.1),
            (60.0, 128.9),
            (240.0, 300.0),
            (120.0, 66.1),
            (180.0, 72.2),
            (180.0, 35.2),
        ],
    );
    on(
        "ellipseRibbon",
        480.0,
        300.0,
        &[30000.0, 60000.0, 20000.0],
        &[(120.0, 45.0), (240.0, 300.0), (96.0, 68.4)],
    );
    on(
        "ellipseRibbon",
        480.0,
        300.0,
        &[80000.0, 20000.0, 60000.0],
        &[(240.0, 240.0), (1.0, 61.75)],
    );
    on(
        "ellipseRibbon2",
        480.0,
        300.0,
        &[],
        &[(0.0, 300.0), (240.0, 0.0)],
    );
    on(
        "leftRightRibbon",
        480.0,
        300.0,
        &[],
        &[
            (0.0, 125.0),
            (150.0, 0.0),
            (150.0, 50.0),
            (255.0, 62.5),
            (225.0, 87.5),
            (480.0, 175.0),
            (330.0, 300.0),
        ],
    );
    on(
        "leftRightRibbon",
        480.0,
        300.0,
        &[90000.0, 10000.0, 60000.0],
        &[(0.0, 100.0), (480.0, 200.0), (240.0, 300.0)],
    );
}

#[test]
fn gears_and_funnel_match_reference_points() {
    // Square gear6: body radius 105, top tooth tip 3.5 inside the frame, tip half-width ~16.3.
    on(
        "gear6",
        300.0,
        300.0,
        &[],
        &[(150.0, 3.5), (133.7, 3.5), (166.3, 3.5), (45.0, 150.0)],
    );
    on("gear9", 300.0, 300.0, &[], &[(150.0, 1.0), (30.0, 150.0)]);
    on("gear6", 480.0, 300.0, &[], &[(240.0, 3.5), (201.0, 3.5)]);
    on(
        "funnel",
        480.0,
        300.0,
        &[],
        &[
            (240.0, 0.0),
            (0.0, 75.0),
            (240.0, 15.0),
            (15.0, 75.0),
            (240.0, 300.0),
            (60.5, 150.0),
        ],
    );
    let g = build("funnel", 480.0, 300.0, &[]).unwrap_or_else(|| panic!("funnel"));
    assert!(g.paths[0].even_odd);
}

#[test]
fn circular_arrows_match_reference_points() {
    on(
        "circularArrow",
        300.0,
        300.0,
        &[],
        &[
            (19.0, 150.0),
            (56.5, 150.0),
            (150.0, 19.0),
            (262.4, 150.0),
            (218.6, 113.4),
            (293.6, 113.4),
        ],
    );
    on(
        "circularArrow",
        300.0,
        300.0,
        &[20000.0, 1800000.0, 18000000.0, 5400000.0, 20000.0],
        &[
            (150.0, 270.0),
            (150.0, 210.0),
            (247.0, 42.1),
            (143.0, 102.1),
            (227.9, 105.0),
        ],
    );
    on(
        "leftCircularArrow",
        300.0,
        300.0,
        &[],
        &[(37.6, 150.0), (262.4, 150.0), (150.0, 281.0)],
    );
    on(
        "leftRightCircularArrow",
        300.0,
        300.0,
        &[],
        &[(262.4, 150.0), (37.6, 150.0), (150.0, 19.0)],
    );
    // The band never grows past twice the head's half-width.
    on(
        "circularArrow",
        300.0,
        300.0,
        &[25000.0, 1142319.0, 20457681.0, 10800000.0, 5000.0],
        &[(0.0, 150.0), (30.0, 150.0)],
    );
}
