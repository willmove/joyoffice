use joysheet_core::{CellError, Value};

/// The pieces expand exactly as `regex::Captures::expand` does.
#[test]
fn template_expands_like_regex() {
    let re = regex::Regex::new(r"(?<y>\d+)-(\d+)").unwrap();
    let cap = re.captures("x 2026-10 y").unwrap();
    for rep in ["$1", "${1}", "$1a", "${1}a", "$y", "${y}!", "$$", "$$1", "a$", "$}", "${", "${}", "${y", "$0|$2|$9", "$-1", "€$1€", ""] {
        let mut want = String::new();
        cap.expand(rep, &mut want);
        let mut got = String::new();
        assert!(super::expand_capped(&cap, &super::template(rep), &mut got, usize::MAX).is_some());
        assert_eq!(got, want, "{rep:?}");
    }
}

use crate::util::testutil::*;

#[test]
fn bahttext() {
    let bt = |x: f64| ev("BAHTTEXT", vec![n(x)]);
    // Microsoft's example.
    is_text(bt(1234.0), "หนึ่งพันสองร้อยสามสิบสี่บาทถ้วน");
    // Tens, เอ็ด, satang, millions and beyond, sign and rounding to satang.
    is_text(bt(0.0), "ศูนย์บาทถ้วน");
    is_text(bt(21.25), "ยี่สิบเอ็ดบาทยี่สิบห้าสตางค์");
    is_text(bt(0.5), "ห้าสิบสตางค์");
    is_text(bt(101.0), "หนึ่งร้อยเอ็ดบาทถ้วน");
    is_text(bt(1_000_000.0), "หนึ่งล้านบาทถ้วน");
    is_text(bt(11_000_000.0), "สิบเอ็ดล้านบาทถ้วน");
    is_text(bt(2_000_005_000_000.0), "สองล้านห้าล้านบาทถ้วน");
    is_text(bt(1e12), "หนึ่งล้านล้านบาทถ้วน");
    is_text(bt(-15.5), "ลบสิบห้าบาทห้าสิบสตางค์");
    is_text(bt(1.999), "สองบาทถ้วน");
    is_text(bt(0.005), "หนึ่งสตางค์");
    is_text(bt(-0.001), "ศูนย์บาทถ้วน");
    // A final one is เอ็ด after higher digits in any group, millions included (#1234).
    is_text(bt(21_000_001.0), "ยี่สิบเอ็ดล้านเอ็ดบาทถ้วน");
    is_text(bt(1_000_001.0), "หนึ่งล้านเอ็ดบาทถ้วน");
    is_text(bt(1_000_001_000_000.0), "หนึ่งล้านเอ็ดล้านบาทถ้วน");
    is_text(bt(1001.0), "หนึ่งพันเอ็ดบาทถ้วน");
    is_text(bt(11.0), "สิบเอ็ดบาทถ้วน");
    is_text(bt(1.0), "หนึ่งบาทถ้วน");
    is_text(bt(0.01), "หนึ่งสตางค์");
    is_text(bt(0.21), "ยี่สิบเอ็ดสตางค์");
    is_text(bt(1250.5), "หนึ่งพันสองร้อยห้าสิบบาทห้าสิบสตางค์");
    is_text(bt(0.25), "ยี่สิบห้าสตางค์");
    is_err(ev("BAHTTEXT", vec![t("abc")]), CellError::Value);
    // Huge numbers give text (or #NUM! where rounding overflows), never a panic.
    assert!(matches!(bt(-1e300), Value::Text(_)));
    assert!(matches!(bt(f64::MAX), Value::Text(_) | Value::Error(CellError::Num)));
}

#[test]
fn concat_family() {
    is_text(ev("CONCATENATE", vec![t("a"), n(1.5), b(true)]), "a1.5TRUE");
    is_text(ev("CONCAT", vec![av(arr(vec![vec![tv("a"), tv("b")], vec![nv(1.0), Value::Empty]])), t("!")]), "ab1!");
    is_text(ev("TEXTJOIN", vec![t("-"), b(true), av(arr(vec![vec![tv("a"), tv(""), tv("b")]])), t("c")]), "a-b-c");
    is_text(ev("TEXTJOIN", vec![t("-"), b(false), av(arr(vec![vec![tv("a"), tv(""), tv("b")]]))]), "a--b");
    is_text(ev("TEXTJOIN", vec![av(row_t(&[",", ";"])), b(true), t("a"), t("b"), t("c")]), "a,b;c");
    is_err(ev("CONCAT", vec![t("a"), e(CellError::NA)]), CellError::NA);
    // Lifting: CONCATENATE over an array.
    assert_eq!(rows_of(&ev("CONCATENATE", vec![av(row_t(&["a", "b"])), t("x")])), vec![vec![tv("ax"), tv("bx")]]);
}

