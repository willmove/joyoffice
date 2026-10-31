use super::*;
#[test]
fn phase_survives_polyline_corners() {
    let result = dashed(&[[0., 0.], [3., 0.], [3., 7.]], 5., 3.);
    assert_eq!(result.first().unwrap().first().unwrap(), &[0., 0.]);
    assert!(result.iter().any(|s| s.last() == Some(&[3., 2.])));
}
