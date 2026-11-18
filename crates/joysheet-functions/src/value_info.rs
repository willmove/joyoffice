//! Information functions inspect the stored type, including the distinction
//! between an empty cell and an empty string. Only the parity predicates coerce.
use joysheet_core::{Array, CellError, Value};

use crate::util::{R, number_with_decimal_digits, scalar_argument};
use crate::{Arg, Ctx};

pub(crate) const BLANK: u8 = 1;
pub(crate) const NUMBER: u8 = 2;
pub(crate) const TEXT: u8 = 4;
pub(crate) const BOOLEAN: u8 = 8;
pub(crate) const NA: u8 = 16;
pub(crate) const OTHER_ERROR: u8 = 32;
const ARRAY: u8 = 64;

fn kind(value: &Value) -> u8 {
    match value {
        Value::Empty => BLANK,
        Value::Number(_) => NUMBER,
        Value::Text(_) => TEXT,
        Value::Bool(_) => BOOLEAN,
        Value::Error(CellError::NA) => NA,
        Value::Error(_) => OTHER_ERROR,
        Value::Array(_) => ARRAY,
    }
}

pub(crate) fn classify<const ACCEPTED: u8>(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Bool(kind(scalar_argument(args, 0)?) & ACCEPTED != 0))
}

pub(crate) fn parity<const ODD: bool>(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let n = number_with_decimal_digits(args, 0, false)?;
    if !n.is_finite() {
        return Err(CellError::Num);
    }
    // Beyond 2^53 every representable binary64 integer is even.
    Ok(Value::Bool((n.trunc() % 2. != 0.) == ODD))
}

pub(crate) fn type_code(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let arg = args.first().ok_or(CellError::Value)?;
    let value = match &arg.value {
        Value::Array(a) if arg.from_ref && a.rows == 1 && a.cols == 1 => a.data.first().unwrap_or(&Value::Empty),
        value => value,
    };
    Ok(Value::Number(match kind(value) {
        BLANK | NUMBER => 1.,
        TEXT => 2.,
        BOOLEAN => 4.,
        ARRAY => 64.,
        _ => 16.,
    }))
}

pub(crate) fn error_code(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    match scalar_argument(args, 0)? {
        Value::Error(e) => Ok(Value::Number(f64::from(e.code()))),
        _ => Err(CellError::NA),
    }
}

pub(crate) fn not_available(_: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    Err(CellError::NA)
}

pub(crate) fn numeric(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    fn coerce(value: &Value) -> Value {
        match value {
            Value::Error(e) => Value::Error(*e),
            Value::Number(n) => Value::Number(*n),
            Value::Bool(true) => Value::Number(1.),
            _ => Value::Number(0.),
        }
    }
    if let Some(Arg { value: Value::Array(a), from_ref: false }) = args.first() {
        return Array::new(a.rows, a.cols, a.data.iter().map(coerce).collect()).map(Value::from).ok_or(CellError::Calc);
    }
    Ok(coerce(scalar_argument(args, 0)?))
}
