use super::*;

#[test]
fn zero_numbered_styles_do_not_underflow() {
    assert_eq!(parse_style_name("TableStyleLight0"), ("Light", 1));
    assert_eq!(parse_style_name("TableStyleMedium0"), ("Medium", 1));
    for name in ["TableStyleLight0", "TableStyleMedium0", "TableStyleDark0"] {
        let _ = look(name);
    }
}
