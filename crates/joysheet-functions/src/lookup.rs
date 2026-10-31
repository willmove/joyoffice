//! Lookup and reference functions over evaluated arrays: VLOOKUP, HLOOKUP, LOOKUP, MATCH,
//! XLOOKUP, XMATCH, INDEX, ROWS, COLUMNS, TRANSPOSE, ADDRESS.
//!
//! Reference-only behaviour (INDEX returning a reference, OFFSET, ROW…) belongs to the host.

use std::cmp::Ordering;

use joysheet_core::{Array, CellError, Value, col_to_letters, compare};

use crate::criteria::{has_wildcards, lookup_equal};
use crate::util::{R, arg, array_val, as_array, has, int, num, opt_bool, opt_int, opt_num, text, text_val};
use crate::{Arg, Ctx, FnSpec};

// ---------------------------------------------------------------------------------------------
// Search helpers

/// Lookup needle: the scalar to look for. Blank looks for 0; errors propagate.
fn needle(args: &[Arg], i: usize) -> R<Value> {
    match arg(args, i)?.value.scalar() {
        Value::Error(e) => Err(e),
        Value::Empty => Ok(Value::Number(0.0)),
        v => Ok(v),
    }
}

/// Order of two values for the approximate and binary searches: [`compare`], except that numbers
/// compare exactly. Excel's `=` treats numbers equal to 15 significant digits as equal, but its
/// lookups don't. Observed in Excel 365 with A1 holding
/// `=0.1+0.2` (0.30000000000000004): `A1=0.3` is TRUE, yet `MATCH(0.3,A1,1)` is #N/A because
/// A1 is larger, and in {0.1;0.2;0.3;A1;0.4} `MATCH(0.3,…,1)` is 3 and `MATCH(A1,…,1)` is 4.
/// The same holds for `=100+1E-14` against 100, and in every XLOOKUP / XMATCH mode.
fn lookup_order(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.partial_cmp(y).unwrap_or_else(|| compare(a, b)),
        _ => compare(a, b),
    }
}

fn same_type(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Number(_), Value::Number(_)) | (Value::Text(_), Value::Text(_)) | (Value::Bool(_), Value::Bool(_)))
}

/// Exact search (first match, or last when `reverse`). Text needles match wildcards when `wild`.
fn find_exact(x: &Value, hay: &[Value], wild: bool, reverse: bool) -> Option<usize> {
    let wild = wild && matches!(x, Value::Text(t) if has_wildcards(t));
    if reverse { hay.iter().rposition(|v| lookup_equal(x, v, wild)) } else { hay.iter().position(|v| lookup_equal(x, v, wild)) }
}

/// Approximate match of VLOOKUP, HLOOKUP, LOOKUP and MATCH: the position of the largest entry
/// `<= x` in data sorted ascending (`desc = false`), or of the smallest entry `>= x` in data
/// sorted descending. Like Excel this is a bisection over the whole vector, so on unsorted data
/// it answers what Excel answers rather than a linear "best" entry:
/// - the probe is the middle entry of the remaining window; a probe whose type differs from the
///   needle (text vs number vs logical, blanks, errors) is replaced by the nearest entry of the
///   needle's type below it in the window; when there is none the window moves up past it, so
///   entries of other types are skipped (`LOOKUP(9.9E+307, range)` finds the last number);
/// - a probe equal to the needle ends the search at once (the run of equal entries that follows
///   it is taken, so sorted duplicates give the last one).
///
/// Observed in Excel: `VLOOKUP(534, {"INTEGRAL";0;1;2;534;9999999999;-9999999999}, …)` finds 534,
/// `HLOOKUP("STRING", {"INTEGRAL","DOUBLE","BLANK","STRING","REF","AREA (rows)",<blank>}, …)`
/// finds "STRING" and `LOOKUP("DOUBLE", {"DOUBLE","BLANK","STRING","REF","AREA (rows)"}, …)`
/// finds "DOUBLE". Work is linear in the vector length at worst.
fn bsearch_last(x: &Value, hay: &[Value], desc: bool) -> Option<usize> {
    let mut hi = hay.len().checked_sub(1)?;
    let mut lo = 0usize;
    let mut best = None;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let Some(j) = (lo..=mid).rev().find(|&j| hay.get(j).is_some_and(|v| same_type(x, v))) else {
            lo = mid + 1;
            continue;
        };
        let v = hay.get(j)?;
        let o = lookup_order(v, x);
        if o == Ordering::Equal {
            let mut k = j;
            while hay.get(k + 1).is_some_and(|w| same_type(x, w) && lookup_order(w, x) == Ordering::Equal) {
                k += 1;
            }
            return Some(k);
        }
        let before = if desc { o == Ordering::Greater } else { o == Ordering::Less };
        if before {
            best = Some(j);
            lo = mid + 1;
        } else if j == 0 {
            break;
        } else {
            hi = j - 1;
        }
    }
    best
}

