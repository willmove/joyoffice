//! Original vector symbols for the engine's conditional-format icon sets.
use gpui_kit::*;
pub fn paint(set: &str, index: usize, b: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let count = set.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(3).clamp(3, 5) as usize;
    let index = index.min(count - 1);
    let f = index as f32 / (count - 1) as f32;
    let k = (f32::from(b.size.height).min(f32::from(b.size.width)) / 16.).max(0.1);
    let origin = b.origin + point(px(f32::from(b.size.width) / 2.), px(f32::from(b.size.height) / 2.));
    let ink = if set.contains("Gray") {
        [105, 114, 122, 255]
    } else if f < 0.34 {
        [203, 69, 69, 255]
    } else if f < 0.67 {
        [190, 151, 23, 255]
    } else {
        [20, 130, 86, 255]
    };
    let path = |pts: &[[f32; 2]], fill, stroke, window: &mut Window| crate::charts::path(pts, true, fill, stroke, origin, k, window);
    let circle = |fill, stroke, window: &mut Window, cx: &mut App| {
        crate::charts::paint_primitive(&joysheet_chart::Prim::Circle { cx: 0., cy: 0., r: 5.7, fill, stroke }, origin, k, window, cx)
    };
    if set.contains("Quarters") {
        circle(Some([255, 255, 255, 255]), Some(([75, 87, 94, 255], 1.)), window, cx);
        if index > 0 {
            crate::charts::paint_primitive(
                &joysheet_chart::Prim::Wedge {
                    cx: 0.,
                    cy: 0.,
                    r_outer: 5.,
                    r_inner: 0.,
                    a0: 0.,
                    a1: f * std::f32::consts::TAU,
                    fill: [75, 87, 94, 255],
                    stroke: None,
                },
                origin,
                k,
                window,
                cx,
            );
        }
    } else if set.contains("Rating") {
        for i in 0..count {
            let h = 3. + i as f32 * 2.;
            crate::charts::paint_primitive(
                &joysheet_chart::Prim::Rect {
                    x: -6. + i as f32 * 3.,
                    y: 6. - h,
                    w: 2.,
                    h,
                    fill: Some(if i <= index { [63, 116, 150, 255] } else { [210, 219, 224, 255] }),
                    stroke: None,
                    radius: 0.,
                },
                origin,
                k,
                window,
                cx,
            );
        }
    } else if set.contains("Flags") {
        crate::charts::path(&[[-4., 7.], [-4., -6.]], false, None, Some(([92, 103, 111, 255], 1.)), origin, k, window);
        path(&[[-3., -6.], [6., -6.], [3., -2.], [6., 1.], [-3., 1.]], Some(ink), None, window);
    } else if set.contains("Symbols") {
        if set.ends_with('1') {
            circle(Some(ink), None, window, cx);
        }
        let symbol_ink = if set.ends_with('1') { [255, 255, 255, 255] } else { ink };
        let pts = if index == 0 {
            vec![[-3., -3.], [3., 3.], [0., 0.], [-3., 3.], [3., -3.]]
        } else if index == 1 {
            vec![[0., -4.], [0., 1.], [0., 4.], [0., 5.]]
        } else {
            vec![[-4., 0.], [-1., 3.], [4., -4.]]
        };
        crate::charts::path(&pts, false, None, Some((symbol_ink, 1.8)), origin, k, window);
    } else if set.contains("Signs") {
        match index {
            0 => path(&[[0., -6.], [6., 0.], [0., 6.], [-6., 0.]], Some(ink), None, window),
            1 => path(&[[0., -6.], [6., 5.], [-6., 5.]], Some(ink), None, window),
            _ => circle(Some(ink), None, window, cx),
        }
    } else if set.contains("Traffic") || set.contains("RedToBlack") {
        let ink = if set.contains("RedToBlack") {
            [((1. - f) * 210.) as u8, 30, 30, 255]
        } else if count == 4 && index == 0 {
            [55, 60, 65, 255]
        } else {
            ink
        };
        circle(Some(ink), if set.ends_with('2') { Some(([45, 55, 50, 255], 1.)) } else { None }, window, cx);
    } else {
        let angle = (1. - f) * std::f32::consts::PI;
        let (s, c) = angle.sin_cos();
        let pts = [[0., -7.], [6., -1.], [2., -1.], [2., 6.], [-2., 6.], [-2., -1.], [-6., -1.]].map(|[x, y]| [x * c - y * s, x * s + y * c]);
        path(&pts, Some(ink), None, window);
    }
}
