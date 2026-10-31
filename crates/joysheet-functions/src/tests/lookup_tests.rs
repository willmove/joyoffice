use joysheet_core::{CellError, Value};

use crate::util::testutil::*;

fn table() -> Value {
    arr(vec![
        vec![nv(1.0), tv("one"), nv(10.0)],
        vec![nv(2.0), tv("two"), nv(20.0)],
        vec![nv(3.0), tv("three"), nv(30.0)],
        vec![nv(5.0), tv("five"), Value::Empty],
    ])
}

fn names() -> Value {
    arr(vec![vec![tv("Apple"), nv(1.0)], vec![tv("Banana"), nv(2.0)], vec![tv("Cherry"), nv(3.0)]])
}

#[test]
fn vlookup() {
    is_text(ev("VLOOKUP", vec![n(2.0), rf(table()), n(2.0)]), "two");
    is_text(ev("VLOOKUP", vec![n(4.0), rf(table()), n(2.0)]), "three");
    is_text(ev("VLOOKUP", vec![n(4.0), rf(table()), n(2.0), b(true)]), "three");
    is_text(ev("VLOOKUP", vec![n(99.0), rf(table()), n(2.0)]), "five");
    is_err(ev("VLOOKUP", vec![n(0.5), rf(table()), n(2.0)]), CellError::NA);
    is_err(ev("VLOOKUP", vec![n(4.0), rf(table()), n(2.0), b(false)]), CellError::NA);
    is_err(ev("VLOOKUP", vec![n(4.0), rf(table()), n(2.0), empty()]), CellError::NA);
    close(ev("VLOOKUP", vec![n(3.0), rf(table()), n(3.0), b(false)]), 30.0);
    close(ev("VLOOKUP", vec![n(5.0), rf(table()), n(3.0), b(false)]), 0.0);
    is_err(ev("VLOOKUP", vec![n(3.0), rf(table()), n(4.0)]), CellError::Ref);
    is_err(ev("VLOOKUP", vec![n(3.0), rf(table()), n(0.0)]), CellError::Value);
    close(ev("VLOOKUP", vec![t("b*"), rf(names()), n(2.0), b(false)]), 2.0);
    close(ev("VLOOKUP", vec![t("cherry"), rf(names()), n(2.0), b(false)]), 3.0);
    close(ev("VLOOKUP", vec![t("?pple"), rf(names()), n(2.0), b(false)]), 1.0);
    close(ev("VLOOKUP", vec![t("Bz"), rf(names()), n(2.0)]), 2.0);
    is_err(ev("VLOOKUP", vec![e(CellError::Div0), rf(names()), n(2.0)]), CellError::Div0);
    // Lifting over the lookup value.
    let r = ev("VLOOKUP", vec![av(col(&[1.0, 3.0])), rf(table()), n(3.0), b(false)]);
    assert_eq!(rows_of(&r), vec![vec![nv(10.0)], vec![nv(30.0)]]);
}

#[test]
fn hlookup() {
    let t2 = arr(vec![vec![nv(1.0), nv(2.0), nv(3.0)], vec![tv("a"), tv("b"), tv("c")]]);
    is_text(ev("HLOOKUP", vec![n(2.0), rf(t2.clone()), n(2.0), b(false)]), "b");
    is_text(ev("HLOOKUP", vec![n(2.5), rf(t2.clone()), n(2.0)]), "b");
    is_err(ev("HLOOKUP", vec![n(2.0), rf(t2), n(3.0)]), CellError::Ref);
}

#[test]
fn lookup_fn() {
    close(ev("LOOKUP", vec![n(4.0), av(row(&[1.0, 3.0, 5.0])), av(row(&[10.0, 30.0, 50.0]))]), 30.0);
    close(ev("LOOKUP", vec![n(5.0), av(row(&[1.0, 3.0, 5.0])), av(col(&[10.0, 30.0, 50.0]))]), 50.0);
    is_err(ev("LOOKUP", vec![n(0.0), av(row(&[1.0, 3.0, 5.0]))]), CellError::NA);
    close(ev("LOOKUP", vec![n(2.5), rf(table())]), 20.0);
    let wide = arr(vec![vec![nv(1.0), nv(2.0), nv(3.0)], vec![tv("a"), tv("b"), tv("c")]]);
    is_text(ev("LOOKUP", vec![n(2.0), av(wide)]), "b");
    // Text mixed in is skipped.
    let mixed = arr(vec![vec![nv(1.0), tv("x"), nv(3.0), tv("y")]]);
    close(ev("LOOKUP", vec![n(9.0), av(mixed.clone()), av(row(&[1.0, 2.0, 3.0, 4.0]))]), 3.0);
    close(ev("LOOKUP", vec![t("zz"), av(mixed), av(row(&[1.0, 2.0, 3.0, 4.0]))]), 4.0);
}

