//! Engineering functions: base conversion, bit operations, unit conversion, error function,
//! complex numbers and Bessel functions.

use std::f64::consts::PI;

use joysheet_core::{CellError, Value, number_to_text};

use crate::special::{erf, erfc};
use crate::util::{A, R, S, flatten, has, num, num_val, opt_num, scalar, text, text_val};
use crate::{Arg, Ctx, FnSpec, VAR};

// ---------------------------------------------------------------------------------------------
// Base conversion

/// Source text of a base-conversion argument (numbers are written out as integers).
fn base_source(a: &[Arg]) -> R<String> {
    let v = a.first().map(scalar).unwrap_or(Value::Empty);
    match v {
        Value::Error(e) => Err(e),
        Value::Bool(_) => Err(CellError::Value),
        Value::Number(n) => {
            if n < 0.0 || n != n.trunc() {
                return Err(CellError::Num);
            }
            Ok(number_to_text(n))
        }
        Value::Empty => Ok(String::new()),
        v => Ok(v.to_text()?.trim().to_string()),
    }
}

/// Parses a 10-digit two's-complement number in `radix`.
fn parse_base(s: &str, radix: u32) -> R<i64> {
    if s.len() > 10 {
        return Err(CellError::Num);
    }
    if s.is_empty() {
        return Ok(0);
    }
    let v = i64::from_str_radix(s, radix).map_err(|_| CellError::Num)?;
    if s.starts_with(['+', '-']) {
        return Err(CellError::Num);
    }
    let bits = match radix {
        2 => 10,
        8 => 30,
        _ => 40,
    };
    // A full 10-digit number with the top bit set is negative.
    if s.len() == 10 && v >= 1i64 << (bits - 1) { Ok(v - (1i64 << bits)) } else { Ok(v) }
}

/// Writes `v` in `radix`, using 10-digit two's complement for negatives and padding to `places`.
fn format_base(v: i64, radix: u32, places: Option<f64>) -> R<Value> {
    let bits: u32 = match radix {
        2 => 10,
        8 => 30,
        _ => 40,
    };
    let min = -(1i64 << (bits - 1));
    let max = (1i64 << (bits - 1)) - 1;
    if v < min || v > max {
        return Err(CellError::Num);
    }
    if v < 0 {
        let u = v + (1i64 << bits);
        return text_val(to_radix(u, radix));
    }
    let s = to_radix(v, radix);
    match places {
        None => text_val(s),
        Some(p) => {
            let p = p.trunc();
            if !(1.0..=10.0).contains(&p) {
                return Err(CellError::Num);
            }
            let p = p as usize;
            if s.len() > p {
                return Err(CellError::Num);
            }
            text_val(format!("{}{}", "0".repeat(p - s.len()), s))
        }
    }
}

fn to_radix(mut v: i64, radix: u32) -> String {
    if v == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while v > 0 {
        let d = (v % radix as i64) as u32;
        out.push(char::from_digit(d, radix).unwrap_or('0').to_ascii_uppercase());
        v /= radix as i64;
    }
    out.iter().rev().collect()
}

fn places_arg(a: &[Arg]) -> R<Option<f64>> {
    if has(a, 1) { Ok(Some(num(a, 1)?)) } else { Ok(None) }
}

fn convert_base(a: &[Arg], from: u32, to: u32) -> R<Value> {
    let s = base_source(a)?;
    let v = parse_base(&s, from)?;
    let places = places_arg(a)?;
    if to == 10 {
        return Ok(Value::Number(v as f64));
    }
    format_base(v, to, places)
}

fn dec_to(a: &[Arg], to: u32) -> R<Value> {
    let v = a.first().map(scalar).unwrap_or(Value::Empty);
    if matches!(v, Value::Bool(_)) {
        return Err(CellError::Value);
    }
    let n = v.to_number()?.trunc();
    if n.abs() > 1e15 {
        return Err(CellError::Num);
    }
    format_base(n as i64, to, places_arg(a)?)
}

macro_rules! base_fn {
    ($name:ident, $from:expr, $to:expr) => {
        fn $name(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
            convert_base(a, $from, $to)
        }
    };
}
base_fn!(bin2dec, 2, 10);
base_fn!(bin2hex, 2, 16);
base_fn!(bin2oct, 2, 8);
base_fn!(hex2bin, 16, 2);
base_fn!(hex2dec, 16, 10);
base_fn!(hex2oct, 16, 8);
base_fn!(oct2bin, 8, 2);
base_fn!(oct2dec, 8, 10);
base_fn!(oct2hex, 8, 16);

