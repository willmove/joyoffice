use super::*;

#[test]
fn rounding() {
    assert_eq!(fixed(2.675, 2), ("2".into(), "68".into()));
    assert_eq!(fixed(1.005, 2), ("1".into(), "01".into()));
    assert_eq!(fixed(0.5, 0), ("1".into(), "".into()));
    assert_eq!(fixed(0.04, 1), ("".into(), "0".into()));
    assert_eq!(fixed(0.05, 1), ("".into(), "1".into()));
    assert_eq!(fixed(9.999, 2), ("10".into(), "00".into()));
    assert_eq!(fixed(0.0004, 3), ("".into(), "000".into()));
    assert_eq!(fixed(0.0005, 3), ("".into(), "001".into()));
    assert_eq!(fixed(1e20, 0).0, "100000000000000000000");
    assert_eq!(fixed(0.0, 2), ("".into(), "00".into()));
}

#[test]
fn general_fit() {
    assert_eq!(format_general_fit(1e11, 11).as_deref(), Some("1E+11"));
    assert_eq!(format_general_fit(12345678901.0, 11).as_deref(), Some("12345678901"));
    assert_eq!(format_general_fit(123456789012.0, 11).as_deref(), Some("1.23457E+11"));
    assert_eq!(format_general_fit(0.1 + 0.2, 11).as_deref(), Some("0.3"));
    assert_eq!(format_general_fit(1.0 / 3.0, 11).as_deref(), Some("0.333333333"));
    assert_eq!(format_general_fit(123456.789012, 11).as_deref(), Some("123456.789"));
    assert_eq!(format_general_fit(1e-10, 11).as_deref(), Some("1E-10"));
    assert_eq!(format_general_fit(0.0000123456789, 11).as_deref(), Some("1.23457E-05"));
    assert_eq!(format_general_fit(0.0001, 11).as_deref(), Some("0.0001"));
    assert_eq!(format_general_fit(-5.0, 11).as_deref(), Some("-5"));
    assert_eq!(format_general_fit(0.0, 11).as_deref(), Some("0"));
    assert_eq!(format_general_fit(1234.5678, 6).as_deref(), Some("1234.6"));
    assert_eq!(format_general_fit(1234.5678, 4).as_deref(), Some("1235"));
    assert_eq!(format_general_fit(123456.0, 5).as_deref(), Some("1E+05"));
    assert_eq!(format_general_fit(123456.0, 3), None);
    assert_eq!(format_general_fit(9.99999999999, 5).as_deref(), Some("10"));
    assert_eq!(format_general_fit(f64::NAN, 11), None);
    assert_eq!(format_general_fit(1.7976931348623157e308, 11).as_deref(), Some("1.7977E+308"));
}