/// Approximate matches over unsorted, mixed-type data as observed in Excel
/// (FormulaEvalTestData rows "Vlookup", "Hlookup", "Lookup"): Excel bisects the whole
/// vector and stops at an equal entry, so answers differ from a "best entry" scan.
#[test]
fn approximate_match_follows_bisection() {
    // B6:H12 of the test data: header row, then one row per data kind.
    let data = arr(vec![
        vec![tv("INTEGRAL"), tv("DOUBLE"), tv("BLANK"), tv("STRING"), tv("REF"), tv("AREA (rows)"), Value::Empty],
        vec![nv(0.0), nv(0.0), Value::Empty, tv(""), nv(0.0), nv(1.0), nv(2.0)],
        vec![nv(1.0), nv(0.00001), Value::Empty, tv("nospaces"), nv(1.0), nv(1.001), nv(2.999)],
        vec![nv(2.0), nv(1.1), Value::Empty, tv("sp ace s"), nv(9999999999.0), nv(2.3456), nv(3.4567)],
        vec![nv(534.0), nv(1.00001), Value::Empty, tv("endsWithSpace "), nv(0.00001), tv("alphanum"), nv(3.0)],
        vec![nv(9999999999.0), nv(2.99999), Value::Empty, tv(" beginsWithSpace"), nv(0.0), tv("blanksIncl"), Value::Empty],
        vec![nv(-9999999999.0), nv(999999.99999), Value::Empty, tv("non-numeric"), tv("sp ace s"), tv("E8"), nv(545.0)],
    ]);
    close(ev("VLOOKUP", vec![n(534.0), rf(data.clone()), n(2.0)]), 1.00001);
    is_text(ev("VLOOKUP", vec![n(534.0), rf(data.clone()), n(4.0)]), "endsWithSpace ");
    is_text(ev("VLOOKUP", vec![n(534.0), rf(data.clone()), n(4.0), b(true)]), "endsWithSpace ");
    is_text(ev("HLOOKUP", vec![t("STRING"), rf(data.clone()), n(6.0), b(true)]), " beginsWithSpace");
    let keys = arr(vec![vec![tv("DOUBLE"), tv("BLANK"), tv("STRING"), tv("REF"), tv("AREA (rows)")]]);
    let results = arr(vec![vec![nv(1.1), Value::Empty, tv("sp ace s"), nv(9999999999.0), nv(2.3456)]]);
    close(ev("LOOKUP", vec![t("DOUBLE"), rf(keys), rf(results)]), 1.1);
    let b6_b15 = col(&[0.0, 1.0, 2.0, 534.0, 9999999999.0, -9999999999.0, -534.0, -2.0, -1.0]);
    let mut keys = vec![tv("INTEGRAL")];
    keys.extend(rows_of(&b6_b15).into_iter().flatten());
    let words = ["STRING", "", "nospaces", "sp ace s", "endsWithSpace ", " beginsWithSpace", "non-numeric", "0", "0.000001", "1"];
    let e_col = arr(words.iter().map(|w| vec![tv(w)]).collect());
    is_text(ev("LOOKUP", vec![n(534.0), rf(arr(keys.into_iter().map(|k| vec![k]).collect())), rf(e_col)]), "endsWithSpace ");
    // Sorted duplicates give the last one; descending data with MATCH -1 too.
    close(ev("MATCH", vec![n(2.0), av(row(&[1.0, 2.0, 2.0, 2.0, 3.0])), n(1.0)]), 4.0);
    close(ev("MATCH", vec![n(20.0), av(row(&[40.0, 20.0, 20.0, 10.0])), n(-1.0)]), 3.0);
    // Only text and blanks: no number to find.
    is_err(ev("MATCH", vec![n(5.0), av(arr(vec![vec![tv("a"), Value::Empty, tv("b")]]))]), CellError::NA);
    // A long run of trailing blanks (whole-column lookups) still finds the last number.
    let mut tall = vec![vec![nv(1.0)], vec![nv(2.0)], vec![nv(3.0)]];
    tall.extend((0..10_000).map(|_| vec![Value::Empty]));
    close(ev("MATCH", vec![n(9.0), rf(arr(tall))]), 3.0);
}