/// Order of an entry relative to the needle for XLOOKUP / XMATCH binary search (search mode ±2):
/// Excel's sort order over all types (numbers < text < logicals < errors), reversed for
/// descending data, with blanks always last like a sort leaves them. Unlike the legacy
/// approximate match no entry is skipped (observed: `XLOOKUP(46523, {9700;"xyz";"xyz";160726;
/// 204100;510300}, …, 0, 1, 2)` returns the row of the first "xyz").
fn binary_order(v: &Value, x: &Value, desc: bool) -> Ordering {
    match v {
        Value::Empty => Ordering::Greater,
        _ if desc => lookup_order(v, x).reverse(),
        _ => lookup_order(v, x),
    }
}

/// Binary search for the first entry not ordered before the needle (`>= x` ascending, `<= x`
/// descending).
fn bsearch_first(x: &Value, hay: &[Value], desc: bool) -> Option<usize> {
    let p = hay.partition_point(|v| binary_order(v, x, desc) == Ordering::Less);
    (p < hay.len()).then_some(p)
}

/// Binary search for the last entry not ordered after the needle (`<= x` ascending, `>= x`
/// descending).
fn bsearch_upto(x: &Value, hay: &[Value], desc: bool) -> Option<usize> {
    hay.partition_point(|v| binary_order(v, x, desc) != Ordering::Greater).checked_sub(1)
}

/// A 1-D view of an array (row or column), `None` when it is 2-D.
fn vector(a: &Array) -> Option<Vec<Value>> {
    if a.rows == 1 || a.cols == 1 { Some(a.data.clone()) } else { None }
}

fn column(a: &Array, c: usize) -> Vec<Value> {
    (0..a.rows).map(|r| a.get(r, c).cloned().unwrap_or(Value::Empty)).collect()
}

fn row(a: &Array, r: usize) -> Vec<Value> {
    (0..a.cols).map(|c| a.get(r, c).cloned().unwrap_or(Value::Empty)).collect()
}

/// A looked-up cell value: blanks read as 0 like a reference to an empty cell.
fn cell_result(v: Value) -> Value {
    if matches!(v, Value::Empty) { Value::Number(0.0) } else { v }
}

// ---------------------------------------------------------------------------------------------
// VLOOKUP / HLOOKUP / LOOKUP / MATCH

fn vh_lookup(args: &[Arg], vertical: bool) -> R<Value> {
    let x = needle(args, 0)?;
    let table = as_array(&arg(args, 1)?.value);
    if let Value::Error(e) = &arg(args, 1)?.value {
        return Err(*e);
    }
    let idx = num(args, 2)?;
    // NaN too: it would pass `idx < 1.0` and become index 0 (#2163).
    if idx.is_nan() || idx < 1.0 {
        return Err(CellError::Value);
    }
    let idx = (idx.trunc() as usize).saturating_sub(1);
    let approx = opt_bool(args, 3, true)?;
    let (keys, len) = if vertical { (column(&table, 0), table.cols) } else { (row(&table, 0), table.rows) };
    if idx >= len {
        return Err(CellError::Ref);
    }
    let pos = if approx { bsearch_last(&x, &keys, false) } else { find_exact(&x, &keys, true, false) };
    let pos = pos.ok_or(CellError::NA)?;
    let v = if vertical { table.get(pos, idx) } else { table.get(idx, pos) };
    Ok(cell_result(v.cloned().unwrap_or(Value::Empty)))
}

fn vlookup(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    vh_lookup(a, true)
}

fn hlookup(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    vh_lookup(a, false)
}