fn row_t(xs: &[&str]) -> Value {
    arr(vec![xs.iter().map(|x| tv(x)).collect()])
}

#[test]
fn slicing() {
    is_text(ev("LEFT", vec![t("Hello")]), "H");
    is_text(ev("LEFT", vec![t("Hello"), n(3.0)]), "Hel");
    is_text(ev("LEFT", vec![t("Hello"), n(99.0)]), "Hello");
    is_err(ev("LEFT", vec![t("Hello"), n(-1.0)]), CellError::Value);
    is_text(ev("RIGHT", vec![t("Hello"), n(2.0)]), "lo");
    is_text(ev("RIGHT", vec![t("héllo"), n(4.0)]), "éllo");
    is_text(ev("MID", vec![t("Hello"), n(2.0), n(3.0)]), "ell");
    is_text(ev("MID", vec![t("Hello"), n(10.0), n(3.0)]), "");
    is_err(ev("MID", vec![t("Hello"), n(0.0), n(3.0)]), CellError::Value);
    close(ev("LEN", vec![t("héllo")]), 5.0);
    close(ev("LEN", vec![n(123.45)]), 6.0);
    close(ev("LEN", vec![empty()]), 0.0);
    is_text(ev("LEFT", vec![n(12345.0), n(2.0)]), "12");
}

#[test]
fn case_and_cleanup() {
    is_text(ev("UPPER", vec![t("abc")]), "ABC");
    is_text(ev("LOWER", vec![t("ABC")]), "abc");
    is_text(ev("PROPER", vec![t("this is a TITLE")]), "This Is A Title");
    is_text(ev("PROPER", vec![t("2-way street's")]), "2-Way Street'S");
    is_text(ev("TRIM", vec![t("  a   b  c ")]), "a b c");
    is_text(ev("CLEAN", vec![t("a\u{7}b\nc")]), "abc");
}

#[test]
fn substitute_replace_rept() {
    is_text(ev("SUBSTITUTE", vec![t("a-b-c"), t("-"), t("+")]), "a+b+c");
    is_text(ev("SUBSTITUTE", vec![t("a-b-c"), t("-"), t("+"), n(2.0)]), "a-b+c");
    is_text(ev("SUBSTITUTE", vec![t("a-b-c"), t("-"), t("+"), n(5.0)]), "a-b-c");
    is_err(ev("SUBSTITUTE", vec![t("a"), t("a"), t("b"), n(0.0)]), CellError::Value);
    is_text(ev("SUBSTITUTE", vec![t("abc"), t(""), t("x")]), "abc");
    is_text(ev("REPLACE", vec![t("abcdef"), n(2.0), n(3.0), t("XY")]), "aXYef");
    is_text(ev("REPLACE", vec![t("abc"), n(10.0), n(1.0), t("Z")]), "abcZ");
    is_text(ev("REPT", vec![t("ab"), n(3.0)]), "ababab");
    is_text(ev("REPT", vec![t("ab"), n(0.0)]), "");
    is_err(ev("REPT", vec![t("ab"), n(-1.0)]), CellError::Value);
    is_err(ev("REPT", vec![t("ab"), n(20000.0)]), CellError::Value);
    is_err(ev("REPT", vec![t("ab"), n(1e300)]), CellError::Value);
}