#[test]
fn match_fn() {
    let v = col(&[10.0, 20.0, 30.0, 40.0]);
    close(ev("MATCH", vec![n(30.0), rf(v.clone()), n(0.0)]), 3.0);
    close(ev("MATCH", vec![n(35.0), rf(v.clone())]), 3.0);
    close(ev("MATCH", vec![n(35.0), rf(v.clone()), n(1.0)]), 3.0);
    close(ev("MATCH", vec![n(100.0), rf(v.clone())]), 4.0);
    is_err(ev("MATCH", vec![n(5.0), rf(v.clone())]), CellError::NA);
    is_err(ev("MATCH", vec![n(35.0), rf(v), n(0.0)]), CellError::NA);
    let d = col(&[40.0, 30.0, 20.0, 10.0]);
    close(ev("MATCH", vec![n(25.0), rf(d.clone()), n(-1.0)]), 2.0);
    close(ev("MATCH", vec![n(40.0), rf(d.clone()), n(-1.0)]), 1.0);
    is_err(ev("MATCH", vec![n(50.0), rf(d), n(-1.0)]), CellError::NA);
    let s = arr(vec![vec![tv("apple"), tv("banana"), tv("cherry")]]);
    close(ev("MATCH", vec![t("B*"), av(s.clone()), n(0.0)]), 2.0);
    close(ev("MATCH", vec![t("CHERRY"), av(s.clone()), n(0.0)]), 3.0);
    close(ev("MATCH", vec![t("c"), av(s.clone())]), 2.0);
    is_err(ev("MATCH", vec![n(1.0), rf(table())]), CellError::NA);
    close(ev("MATCH", vec![b(true), av(arr(vec![vec![nv(1.0), Value::Bool(true)]])), n(0.0)]), 2.0);
}

#[test]
fn xlookup() {
    let keys = col(&[1.0, 2.0, 3.0, 5.0]);
    let vals = col(&[10.0, 20.0, 30.0, 50.0]);
    close(ev("XLOOKUP", vec![n(3.0), rf(keys.clone()), rf(vals.clone())]), 30.0);
    is_err(ev("XLOOKUP", vec![n(4.0), rf(keys.clone()), rf(vals.clone())]), CellError::NA);
    is_text(ev("XLOOKUP", vec![n(4.0), rf(keys.clone()), rf(vals.clone()), t("none")]), "none");
    close(ev("XLOOKUP", vec![n(4.0), rf(keys.clone()), rf(vals.clone()), empty(), n(-1.0)]), 30.0);
    close(ev("XLOOKUP", vec![n(4.0), rf(keys.clone()), rf(vals.clone()), empty(), n(1.0)]), 50.0);
    is_err(ev("XLOOKUP", vec![n(6.0), rf(keys.clone()), rf(vals.clone()), empty(), n(1.0)]), CellError::NA);
    close(ev("XLOOKUP", vec![n(4.0), rf(keys.clone()), rf(vals.clone()), empty(), n(-1.0), n(2.0)]), 30.0);
    close(ev("XLOOKUP", vec![n(4.0), rf(keys.clone()), rf(vals.clone()), empty(), n(1.0), n(2.0)]), 50.0);
    close(ev("XLOOKUP", vec![n(5.0), rf(keys.clone()), rf(vals.clone()), empty(), n(0.0), n(2.0)]), 50.0);
    let dup = col(&[1.0, 2.0, 1.0]);
    let dv = col(&[10.0, 20.0, 30.0]);
    close(ev("XLOOKUP", vec![n(1.0), rf(dup.clone()), rf(dv.clone())]), 10.0);
    close(ev("XLOOKUP", vec![n(1.0), rf(dup), rf(dv), empty(), n(0.0), n(-1.0)]), 30.0);
    // Descending binary search.
    let desc = col(&[50.0, 30.0, 20.0, 10.0]);
    close(ev("XLOOKUP", vec![n(25.0), rf(desc.clone()), rf(col(&[1.0, 2.0, 3.0, 4.0])), empty(), n(-1.0), n(-2.0)]), 3.0);
    close(ev("XLOOKUP", vec![n(25.0), rf(desc), rf(col(&[1.0, 2.0, 3.0, 4.0])), empty(), n(1.0), n(-2.0)]), 2.0);
    // Wildcards only in mode 2.
    let names_col = arr(vec![vec![tv("Apple")], vec![tv("Banana")]]);
    is_err(ev("XLOOKUP", vec![t("b*"), rf(names_col.clone()), rf(col(&[1.0, 2.0]))]), CellError::NA);
    close(ev("XLOOKUP", vec![t("b*"), rf(names_col), rf(col(&[1.0, 2.0])), empty(), n(2.0)]), 2.0);
    // Returning a whole row.
    let r = ev("XLOOKUP", vec![n(2.0), rf(col(&[1.0, 2.0, 3.0])), rf(table_rows3())]);
    assert_eq!(rows_of(&r), vec![vec![nv(2.0), tv("two"), nv(20.0)]]);
    // Horizontal lookup returning a column.
    let hk = row(&[1.0, 2.0]);
    let hv = arr(vec![vec![tv("a"), tv("b")], vec![tv("c"), tv("d")]]);
    assert_eq!(rows_of(&ev("XLOOKUP", vec![n(2.0), av(hk.clone()), av(hv)])), vec![vec![tv("b")], vec![tv("d")]]);
    is_err(ev("XLOOKUP", vec![n(2.0), av(hk), av(col(&[1.0, 2.0, 3.0]))]), CellError::Value);
    is_err(ev("XLOOKUP", vec![n(2.0), rf(keys.clone()), rf(vals.clone()), empty(), n(3.0)]), CellError::Value);
    is_err(ev("XLOOKUP", vec![n(2.0), rf(table()), rf(vals)]), CellError::Value);
}

