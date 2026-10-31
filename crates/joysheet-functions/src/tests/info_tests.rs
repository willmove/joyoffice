use joysheet_core::{CellError, Value};

use crate::util::testutil::*;

#[test]
fn is_functions() {
    assert_eq!(ev("ISBLANK", vec![rf(Value::Empty)]), Value::Bool(true));
    assert_eq!(ev("ISBLANK", vec![t("")]), Value::Bool(false));
    assert_eq!(ev("ISERR", vec![e(CellError::NA)]), Value::Bool(false));
    assert_eq!(ev("ISERR", vec![e(CellError::Div0)]), Value::Bool(true));
    assert_eq!(ev("ISERROR", vec![e(CellError::NA)]), Value::Bool(true));
    assert_eq!(ev("ISERROR", vec![n(1.0)]), Value::Bool(false));
    assert_eq!(ev("ISNA", vec![e(CellError::NA)]), Value::Bool(true));
    assert_eq!(ev("ISNA", vec![e(CellError::Value)]), Value::Bool(false));
    assert_eq!(ev("ISNUMBER", vec![n(1.0)]), Value::Bool(true));
    assert_eq!(ev("ISNUMBER", vec![t("1")]), Value::Bool(false));
    assert_eq!(ev("ISTEXT", vec![t("a")]), Value::Bool(true));
    assert_eq!(ev("ISNONTEXT", vec![rf(Value::Empty)]), Value::Bool(true));
    assert_eq!(ev("ISNONTEXT", vec![t("a")]), Value::Bool(false));
    assert_eq!(ev("ISLOGICAL", vec![b(false)]), Value::Bool(true));
    assert_eq!(ev("ISLOGICAL", vec![n(0.0)]), Value::Bool(false));
    assert_eq!(ev("ISEVEN", vec![n(-2.5)]), Value::Bool(true));
    assert_eq!(ev("ISODD", vec![n(3.9)]), Value::Bool(true));
    assert_eq!(ev("ISODD", vec![n(-3.0)]), Value::Bool(true));
    is_err(ev("ISEVEN", vec![t("x")]), CellError::Value);
    is_err(ev("ISEVEN", vec![b(true)]), CellError::Value);
    assert_eq!(ev("ISEVEN", vec![rf(Value::Empty)]), Value::Bool(true));
    assert_eq!(rows_of(&ev("ISEVEN", vec![av(row(&[1.0, 2.0]))])), vec![vec![false.into(), true.into()]]);
    let range = arr(vec![vec![nv(1.0), Value::Empty], vec![tv("x"), Value::Empty]]);
    assert_eq!(rows_of(&ev("ISBLANK", vec![rf(range)])), vec![vec![false.into(), true.into()], vec![false.into(), true.into()]]);
}

#[test]
fn type_error_n() {
    close(ev("TYPE", vec![n(1.0)]), 1.0);
    close(ev("TYPE", vec![t("a")]), 2.0);
    close(ev("TYPE", vec![b(true)]), 4.0);
    close(ev("TYPE", vec![e(CellError::Ref)]), 16.0);
    close(ev("TYPE", vec![av(row(&[1.0, 2.0]))]), 64.0);
    close(ev("TYPE", vec![rf(arr(vec![vec![tv("x")]]))]), 2.0);
    close(ev("TYPE", vec![rf(Value::Empty)]), 1.0);
    close(ev("ERROR.TYPE", vec![e(CellError::Div0)]), 2.0);
    close(ev("ERROR.TYPE", vec![e(CellError::NA)]), 7.0);
    is_err(ev("ERROR.TYPE", vec![n(1.0)]), CellError::NA);
    is_err(ev("NA", vec![]), CellError::NA);
    close(ev("N", vec![n(7.0)]), 7.0);
    close(ev("N", vec![b(true)]), 1.0);
    close(ev("N", vec![t("7")]), 0.0);
    is_err(ev("N", vec![e(CellError::Num)]), CellError::Num);
    close(ev("N", vec![rf(row(&[3.0, 4.0]))]), 3.0);
    let lifted = ev("N", vec![av(arr(vec![vec![Value::Bool(true), tv("x")], vec![Value::Error(CellError::Div0), nv(4.0)]]))]);
    assert_eq!(rows_of(&lifted), vec![vec![nv(1.0), nv(0.0)], vec![Value::Error(CellError::Div0), nv(4.0)]]);
    is_text(ev("INFO", vec![t("RECALC")]), "Automatic");
    is_err(ev("INFO", vec![t("bogus")]), CellError::Value);
}

