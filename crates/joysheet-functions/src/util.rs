//! Shared argument coercion and collection helpers.

use std::borrow::Cow;
use std::sync::Arc;

use joysheet_core::{Array, CellError, Value};

use crate::Arg;

pub(crate) type R<T> = Result<T, CellError>;

/// Largest array a function may build.
pub(crate) const MAX_CELLS: usize = 10_000_000;
/// Longest text a cell can hold.
pub(crate) const MAX_TEXT: usize = 32_767;

/// Scalar-position lifting tables.
pub(crate) const S: &[bool] = &[true];
/// No lifting (all positions take arrays / ranges).
pub(crate) const A: &[bool] = &[false];

/// Turns an implementation result into a cell value: errors become error values, non-finite
/// numbers become `#NUM!`, text longer than 32,767 characters becomes `#VALUE!`.
pub(crate) fn finish(r: R<Value>) -> Value {
    match r {
        Ok(Value::Number(n)) => Value::number(n),
        Ok(Value::Text(t)) if t.len() > MAX_TEXT && t.chars().count() > MAX_TEXT => Value::Error(CellError::Value),
        Ok(v) => v,
        Err(e) => Value::Error(e),
    }
}

/// A finite number or `#NUM!`.
pub(crate) fn num_val(n: f64) -> R<Value> {
    if n.is_finite() { Ok(Value::Number(if n == 0.0 { 0.0 } else { n })) } else { Err(CellError::Num) }
}

pub(crate) fn text_val(s: impl Into<Arc<str>>) -> R<Value> {
    Ok(Value::Text(s.into()))
}

/// Scalar value of an argument (top-left element for arrays).
pub(crate) fn scalar(a: &Arg) -> Value {
    a.value.scalar()
}

/// Argument `i`, or `#VALUE!` when it is missing.
pub(crate) fn arg(args: &[Arg], i: usize) -> R<&Arg> {
    args.get(i).ok_or(CellError::Value)
}

/// Whether argument `i` was supplied (present and not an empty literal like `F(1,)`).
pub(crate) fn has(args: &[Arg], i: usize) -> bool {
    matches!(args.get(i), Some(a) if !matches!(a.value, Value::Empty) || a.from_ref)
}

/// Number coercion for a scalar parameter: empty = 0, TRUE = 1, numeric text parses.
pub(crate) fn num(args: &[Arg], i: usize) -> R<f64> {
    let a = arg(args, i)?;
    scalar(a).to_number()
}

// Decimal scripts accepted by the worksheet conversion and parity functions.
const DECIMAL_ZERO: [u32; 18] =
    [0x0660, 0x06f0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66, 0x0ce6, 0x0d66, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x17e0, 0x1810, 0xff10];

pub(crate) fn scalar_argument(args: &[Arg], index: usize) -> R<&Value> {
    Ok(match &arg(args, index)?.value {
        Value::Array(array) => array.data.first().unwrap_or(&Value::Empty),
        value => value,
    })
}

pub(crate) fn number_with_decimal_digits(args: &[Arg], index: usize, allow_logical: bool) -> R<f64> {
    let value = scalar_argument(args, index)?;
    if let Value::Number(n) = value {
        return Ok(*n);
    }
    if !allow_logical && matches!(value, Value::Bool(_)) {
        return Err(CellError::Value);
    }
    if let Value::Text(t) = value
        && !t.is_ascii()
    {
        let normalized: String = t
            .chars()
            .map(|c| {
                let code = u32::from(c);
                let pos = DECIMAL_ZERO.partition_point(|&start| start <= code);
                pos.checked_sub(1).and_then(|i| char::from_digit(code - DECIMAL_ZERO[i], 10)).unwrap_or(c)
            })
            .collect();
        return Value::from(normalized).to_number();
    }
    value.to_number()
}

/// Optional number: `default` when missing, otherwise coerced (an explicit empty is 0).
pub(crate) fn opt_num(args: &[Arg], i: usize, default: f64) -> R<f64> {
    match args.get(i) {
        None => Ok(default),
        Some(a) => scalar(a).to_number(),
    }
}

