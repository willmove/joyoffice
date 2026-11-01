use super::*;
#[test]
fn holes_and_rotated_shapes_use_real_geometry() {
    let mut donut = object("donut").unwrap();
    donut.width = 100.;
    donut.height = 100.;
    assert!(!hit(&donut, 50., 50.));
    assert!(hit(&donut, 10., 50.));
    let mut triangle = object("triangle").unwrap();
    triangle.width = 100.;
    triangle.height = 100.;
    triangle.rotation = 90.;
    triangle.x = 0.;
    triangle.y = 0.;
    assert!(!crate::geometry::hit(&triangle, 99., 1.));
    assert!(crate::geometry::hit(&triangle, 50., 50.));
}
#[test]
fn unsupported_and_hostile_parameters_are_rejected() {
    assert!(object("unknown-shape").is_err());
    let mut o = object("triangle").unwrap();
    o.preset.as_mut().unwrap().adjustments = vec![f64::NAN];
    assert!(validate(&o).is_err());
    o.preset.as_mut().unwrap().adjustments = vec![1., 2.];
    assert!(validate(&o).is_err());
    o.preset = None;
    assert!(validate(&o).is_err());
}
