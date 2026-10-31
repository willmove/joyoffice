use crate::util::testutil::*;
use joysheet_core::{CellError, Value};

/// The classic orchard table.
fn db() -> Value {
    arr(vec![
        vec![tv("Tree"), tv("Height"), tv("Age"), tv("Yield"), tv("Profit")],
        vec![tv("Apple"), nv(18.0), nv(20.0), nv(14.0), nv(105.0)],
        vec![tv("Pear"), nv(12.0), nv(12.0), nv(10.0), nv(96.0)],
        vec![tv("Cherry"), nv(13.0), nv(14.0), nv(9.0), nv(105.0)],
        vec![tv("Apple"), nv(14.0), nv(15.0), nv(10.0), nv(75.0)],
        vec![tv("Pear"), nv(9.0), nv(8.0), nv(8.0), nv(76.8)],
        vec![tv("Apple"), nv(8.0), nv(9.0), nv(6.0), nv(45.0)],
    ])
}

/// Apple with height > 10, OR any pear.
fn crit() -> Value {
    arr(vec![
        vec![tv("Tree"), tv("Height"), tv("Age"), tv("Yield"), tv("Profit"), tv("Height")],
        vec![tv("=Apple"), tv(">10"), Value::Empty, Value::Empty, Value::Empty, tv("<16")],
        vec![tv("=Pear"), Value::Empty, Value::Empty, Value::Empty, Value::Empty, Value::Empty],
    ])
}

fn d(f: &str, field: crate::Arg) -> Value {
    ev(f, vec![rf(db()), field, rf(crit())])
}

#[test]
fn orchard() {
    close(d("DCOUNT", t("Age")), 3.0);
    close(d("DCOUNTA", t("Profit")), 3.0);
    close(d("DMAX", t("Profit")), 96.0);
    close(d("DMIN", t("Profit")), 75.0);
    close(d("DSUM", t("Profit")), 247.8);
    close(d("DSUM", n(5.0)), 247.8);
    close(d("DPRODUCT", t("Yield")), 800.0);
    close(d("DAVERAGE", t("Yield")), 28.0 / 3.0);
    close_tol(d("DSTDEV", t("Yield")), 1.154700538, 1e-9);
    close_tol(d("DSTDEVP", t("Yield")), 0.942809042, 1e-9);
    close_tol(d("DVAR", t("Yield")), 1.333333333, 1e-9);
    close_tol(d("DVARP", t("Yield")), 0.888888889, 1e-9);
    close(d("DCOUNT", empty()), 3.0);
    is_err(d("DSUM", t("Nope")), CellError::Value);
    is_err(d("DSUM", n(9.0)), CellError::Value);
}

#[test]
fn simple_criteria() {
    let c = |cells: Vec<Value>| arr(vec![vec![tv("Tree"), tv("Height")], cells]);
    close(ev("DSUM", vec![rf(db()), t("Profit"), rf(c(vec![tv("Apple"), Value::Empty]))]), 225.0);
    // Plain text matches values that begin with it.
    close(ev("DCOUNT", vec![rf(db()), t("Age"), rf(c(vec![tv("P"), Value::Empty]))]), 2.0);
    close(ev("DCOUNT", vec![rf(db()), t("Age"), rf(c(vec![tv("=P"), Value::Empty]))]), 0.0);
    close(ev("DSUM", vec![rf(db()), t("Yield"), rf(c(vec![Value::Empty, tv(">=13")]))]), 33.0);
    close(ev("DSUM", vec![rf(db()), t("Yield"), rf(c(vec![Value::Empty, nv(18.0)]))]), 14.0);
    close(ev("DSUM", vec![rf(db()), t("Yield"), rf(c(vec![Value::Empty, Value::Empty]))]), 57.0);
    close(ev("DSUM", vec![rf(db()), t("Yield"), rf(c(vec![tv("*e*"), tv("<10")]))]), 14.0);
    // Header only: every record matches.
    close(ev("DCOUNTA", vec![rf(db()), t("tree"), rf(arr(vec![vec![tv("Tree")]]))]), 6.0);
    // Unknown criteria label never matches.
    close(ev("DCOUNT", vec![rf(db()), t("Age"), rf(arr(vec![vec![tv("Color")], vec![tv("red")]]))]), 0.0);
}