fn lookup(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = needle(args, 0)?;
    let lv = &arg(args, 1)?.value;
    if let Value::Error(e) = lv {
        return Err(*e);
    }
    let la = as_array(lv);
    if args.len() >= 3 {
        let keys = if la.rows == 1 || la.cols == 1 { la.data.clone() } else { column(&la, 0) };
        let pos = bsearch_last(&x, &keys, false).ok_or(CellError::NA)?;
        let rv = &arg(args, 2)?.value;
        if let Value::Error(e) = rv {
            return Err(*e);
        }
        let ra = as_array(rv);
        let v = if ra.rows == 1 || ra.cols == 1 {
            ra.data.get(pos).cloned()
        } else if ra.rows >= keys.len() {
            ra.get(pos, 0).cloned()
        } else {
            None
        };
        return v.map(cell_result).ok_or(CellError::NA);
    }
    // Array form: search the first row of a wide array, else the first column; return the
    // matching entry from the last row / column.
    if la.cols > la.rows {
        let keys = row(&la, 0);
        let pos = bsearch_last(&x, &keys, false).ok_or(CellError::NA)?;
        Ok(cell_result(la.get(la.rows - 1, pos).cloned().unwrap_or(Value::Empty)))
    } else {
        let keys = column(&la, 0);
        let pos = bsearch_last(&x, &keys, false).ok_or(CellError::NA)?;
        Ok(cell_result(la.get(pos, la.cols - 1).cloned().unwrap_or(Value::Empty)))
    }
}

fn match_fn(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = needle(args, 0)?;
    let av = &arg(args, 1)?.value;
    if let Value::Error(e) = av {
        return Err(*e);
    }
    let hay = vector(&as_array(av)).ok_or(CellError::NA)?;
    let mt = opt_num(args, 2, 1.0)?;
    let pos = if mt == 0.0 {
        find_exact(&x, &hay, true, false)
    } else if mt > 0.0 {
        bsearch_last(&x, &hay, false)
    } else {
        bsearch_last(&x, &hay, true)
    };
    pos.map(|p| Value::Number((p + 1) as f64)).ok_or(CellError::NA)
}

// ---------------------------------------------------------------------------------------------
// XLOOKUP / XMATCH

/// Core XMATCH search: 0-based position in `hay`.
fn xsearch(x: &Value, hay: &[Value], match_mode: i64, search_mode: i64) -> R<Option<usize>> {
    if !matches!(match_mode, -1..=2) || !matches!(search_mode, -2 | -1 | 1 | 2) {
        return Err(CellError::Value);
    }
    if search_mode.abs() == 2 {
        let desc = search_mode == -2;
        if match_mode == 2 {
            // Wildcards are not supported with binary search: Excel falls back to exact.
            return Ok(bsearch_exact(x, hay, desc));
        }
        return Ok(match match_mode {
            0 => bsearch_exact(x, hay, desc),
            // Next smaller: last <= x (ascending) / first <= x (descending).
            -1 => {
                if desc {
                    bsearch_first(x, hay, true)
                } else {
                    bsearch_upto(x, hay, false)
                }
            }
            // Next larger: first >= x (ascending) / last >= x (descending).
            _ => {
                if desc {
                    bsearch_upto(x, hay, true)
                } else {
                    bsearch_first(x, hay, false)
                }
            }
        });
    }
    let reverse = search_mode == -1;
    if let Some(p) = find_exact(x, hay, match_mode == 2, reverse) {
        return Ok(Some(p));
    }
    if match_mode == 0 || match_mode == 2 {
        return Ok(None);
    }
    // Best candidate: the largest value below (mode -1) or smallest above (mode 1).
    let order: Box<dyn Iterator<Item = usize>> = if reverse { Box::new((0..hay.len()).rev()) } else { Box::new(0..hay.len()) };
    let mut best: Option<usize> = None;
    for i in order {
        let Some(v) = hay.get(i) else { continue };
        if !same_type(x, v) {
            continue;
        }
        let o = lookup_order(v, x);
        let candidate = if match_mode == -1 { o == Ordering::Less } else { o == Ordering::Greater };
        if !candidate {
            continue;
        }
        let better = match best.and_then(|b| hay.get(b)) {
            None => true,
            Some(bv) => {
                let c = lookup_order(v, bv);
                if match_mode == -1 { c == Ordering::Greater } else { c == Ordering::Less }
            }
        };
        if better {
            best = Some(i);
        }
    }
    Ok(best)
}

fn bsearch_exact(x: &Value, hay: &[Value], desc: bool) -> Option<usize> {
    let p = bsearch_first(x, hay, desc)?;
    let v = hay.get(p)?;
    if binary_order(v, x, desc) == Ordering::Equal { Some(p) } else { None }
}

