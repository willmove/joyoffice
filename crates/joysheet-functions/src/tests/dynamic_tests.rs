use joysheet_core::{CellError, Value};

use crate::util::testutil::*;

fn grid() -> Value {
    arr(vec![vec![tv("b"), nv(2.0), nv(20.0)], vec![tv("a"), nv(3.0), nv(10.0)], vec![tv("c"), nv(1.0), nv(30.0)]])
}

#[test]
fn filter() {
    let inc = arr(vec![vec![Value::Bool(true)], vec![Value::Bool(false)], vec![Value::Bool(true)]]);
    let r = ev("FILTER", vec![rf(grid()), av(inc)]);
    assert_eq!(rows_of(&r), vec![vec![tv("b"), nv(2.0), nv(20.0)], vec![tv("c"), nv(1.0), nv(30.0)]]);
    let r = ev("FILTER", vec![rf(grid()), av(col(&[0.0, 1.0, 0.0]))]);
    assert_eq!(rows_of(&r), vec![vec![tv("a"), nv(3.0), nv(10.0)]]);
    let r = ev("FILTER", vec![rf(grid()), av(row(&[1.0, 0.0, 1.0]))]);
    assert_eq!(rows_of(&r).first().unwrap(), &vec![tv("b"), nv(20.0)]);
    is_err(ev("FILTER", vec![rf(grid()), av(col(&[0.0, 0.0, 0.0]))]), CellError::Calc);
    is_text(ev("FILTER", vec![rf(grid()), av(col(&[0.0, 0.0, 0.0])), t("none")]), "none");
    is_err(ev("FILTER", vec![rf(grid()), av(col(&[1.0, 0.0]))]), CellError::Value);
    let errinc = arr(vec![vec![nv(1.0)], vec![Value::Error(CellError::Div0)], vec![nv(1.0)]]);
    is_err(ev("FILTER", vec![rf(grid()), av(errinc)]), CellError::Div0);
}

#[test]
fn sort() {
    let r = ev("SORT", vec![rf(grid())]);
    assert_eq!(rows_of(&r).iter().map(|l| l[0].clone()).collect::<Vec<_>>(), vec![tv("a"), tv("b"), tv("c")]);
    let r = ev("SORT", vec![rf(grid()), n(2.0), n(-1.0)]);
    assert_eq!(rows_of(&r).iter().map(|l| l[1].clone()).collect::<Vec<_>>(), vec![nv(3.0), nv(2.0), nv(1.0)]);
    let r = ev("SORT", vec![av(row(&[3.0, 1.0, 2.0])), n(1.0), n(1.0), b(true)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(2.0), nv(3.0)]]);
    is_err(ev("SORT", vec![rf(grid()), n(4.0)]), CellError::Value);
    is_err(ev("SORT", vec![rf(grid()), n(1.0), n(2.0)]), CellError::Value);
    let mixed = arr(vec![vec![tv("x")], vec![nv(5.0)], vec![Value::Bool(true)], vec![nv(-1.0)]]);
    assert_eq!(rows_of(&ev("SORT", vec![av(mixed)])), vec![vec![nv(-1.0)], vec![nv(5.0)], vec![tv("x")], vec![Value::Bool(true)]]);
    // Multiple keys.
    let m = arr(vec![vec![nv(1.0), nv(2.0)], vec![nv(1.0), nv(1.0)], vec![nv(0.0), nv(5.0)]]);
    let r = ev("SORT", vec![av(m), av(row(&[1.0, 2.0])), av(row(&[1.0, -1.0]))]);
    assert_eq!(rows_of(&r), vec![vec![nv(0.0), nv(5.0)], vec![nv(1.0), nv(2.0)], vec![nv(1.0), nv(1.0)]]);
}

#[test]
fn sortby() {
    let r = ev("SORTBY", vec![av(col(&[1.0, 2.0, 3.0])), av(col(&[30.0, 10.0, 20.0]))]);
    assert_eq!(rows_of(&r), vec![vec![nv(2.0)], vec![nv(3.0)], vec![nv(1.0)]]);
    let r = ev("SORTBY", vec![av(col(&[1.0, 2.0, 3.0])), av(col(&[30.0, 10.0, 20.0])), n(-1.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0)], vec![nv(3.0)], vec![nv(2.0)]]);
    let r = ev("SORTBY", vec![av(row(&[1.0, 2.0])), av(row(&[2.0, 1.0]))]);
    assert_eq!(rows_of(&r), vec![vec![nv(2.0), nv(1.0)]]);
    is_err(ev("SORTBY", vec![av(col(&[1.0, 2.0, 3.0])), av(col(&[1.0, 2.0]))]), CellError::Value);
    is_err(ev("SORTBY", vec![av(col(&[1.0, 2.0])), av(col(&[1.0, 2.0])), n(0.0)]), CellError::Value);
}