/// Integer coercion (truncates toward zero). Values beyond ±2^53 give `#NUM!`.
pub(crate) fn int(args: &[Arg], i: usize) -> R<i64> {
    to_int(num(args, i)?)
}

pub(crate) fn opt_int(args: &[Arg], i: usize, default: i64) -> R<i64> {
    match args.get(i) {
        None => Ok(default),
        Some(_) => int(args, i),
    }
}

pub(crate) fn to_int(n: f64) -> R<i64> {
    if !n.is_finite() || n.abs() > 9.007_199_254_740_992e15 {
        return Err(CellError::Num);
    }
    Ok(n.trunc() as i64)
}

/// Text coercion for a scalar parameter.
pub(crate) fn text(args: &[Arg], i: usize) -> R<String> {
    let a = arg(args, i)?;
    scalar(a).to_text()
}

/// Boolean coercion for a scalar parameter.
pub(crate) fn boolean(args: &[Arg], i: usize) -> R<bool> {
    let a = arg(args, i)?;
    scalar(a).to_bool()
}

pub(crate) fn opt_bool(args: &[Arg], i: usize, default: bool) -> R<bool> {
    match args.get(i) {
        None => Ok(default),
        Some(a) => scalar(a).to_bool(),
    }
}

/// The value viewed as an array (scalars become 1×1).
pub(crate) fn as_array(v: &Value) -> Cow<'_, Array> {
    match v {
        Value::Array(a) => Cow::Borrowed(a.as_ref()),
        other => Cow::Owned(Array::scalar(other.clone())),
    }
}

/// Wraps an array result. Rejects empty and oversized arrays.
pub(crate) fn array_val(rows: usize, cols: usize, data: Vec<Value>) -> R<Value> {
    if rows.checked_mul(cols).is_none_or(|n| n > MAX_CELLS) {
        return Err(CellError::Num);
    }
    Array::new(rows, cols, data).map(Value::from).ok_or(CellError::Calc)
}

/// Checks that a requested result size is acceptable.
pub(crate) fn check_size(rows: f64, cols: f64) -> R<(usize, usize)> {
    if rows.is_nan() || cols.is_nan() || rows < 1.0 || cols < 1.0 || !rows.is_finite() || !cols.is_finite() {
        return Err(CellError::Value);
    }
    let (r, c) = (rows as usize, cols as usize);
    if r > 1_048_576 || c > 16_384 || r.saturating_mul(c) > MAX_CELLS {
        return Err(CellError::Num);
    }
    Ok((r, c))
}

/// Every value of every argument, flattened row-major (scalars as themselves).
pub(crate) fn flatten(args: &[Arg]) -> Vec<Value> {
    let mut out = Vec::new();
    for a in args {
        match &a.value {
            Value::Array(arr) => out.extend(arr.data.iter().cloned()),
            v => out.push(v.clone()),
        }
    }
    out
}

/// Numbers for aggregate functions (SUM, AVERAGE, MIN, STDEV…): inside references and arrays
/// only numbers count (text, booleans and blanks are skipped); direct scalar arguments are
/// coerced ("3" → 3, TRUE → 1, other text → `#VALUE!`). Errors propagate.
pub(crate) fn visit_numbers(args: &[Arg], mut accept: impl FnMut(f64)) -> R<()> {
    for arg in args {
        let (cells, reference) = match &arg.value {
            Value::Array(array) => (array.data.as_slice(), true),
            scalar => (std::slice::from_ref(scalar), arg.from_ref),
        };
        for cell in cells {
            match cell {
                Value::Number(number) => accept(*number),
                Value::Error(error) => return Err(*error),
                Value::Empty => {}
                _ if reference => {}
                literal => accept(literal.to_number()?),
            }
        }
    }
    Ok(())
}

pub(crate) fn numbers(args: &[Arg]) -> R<Vec<f64>> {
    let mut collected = Vec::new();
    visit_numbers(args, |n| collected.push(n))?;
    Ok(collected)
}