fn xlookup(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = needle(args, 0)?;
    let lv = &arg(args, 1)?.value;
    if let Value::Error(e) = lv {
        return Err(*e);
    }
    let rv = &arg(args, 2)?.value;
    if let Value::Error(e) = rv {
        return Err(*e);
    }
    let la = as_array(lv);
    let ra = as_array(rv);
    let hay = vector(&la).ok_or(CellError::Value)?;
    // Orientation: a column lookup returns rows of the return array; a row lookup returns columns.
    let vertical = if la.rows == 1 && la.cols == 1 { ra.rows == 1 || ra.cols != 1 } else { la.cols == 1 };
    if vertical && ra.rows != la.rows || !vertical && ra.cols != la.cols {
        return Err(CellError::Value);
    }
    let mm = if has(args, 4) { opt_int(args, 4, 0)? } else { 0 };
    let sm = if has(args, 5) { opt_int(args, 5, 1)? } else { 1 };
    match xsearch(&x, &hay, mm, sm)? {
        Some(p) => {
            if vertical {
                let r = row(&ra, p);
                if r.len() == 1 {
                    return Ok(cell_result(r.into_iter().next().unwrap_or(Value::Empty)));
                }
                array_val(1, r.len(), r)
            } else {
                let c = column(&ra, p);
                if c.len() == 1 {
                    return Ok(cell_result(c.into_iter().next().unwrap_or(Value::Empty)));
                }
                array_val(c.len(), 1, c)
            }
        }
        None => {
            if has(args, 3) {
                Ok(arg(args, 3)?.value.clone())
            } else {
                Err(CellError::NA)
            }
        }
    }
}

fn xmatch(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = needle(args, 0)?;
    let lv = &arg(args, 1)?.value;
    if let Value::Error(e) = lv {
        return Err(*e);
    }
    let hay = vector(&as_array(lv)).ok_or(CellError::Value)?;
    let mm = if has(args, 2) { int(args, 2)? } else { 0 };
    let sm = if has(args, 3) { int(args, 3)? } else { 1 };
    xsearch(&x, &hay, mm, sm)?.map(|p| Value::Number((p + 1) as f64)).ok_or(CellError::NA)
}

// ---------------------------------------------------------------------------------------------
// INDEX / ROWS / COLUMNS / TRANSPOSE / ADDRESS

fn index(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let av = &arg(args, 0)?.value;
    if let Value::Error(e) = av {
        return Err(*e);
    }
    let a = as_array(av);
    let r = if has(args, 1) { num(args, 1)? } else { 0.0 };
    let c = if has(args, 2) { num(args, 2)? } else { 0.0 };
    if has(args, 3) && num(args, 3)?.trunc() != 1.0 {
        return Err(CellError::Ref);
    }
    if r < 0.0 || c < 0.0 {
        return Err(CellError::Value);
    }
    let (mut r, mut c) = (r.trunc() as usize, c.trunc() as usize);
    // A single row with only one index: the index selects the column.
    if a.rows == 1 && args.len() == 2 && a.cols > 1 {
        c = r;
        r = 1;
    }
    if r > a.rows || c > a.cols {
        return Err(CellError::Ref);
    }
    match (r, c) {
        (0, 0) => Ok(Value::Array(std::sync::Arc::new(a.into_owned()))),
        (0, c) => {
            let v = column(&a, c - 1);
            if v.len() == 1 {
                return Ok(v.into_iter().next().unwrap_or(Value::Empty));
            }
            array_val(v.len(), 1, v)
        }
        (r, 0) => {
            let v = row(&a, r - 1);
            if v.len() == 1 {
                return Ok(v.into_iter().next().unwrap_or(Value::Empty));
            }
            array_val(1, v.len(), v)
        }
        (r, c) => a.get(r - 1, c - 1).cloned().ok_or(CellError::Ref),
    }
}

fn rows_fn(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    match &arg(args, 0)?.value {
        Value::Array(a) => Ok(Value::Number(a.rows as f64)),
        Value::Error(e) => Err(*e),
        _ => Ok(Value::Number(1.0)),
    }
}

fn columns_fn(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    match &arg(args, 0)?.value {
        Value::Array(a) => Ok(Value::Number(a.cols as f64)),
        Value::Error(e) => Err(*e),
        _ => Ok(Value::Number(1.0)),
    }
}

fn transpose(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    match &arg(args, 0)?.value {
        Value::Array(a) => Ok(Value::from(a.transpose())),
        v => Ok(v.clone()),
    }
}

/// Sheet text for ADDRESS, quoted when Excel quotes it. A `[Book]Sheet` prefix is judged by its
/// parts (Microsoft's ADDRESS examples: `"[Book1]Sheet1"` stays bare, `"EXCEL SHEET"` becomes
/// `'EXCEL SHEET'`). Unlike in a formula, `TRUE` and `FALSE` stay bare.
fn quote_sheet(s: &str) -> String {
    let bare = |p: &str| p.eq_ignore_ascii_case("TRUE") || p.eq_ignore_ascii_case("FALSE") || !joysheet_core::sheet_needs_quotes(p);
    let word = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
    let plain = match s.strip_prefix('[').and_then(|r| r.split_once(']')) {
        Some((book, sheet)) => word(book) && bare(sheet),
        None => bare(s),
    };
    if plain { s.to_string() } else { format!("'{}'", s.replace('\'', "''")) }
}