#[test]
fn unique_keeps_rows_containing_separator_apart() {
    let m = arr(vec![vec![tv("a"), tv("b\u{1}tc")], vec![tv("a\u{1}tb"), tv("c")]]);
    assert_eq!(rows_of(&ev("UNIQUE", vec![av(m)])).len(), 2);
}

#[test]
fn unique() {
    let v = arr(vec![vec![tv("a")], vec![tv("B")], vec![tv("A")], vec![tv("c")], vec![tv("b")]]);
    assert_eq!(rows_of(&ev("UNIQUE", vec![av(v.clone())])), vec![vec![tv("a")], vec![tv("B")], vec![tv("c")]]);
    assert_eq!(rows_of(&ev("UNIQUE", vec![av(v.clone()), b(false), b(true)])), vec![vec![tv("c")]]);
    assert_eq!(rows_of(&ev("UNIQUE", vec![av(row(&[1.0, 2.0, 1.0])), b(true)])), vec![vec![nv(1.0), nv(2.0)]]);
    is_err(ev("UNIQUE", vec![av(col(&[1.0, 1.0])), b(false), b(true)]), CellError::Calc);
    let m = arr(vec![vec![nv(1.0), nv(2.0)], vec![nv(1.0), nv(2.0)], vec![nv(1.0), nv(3.0)]]);
    assert_eq!(rows_of(&ev("UNIQUE", vec![av(m)])).len(), 2);
}

#[test]
fn sequence_and_rand() {
    assert_eq!(rows_of(&ev("SEQUENCE", vec![n(3.0)])), vec![vec![nv(1.0)], vec![nv(2.0)], vec![nv(3.0)]]);
    assert_eq!(rows_of(&ev("SEQUENCE", vec![n(2.0), n(2.0), n(0.0), n(5.0)])), vec![vec![nv(0.0), nv(5.0)], vec![nv(10.0), nv(15.0)]]);
    assert_eq!(rows_of(&ev("SEQUENCE", vec![empty(), n(3.0)])), vec![vec![nv(1.0), nv(2.0), nv(3.0)]]);
    is_err(ev("SEQUENCE", vec![n(0.0)]), CellError::Calc);
    is_err(ev("SEQUENCE", vec![n(-1.0)]), CellError::Value);
    is_err(ev("SEQUENCE", vec![n(1e6), n(1e6)]), CellError::Num);
    is_err(ev("SEQUENCE", vec![t("x")]), CellError::Value);
    let r = ev("RANDARRAY", vec![n(3.0), n(4.0), n(1.0), n(6.0), b(true)]);
    let rows = rows_of(&r);
    assert_eq!((rows.len(), rows[0].len()), (3, 4));
    for v in rows.iter().flatten() {
        let x = v.as_f64().unwrap();
        assert!((1.0..=6.0).contains(&x) && x.fract() == 0.0);
    }
    let x = ev("RANDARRAY", vec![]);
    assert!(x.as_f64().is_some_and(|x| (0.0..1.0).contains(&x)) || matches!(x, Value::Array(_)));
    is_err(ev("RANDARRAY", vec![n(1.0), n(1.0), n(5.0), n(1.0)]), CellError::Value);
    assert!(crate::lookup("RANDARRAY").unwrap().volatile);
}

#[test]
fn stacking() {
    let r = ev("VSTACK", vec![av(row(&[1.0, 2.0])), av(row(&[3.0]))]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(2.0)], vec![nv(3.0), Value::Error(CellError::NA)]]);
    let r = ev("HSTACK", vec![av(col(&[1.0, 2.0])), n(3.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(3.0)], vec![nv(2.0), Value::Error(CellError::NA)]]);
    let m = arr(vec![vec![nv(1.0), Value::Empty], vec![Value::Error(CellError::Div0), nv(4.0)]]);
    assert_eq!(rows_of(&ev("TOROW", vec![av(m.clone())])).first().unwrap().len(), 4);
    assert_eq!(rows_of(&ev("TOROW", vec![av(m.clone()), n(1.0)])), vec![vec![nv(1.0), Value::Error(CellError::Div0), nv(4.0)]]);
    assert_eq!(rows_of(&ev("TOCOL", vec![av(m.clone()), n(3.0)])), vec![vec![nv(1.0)], vec![nv(4.0)]]);
    assert_eq!(rows_of(&ev("TOCOL", vec![av(m.clone()), n(2.0), b(true)])), vec![vec![nv(1.0)], vec![Value::Empty], vec![nv(4.0)]]);
    is_err(ev("TOCOL", vec![av(m), n(4.0)]), CellError::Value);
    is_err(ev("TOROW", vec![av(arr(vec![vec![Value::Empty]])), n(1.0)]), CellError::Calc);
}