/// Fold in spreadsheet argument and row order, using constant scratch space.
pub(crate) fn fold_numbers(args: &[Arg], initial: f64, combine: impl Fn(f64, f64) -> f64) -> R<(usize, f64)> {
    let mut result = initial;
    let mut count = 0;
    visit_numbers(args, |n| {
        result = combine(result, n);
        count += 1;
    })?;
    Ok((count, result))
}

/// Numbers for the "A" aggregates (AVERAGEA, MAXA, STDEVA…): inside references and arrays text
/// counts as 0 and booleans as 1/0, blanks are skipped; direct arguments coerce as in
/// [`numbers`].
pub(crate) fn numbers_a(args: &[Arg]) -> R<Vec<f64>> {
    let mut out = Vec::new();
    for a in args {
        match &a.value {
            Value::Array(arr) => {
                for v in arr.iter() {
                    match v {
                        Value::Number(n) => out.push(*n),
                        Value::Bool(b) => out.push(if *b { 1.0 } else { 0.0 }),
                        Value::Text(_) => out.push(0.0),
                        Value::Error(e) => return Err(*e),
                        _ => {}
                    }
                }
            }
            Value::Error(e) => return Err(*e),
            Value::Empty => {}
            Value::Text(_) if a.from_ref => out.push(0.0),
            v => out.push(v.to_number()?),
        }
    }
    Ok(out)
}

/// Numbers of a single array-like argument where only numbers count (text, booleans, blanks
/// skipped), errors propagate. Used for data arrays of statistical functions.
pub(crate) fn array_numbers(v: &Value) -> R<Vec<f64>> {
    let mut out = Vec::new();
    match v {
        Value::Array(arr) => {
            for x in arr.iter() {
                match x {
                    Value::Number(n) => out.push(*n),
                    Value::Error(e) => return Err(*e),
                    _ => {}
                }
            }
        }
        Value::Number(n) => out.push(*n),
        Value::Error(e) => return Err(*e),
        Value::Empty => {}
        other => out.push(other.to_number()?),
    }
    Ok(out)
}

/// Rounds half away from zero at `digits` decimals using the 15-significant-digit decimal
/// representation of `x`, so ROUND(2.675, 2) = 2.68 like Excel.
pub(crate) fn round_half_away(x: f64, digits: i32) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let digits = digits.clamp(-400, 400) as i64;
    let s = format!("{:.14e}", x.abs());
    let Some((mant, exp)) = s.split_once('e') else { return x };
    let Ok(e) = exp.parse::<i64>() else { return x };
    let mut d: Vec<u8> = mant.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    // value = 0.d1d2…d15 × 10^(e+1); keep the first `keep` digits.
    let keep = e + 1 + digits;
    if keep >= d.len() as i64 {
        // Nothing to round off, but Excel still keeps only 15 significant digits, which drops binary noise
        // such as 0.1 + 0.2 = 0.30000000000000004.
        return to_sig_digits(x, d.len());
    }
    if keep < 0 {
        return 0.0;
    }
    let keep = keep as usize;
    let round_up = d.get(keep).is_some_and(|&v| v >= 5);
    d.truncate(keep);
    let mut exp10 = e + 1;
    if round_up {
        let mut i = keep;
        loop {
            if i == 0 {
                d.insert(0, 1);
                exp10 += 1;
                break;
            }
            i -= 1;
            if let Some(v) = d.get_mut(i) {
                if *v == 9 {
                    *v = 0;
                } else {
                    *v += 1;
                    break;
                }
            }
        }
    }
    if d.is_empty() {
        return 0.0;
    }
    let digits_str: String = d.iter().map(|v| char::from(b'0' + v)).collect();
    let text = format!("0.{digits_str}e{exp10}");
    let r: f64 = text.parse().unwrap_or(x);
    if x < 0.0 { -r } else { r }
}

/// `x` rounded to `sig` significant decimal digits (via decimal formatting).
pub(crate) fn to_sig_digits(x: f64, sig: usize) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let s = format!("{:.*e}", sig.saturating_sub(1).min(30), x);
    s.parse().unwrap_or(x)
}

#[cfg(test)]
#[path = "tests/util_testutil.rs"]
pub(crate) mod testutil;

#[cfg(test)]
#[path = "tests/util_tests.rs"]
mod tests;
