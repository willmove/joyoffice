//! Information, logical, web and cube functions.

use std::cmp::Ordering;

use joysheet_core::broadcast::dimensions;
use joysheet_core::{Array, CellError, Value, compare_numbers, compare_text};

use crate::util::{A, MAX_CELLS, R, S, has, scalar, text};
use crate::value_info::{self, BLANK, BOOLEAN, NA, NUMBER, OTHER_ERROR, TEXT, classify};
use crate::{Arg, Ctx, FnSpec, VAR};

// ---------------------------------------------------------------------------------------------
// Information

fn first(a: &[Arg]) -> Value {
    a.first().map(scalar).unwrap_or(Value::Empty)
}

fn info(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let t = text(a, 0)?.to_ascii_lowercase();
    let v: Value = match t.as_str() {
        "directory" => "/".into(),
        "numfile" => Value::Number(1.0),
        "origin" => "$A:$A$1".into(),
        "osversion" => "GridCraft".into(),
        "recalc" => "Automatic".into(),
        "release" => "16.0".into(),
        "system" => "pcdos".into(),
        _ => return Err(CellError::Value),
    };
    Ok(v)
}

// ---------------------------------------------------------------------------------------------
// Logical

/// AND, OR and XOR consume the range in row order, including errors after a decisive value.
/// Only scalar arguments coerce text or blanks. Keep a single bit of reduction state.
fn reduce_logical(args: &[Arg], combine: impl Fn(bool, bool) -> bool) -> R<Value> {
    let mut result = None;
    let mut accept = |value| result = Some(result.map_or(value, |previous| combine(previous, value)));
    for arg in args {
        let (values, referenced) = match &arg.value {
            Value::Array(array) => (array.data.as_slice(), true),
            value => (std::slice::from_ref(value), arg.from_ref),
        };
        for value in values {
            match value {
                Value::Text(_) | Value::Empty | Value::Array(_) if referenced => continue,
                value => accept(value.to_bool()?),
            }
        }
    }
    result.map(Value::Bool).ok_or(CellError::Value)
}

fn and(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    reduce_logical(a, |a, b| a && b)
}
fn or(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    reduce_logical(a, |a, b| a || b)
}
fn xor(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    reduce_logical(a, |a, b| a ^ b)
}
fn not(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Bool(!first(a).to_bool()?))
}

/// Applies `f` to scalar argument values. When any argument at a `drivers` position is an
/// array, `f` runs per element over the broadcast shape of all arguments; otherwise it receives
/// the arguments whole (so the chosen value may itself be an array).
fn drive(a: &[Arg], drivers: &dyn Fn(usize) -> bool, f: &dyn Fn(&[&Value]) -> R<Value>) -> R<Value> {
    let any = a.iter().enumerate().any(|(i, x)| drivers(i) && matches!(x.value, Value::Array(_)));
    if !any {
        let vals: Vec<_> = a.iter().map(|arg| &arg.value).collect();
        return f(&vals).map(empty_to_zero);
    }
    let (rows, cols, count) = dimensions(a.iter().map(|arg| arg.value.array_dimensions()), MAX_CELLS).ok_or(CellError::Num)?;
    let mut data = Vec::with_capacity(count);
    let mut vals = Vec::with_capacity(a.len());
    let mut input_rows = Vec::with_capacity(a.len());
    for r in 0..rows {
        input_rows.clear();
        input_rows.extend(a.iter().map(|arg| arg.value.broadcast_row(r)));
        for c in 0..cols {
            vals.clear();
            vals.extend(input_rows.iter().map(|row| row.cell(c)));
            let out = match f(&vals) {
                Ok(Value::Array(arr)) => arr.data.first().cloned().unwrap_or(Value::Empty),
                Ok(v) => v,
                Err(e) => Value::Error(e),
            };
            data.push(out);
        }
    }
    Array::new(rows, cols, data).map(Value::from).ok_or(CellError::Calc)
}

fn empty_to_zero(v: Value) -> Value {
    if matches!(v, Value::Empty) { Value::Number(0.0) } else { v }
}

fn if_fn(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    drive(a, &|i| i == 0, &|v| {
        let cond = v.first().copied().unwrap_or(&Value::Empty).to_bool()?;
        if cond { Ok(v.get(1).copied().cloned().unwrap_or(Value::Bool(true))) } else { Ok(v.get(2).copied().cloned().unwrap_or(Value::Bool(false))) }
    })
}

fn ifs(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    if !a.len().is_multiple_of(2) {
        return Err(CellError::NA);
    }
    drive(a, &|i| i % 2 == 0, &|v| {
        for pair in v.chunks(2) {
            if let [c, r] = pair
                && c.to_bool()?
            {
                return Ok((*r).clone());
            }
        }
        Err(CellError::NA)
    })
}

fn iferror(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    drive(a, &|i| i == 0, &|v| match v.first() {
        Some(Value::Error(_)) => Ok(v.get(1).copied().cloned().unwrap_or(Value::Empty)),
        Some(x) => Ok((*x).clone()),
        None => Err(CellError::Value),
    })
}