#[test]
fn output_size_checked_before_padding() {
    assert_eq!(super::check_output_size(1_000_001, 10), Err(CellError::Num));
    assert_eq!(super::check_output_size(usize::MAX, 2), Err(CellError::Num));
    assert_eq!(super::check_output_size(1_000_000, 10), Ok(()));
    // A 1M-row column beside a 9-wide row: ~1M input cells, but the padded result would be 10,000,010.
    let tall = Value::from(joysheet_core::Array::column(vec![nv(1.0); 1_000_001]));
    let wide = row(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    is_err(ev("HSTACK", vec![av(tall), av(wide)]), CellError::Num);
}

#[test]
fn wrapping() {
    let r = ev("WRAPROWS", vec![av(row(&[1.0, 2.0, 3.0, 4.0, 5.0])), n(2.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(2.0)], vec![nv(3.0), nv(4.0)], vec![nv(5.0), Value::Error(CellError::NA)]]);
    let r = ev("WRAPCOLS", vec![av(row(&[1.0, 2.0, 3.0])), n(2.0), n(0.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(3.0)], vec![nv(2.0), nv(0.0)]]);
    is_err(ev("WRAPROWS", vec![rf(grid()), n(2.0)]), CellError::Value);
    is_err(ev("WRAPROWS", vec![av(row(&[1.0])), n(0.0)]), CellError::Num);
}

#[test]
fn take_drop_choose_expand() {
    let r = ev("TAKE", vec![rf(grid()), n(2.0)]);
    assert_eq!(rows_of(&r).len(), 2);
    let r = ev("TAKE", vec![rf(grid()), n(-1.0), n(1.0)]);
    assert_eq!(rows_of(&r), vec![vec![tv("c")]]);
    let r = ev("TAKE", vec![rf(grid()), empty(), n(-1.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(20.0)], vec![nv(10.0)], vec![nv(30.0)]]);
    assert_eq!(rows_of(&ev("TAKE", vec![rf(grid()), n(10.0)])).len(), 3);
    is_err(ev("TAKE", vec![rf(grid()), n(0.0)]), CellError::Calc);
    let r = ev("DROP", vec![rf(grid()), n(1.0), n(2.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(10.0)], vec![nv(30.0)]]);
    let r = ev("DROP", vec![rf(grid()), n(-2.0)]);
    assert_eq!(rows_of(&r), vec![vec![tv("b"), nv(2.0), nv(20.0)]]);
    is_err(ev("DROP", vec![rf(grid()), n(3.0)]), CellError::Calc);
    let r = ev("CHOOSEROWS", vec![rf(grid()), n(3.0), n(-3.0)]);
    assert_eq!(rows_of(&r), vec![vec![tv("c"), nv(1.0), nv(30.0)], vec![tv("b"), nv(2.0), nv(20.0)]]);
    let r = ev("CHOOSECOLS", vec![rf(grid()), av(row(&[1.0, 3.0]))]);
    assert_eq!(rows_of(&r).first().unwrap(), &vec![tv("b"), nv(20.0)]);
    is_err(ev("CHOOSEROWS", vec![rf(grid()), n(4.0)]), CellError::Value);
    is_err(ev("CHOOSECOLS", vec![rf(grid()), n(0.0)]), CellError::Value);
    let r = ev("EXPAND", vec![av(row(&[1.0, 2.0])), n(2.0), empty(), n(0.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), nv(2.0)], vec![nv(0.0), nv(0.0)]]);
    let r = ev("EXPAND", vec![av(row(&[1.0])), n(1.0), n(2.0)]);
    assert_eq!(rows_of(&r), vec![vec![nv(1.0), Value::Error(CellError::NA)]]);
    is_err(ev("EXPAND", vec![rf(grid()), n(2.0)]), CellError::Value);
    is_err(ev("EXPAND", vec![av(row(&[1.0])), n(1e7), n(1e4)]), CellError::Num);
}

fn sales() -> (Value, Value, Value) {
    let region = arr(vec![vec![tv("Region")], vec![tv("East")], vec![tv("West")], vec![tv("East")], vec![tv("West")], vec![tv("North")]]);
    let item = arr(vec![vec![tv("Item")], vec![tv("A")], vec![tv("A")], vec![tv("B")], vec![tv("B")], vec![tv("A")]]);
    let amt = arr(vec![vec![tv("Amount")], vec![nv(10.0)], vec![nv(20.0)], vec![nv(5.0)], vec![nv(7.0)], vec![nv(1.0)]]);
    (region, item, amt)
}