#[test]
fn find_search() {
    close(ev("FIND", vec![t("b"), t("abcb")]), 2.0);
    close(ev("FIND", vec![t("b"), t("abcb"), n(3.0)]), 4.0);
    is_err(ev("FIND", vec![t("B"), t("abc")]), CellError::Value);
    close(ev("FIND", vec![t(""), t("abc")]), 1.0);
    is_err(ev("FIND", vec![t("a"), t("abc"), n(0.0)]), CellError::Value);
    close(ev("SEARCH", vec![t("B"), t("abc")]), 2.0);
    close(ev("SEARCH", vec![t("c*"), t("abcd")]), 3.0);
    close(ev("SEARCH", vec![t("?d"), t("abcd")]), 3.0);
    close(ev("SEARCH", vec![t("~*"), t("a*b")]), 2.0);
    is_err(ev("SEARCH", vec![t("z"), t("abc")]), CellError::Value);
    is_err(ev("SEARCH", vec![t("a"), t("abc"), n(5.0)]), CellError::Value);
    close(ev("SEARCH", vec![t("b"), t("\u{130}b")]), 2.0);
    close(ev("SEARCH", vec![t("B"), t("\u{130}b")]), 2.0);
    close(ev("SEARCH", vec![t("b?"), t("a\u{130}bc")]), 3.0);
    close(ev("SEARCH", vec![t("i"), t("a\u{130}bc")]), 2.0);
    is_err(ev("SEARCH", vec![t("b"), t("a\u{130}bc"), n(4.0)]), CellError::Value);
    for f in ["FIND", "SEARCH"] {
        close(ev(f, vec![t(""), t("abc"), n(3.0)]), 3.0);
        is_err(ev(f, vec![t(""), t("abc"), n(4.0)]), CellError::Value);
    }
    assert_eq!(ev("EXACT", vec![t("a"), t("A")]), Value::Bool(false));
    assert_eq!(ev("EXACT", vec![t("a"), t("a")]), Value::Bool(true));
}

#[test]
fn codes() {
    is_text(ev("CHAR", vec![n(65.0)]), "A");
    is_text(ev("CHAR", vec![n(128.0)]), "€");
    is_err(ev("CHAR", vec![n(0.0)]), CellError::Value);
    is_err(ev("CHAR", vec![n(256.0)]), CellError::Value);
    close(ev("CODE", vec![t("A")]), 65.0);
    close(ev("CODE", vec![t("€")]), 128.0);
    is_err(ev("CODE", vec![t("")]), CellError::Value);
    is_text(ev("UNICHAR", vec![n(9731.0)]), "☃");
    close(ev("UNICODE", vec![t("☃")]), 9731.0);
    is_err(ev("UNICHAR", vec![n(0.0)]), CellError::Value);
    is_err(ev("UNICHAR", vec![n(55296.0)]), CellError::NA);
}

/// VALUE cases observed in Excel (ValueFunctionOfBlank, FormulaEvalTestData); a blank cell
/// still reads as 0.
#[test]
fn value_observed() {
    let cases: Vec<(crate::Arg, Result<f64, CellError>)> = vec![
        (t(""), Err(CellError::Value)),
        (t("ABC"), Err(CellError::Value)),
        (t("0.33"), Ok(0.33)),
        (t("$1,000"), Ok(1000.0)),
        (rf(Value::Empty), Ok(0.0)),
    ];
    for (arg, want) in cases {
        match want {
            Ok(x) => close(ev("VALUE", vec![arg]), x),
            Err(err) => is_err(ev("VALUE", vec![arg]), err),
        }
    }
}

#[test]
fn numbers_from_text() {
    close(ev("VALUE", vec![t("1,234.5")]), 1234.5);
    close(ev("VALUE", vec![t("50%")]), 0.5);
    close(ev("VALUE", vec![t("$10")]), 10.0);
    close(ev("VALUE", vec![t("12:00")]), 0.5);
    close(ev("VALUE", vec![t("2020-01-01")]), 43831.0);
    is_err(ev("VALUE", vec![t("abc")]), CellError::Value);
    // Accounting negatives, as Excel 365 reads them.
    close(ev("VALUE", vec![t("$(5)")]), -5.0);
    close(ev("VALUE", vec![t("$ (1,000.50)")]), -1000.5);
    close(ev("VALUE", vec![t("(5)%")]), -0.05);
    for s in ["$(5)%", "$(5%)", "$5%", "(-5)", "((5))", "$(-5)", "(1/2)"] {
        is_err(ev("VALUE", vec![t(s)]), CellError::Value);
    }
    close(ev("NUMBERVALUE", vec![t("2.500,27"), t(","), t(".")]), 2500.27);
    close(ev("NUMBERVALUE", vec![t("3.5%")]), 0.035);
    close(ev("NUMBERVALUE", vec![t(" 1 000 ")]), 1000.0);
    close(ev("NUMBERVALUE", vec![t("")]), 0.0);
    is_err(ev("NUMBERVALUE", vec![t("1.2.3")]), CellError::Value);
    is_err(ev("NUMBERVALUE", vec![t("1.5"), t(","), t(" ")]), CellError::Value);
    is_err(ev("NUMBERVALUE", vec![t("1,5"), t("."), t(" ")]), CellError::Value);
    is_text(ev("FIXED", vec![n(1234.567), n(1.0)]), "1,234.6");
    is_text(ev("FIXED", vec![n(1234.567), n(-1.0)]), "1,230");
    is_text(ev("FIXED", vec![n(-1234.567), n(1.0), b(true)]), "-1234.6");
    is_text(ev("FIXED", vec![n(44.332)]), "44.33");
    is_text(ev("FIXED", vec![n(2.675), n(2.0)]), "2.68");
    is_text(ev("DOLLAR", vec![n(1234.567)]), "$1,234.57");
    is_text(ev("DOLLAR", vec![n(-1234.567), n(1.0)]), "($1,234.6)");
    is_text(ev("DOLLAR", vec![n(0.5), n(0.0)]), "$1");
    is_text(ev("USDOLLAR", vec![n(1.0)]), "$1.00");
    is_text(ev("USDOLLAR", vec![n(-1234.567), n(1.0)]), "($1,234.6)");
    is_text(ev("T", vec![t("x")]), "x");
    is_text(ev("T", vec![n(1.0)]), "");
}