fn dec2bin(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    dec_to(a, 2)
}
fn dec2oct(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    dec_to(a, 8)
}
fn dec2hex(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    dec_to(a, 16)
}

// ---------------------------------------------------------------------------------------------
// Bits

const BIT_LIMIT: f64 = 281_474_976_710_656.0; // 2^48

fn bit_operand(a: &[Arg], i: usize) -> R<u64> {
    let n = num(a, i)?;
    if !(0.0..BIT_LIMIT).contains(&n) || n != n.trunc() {
        return Err(CellError::Num);
    }
    Ok(n as u64)
}

fn bitand(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Number((bit_operand(a, 0)? & bit_operand(a, 1)?) as f64))
}
fn bitor(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Number((bit_operand(a, 0)? | bit_operand(a, 1)?) as f64))
}
fn bitxor(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Number((bit_operand(a, 0)? ^ bit_operand(a, 1)?) as f64))
}

fn shift(a: &[Arg], left: bool) -> R<Value> {
    let n = bit_operand(a, 0)?;
    let s = num(a, 1)?.trunc();
    if s.abs() > 53.0 {
        return Err(CellError::Num);
    }
    let s = if left { s as i64 } else { -(s as i64) };
    let r = if s >= 0 {
        let r = (n as u128) << s;
        if r >= BIT_LIMIT as u128 {
            return Err(CellError::Num);
        }
        r as u64
    } else {
        n >> (-s)
    };
    Ok(Value::Number(r as f64))
}
fn bitlshift(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    shift(a, true)
}
fn bitrshift(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    shift(a, false)
}

// ---------------------------------------------------------------------------------------------
// DELTA, GESTEP, ERF

fn delta(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = num(a, 0)?;
    let y = opt_num(a, 1, 0.0)?;
    Ok(Value::Number(if x == y { 1.0 } else { 0.0 }))
}
fn gestep(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = num(a, 0)?;
    let y = opt_num(a, 1, 0.0)?;
    Ok(Value::Number(if x >= y { 1.0 } else { 0.0 }))
}
fn erf_fn(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let lo = num(a, 0)?;
    if has(a, 1) {
        let hi = num(a, 1)?;
        return num_val(erf(hi) - erf(lo));
    }
    num_val(erf(lo))
}
fn erf_precise(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    num_val(erf(num(a, 0)?))
}
fn erfc_fn(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    num_val(erfc(num(a, 0)?))
}

macro_rules! im1 {
    ($name:ident, $f:expr) => {
        fn $name(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
            im_unary(a, $f)
        }
    };
}

mod bessel;
mod complex;
mod units;

use bessel::{besseli, besselj, besselk, bessely};
#[cfg(test)]
use complex::parse_complex;
use complex::{Cx, ONE, complex, im_unary, imabs, imaginary, imargument, imdiv, impower, improduct, imreal, imsub, imsum};
use units::convert;

