//! Database functions (DSUM, DCOUNT, DGET…): aggregate one field of the records of a table
//! that satisfy a criteria table.
//!
//! The database and the criteria are arrays whose first row holds column labels. Each criteria
//! row is a set of conditions that must all hold (AND); a record matches when any criteria row
//! holds (OR). An empty criteria cell matches anything. Plain text conditions match values that
//! begin with the text, as in Excel's advanced filter, so an empty string matches any text. A
//! formula under a label that names no field is a computed criterion, which the host evaluates
//! for each record ([`Ctx::computed_criterion`]).

use std::borrow::Cow;

use joysheet_core::{Array, CellError, Value, compare_text};

use crate::criteria::Criterion;
use crate::util::{A, R, as_array, has, num_val};
use crate::{Arg, Ctx, FnSpec};

/// Column index of a label in the header row.
fn find_column(db: &Array, label: &str) -> Option<usize> {
    let label = label.trim();
    (0..db.cols).find(|&c| match db.get(0, c) {
        Some(Value::Text(t)) => compare_text(t.trim(), label).is_eq(),
        Some(Value::Number(n)) => joysheet_core::number_to_text(*n) == label,
        _ => false,
    })
}

/// Resolves the field argument: a label or a 1-based column number. `None` when omitted.
fn field_index(db: &Array, a: &[Arg]) -> R<Option<usize>> {
    if !has(a, 1) {
        return Ok(None);
    }
    let v = a.get(1).map(|x| x.value.scalar()).unwrap_or(Value::Empty);
    match v {
        Value::Number(n) => {
            let i = n.trunc();
            // NaN too: it would pass both comparisons and become index 0 (#2163).
            if i.is_nan() || i < 1.0 || i > db.cols as f64 {
                return Err(CellError::Value);
            }
            Ok(Some((i as usize).saturating_sub(1)))
        }
        Value::Text(t) => find_column(db, &t).map(Some).ok_or(CellError::Value),
        Value::Bool(_) => Err(CellError::Value),
        Value::Error(e) => Err(e),
        _ => Ok(None),
    }
}

/// One criteria cell as a matcher. Plain text becomes a "begins with" pattern.
fn cell_criterion(v: &Value) -> Option<Criterion> {
    match v {
        Value::Empty => None,
        Value::Text(t) => {
            let s: &str = t;
            let has_op = s.starts_with(['=', '<', '>']);
            let plain_text = !has_op
                && joysheet_core::parse::parse_number_text(s).is_none()
                && !s.eq_ignore_ascii_case("TRUE")
                && !s.eq_ignore_ascii_case("FALSE")
                && CellError::parse(s).is_none();
            if plain_text && !s.ends_with('*') { Some(Criterion::parse(&Value::from(format!("{s}*")))) } else { Some(Criterion::parse(v)) }
        }
        other => Some(Criterion::parse(other)),
    }
}

/// Rows (1-based within the database, excluding the header) of matching records.
fn matching_records(db: &Array, crit: &Array, ctx: &mut dyn Ctx) -> Vec<usize> {
    // Per criteria row: its index and a list of (criteria column, database column or None when
    // the label is unknown, criterion).
    let mut rows: Vec<(usize, Vec<(usize, Option<usize>, Criterion)>)> = Vec::new();
    for r in 1..crit.rows {
        let mut conds = Vec::new();
        for c in 0..crit.cols {
            let Some(cell) = crit.get(r, c) else { continue };
            let Some(cr) = cell_criterion(cell) else { continue };
            let col = match crit.get(0, c) {
                Some(Value::Text(t)) => find_column(db, t),
                Some(Value::Number(n)) => find_column(db, &joysheet_core::number_to_text(*n)),
                _ => None,
            };
            conds.push((c, col, cr));
        }
        rows.push((r, conds));
    }
    let all_match = rows.is_empty();
    (1..db.rows)
        .filter(|&rec| {
            all_match
                || rows.iter().any(|(r, conds)| {
                    conds.iter().all(|(c, col, cr)| match col {
                        Some(f) => cr.matches(db.get(rec, *f).unwrap_or(&Value::Empty)),
                        // Under a label that names no field, only a formula (a computed
                        // criterion) can hold.
                        None => ctx.computed_criterion(*r, *c, db, rec).unwrap_or(false),
                    })
                })
        })
        .collect()
}

struct Selection<'a> {
    db: Cow<'a, Array>,
    field: Option<usize>,
    records: Vec<usize>,
}

fn select<'a>(a: &'a [Arg], ctx: &mut dyn Ctx) -> R<Selection<'a>> {
    let dbv = &a.first().ok_or(CellError::Value)?.value;
    let critv = &a.get(2).ok_or(CellError::Value)?.value;
    if let Value::Error(e) = dbv {
        return Err(*e);
    }
    if let Value::Error(e) = critv {
        return Err(*e);
    }
    let db = as_array(dbv);
    let crit = as_array(critv);
    let field = field_index(&db, a)?;
    let records = matching_records(&db, &crit, ctx);
    Ok(Selection { db, field, records })
}