#[test]
fn dget_and_empty() {
    let c = |tree: &str| arr(vec![vec![tv("Tree")], vec![tv(tree)]]);
    close(ev("DGET", vec![rf(db()), t("Yield"), rf(c("=Cherry"))]), 9.0);
    is_err(ev("DGET", vec![rf(db()), t("Yield"), rf(c("=Apple"))]), CellError::Num);
    is_err(ev("DGET", vec![rf(db()), t("Yield"), rf(c("=Plum"))]), CellError::Value);
    is_err(ev("DAVERAGE", vec![rf(db()), t("Yield"), rf(c("=Plum"))]), CellError::Div0);
    close(ev("DMAX", vec![rf(db()), t("Yield"), rf(c("=Plum"))]), 0.0);
    close(ev("DPRODUCT", vec![rf(db()), t("Yield"), rf(c("=Plum"))]), 0.0);
    is_err(ev("DSTDEV", vec![rf(db()), t("Yield"), rf(c("=Cherry"))]), CellError::Div0);
    close(ev("DSTDEVP", vec![rf(db()), t("Yield"), rf(c("=Cherry"))]), 0.0);
    is_err(ev("DSUM", vec![e(CellError::Ref), t("Yield"), rf(c("=Cherry"))]), CellError::Ref);
    close(ev("DCOUNTA", vec![rf(db()), empty(), rf(c("=Pear"))]), 2.0);
}

/// Expected results are Excel's, from Apache POI's `DGet.xls` (rows 74-86).
#[test]
fn empty_string_criterion() {
    let db = arr(vec![
        vec![tv("Index"), tv("Value")],
        vec![tv("a"), tv("=")],
        vec![tv("b"), Value::Empty],
        vec![tv("c"), tv("")],
        vec![tv("d"), tv(" ")],
        vec![tv("f"), Value::Error(CellError::Value)],
        vec![tv("h"), tv("abc")],
        vec![tv("i"), nv(0.0)],
    ]);
    let get = |index: &str| {
        let crit = arr(vec![vec![tv("Index"), tv("Value")], vec![tv(index), tv("")]]);
        ev("DGET", vec![rf(db.clone()), t("Index"), rf(crit)])
    };
    // Like any plain text, "" is a "begins with" pattern: it matches every text value.
    is_text(get("c"), "c");
    is_text(get("h"), "h");
    is_err(get("b"), CellError::Value);
    is_err(get("i"), CellError::Value);
    is_err(get("f"), CellError::Value);
    let all = arr(vec![vec![tv("Value")], vec![tv("")]]);
    close(ev("DCOUNTA", vec![rf(db.clone()), t("Index"), rf(all)]), 4.0);

    // Under a label that names no field, "" is a condition that never holds.
    let db = arr(vec![vec![tv("One"), tv("Two"), tv("Three")], vec![tv("a"), tv("c"), tv("x")], vec![tv("b"), tv("d"), tv("y")]]);
    let get = |label: Value, value: Value| {
        let crit = arr(vec![vec![tv("One"), label, tv("Three")], vec![tv("a"), value, tv("x")]]);
        ev("DGET", vec![rf(db.clone()), t("One"), rf(crit)])
    };
    is_text(get(Value::Empty, Value::Empty), "a");
    is_text(get(tv(""), Value::Empty), "a");
    is_err(get(Value::Empty, tv("")), CellError::Value);
    is_err(get(tv(""), tv("")), CellError::Value);
}

/// Expected results are Excel's, from Apache POI's `DGet.xls` (rows 108-119).
#[test]
fn dget_skips_empty_fields() {
    let db = arr(vec![
        vec![tv("One"), tv("Two")],
        vec![tv("a"), Value::Empty],
        vec![tv("b"), tv(" ")],
        vec![tv("c"), tv("")],
        vec![tv("d"), Value::Error(CellError::NA)],
        vec![tv("j"), Value::Empty],
        vec![tv("j"), tv("Text")],
        vec![tv("j"), Value::Empty],
        vec![tv("k"), Value::Empty],
        vec![tv("k"), tv("")],
    ]);
    let get = |one: &str| {
        let crit = arr(vec![vec![tv("One")], vec![tv(one)]]);
        ev("DGET", vec![rf(db.clone()), t("Two"), rf(crit)])
    };
    is_err(get("a"), CellError::Value);
    is_text(get("b"), " ");
    is_err(get("c"), CellError::Value);
    is_err(get("d"), CellError::NA);
    is_text(get("j"), "Text");
    is_err(get("k"), CellError::Value);
}