im1!(imconjugate, |z: Cx| Ok(Cx::new(z.re, -z.im)));
im1!(imexp, |z: Cx| Ok(z.exp()));
im1!(imln, |z: Cx| z.ln());
im1!(imlog10, |z: Cx| z.ln().map(|l| Cx::new(l.re / std::f64::consts::LN_10, l.im / std::f64::consts::LN_10)));
im1!(imlog2, |z: Cx| z.ln().map(|l| Cx::new(l.re / std::f64::consts::LN_2, l.im / std::f64::consts::LN_2)));
im1!(imsqrt, |z: Cx| {
    let m = z.abs();
    let re = ((m + z.re) / 2.0).sqrt();
    let im = ((m - z.re) / 2.0).sqrt();
    Ok(Cx::new(re, if z.im < 0.0 { -im } else { im }))
});
im1!(imsin, |z: Cx| Ok(z.sin()));
im1!(imcos, |z: Cx| Ok(z.cos()));
im1!(imtan, |z: Cx| z.sin().div(z.cos()));
im1!(imcot, |z: Cx| z.cos().div(z.sin()));
im1!(imsec, |z: Cx| ONE.div(z.cos()));
im1!(imcsc, |z: Cx| ONE.div(z.sin()));
im1!(imsinh, |z: Cx| Ok(z.sinh()));
im1!(imcosh, |z: Cx| Ok(z.cosh()));
im1!(imsech, |z: Cx| ONE.div(z.cosh()));
im1!(imcsch, |z: Cx| ONE.div(z.sinh()));

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!("BIN2DEC", 1, 1, Engineering, S, "BIN2DEC(number)", "Converts a binary number to decimal.", bin2dec),
        f!("BIN2HEX", 1, 2, Engineering, S, "BIN2HEX(number, [places])", "Converts a binary number to hexadecimal.", bin2hex),
        f!("BIN2OCT", 1, 2, Engineering, S, "BIN2OCT(number, [places])", "Converts a binary number to octal.", bin2oct),
        f!("DEC2BIN", 1, 2, Engineering, S, "DEC2BIN(number, [places])", "Converts a decimal number to binary.", dec2bin),
        f!("DEC2HEX", 1, 2, Engineering, S, "DEC2HEX(number, [places])", "Converts a decimal number to hexadecimal.", dec2hex),
        f!("DEC2OCT", 1, 2, Engineering, S, "DEC2OCT(number, [places])", "Converts a decimal number to octal.", dec2oct),
        f!("HEX2BIN", 1, 2, Engineering, S, "HEX2BIN(number, [places])", "Converts a hexadecimal number to binary.", hex2bin),
        f!("HEX2DEC", 1, 1, Engineering, S, "HEX2DEC(number)", "Converts a hexadecimal number to decimal.", hex2dec),
        f!("HEX2OCT", 1, 2, Engineering, S, "HEX2OCT(number, [places])", "Converts a hexadecimal number to octal.", hex2oct),
        f!("OCT2BIN", 1, 2, Engineering, S, "OCT2BIN(number, [places])", "Converts an octal number to binary.", oct2bin),
        f!("OCT2DEC", 1, 1, Engineering, S, "OCT2DEC(number)", "Converts an octal number to decimal.", oct2dec),
        f!("OCT2HEX", 1, 2, Engineering, S, "OCT2HEX(number, [places])", "Converts an octal number to hexadecimal.", oct2hex),
        f!("BITAND", 2, 2, Engineering, S, "BITAND(number1, number2)", "Bitwise AND of two non-negative integers.", bitand),
        f!("BITOR", 2, 2, Engineering, S, "BITOR(number1, number2)", "Bitwise OR of two non-negative integers.", bitor),
        f!("BITXOR", 2, 2, Engineering, S, "BITXOR(number1, number2)", "Bitwise exclusive OR of two non-negative integers.", bitxor),
        f!(
            "BITLSHIFT",
            2,
            2,
            Engineering,
            S,
            "BITLSHIFT(number, shift_amount)",
            "Shifts the bits of a number left (right for negative shifts).",
            bitlshift
        ),
        f!(
            "BITRSHIFT",
            2,
            2,
            Engineering,
            S,
            "BITRSHIFT(number, shift_amount)",
            "Shifts the bits of a number right (left for negative shifts).",
            bitrshift
        ),
        f!("CONVERT", 3, 3, Engineering, S, "CONVERT(number, from_unit, to_unit)", "Converts a measurement from one unit to another.", convert),
        f!("DELTA", 1, 2, Engineering, S, "DELTA(number1, [number2])", "Returns 1 when two numbers are equal, otherwise 0.", delta),
        f!("GESTEP", 1, 2, Engineering, S, "GESTEP(number, [step])", "Returns 1 when a number is at least the step, otherwise 0.", gestep),
        f!(
            "ERF",
            1,
            2,
            Engineering,
            S,
            "ERF(lower_limit, [upper_limit])",
            "Error function integrated from 0 to a limit, or between two limits.",
            erf_fn
        ),
        f!("ERF.PRECISE", 1, 1, Engineering, S, "ERF.PRECISE(x)", "Error function integrated from 0 to x.", erf_precise),
        f!("ERFC", 1, 1, Engineering, S, "ERFC(x)", "Complementary error function, 1 - ERF(x).", erfc_fn),
        f!("ERFC.PRECISE", 1, 1, Engineering, S, "ERFC.PRECISE(x)", "Complementary error function, 1 - ERF(x).", erfc_fn),
        f!(
            "COMPLEX",
            2,
            3,
            Engineering,
            S,
            "COMPLEX(real_num, i_num, [suffix])",
            "Builds a complex number as text from real and imaginary parts.",
            complex
        ),
        f!("IMABS", 1, 1, Engineering, S, "IMABS(inumber)", "Absolute value (modulus) of a complex number.", imabs),
        f!("IMAGINARY", 1, 1, Engineering, S, "IMAGINARY(inumber)", "Imaginary coefficient of a complex number.", imaginary),
        f!("IMREAL", 1, 1, Engineering, S, "IMREAL(inumber)", "Real coefficient of a complex number.", imreal),
        f!("IMARGUMENT", 1, 1, Engineering, S, "IMARGUMENT(inumber)", "Angle of a complex number in radians.", imargument),
        f!("IMCONJUGATE", 1, 1, Engineering, S, "IMCONJUGATE(inumber)", "Complex conjugate of a complex number.", imconjugate),
        f!("IMCOS", 1, 1, Engineering, S, "IMCOS(inumber)", "Cosine of a complex number.", imcos),
        f!("IMCOSH", 1, 1, Engineering, S, "IMCOSH(inumber)", "Hyperbolic cosine of a complex number.", imcosh),
        f!("IMCOT", 1, 1, Engineering, S, "IMCOT(inumber)", "Cotangent of a complex number.", imcot),
        f!("IMCSC", 1, 1, Engineering, S, "IMCSC(inumber)", "Cosecant of a complex number.", imcsc),
        f!("IMCSCH", 1, 1, Engineering, S, "IMCSCH(inumber)", "Hyperbolic cosecant of a complex number.", imcsch),
        f!("IMDIV", 2, 2, Engineering, S, "IMDIV(inumber1, inumber2)", "Quotient of two complex numbers.", imdiv),
        f!("IMEXP", 1, 1, Engineering, S, "IMEXP(inumber)", "Exponential of a complex number.", imexp),
        f!("IMLN", 1, 1, Engineering, S, "IMLN(inumber)", "Natural logarithm of a complex number.", imln),
        f!("IMLOG10", 1, 1, Engineering, S, "IMLOG10(inumber)", "Base-10 logarithm of a complex number.", imlog10),
        f!("IMLOG2", 1, 1, Engineering, S, "IMLOG2(inumber)", "Base-2 logarithm of a complex number.", imlog2),
        f!("IMPOWER", 2, 2, Engineering, S, "IMPOWER(inumber, number)", "Raises a complex number to a power.", impower),
        f!("IMPRODUCT", 1, VAR, Engineering, A, "IMPRODUCT(inumber1, [inumber2], ...)", "Product of complex numbers.", improduct),
        f!("IMSEC", 1, 1, Engineering, S, "IMSEC(inumber)", "Secant of a complex number.", imsec),
        f!("IMSECH", 1, 1, Engineering, S, "IMSECH(inumber)", "Hyperbolic secant of a complex number.", imsech),
        f!("IMSIN", 1, 1, Engineering, S, "IMSIN(inumber)", "Sine of a complex number.", imsin),
        f!("IMSINH", 1, 1, Engineering, S, "IMSINH(inumber)", "Hyperbolic sine of a complex number.", imsinh),
        f!("IMSQRT", 1, 1, Engineering, S, "IMSQRT(inumber)", "Square root of a complex number.", imsqrt),
        f!("IMSUB", 2, 2, Engineering, S, "IMSUB(inumber1, inumber2)", "Difference of two complex numbers.", imsub),
        f!("IMSUM", 1, VAR, Engineering, A, "IMSUM(inumber1, [inumber2], ...)", "Sum of complex numbers.", imsum),
        f!("IMTAN", 1, 1, Engineering, S, "IMTAN(inumber)", "Tangent of a complex number.", imtan),
        f!("BESSELJ", 2, 2, Engineering, S, "BESSELJ(x, n)", "Bessel function of the first kind, J_n(x).", besselj),
        f!("BESSELY", 2, 2, Engineering, S, "BESSELY(x, n)", "Bessel function of the second kind, Y_n(x).", bessely),
        f!("BESSELI", 2, 2, Engineering, S, "BESSELI(x, n)", "Modified Bessel function of the first kind, I_n(x).", besseli),
        f!("BESSELK", 2, 2, Engineering, S, "BESSELK(x, n)", "Modified Bessel function of the second kind, K_n(x).", besselk),
    ]
}

#[cfg(test)]
#[path = "tests/engineering_tests.rs"]
mod tests;