#[test]
fn before_after() {
    is_text(ev("TEXTBEFORE", vec![t("Red riding hood"), t(" ")]), "Red");
    is_text(ev("TEXTBEFORE", vec![t("Red riding hood"), t(" "), n(2.0)]), "Red riding");
    is_text(ev("TEXTBEFORE", vec![t("Red riding hood"), t(" "), n(-1.0)]), "Red riding");
    is_text(ev("TEXTAFTER", vec![t("Red riding hood"), t(" ")]), "riding hood");
    is_text(ev("TEXTAFTER", vec![t("Red riding hood"), t(" "), n(-1.0)]), "hood");
    is_err(ev("TEXTAFTER", vec![t("abc"), t("x")]), CellError::NA);
    is_text(ev("TEXTAFTER", vec![t("abc"), t("x"), n(1.0), n(0.0), n(0.0), t("none")]), "none");
    is_text(ev("TEXTAFTER", vec![t("aXb"), t("x"), n(1.0), n(1.0)]), "b");
    is_err(ev("TEXTAFTER", vec![t("aXb"), t("x")]), CellError::NA);
    is_text(ev("TEXTBEFORE", vec![t("abc"), t("x"), n(1.0), n(0.0), n(1.0)]), "abc");
    is_text(ev("TEXTAFTER", vec![t("a-b+c"), av(row_t(&["+", "-"]))]), "b+c");
    is_err(ev("TEXTBEFORE", vec![t("abc"), t("b"), n(0.0)]), CellError::Value);
    is_text(ev("TEXTBEFORE", vec![t("abc"), t("")]), "");
    is_text(ev("TEXTAFTER", vec![t("abc"), t("")]), "abc");
    // Negative instances search from the end, so overlapping delimiters match differently.
    is_text(ev("TEXTBEFORE", vec![t("aaa"), t("aa"), n(-1.0)]), "a");
    is_text(ev("TEXTAFTER", vec![t("aaa"), t("aa"), n(-1.0)]), "");
    is_text(ev("TEXTBEFORE", vec![t("aaaa"), t("aa"), n(-2.0)]), "");
    is_text(ev("TEXTBEFORE", vec![t("AaA"), t("aa"), n(-1.0), n(1.0)]), "A");
    is_text(ev("TEXTAFTER", vec![t("aaa"), t("aa"), n(1.0)]), "a");
}