fn address(args: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let r = num(args, 0)?.trunc();
    let c = num(args, 1)?.trunc();
    let abs = if has(args, 2) { int(args, 2)? } else { 1 };
    let a1 = if has(args, 3) { opt_bool(args, 3, true)? } else { true };
    if !(1.0..=1_048_576.0).contains(&r) || !(1.0..=16_384.0).contains(&c) || !(1..=4).contains(&abs) {
        return Err(CellError::Value);
    }
    let (ri, ci) = (r as u32, c as u32);
    let (row_abs, col_abs) = match abs {
        1 => (true, true),
        2 => (true, false),
        3 => (false, true),
        _ => (false, false),
    };
    let cell = if a1 {
        format!("{}{}{}{}", if col_abs { "$" } else { "" }, col_to_letters(ci - 1), if row_abs { "$" } else { "" }, ri)
    } else {
        let rp = if row_abs { format!("R{ri}") } else { format!("R[{ri}]") };
        let cp = if col_abs { format!("C{ci}") } else { format!("C[{ci}]") };
        format!("{rp}{cp}")
    };
    if has(args, 4) {
        let sheet = text(args, 4)?;
        if sheet.is_empty() {
            return text_val(format!("!{cell}"));
        }
        return text_val(format!("{}!{cell}", quote_sheet(&sheet)));
    }
    text_val(cell)
}

const XL: &[bool] = &[true, false, false, false, false, false];
const VL: &[bool] = &[true, false, true, true];
const ML: &[bool] = &[true, false, true, true];
const IDX: &[bool] = &[false, true, true, true];

pub(crate) fn specs() -> Vec<FnSpec> {
    use crate::util::{A, S};
    vec![
        f!(
            "VLOOKUP",
            3,
            4,
            Lookup,
            VL,
            "VLOOKUP(lookup_value, table_array, col_index_num, [range_lookup])",
            "Finds a value in the first column of a table and returns the value in the same row of another column.",
            vlookup
        ),
        f!(
            "HLOOKUP",
            3,
            4,
            Lookup,
            VL,
            "HLOOKUP(lookup_value, table_array, row_index_num, [range_lookup])",
            "Finds a value in the first row of a table and returns the value in the same column of another row.",
            hlookup
        ),
        f!(
            "LOOKUP",
            2,
            3,
            Lookup,
            &[true, false, false],
            "LOOKUP(lookup_value, lookup_vector, [result_vector])",
            "Approximate lookup in a sorted vector or the first row/column of an array.",
            lookup
        ),
        f!(
            "MATCH",
            2,
            3,
            Lookup,
            &[true, false, true],
            "MATCH(lookup_value, lookup_array, [match_type])",
            "Returns the 1-based position of a value in a row or column.",
            match_fn
        ),
        f!(
            "XLOOKUP",
            3,
            6,
            Lookup,
            XL,
            "XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])",
            "Searches a row or column and returns the matching item, row or column from another array.",
            xlookup
        ),
        f!(
            "XMATCH",
            2,
            4,
            Lookup,
            ML,
            "XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])",
            "Returns the position of a value in a row or column with flexible match and search modes.",
            xmatch
        ),
        f!(
            "INDEX",
            2,
            4,
            Lookup,
            IDX,
            "INDEX(array, row_num, [column_num], [area_num])",
            "Returns the element, row or column of an array at the given position.",
            index
        ),
        f!("ROWS", 1, 1, Lookup, A, "ROWS(array)", "Counts the rows in an array or range.", rows_fn),
        f!("COLUMNS", 1, 1, Lookup, A, "COLUMNS(array)", "Counts the columns in an array or range.", columns_fn),
        f!("TRANSPOSE", 1, 1, Lookup, A, "TRANSPOSE(array)", "Swaps the rows and columns of an array.", transpose),
        f!(
            "ADDRESS",
            2,
            5,
            Lookup,
            S,
            "ADDRESS(row_num, column_num, [abs_num], [a1], [sheet_text])",
            "Builds a cell address as text from row and column numbers.",
            address
        ),
    ]
}

#[cfg(test)]
#[path = "tests/lookup_tests.rs"]
mod tests;