#[test]
fn logical() {
    assert_eq!(ev("AND", vec![b(true), n(1.0)]), Value::Bool(true));
    assert_eq!(ev("AND", vec![b(true), n(0.0)]), Value::Bool(false));
    assert_eq!(ev("AND", vec![rf(arr(vec![vec![b(true).value, tv("x"), Value::Empty]]))]), Value::Bool(true));
    is_err(ev("AND", vec![rf(arr(vec![vec![tv("x")]]))]), CellError::Value);
    is_err(ev("AND", vec![t("abc")]), CellError::Value);
    assert_eq!(ev("AND", vec![t("TRUE")]), Value::Bool(true));
    is_err(ev("OR", vec![b(false), e(CellError::Div0)]), CellError::Div0);
    assert_eq!(ev("OR", vec![b(false), n(2.0)]), Value::Bool(true));
    assert_eq!(ev("OR", vec![b(false), b(false)]), Value::Bool(false));
    assert_eq!(ev("XOR", vec![b(true), b(true)]), Value::Bool(false));
    assert_eq!(ev("XOR", vec![b(true), b(true), b(true)]), Value::Bool(true));
    assert_eq!(ev("NOT", vec![b(false)]), Value::Bool(true));
    assert_eq!(ev("NOT", vec![n(5.0)]), Value::Bool(false));
    is_err(ev("NOT", vec![t("x")]), CellError::Value);
    assert_eq!(ev("TRUE", vec![]), Value::Bool(true));
    assert_eq!(ev("FALSE", vec![]), Value::Bool(false));
}

#[test]
fn conditionals() {
    is_text(ev("IF", vec![b(true), t("y"), t("n")]), "y");
    is_text(ev("IF", vec![n(0.0), t("y"), t("n")]), "n");
    assert_eq!(ev("IF", vec![b(false), t("y")]), Value::Bool(false));
    close(ev("IF", vec![b(true), empty()]), 0.0);
    is_err(ev("IF", vec![e(CellError::Ref), n(1.0), n(2.0)]), CellError::Ref);
    let r = ev("IF", vec![av(arr(vec![vec![b(true).value, b(false).value]])), av(row(&[1.0, 2.0])), av(row(&[3.0, 4.0]))]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(4.0)]]);
    // Scalar condition returns a whole array value.
    assert_eq!(rows_of(&ev("IF", vec![b(true), av(row(&[1.0, 2.0])), n(0.0)])), vec![vec![nv(1.0), nv(2.0)]]);
    is_text(ev("IFS", vec![b(false), t("a"), b(true), t("b")]), "b");
    is_err(ev("IFS", vec![b(false), t("a")]), CellError::NA);
    close(ev("IFERROR", vec![e(CellError::Div0), n(9.0)]), 9.0);
    close(ev("IFERROR", vec![n(3.0), n(9.0)]), 3.0);
    let r = ev("IFERROR", vec![av(arr(vec![vec![nv(1.0), Value::Error(CellError::Div0)]])), n(0.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(0.0)]]);
    close(ev("IFNA", vec![e(CellError::NA), n(1.0)]), 1.0);
    is_err(ev("IFNA", vec![e(CellError::Value), n(1.0)]), CellError::Value);
    is_text(ev("SWITCH", vec![n(2.0), n(1.0), t("one"), n(2.0), t("two")]), "two");
    is_text(ev("SWITCH", vec![n(3.0), n(1.0), t("one"), t("other")]), "other");
    is_err(ev("SWITCH", vec![n(3.0), n(1.0), t("one")]), CellError::NA);
    is_text(ev("SWITCH", vec![t("B"), t("a"), t("x"), t("b"), t("y")]), "y");
    is_text(ev("CHOOSE", vec![n(2.0), t("a"), t("b"), t("c")]), "b");
    is_err(ev("CHOOSE", vec![n(4.0), t("a"), t("b"), t("c")]), CellError::Value);
    is_err(ev("CHOOSE", vec![n(0.0), t("a")]), CellError::Value);
    let r = ev("CHOOSE", vec![av(row(&[1.0, 2.0])), av(row(&[10.0, 20.0])), av(row(&[30.0, 40.0]))]);
    assert_eq!(rows_of(&r), vec![vec![nv(10.0), nv(40.0)]]);
}

#[test]
fn web_and_cube() {
    is_text(ev("HYPERLINK", vec![t("http://x.com")]), "http://x.com");
    is_text(ev("HYPERLINK", vec![t("http://x.com"), t("Click")]), "Click");
    close(ev("HYPERLINK", vec![t("http://x.com"), n(5.0)]), 5.0);
    is_text(ev("ENCODEURL", vec![t("a b&c/é")]), "a%20b%26c%2F%C3%A9");
    is_err(ev("WEBSERVICE", vec![t("http://x")]), CellError::Value);
    is_err(ev("FILTERXML", vec![t("<a/>"), t("//a")]), CellError::Value);
    is_err(ev("CUBEVALUE", vec![t("conn")]), CellError::NA);
    is_err(ev("CUBESETCOUNT", vec![t("set")]), CellError::NA);
}