#[test]
fn split() {
    let v = ev("TEXTSPLIT", vec![t("a,b;c,d"), t(","), t(";")]);
    assert_eq!(rows_of(&v), vec![vec![tv("a"), tv("b")], vec![tv("c"), tv("d")]]);
    let v = ev("TEXTSPLIT", vec![t("a,b;c"), t(","), t(";")]);
    assert_eq!(rows_of(&v), vec![vec![tv("a"), tv("b")], vec![tv("c"), Value::Error(CellError::NA)]]);
    let v = ev("TEXTSPLIT", vec![t("a,,b"), t(","), empty(), b(true)]);
    assert_eq!(rows_of(&v), vec![vec![tv("a"), tv("b")]]);
    let v = ev("TEXTSPLIT", vec![t("a,,b"), t(",")]);
    assert_eq!(rows_of(&v), vec![vec![tv("a"), tv(""), tv("b")]]);
    let v = ev("TEXTSPLIT", vec![t("a b-c"), av(row_t(&[" ", "-"]))]);
    assert_eq!(rows_of(&v), vec![vec![tv("a"), tv("b"), tv("c")]]);
    let v = ev("TEXTSPLIT", vec![t("1;2"), empty(), t(";")]);
    assert_eq!(rows_of(&v), vec![vec![tv("1")], vec![tv("2")]]);
    let v = ev("TEXTSPLIT", vec![t("aXbxc"), t("x"), empty(), b(false), n(1.0)]);
    assert_eq!(rows_of(&v), vec![vec![tv("a"), tv("b"), tv("c")]]);
    let v = ev("TEXTSPLIT", vec![t("a,b;c"), t(","), t(";"), b(false), n(0.0), t("-")]);
    assert_eq!(rows_of(&v)[1], vec![tv("c"), tv("-")]);
}

#[test]
fn split_rejects_oversized_padded_grid() {
    // One row of 8000 columns followed by 8000 one-cell rows: the text is under the cell-text limit, the padded grid is not.
    let text = format!("{}{}", "a,".repeat(8000), "b;".repeat(8000));
    is_err(ev("TEXTSPLIT", vec![t(&text), t(","), t(";")]), CellError::Num);
}

#[test]
fn to_text() {
    is_text(ev("VALUETOTEXT", vec![t("a\"b")]), "a\"b");
    is_text(ev("VALUETOTEXT", vec![t("ab"), n(1.0)]), "\"ab\"");
    is_text(ev("VALUETOTEXT", vec![n(1.5), n(1.0)]), "1.5");
    is_text(ev("VALUETOTEXT", vec![e(CellError::Div0)]), "#DIV/0!");
    is_err(ev("VALUETOTEXT", vec![n(1.0), n(2.0)]), CellError::Value);
    let a = av(arr(vec![vec![nv(1.0), tv("a")], vec![Value::Bool(true), Value::Error(CellError::NA)]]));
    is_text(ev("ARRAYTOTEXT", vec![a.clone()]), "1, a, TRUE, #N/A");
    is_text(ev("ARRAYTOTEXT", vec![a, n(1.0)]), "{1,\"a\";TRUE,#N/A}");
}

#[test]
fn regex() {
    assert_eq!(ev("REGEXTEST", vec![t("abc123"), t("[0-9]+")]), Value::Bool(true));
    assert_eq!(ev("REGEXTEST", vec![t("ABC"), t("abc")]), Value::Bool(false));
    assert_eq!(ev("REGEXTEST", vec![t("ABC"), t("abc"), n(1.0)]), Value::Bool(true));
    is_err(ev("REGEXTEST", vec![t("a"), t("(")]), CellError::Value);
    is_text(ev("REGEXEXTRACT", vec![t("tel 555-1234 or 555-9876"), t("[0-9]{3}-[0-9]{4}")]), "555-1234");
    let v = ev("REGEXEXTRACT", vec![t("tel 555-1234 or 555-9876"), t("[0-9]{3}-[0-9]{4}"), n(1.0)]);
    assert_eq!(rows_of(&v), vec![vec![tv("555-1234")], vec![tv("555-9876")]]);
    let v = ev("REGEXEXTRACT", vec![t("John Smith"), t("(\\w+) (\\w+)"), n(2.0)]);
    assert_eq!(rows_of(&v), vec![vec![tv("John"), tv("Smith")]]);
    is_err(ev("REGEXEXTRACT", vec![t("abc"), t("z")]), CellError::NA);
    is_text(ev("REGEXREPLACE", vec![t("a1b22c"), t("[0-9]+"), t("#")]), "a#b#c");
    is_text(ev("REGEXREPLACE", vec![t("a1b22c"), t("[0-9]+"), t("#"), n(2.0)]), "a1b#c");
    is_text(ev("REGEXREPLACE", vec![t("a1b22c"), t("[0-9]+"), t("#"), n(-2.0)]), "a#b22c");
    is_text(ev("REGEXREPLACE", vec![t("John Smith"), t("(\\w+) (\\w+)"), t("$2, $1")]), "Smith, John");
    is_text(ev("REGEXREPLACE", vec![t("ab"), t("(a)"), t("$1x")]), "axb");
}
