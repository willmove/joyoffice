use super::*;
#[::core::prelude::v1::test]
fn rotations_preserve_anchor_and_cover_corners() {
    let b = rotated_box(-40., -10., 80., 20., std::f32::consts::FRAC_PI_2);
    assert!((b[0] + 10.).abs() < 0.001 && (b[1] + 40.).abs() < 0.001);
    assert!((b[2] - 20.).abs() < 0.001 && (b[3] - 80.).abs() < 0.001);
    assert_eq!(escape("<&\"中文"), "&lt;&amp;&quot;中文");
}