/// XLOOKUP binary search (search mode ±2). Observed in Excel (xlookup.xlsx, Sheet2): next
/// larger over ascending data with text mixed in (0.22) and over descending data (0.24); the
/// bisection compares across types in sort order (numbers < text) instead of skipping text.
/// The other modes follow from the same order.
#[test]
fn xlookup_binary_search_observed() {
    let rates = col(&[0.1, 0.22, 0.24, 0.32, 0.35, 0.37]);
    let incomes = arr(vec![vec![nv(9700.0)], vec![tv("xyz")], vec![tv("xyz")], vec![nv(160726.0)], vec![nv(204100.0)], vec![nv(510300.0)]]);
    close(ev("XLOOKUP", vec![n(46523.0), rf(incomes.clone()), rf(rates.clone()), n(0.0), n(1.0), n(2.0)]), 0.22);
    close(ev("XLOOKUP", vec![n(46523.0), rf(incomes), rf(rates), n(0.0), n(-1.0), n(2.0)]), 0.1);
    let desc_incomes = col(&[510300.0, 204100.0, 160726.0, 84200.0, 39475.0, 9700.0]);
    let desc_rates = col(&[0.37, 0.35, 0.32, 0.24, 0.22, 0.1]);
    close(ev("XLOOKUP", vec![n(46523.0), rf(desc_incomes.clone()), rf(desc_rates.clone()), n(0.0), n(1.0), n(-2.0)]), 0.24);
    close(ev("XLOOKUP", vec![n(46523.0), rf(desc_incomes.clone()), rf(desc_rates.clone()), n(0.0), n(-1.0), n(-2.0)]), 0.22);
    close(ev("XLOOKUP", vec![n(84200.0), rf(desc_incomes), rf(desc_rates), n(0.0), n(0.0), n(-2.0)]), 0.24);
}

fn table_rows3() -> Value {
    arr(vec![vec![nv(1.0), tv("one"), nv(10.0)], vec![nv(2.0), tv("two"), nv(20.0)], vec![nv(3.0), tv("three"), nv(30.0)]])
}

#[test]
fn xmatch() {
    let v = col(&[10.0, 20.0, 30.0]);
    close(ev("XMATCH", vec![n(20.0), rf(v.clone())]), 2.0);
    is_err(ev("XMATCH", vec![n(25.0), rf(v.clone())]), CellError::NA);
    close(ev("XMATCH", vec![n(25.0), rf(v.clone()), n(1.0)]), 3.0);
    close(ev("XMATCH", vec![n(25.0), rf(v.clone()), n(-1.0)]), 2.0);
    close(ev("XMATCH", vec![n(30.0), rf(v.clone()), n(0.0), n(2.0)]), 3.0);
    close(ev("XMATCH", vec![t("a?c"), av(arr(vec![vec![tv("xyz"), tv("abc")]])), n(2.0)]), 2.0);
    let r = ev("XMATCH", vec![av(row(&[30.0, 10.0])), rf(v)]);
    assert_eq!(rows_of(&r), vec![vec![nv(3.0), nv(1.0)]]);
}