/// Numbers of the field among matching records (field required).
fn field_numbers(a: &[Arg], c: &mut dyn Ctx) -> R<Vec<f64>> {
    let s = select(a, c)?;
    let f = s.field.ok_or(CellError::Value)?;
    Ok(s.records.iter().filter_map(|&r| s.db.get(r, f).and_then(Value::as_f64)).collect())
}

fn dsum(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(field_numbers(a, c)?.iter().sum())
}

fn daverage(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let v = field_numbers(a, c)?;
    if v.is_empty() {
        return Err(CellError::Div0);
    }
    num_val(v.iter().sum::<f64>() / v.len() as f64)
}

fn dcount(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let s = select(a, c)?;
    let n = match s.field {
        Some(f) => s.records.iter().filter(|&&r| matches!(s.db.get(r, f), Some(Value::Number(_)))).count(),
        None => s.records.len(),
    };
    num_val(n as f64)
}

fn dcounta(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let s = select(a, c)?;
    let n = match s.field {
        Some(f) => s.records.iter().filter(|&&r| !matches!(s.db.get(r, f), None | Some(Value::Empty))).count(),
        None => s.records.len(),
    };
    num_val(n as f64)
}

fn dget(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let s = select(a, c)?;
    let f = s.field.ok_or(CellError::Value)?;
    // Records whose field is blank (or "") don't count, as in Excel.
    let mut found =
        s.records.iter().filter_map(|&r| s.db.get(r, f)).filter(|v| !matches!(v, Value::Empty) && !matches!(v, Value::Text(t) if t.is_empty()));
    match (found.next(), found.next()) {
        (None, _) => Err(CellError::Value),
        (Some(v), None) => Ok(v.clone()),
        _ => Err(CellError::Num),
    }
}

fn dmax(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let v = field_numbers(a, c)?;
    num_val(v.iter().copied().reduce(f64::max).unwrap_or(0.0))
}

fn dmin(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let v = field_numbers(a, c)?;
    num_val(v.iter().copied().reduce(f64::min).unwrap_or(0.0))
}

fn dproduct(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let v = field_numbers(a, c)?;
    if v.is_empty() {
        return num_val(0.0);
    }
    num_val(v.iter().product())
}

fn variance(v: &[f64], sample: bool) -> R<f64> {
    let n = v.len() as f64;
    let min = if sample { 2.0 } else { 1.0 };
    if n < min {
        return Err(CellError::Div0);
    }
    let mean = v.iter().sum::<f64>() / n;
    let ss: f64 = v.iter().map(|x| (x - mean) * (x - mean)).sum();
    Ok(ss / if sample { n - 1.0 } else { n })
}

fn dvar(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(variance(&field_numbers(a, c)?, true)?)
}

fn dvarp(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(variance(&field_numbers(a, c)?, false)?)
}

fn dstdev(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(variance(&field_numbers(a, c)?, true)?.sqrt())
}

fn dstdevp(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    num_val(variance(&field_numbers(a, c)?, false)?.sqrt())
}

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!("DSUM", 3, 3, Database, A, "DSUM(database, field, criteria)", "Adds the numbers in a field of the records that meet the criteria.", dsum),
        f!(
            "DAVERAGE",
            3,
            3,
            Database,
            A,
            "DAVERAGE(database, field, criteria)",
            "Averages the numbers in a field of the records that meet the criteria.",
            daverage
        ),
        f!(
            "DCOUNT",
            3,
            3,
            Database,
            A,
            "DCOUNT(database, [field], criteria)",
            "Counts numeric cells in a field of the matching records (or the matching records).",
            dcount
        ),
        f!(
            "DCOUNTA",
            3,
            3,
            Database,
            A,
            "DCOUNTA(database, [field], criteria)",
            "Counts non-blank cells in a field of the matching records (or the matching records).",
            dcounta
        ),
        f!(
            "DGET",
            3,
            3,
            Database,
            A,
            "DGET(database, field, criteria)",
            "Returns the field value of the single record that meets the criteria.",
            dget
        ),
        f!("DMAX", 3, 3, Database, A, "DMAX(database, field, criteria)", "Largest number in a field of the matching records.", dmax),
        f!("DMIN", 3, 3, Database, A, "DMIN(database, field, criteria)", "Smallest number in a field of the matching records.", dmin),
        f!(
            "DPRODUCT",
            3,
            3,
            Database,
            A,
            "DPRODUCT(database, field, criteria)",
            "Multiplies the numbers in a field of the matching records.",
            dproduct
        ),
        f!(
            "DSTDEV",
            3,
            3,
            Database,
            A,
            "DSTDEV(database, field, criteria)",
            "Sample standard deviation of a field over the matching records.",
            dstdev
        ),
        f!(
            "DSTDEVP",
            3,
            3,
            Database,
            A,
            "DSTDEVP(database, field, criteria)",
            "Population standard deviation of a field over the matching records.",
            dstdevp
        ),
        f!("DVAR", 3, 3, Database, A, "DVAR(database, field, criteria)", "Sample variance of a field over the matching records.", dvar),
        f!("DVARP", 3, 3, Database, A, "DVARP(database, field, criteria)", "Population variance of a field over the matching records.", dvarp),
    ]
}

#[cfg(test)]
#[path = "tests/database_tests.rs"]
mod tests;