fn ifna(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    drive(a, &|i| i == 0, &|v| match v.first() {
        Some(Value::Error(CellError::NA)) => Ok(v.get(1).copied().cloned().unwrap_or(Value::Empty)),
        Some(x) => Ok((*x).clone()),
        None => Err(CellError::Value),
    })
}

fn switch_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Text(x), Value::Text(y)) => compare_text(x, y) == Ordering::Equal,
        (Value::Number(x), Value::Number(y)) => compare_numbers(*x, *y) == Ordering::Equal,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Empty, Value::Empty) => true,
        (Value::Empty, Value::Number(x)) | (Value::Number(x), Value::Empty) => *x == 0.0,
        (Value::Empty, Value::Text(t)) | (Value::Text(t), Value::Empty) => t.is_empty(),
        _ => false,
    }
}

fn switch(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    drive(a, &|i| i == 0 || (i % 2 == 1 && i + 1 < a.len()), &|v| {
        let Some((expr, rest)) = v.split_first() else { return Err(CellError::Value) };
        if let Value::Error(e) = expr {
            return Err(*e);
        }
        let mut it = rest.chunks(2);
        for pair in it.by_ref() {
            match pair {
                [case, result] => {
                    if let Value::Error(e) = case {
                        return Err(*e);
                    }
                    if switch_equal(expr, case) {
                        return Ok((*result).clone());
                    }
                }
                [default] => return Ok((*default).clone()),
                _ => {}
            }
        }
        Err(CellError::NA)
    })
}

fn choose(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    drive(a, &|i| i == 0, &|v| {
        let idx = v.first().copied().unwrap_or(&Value::Empty).to_number()?.trunc();
        if idx < 1.0 || idx >= v.len() as f64 {
            return Err(CellError::Value);
        }
        v.get(idx as usize).copied().cloned().ok_or(CellError::Value)
    })
}

fn true_fn(_a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Bool(true))
}
fn false_fn(_a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Bool(false))
}

// ---------------------------------------------------------------------------------------------
// Web and cube

fn hyperlink(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    if has(a, 1) {
        let v = a.get(1).map(scalar).unwrap_or(Value::Empty);
        return Ok(empty_to_zero(v));
    }
    let link = a.first().map(scalar).unwrap_or(Value::Empty);
    if let Value::Error(e) = link {
        return Err(e);
    }
    Ok(link)
}

fn encodeurl(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    Ok(Value::from(out))
}

