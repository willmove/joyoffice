use super::*;

fn m(c: &str, v: Value) -> bool {
    Criterion::parse(&Value::from(c)).matches(&v)
}

#[test]
fn criteria() {
    assert!(m(">5", Value::Number(6.0)));
    assert!(!m(">5", Value::Number(5.0)));
    assert!(m(">=5", Value::Number(5.0)));
    assert!(m("<>5", Value::from("x")));
    assert!(m("<>5", Value::Empty));
    assert!(!m(">5", Value::from("9")));
    assert!(m("5", Value::from("5")));
    assert!(m("a*", Value::from("Apple")));
    assert!(!m("a*", Value::from("banana")));
    assert!(m("?pple", Value::from("apple")));
    assert!(m("~*", Value::from("*")));
    assert!(!m("~*", Value::from("x")));
    assert!(m("", Value::Empty));
    assert!(m("", Value::from("")));
    assert!(!m("", Value::Number(0.0)));
    assert!(m("=", Value::Empty));
    assert!(!m("=", Value::from("a")));
    assert!(m("<>", Value::from("a")));
    assert!(!m("<>", Value::Empty));
    assert!(m("TRUE", Value::Bool(true)));
    // A bare "<=" or ">=" never matches an empty string (Excel 365: COUNTIF(="", "<=") is 0).
    assert!(!m("<=", Value::from("")));
    assert!(!m(">=", Value::from("")));
    assert!(m("<b", Value::from("apple")));
    assert!(!m("<b", Value::Number(1.0)));
    assert!(m("#N/A", Value::Error(CellError::NA)));
    assert!(Criterion::parse(&Value::Number(3.0)).matches(&Value::Number(3.0)));
    assert!(Criterion::parse(&Value::Number(0.3)).matches(&Value::Number(0.1 + 0.2)));
    assert!(!lookup_equal(&Value::Number(0.3), &Value::Number(0.1 + 0.2), false));
    assert!(lookup_equal(&Value::Number(0.0), &Value::Number(-0.0), false));
    assert!(m("<>a*", Value::from("bcd")));
    assert!(m(">1/1/2020", Value::Number(44000.0)));
    // Text patterns never match blank cells, but an empty string is text.
    assert!(!m("*", Value::Empty));
    assert!(m("*", Value::from("")));
    assert!(m("<>*", Value::Empty));
    assert!(!m("<>*", Value::from("")));
    assert!(!m("?*", Value::Empty));
    assert!(m("<>a*", Value::Empty));
}

#[test]
fn wildcards() {
    assert!(wildcard_match("*", ""));
    assert!(wildcard_match("a*c", "abbbc"));
    assert!(!wildcard_match("a*c", "abbbd"));
    assert!(wildcard_match("*~?", "what?"));
    assert_eq!(wildcard_find("b?d", "abcd abd", 0), Some(1));
    assert_eq!(wildcard_find("b?d", "abxxd bcd", 0), Some(6));
    assert_eq!(wildcard_find("c*", "abcd", 0), Some(2));
    assert_eq!(wildcard_find("z", "abcd", 0), None);
    assert_eq!(wildcard_find("b", "a\u{130}bc", 0), Some(2));
    assert!(wildcard_match("i*", "\u{130}stanbul"));
    assert!(has_wildcards("a*"));
    assert!(!has_wildcards("abc"));
}