/// Lookups compare numbers exactly, unlike `=` (15 significant digits). Observed in Excel 365.
#[test]
fn lookups_compare_numbers_exactly() {
    let e = 0.1 + 0.2;
    let one = || rf(col(&[e]));
    // Exact modes miss a key one bit away.
    is_err(ev("MATCH", vec![n(0.3), one(), n(0.0)]), CellError::NA);
    is_err(ev("VLOOKUP", vec![n(0.3), one(), n(1.0), b(false)]), CellError::NA);
    is_err(ev("XMATCH", vec![n(0.3), one()]), CellError::NA);
    close(ev("MATCH", vec![n(0.3), rf(col(&[e, 0.3])), n(0.0)]), 2.0);
    close(ev("MATCH", vec![n(e), rf(col(&[e, 0.3])), n(0.0)]), 1.0);
    is_err(ev("MATCH", vec![n(100.0), rf(col(&[100.0 + 1e-14])), n(0.0)]), CellError::NA);
    // 0 and -0 are still the same key.
    close(ev("MATCH", vec![n(-0.0), rf(col(&[0.0])), n(0.0)]), 1.0);
    // Approximate modes: 0.3 sorts below 0.30000000000000004.
    is_err(ev("MATCH", vec![n(0.3), one(), n(1.0)]), CellError::NA);
    close(ev("MATCH", vec![n(0.3), one(), n(-1.0)]), 1.0);
    is_err(ev("LOOKUP", vec![n(0.3), one()]), CellError::NA);
    is_err(ev("XMATCH", vec![n(0.3), one(), n(-1.0)]), CellError::NA);
    close(ev("XMATCH", vec![n(0.3), one(), n(1.0)]), 1.0);
    // The bisection stops on 0.3 and doesn't run on into the next entry.
    let list = || rf(col(&[0.1, 0.2, 0.3, e, 0.4]));
    close(ev("MATCH", vec![n(0.3), list(), n(1.0)]), 3.0);
    close(ev("MATCH", vec![n(e), list(), n(1.0)]), 4.0);
    close(ev("XMATCH", vec![n(0.3), list(), n(0.0), n(2.0)]), 3.0);
    close(ev("XMATCH", vec![n(e), list(), n(0.0), n(2.0)]), 4.0);
    let asc = || rf(col(&[0.1, e, 0.5]));
    let desc = || rf(col(&[0.5, e, 0.1]));
    close(ev("MATCH", vec![n(0.3), asc(), n(1.0)]), 1.0);
    close(ev("MATCH", vec![n(0.3), desc(), n(-1.0)]), 2.0);
    is_err(ev("XMATCH", vec![n(0.3), asc(), n(0.0), n(2.0)]), CellError::NA);
    close(ev("XMATCH", vec![n(0.3), asc(), n(-1.0), n(2.0)]), 1.0);
    close(ev("XMATCH", vec![n(0.3), asc(), n(1.0), n(2.0)]), 2.0);
    close(ev("XMATCH", vec![n(0.3), desc(), n(-1.0), n(-2.0)]), 3.0);
    close(ev("XMATCH", vec![n(0.3), asc(), n(-1.0)]), 1.0);
    close(ev("XMATCH", vec![n(0.3), asc(), n(1.0)]), 2.0);
}