fn value_err(_a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Err(CellError::Value)
}

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!("ISBLANK", 1, 1, Information, S, "ISBLANK(value)", "TRUE when the value refers to an empty cell.", classify::<BLANK>),
        f!("ISERR", 1, 1, Information, S, "ISERR(value)", "TRUE when the value is any error other than #N/A.", classify::<OTHER_ERROR>),
        f!("ISERROR", 1, 1, Information, S, "ISERROR(value)", "TRUE when the value is any error.", classify::<{ NA | OTHER_ERROR }>),
        f!("ISNA", 1, 1, Information, S, "ISNA(value)", "TRUE when the value is the #N/A error.", classify::<NA>),
        f!("ISNUMBER", 1, 1, Information, S, "ISNUMBER(value)", "TRUE when the value is a number.", classify::<NUMBER>),
        f!("ISTEXT", 1, 1, Information, S, "ISTEXT(value)", "TRUE when the value is text.", classify::<TEXT>),
        f!("ISNONTEXT", 1, 1, Information, S, "ISNONTEXT(value)", "TRUE when the value is anything other than text.", classify::<{ !TEXT }>),
        f!("ISLOGICAL", 1, 1, Information, S, "ISLOGICAL(value)", "TRUE when the value is TRUE or FALSE.", classify::<BOOLEAN>),
        f!("ISEVEN", 1, 1, Information, S, "ISEVEN(number)", "TRUE when the integer part of a number is even.", value_info::parity::<false>),
        f!("ISODD", 1, 1, Information, S, "ISODD(number)", "TRUE when the integer part of a number is odd.", value_info::parity::<true>),
        f!(
            "TYPE",
            1,
            1,
            Information,
            A,
            "TYPE(value)",
            "Returns a code for the kind of value: 1 number, 2 text, 4 logical, 16 error, 64 array.",
            value_info::type_code
        ),
        f!(
            "ERROR.TYPE",
            1,
            1,
            Information,
            S,
            "ERROR.TYPE(error_val)",
            "Returns the number identifying an error value, or #N/A if it is not an error.",
            value_info::error_code
        ),
        f!("NA", 0, 0, Information, S, "NA()", "Returns the #N/A error value.", value_info::not_available),
        f!(
            "N",
            1,
            1,
            Information,
            A,
            "N(value)",
            "Converts a value to a number: numbers stay, TRUE is 1, everything else is 0.",
            value_info::numeric
        ),
        f!("INFO", 1, 1, Information, S, "INFO(type_text)", "Returns information about the operating environment.", info),
        f!("AND", 1, VAR, Logical, A, "AND(logical1, [logical2], ...)", "TRUE when every argument is TRUE.", and),
        f!("OR", 1, VAR, Logical, A, "OR(logical1, [logical2], ...)", "TRUE when at least one argument is TRUE.", or),
        f!("XOR", 1, VAR, Logical, A, "XOR(logical1, [logical2], ...)", "TRUE when an odd number of arguments are TRUE.", xor),
        f!("NOT", 1, 1, Logical, S, "NOT(logical)", "Reverses a logical value.", not),
        f!("TRUE", 0, 0, Logical, S, "TRUE()", "Returns the logical value TRUE.", true_fn),
        f!("FALSE", 0, 0, Logical, S, "FALSE()", "Returns the logical value FALSE.", false_fn),
        f!(
            "IF",
            2,
            3,
            Logical,
            A,
            "IF(logical_test, [value_if_true], [value_if_false])",
            "Returns one value when a condition is TRUE and another when it is FALSE.",
            if_fn
        ),
        f!(
            "IFS",
            2,
            VAR,
            Logical,
            A,
            "IFS(logical_test1, value_if_true1, ...)",
            "Returns the value paired with the first condition that is TRUE.",
            ifs
        ),
        f!(
            "IFERROR",
            2,
            2,
            Logical,
            A,
            "IFERROR(value, value_if_error)",
            "Returns a fallback when the value is an error, otherwise the value.",
            iferror
        ),
        f!("IFNA", 2, 2, Logical, A, "IFNA(value, value_if_na)", "Returns a fallback when the value is #N/A, otherwise the value.", ifna),
        f!(
            "SWITCH",
            3,
            VAR,
            Logical,
            A,
            "SWITCH(expression, value1, result1, [default_or_value2, result2], ...)",
            "Compares an expression with a list of values and returns the result paired with the first match.",
            switch
        ),
        f!("CHOOSE", 2, VAR, Lookup, A, "CHOOSE(index_num, value1, [value2], ...)", "Picks a value from the list by its position.", choose),
        f!(
            "HYPERLINK",
            1,
            2,
            Information,
            S,
            "HYPERLINK(link_location, [friendly_name])",
            "Creates a link; the cell shows the friendly name, or the link itself.",
            hyperlink
        ),
        f!("ENCODEURL", 1, 1, Web, S, "ENCODEURL(text)", "Percent-encodes text for use in a URL.", encodeurl),
        f!("WEBSERVICE", 1, 1, Web, S, "WEBSERVICE(url)", "Fetches data from a web service (not available; returns #VALUE!).", value_err),
        f!("FILTERXML", 2, 2, Web, S, "FILTERXML(xml, xpath)", "Extracts data from XML with an XPath (not available; returns #VALUE!).", value_err),
        f!(
            "CUBEKPIMEMBER",
            3,
            4,
            Cube,
            A,
            "CUBEKPIMEMBER(connection, kpi_name, kpi_property, [caption])",
            "Returns a key performance indicator property from a cube (no cube connections; #N/A).",
            value_info::not_available
        ),
        f!(
            "CUBEMEMBER",
            2,
            3,
            Cube,
            A,
            "CUBEMEMBER(connection, member_expression, [caption])",
            "Returns a member or tuple from a cube (no cube connections; #N/A).",
            value_info::not_available
        ),
        f!(
            "CUBEMEMBERPROPERTY",
            3,
            3,
            Cube,
            A,
            "CUBEMEMBERPROPERTY(connection, member_expression, property)",
            "Returns a property of a cube member (no cube connections; #N/A).",
            value_info::not_available
        ),
        f!(
            "CUBERANKEDMEMBER",
            3,
            4,
            Cube,
            A,
            "CUBERANKEDMEMBER(connection, set_expression, rank, [caption])",
            "Returns the nth member of a cube set (no cube connections; #N/A).",
            value_info::not_available
        ),
        f!(
            "CUBESET",
            2,
            5,
            Cube,
            A,
            "CUBESET(connection, set_expression, [caption], [sort_order], [sort_by])",
            "Defines a set of cube members (no cube connections; #N/A).",
            value_info::not_available
        ),
        f!(
            "CUBESETCOUNT",
            1,
            1,
            Cube,
            A,
            "CUBESETCOUNT(set)",
            "Counts the items in a cube set (no cube connections; #N/A).",
            value_info::not_available
        ),
        f!(
            "CUBEVALUE",
            1,
            VAR,
            Cube,
            A,
            "CUBEVALUE(connection, [member_expression1], ...)",
            "Returns an aggregated value from a cube (no cube connections; #N/A).",
            value_info::not_available
        ),
    ]
}

#[cfg(test)]
#[path = "tests/info_tests.rs"]
mod tests;
