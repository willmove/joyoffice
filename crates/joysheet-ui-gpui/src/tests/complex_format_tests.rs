use super::*;
#[::core::prelude::v1::test]
fn far_lines_are_clipped_before_expansion() {
    let clip = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(100.)));
    let (lo, hi) = clipped([-1_000_000., 50.], [1_000_000., 50.], clip).unwrap();
    assert!(lo >= 0.5 && hi - lo < 0.001);
    assert!(clipped([0., -10.], [100., -10.], clip).is_none());
    assert_eq!(dash_pattern(BorderStyle::DashDotDot), &[6., 3., 1., 3., 1., 3.]);
}