#[test]
fn index() {
    let t3 = table_rows3();
    is_text(ev("INDEX", vec![rf(t3.clone()), n(2.0), n(2.0)]), "two");
    close(ev("INDEX", vec![rf(t3.clone()), n(3.0), n(3.0)]), 30.0);
    assert_eq!(rows_of(&ev("INDEX", vec![rf(t3.clone()), n(0.0), n(3.0)])), vec![vec![nv(10.0)], vec![nv(20.0)], vec![nv(30.0)]]);
    assert_eq!(rows_of(&ev("INDEX", vec![rf(t3.clone()), n(1.0), n(0.0)])), vec![vec![nv(1.0), tv("one"), nv(10.0)]]);
    assert_eq!(rows_of(&ev("INDEX", vec![rf(t3.clone()), n(1.0)])), vec![vec![nv(1.0), tv("one"), nv(10.0)]]);
    is_err(ev("INDEX", vec![rf(t3.clone()), n(4.0), n(1.0)]), CellError::Ref);
    is_err(ev("INDEX", vec![rf(t3.clone()), n(1.0), n(4.0)]), CellError::Ref);
    is_err(ev("INDEX", vec![rf(t3), n(-1.0), n(1.0)]), CellError::Value);
    close(ev("INDEX", vec![av(row(&[5.0, 6.0, 7.0])), n(2.0)]), 6.0);
    close(ev("INDEX", vec![av(col(&[5.0, 6.0, 7.0])), n(3.0)]), 7.0);
    close(ev("INDEX", vec![n(9.0), n(1.0), n(1.0)]), 9.0);
    let r = ev("INDEX", vec![av(col(&[5.0, 6.0, 7.0])), av(row(&[3.0, 1.0]))]);
    assert_eq!(rows_of(&r), vec![vec![nv(7.0), nv(5.0)]]);
}

#[test]
fn shape_and_address() {
    close(ev("ROWS", vec![rf(table())]), 4.0);
    close(ev("COLUMNS", vec![rf(table())]), 3.0);
    close(ev("ROWS", vec![n(1.0)]), 1.0);
    let tr = ev("TRANSPOSE", vec![av(row(&[1.0, 2.0, 3.0]))]);
    assert_eq!(rows_of(&tr), vec![vec![nv(1.0)], vec![nv(2.0)], vec![nv(3.0)]]);
    is_text(ev("ADDRESS", vec![n(1.0), n(1.0)]), "$A$1");
    is_text(ev("ADDRESS", vec![n(2.0), n(3.0), n(2.0)]), "C$2");
    is_text(ev("ADDRESS", vec![n(2.0), n(3.0), n(3.0)]), "$C2");
    is_text(ev("ADDRESS", vec![n(2.0), n(28.0), n(4.0)]), "AB2");
    is_text(ev("ADDRESS", vec![n(2.0), n(3.0), n(1.0), b(false)]), "R2C3");
    is_text(ev("ADDRESS", vec![n(2.0), n(3.0), n(4.0), b(false)]), "R[2]C[3]");
    is_text(ev("ADDRESS", vec![n(1.0), n(1.0), n(1.0), b(true), t("Sheet1")]), "Sheet1!$A$1");
    is_text(ev("ADDRESS", vec![n(1.0), n(1.0), n(1.0), b(true), t("My Sheet")]), "'My Sheet'!$A$1");
    // Observed in Excel (FormulaEvalTestData) and Microsoft's ADDRESS examples; the last
    // case follows from the quoting rule for sheet names.
    let cases: [(f64, f64, f64, bool, &str, &str); 4] = [
        (2.0, 3.0, 3.0, true, "[Book1]Sheet1", "[Book1]Sheet1!$C2"),
        (2.0, 3.0, 1.0, false, "[Book1]Sheet1", "[Book1]Sheet1!R2C3"),
        (2.0, 3.0, 1.0, false, "EXCEL SHEET", "'EXCEL SHEET'!R2C3"),
        (1.0, 1.0, 1.0, true, "[My Book]Sheet1", "'[My Book]Sheet1'!$A$1"),
    ];
    for (r, c, abs, a1, sheet, want) in cases {
        is_text(ev("ADDRESS", vec![n(r), n(c), n(abs), b(a1), t(sheet)]), want);
    }
    // Sheet names that read as a cell or an R1C1 reference, or start with a dot, are quoted;
    // off-sheet cells and booleans are not (#1458).
    for (sheet, want) in
        [("A1", "'A1'!$A$1"), ("R1C1", "'R1C1'!$A$1"), ("RC", "'RC'!$A$1"), (".x", "'.x'!$A$1"), ("XFE1", "XFE1!$A$1"), ("TRUE", "TRUE!$A$1")]
    {
        is_text(ev("ADDRESS", vec![n(1.0), n(1.0), n(1.0), b(true), t(sheet)]), want);
    }
    is_text(ev("ADDRESS", vec![n(1048576.0), n(16384.0)]), "$XFD$1048576");
    is_err(ev("ADDRESS", vec![n(0.0), n(1.0)]), CellError::Value);
    is_err(ev("ADDRESS", vec![n(1.0), n(16385.0)]), CellError::Value);
    is_err(ev("ADDRESS", vec![n(1.0), n(1.0), n(5.0)]), CellError::Value);
}