#[test]
fn groupby() {
    let (region, _, amt) = sales();
    let r = ev("GROUPBY", vec![rf(region.clone()), rf(amt.clone()), t("SUM")]);
    assert_eq!(
        rows_of(&r),
        vec![
            vec![tv("Region"), tv("Amount")],
            vec![tv("East"), nv(15.0)],
            vec![tv("North"), nv(1.0)],
            vec![tv("West"), nv(27.0)],
            vec![tv("Total"), nv(43.0)],
        ]
    );
    let r = ev("GROUPBY", vec![rf(region.clone()), rf(amt.clone()), n(2.0), n(1.0), n(0.0), n(-1.0)]);
    assert_eq!(rows_of(&r), vec![vec![tv("West"), nv(2.0)], vec![tv("North"), nv(1.0)], vec![tv("East"), nv(2.0)]]);
    let r = ev("GROUPBY", vec![rf(region.clone()), rf(amt.clone()), t("max"), n(1.0), n(0.0), n(-2.0)]);
    assert_eq!(rows_of(&r).first().unwrap(), &vec![tv("West"), nv(20.0)]);
    let filt = arr(vec![
        vec![Value::Empty],
        vec![Value::Bool(true)],
        vec![Value::Bool(false)],
        vec![Value::Bool(true)],
        vec![Value::Bool(true)],
        vec![Value::Bool(true)],
    ]);
    let r = ev("GROUPBY", vec![rf(region.clone()), rf(amt.clone()), t("AVERAGE"), n(1.0), n(0.0), n(1.0), av(filt)]);
    assert_eq!(rows_of(&r), vec![vec![tv("East"), nv(7.5)], vec![tv("North"), nv(1.0)], vec![tv("West"), nv(7.0)]]);
    is_err(ev("GROUPBY", vec![rf(region.clone()), rf(amt.clone()), t("BOGUS")]), CellError::Value);
    is_err(ev("GROUPBY", vec![rf(region), av(col(&[1.0])), t("SUM")]), CellError::Value);
}

#[test]
fn pivotby() {
    let (region, item, amt) = sales();
    let r = ev("PIVOTBY", vec![rf(region), rf(item), rf(amt), t("SUM"), n(1.0)]);
    assert_eq!(
        rows_of(&r),
        vec![
            vec![Value::Empty, tv("A"), tv("B"), tv("Total")],
            vec![tv("East"), nv(10.0), nv(5.0), nv(15.0)],
            vec![tv("North"), nv(1.0), Value::Empty, nv(1.0)],
            vec![tv("West"), nv(20.0), nv(7.0), nv(27.0)],
            vec![tv("Total"), nv(31.0), nv(12.0), nv(43.0)],
        ]
    );
}

#[test]
fn pivotby_size_check_counts_column_header_rows() {
    // One row, n column fields and n value columns: n header rows + value-name row + body row + total row,
    // each 1 + 2n wide. The size checked up front matches the grid PIVOTBY builds.
    let dims = |k: u32, totals: bool| {
        let fields: Vec<f64> = (0..k).map(f64::from).collect();
        let mut args = vec![av(row(&[1.0])), av(row(&fields)), av(row(&fields)), t("SUM")];
        if !totals {
            args.extend([empty(), n(0.0), empty(), n(0.0)]);
        }
        let r = rows_of(&ev("PIVOTBY", args));
        (r.len(), r.first().map_or(0, Vec::len))
    };
    assert_eq!(dims(2, true), super::pivot_dims(3, 2, 2, 2, 1));
    assert_eq!(dims(10, true), (13, 21));
    assert_eq!(dims(10, true), super::pivot_dims(11, 2, 2, 10, 1));
    assert_eq!(dims(10, false), (12, 11));
    assert_eq!(dims(10, false), super::pivot_dims(11, 1, 1, 10, 1));
    // n = 2300: the 2300 header rows alone are 10.58M cells, so the check must count them before any is built.
    let (h, w) = super::pivot_dims(2301, 2, 2, 2300, 1);
    assert!(2300 * w > crate::util::MAX_CELLS && h * w > crate::util::MAX_CELLS);
    let wide: Vec<f64> = (0..2300).map(f64::from).collect();
    is_err(ev("PIVOTBY", vec![av(row(&[1.0])), av(row(&wide)), av(row(&wide)), t("SUM")]), CellError::Num);
}
