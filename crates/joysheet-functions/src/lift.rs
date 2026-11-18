//! Automatic array lifting: a function given an array where it expects a scalar is applied to
//! each element, broadcasting single rows/columns like Excel's dynamic arrays.

use joysheet_core::{Array, CellError, Value, broadcast::dimensions};

use crate::util::MAX_CELLS;
use crate::{Arg, Ctx, FnSpec};

/// Value-parameter masks (see [`FnSpec::value_args`]), observed in Excel 365: a reference to more
/// than one cell at these positions gives `#VALUE!` in dynamic-array and legacy formulas alike (no
/// implicit intersection), while array constants and computed arrays lift. Range parameters
/// (holidays, schedules, cash flows, coefficients) and the WORKDAY.INTL/NETWORKDAYS.INTL weekend
/// (which lifts a range) are `false`.
pub(crate) const VALUE_ARGS: &[(&str, &[bool])] = &[
    // Date and time.
    ("EOMONTH EDATE YEARFRAC WEEKNUM", &[true]),
    ("WORKDAY NETWORKDAYS", &[true, true, false]),
    ("WORKDAY.INTL NETWORKDAYS.INTL", &[true, true, false, false]),
    // Financial.
    (
        "ACCRINT ACCRINTM AMORDEGRC AMORLINC COUPDAYBS COUPDAYS COUPDAYSNC COUPNCD COUPNUM COUPPCD CUMIPMT CUMPRINC DISC \
         DOLLARDE DOLLARFR DURATION EFFECT INTRATE MDURATION NOMINAL ODDFPRICE ODDFYIELD ODDLPRICE ODDLYIELD PRICE PRICEDISC \
         PRICEMAT RECEIVED TBILLEQ TBILLPRICE TBILLYIELD YIELD YIELDDISC YIELDMAT",
        &[true],
    ),
    ("FVSCHEDULE XNPV", &[true, false]),
    ("XIRR", &[false, false, true]),
    // Math and information.
    ("ISEVEN ISODD QUOTIENT MROUND FACTDOUBLE SQRTPI RANDBETWEEN", &[true]),
    ("SERIESSUM", &[true, true, true, false]),
    // Engineering (IMSUM and IMPRODUCT aggregate, so they aren't here).
    (
        "BIN2DEC BIN2HEX BIN2OCT DEC2BIN DEC2HEX DEC2OCT HEX2BIN HEX2DEC HEX2OCT OCT2BIN OCT2DEC OCT2HEX CONVERT DELTA GESTEP \
         ERF ERF.PRECISE ERFC ERFC.PRECISE BESSELI BESSELJ BESSELK BESSELY COMPLEX IMABS IMAGINARY IMARGUMENT IMCONJUGATE \
         IMCOS IMCOSH IMCOT IMCSC IMCSCH IMDIV IMEXP IMLN IMLOG10 IMLOG2 IMPOWER IMREAL IMSEC IMSECH IMSIN IMSINH IMSQRT IMSUB \
         IMTAN",
        &[true],
    ),
];

/// The value-parameter mask of `name` (empty for functions without value parameters).
pub(crate) fn value_args(name: &str) -> &'static [bool] {
    VALUE_ARGS.iter().find(|(names, _)| names.split_ascii_whitespace().any(|n| n == name)).map_or(&[], |(_, mask)| mask)
}

/// A multi-cell reference at a value parameter contributes no shape and yields #VALUE!
/// at the anchor. Other arguments can still determine the result dimensions.
fn reject_multi_cell_refs(spec: &FnSpec, args: &[Arg]) -> Option<Vec<Arg>> {
    let multi = |i: usize, a: &Arg| a.from_ref && spec.is_value_arg(i) && matches!(&a.value, Value::Array(arr) if arr.data.len() > 1);
    if !args.iter().enumerate().any(|(i, a)| multi(i, a)) {
        return None;
    }
    Some(
        args.iter()
            .enumerate()
            .map(|(i, a)| if multi(i, a) { Arg { value: Value::Error(CellError::Value), from_ref: false } } else { a.clone() })
            .collect(),
    )
}

pub(crate) fn call_lifted(spec: &FnSpec, args: &[Arg], ctx: &mut dyn Ctx) -> Value {
    let rejected = if spec.value_args.is_empty() { None } else { reject_multi_cell_refs(spec, args) };
    let original = args;
    let args = rejected.as_deref().unwrap_or(original);
    let drivers: Vec<_> = args
        .iter()
        .enumerate()
        .filter_map(|(i, arg)| match &arg.value {
            Value::Array(array) if spec.is_scalar_arg(i) => Some((i, array.as_ref())),
            _ => None,
        })
        .collect();
    if drivers.is_empty() {
        return (spec.imp)(args, ctx);
    }
    let Some((rows, cols, count)) = dimensions(drivers.iter().map(|(_, a)| (a.rows, a.cols)), MAX_CELLS) else { return Value::Error(CellError::Num) };
    let mut cell_args = args.to_vec();
    let mut input_rows = Vec::with_capacity(drivers.len());
    let mut data = Vec::with_capacity(count);
    for r in 0..rows {
        input_rows.clear();
        input_rows.extend(drivers.iter().map(|&(i, arr)| (i, arr.broadcast_row(r))));
        for c in 0..cols {
            for &(i, row) in &input_rows {
                cell_args[i].value = match row.cell(c) {
                    Value::Array(inner) => inner.data.first().cloned().unwrap_or(Value::Empty),
                    v => v.clone(),
                };
            }
            let out = (spec.imp)(&cell_args, ctx);
            // For these functions Excel rejects a multi-cell value reference in the anchor,
            // then uses its top-left value in cells lifted by another argument.
            if r == 0 && c == 0 && rejected.is_some() && matches!(spec.name, "EDATE" | "EOMONTH" | "WORKDAY" | "YEARFRAC" | "DELTA" | "QUOTIENT") {
                for (i, arg) in original.iter().enumerate() {
                    if arg.from_ref
                        && spec.is_value_arg(i)
                        && let Value::Array(arr) = &arg.value
                        && arr.data.len() > 1
                    {
                        cell_args[i].value = arr.data.first().cloned().unwrap_or(Value::Empty);
                    }
                }
            }
            data.push(match out {
                Value::Array(a) => a.data.first().cloned().unwrap_or(Value::Empty),
                v => v,
            });
        }
    }
    if rows == 1 && cols == 1 && !ctx.preserve_singleton_arrays() {
        return data.pop().unwrap_or(Value::Empty);
    }
    Array::new(rows, cols, data).map(Value::from).unwrap_or(Value::Error(CellError::Calc))
}

#[cfg(test)]
#[path = "tests/lift_tests.rs"]
mod tests;
