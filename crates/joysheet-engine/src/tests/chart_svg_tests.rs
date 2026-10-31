use super::*;

#[test]
fn every_primitive_becomes_an_svg_element() {
    let red = [255, 0, 0, 255];
    let prims = vec![
        Prim::Rect { x: 1.0, y: 2.0, w: 3.0, h: 4.0, fill: Some(red), stroke: None, radius: 0.0 },
        Prim::Line { pts: vec![[0.0, 0.0], [5.0, 5.5]], color: red, width: 1.0, dash: true },
        Prim::Polygon { pts: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], fill: red, stroke: Some(([0, 0, 0, 255], 1.0)) },
        Prim::Wedge { cx: 10.0, cy: 10.0, r_outer: 5.0, r_inner: 2.0, a0: 0.0, a1: 1.0, fill: red, stroke: None },
        Prim::Circle { cx: 1.0, cy: 1.0, r: 2.0, fill: Some([0, 0, 255, 128]), stroke: None },
        Prim::Text {
            x: 5.0,
            y: 6.0,
            text: "A<B".into(),
            size: 10.0,
            color: red,
            bold: true,
            align: HAlign::Center,
            valign: VAlign::Top,
            rotation: -1.5,
        },
    ];
    let svg = prims_svg(&prims, 100.0, 50.0, "left:1px");
    for part in [
        "<rect x=\"1\" y=\"2\" width=\"3\" height=\"4\" fill=\"#ff0000\"/>",
        "<polyline points=\"0,0 5,5.5\" fill=\"none\" stroke=\"#ff0000\" stroke-width=\"1\" stroke-dasharray=\"3 2\"/>",
        "<polygon ",
        "<path d=\"M10 5 A5 5 0 0 1 ",
        "fill-opacity=\"0.50\"",
        "text-anchor=\"middle\"",
        "font-weight=\"bold\"",
        "transform=\"rotate(-85.94 5 6)\"",
        ">A&lt;B</text>",
    ] {
        assert!(svg.contains(part), "{part} in {svg}");
    }
    assert!(svg.starts_with("<svg ") && svg.ends_with("</svg>") && svg.contains("style=\"left:1px\""), "{svg}");
}

#[test]
fn non_finite_numbers_do_not_break_the_markup() {
    let prims = [Prim::Circle { cx: f32::NAN, cy: f32::INFINITY, r: 1.0, fill: None, stroke: None }];
    assert!(prims_svg(&prims, 10.0, 10.0, "").contains("<circle cx=\"0\" cy=\"0\" r=\"1\" fill=\"none\"/>"));
}

#[test]
fn a_full_circle_wedge_keeps_a_drawable_arc() {
    let d = wedge_path(0.0, 0.0, 10.0, 0.0, 0.0, std::f32::consts::TAU);
    assert!(d.contains("A10 10 0 1 1 ") && d.ends_with("L0 0 Z"), "{d}");
}
