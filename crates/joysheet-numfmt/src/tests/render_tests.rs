use super::*;

#[test]
fn approximations() {
    assert_eq!(approximate(0.5, 9), (1, 2));
    assert_eq!(approximate(0.333333, 9), (1, 3));
    assert_eq!(approximate(std::f64::consts::PI - 3.0, 99), (14, 99));
    assert_eq!(approximate(std::f64::consts::PI - 3.0, 9), (1, 7));
    assert_eq!(approximate(0.0, 9), (0, 1));
    assert_eq!(approximate(0.999, 9), (1, 1));
}
