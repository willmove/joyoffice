use super::*;
#[::core::prelude::v1::test]
fn far_lines_are_clipped_before_expansion() {
    let clip = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
    let (lo, hi) = clipped([-1_000_000., 50.], [1_000_000., 50.], clip).unwrap();
    assert!(lo >= 0.5 && hi - lo < 0.001);
    assert!(clipped([0., -10.], [100., -10.], clip).is_none());
    assert_eq!(dash_pattern(BorderStyle::DashDotDot), &[6., 3., 1., 3., 1., 3.]);
}

#[::core::prelude::v1::test]
fn adjacent_cells_share_one_border_at_every_zoom() {
    let line = BorderLine::thin();
    let borders = Borders { left: line, right: line, top: line, bottom: line, ..Default::default() };
    for zoom in [0.5, 1., 1.25, 1.75, 2., 3.] {
        let rect = |x, y| Bounds::new(point(px(x), px(y)), size(px(96. * zoom), px(24. * zoom)));
        let a = cell_border_segments(rect(40.25, 28.5), &borders);
        let right = cell_border_segments(rect(40.25 + 96. * zoom, 28.5), &borders);
        let below = cell_border_segments(rect(40.25, 28.5 + 24. * zoom), &borders);
        assert_eq!(a[2], right[0], "vertical shared edge at {zoom}");
        assert_eq!(a[3], below[1], "horizontal shared edge at {zoom}");
    }
}

#[::core::prelude::v1::test]
fn merged_cell_border_meets_each_adjacent_row() {
    let line = BorderLine::thin();
    let borders = Borders { left: line, right: line, top: line, bottom: line, ..Default::default() };
    let merged = cell_border_segments(Bounds::new(point(px(40.), px(28.)), size(px(96.), px(48.))), &borders);
    let top = cell_border_segments(Bounds::new(point(px(136.), px(28.)), size(px(96.), px(24.))), &borders);
    let bottom = cell_border_segments(Bounds::new(point(px(136.), px(52.)), size(px(96.), px(24.))), &borders);
    assert_eq!(merged[2].1, top[0].1);
    assert_eq!(merged[2].2, bottom[0].2);
    assert_eq!(top[0].2, bottom[0].1);
    assert_eq!(merged[1].2, top[1].1);
    assert_eq!(merged[3].2, bottom[3].1);
}

#[::core::prelude::v1::test]
fn cell_border_geometry_preserves_double_and_diagonal_styles() {
    let double = BorderLine { style: BorderStyle::Double, ..Default::default() };
    let dashed = BorderLine { style: BorderStyle::Dashed, ..Default::default() };
    let borders = Borders { bottom: double, diag_down: dashed, diag_up: double, ..Default::default() };
    let rect = Bounds::new(point(px(10.), px(20.)), size(px(80.), px(30.)));
    let segments = cell_border_segments(rect, &borders);
    assert_eq!(segments[3], (double, [10., 50.], [90., 50.]));
    assert_eq!(segments[4], (dashed, [10., 20.], [90., 50.]));
    assert_eq!(segments[5], (double, [10., 50.], [90., 20.]));
}
