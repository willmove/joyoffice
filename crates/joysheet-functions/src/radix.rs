//! Integer representations used by the engineering and mathematical worksheet functions.
use joysheet_core::{CellError, Value};

use crate::util::{R, has, num_val, number_with_decimal_digits as number, scalar_argument as input, text_val};
use crate::{Arg, Ctx};

const DIGITS: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const BIT_LIMIT: u64 = 1 << 48;

fn digit(byte: u8, radix: u32) -> R<u32> {
    char::from(byte).to_digit(radix).ok_or(CellError::Num)
}

fn encode(mut value: u64, radix: u32, min_length: usize) -> String {
    let mut buf = [b'0'; 64];
    let mut start = buf.len();
    while value != 0 {
        start -= 1;
        buf[start] = DIGITS[(value % u64::from(radix)) as usize];
        value /= u64::from(radix);
    }
    let digits = &buf[start..];
    let mut output = String::with_capacity(min_length.max(digits.len()));
    output.extend(std::iter::repeat_n('0', min_length.saturating_sub(digits.len())));
    output.extend(digits.iter().copied().map(char::from));
    output
}

fn signed_value(args: &[Arg], radix: u32) -> R<i64> {
    let value = input(args, 0)?;
    let mut word = 0;
    if let Value::Number(n) = value {
        if !(0.0..1e10).contains(n) || n.fract() != 0. {
            return Err(CellError::Num);
        }
        // Numeric cell values already hold the decimal digits of the source
        // word exactly. Read them directly without formatting a float.
        let mut input = *n as u64;
        let mut place = 1;
        while input != 0 {
            let d = (input % 10) as u32;
            if d >= radix {
                return Err(CellError::Num);
            }
            word += i64::from(d) * place;
            place *= i64::from(radix);
            input /= 10;
        }
    } else {
        let source = match value {
            Value::Text(s) => s.as_ref(),
            Value::Bool(_) => return Err(CellError::Value),
            _ => &value.to_text()?,
        };
        if source.len() > 10 {
            return Err(CellError::Num);
        }
        for &b in source.as_bytes() {
            word = word * i64::from(radix) + i64::from(digit(b, radix)?);
        }
    }
    let modulus = i64::from(radix).pow(10);
    Ok(if word >= modulus / 2 { word - modulus } else { word })
}

/// BIN/OCT/HEX are signed ten-digit words. Excel checks the optional output
/// length before reading the word, including when a negative result needs all ten digits.
pub(crate) fn convert<const FROM: u32, const TO: u32>(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let places = if has(args, 1) {
        let n = number(args, 1, false)?.trunc();
        if !(1.0..=10.0).contains(&n) {
            return Err(CellError::Num);
        }
        Some(n as usize)
    } else {
        None
    };
    let value = if FROM == 10 {
        let n = number(args, 0, false)?.trunc();
        if !n.is_finite() || n.abs() > (1u64 << 40) as f64 {
            return Err(CellError::Num);
        }
        n as i64
    } else {
        signed_value(args, FROM)?
    };
    if TO == 10 {
        return num_val(value as f64);
    }
    let modulus = i64::from(TO).pow(10);
    if !(-modulus / 2..modulus / 2).contains(&value) {
        return Err(CellError::Num);
    }
    let digits = encode(value.rem_euclid(modulus) as u64, TO, places.unwrap_or(1));
    if value >= 0 && places.is_some_and(|n| digits.len() > n) {
        return Err(CellError::Num);
    }
    text_val(digits)
}

pub(crate) fn base(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let n = number(args, 0, true)?;
    let radix = number(args, 1, true)?.trunc();
    let width = if has(args, 2) { number(args, 2, true)? } else { 1. };
    if !(0.0..(1u64 << 53) as f64).contains(&n) || !(2.0..=36.0).contains(&radix) || !(0.0..256.).contains(&width) {
        return Err(CellError::Num);
    }
    text_val(encode(n as u64, radix as u32, width as usize))
}

pub(crate) fn decimal(args: &[Arg], _: &mut dyn Ctx) -> R<Value> {
    let value = input(args, 0)?;
    let source = match value {
        Value::Text(t) => t.as_ref(),
        // Numeric inputs are read in ordinary decimal notation at Excel's
        // fifteen-digit precision, even when the cell would display an exponent.
        Value::Number(n) if *n >= 1e15 && n.is_finite() && n.fract() == 0. => {
            &format!("{:.0}", joysheet_core::number_to_text(*n).parse::<f64>().map_err(|_| CellError::Num)?)
        }
        _ => &value.to_text()?,
    };
    if source.encode_utf16().nth(255).is_some() {
        return Err(CellError::Value);
    }
    let radix = number(args, 1, true)?.trunc();
    if !(2.0..=36.0).contains(&radix) {
        return Err(CellError::Num);
    }
    let radix = radix as u32;
    // An unprefixed suffix must follow a significant digit. A hexadecimal
    // prefix also introduces a number, including an empty or all-zero body.
    let mut digits = source.trim_start().trim_start_matches('0').as_bytes();
    if digits.len() > 1
        && ((radix == 2 && digits.last().is_some_and(|b| b.eq_ignore_ascii_case(&b'B')))
            || (radix == 16 && digits.last().is_some_and(|b| b.eq_ignore_ascii_case(&b'H'))))
    {
        digits = &digits[..digits.len() - 1];
    }
    if radix == 16 && digits.first().is_some_and(|b| b.eq_ignore_ascii_case(&b'X')) {
        digits = &digits[1..];
    }
    // Excel accumulates from the units digit. The order is observable when
    // an encoded integer is too large to represent exactly in a binary64.
    let mut result = 0.;
    let mut place = 1.;
    for &b in digits.iter().rev() {
        let value = digit(b, radix)?;
        if value != 0 {
            result += place * f64::from(value);
        }
        place *= f64::from(radix);
    }
    num_val(result)
}

fn operand(number: f64) -> R<u64> {
    if !(0.0..BIT_LIMIT as f64).contains(&number) || number.fract() != 0. {
        return Err(CellError::Num);
    }
    Ok(number as u64)
}

pub(crate) fn bits(args: &[Arg], op: fn(u64, u64) -> u64) -> R<Value> {
    let a = number(args, 0, true)?;
    let b = number(args, 1, true)?;
    num_val(op(operand(a)?, operand(b)?) as f64)
}

pub(crate) fn shift(args: &[Arg], left: bool) -> R<Value> {
    let input = number(args, 0, true)?;
    let amount = number(args, 1, true)?.trunc();
    let value = operand(input)?;
    if value == 0 {
        return num_val(0.);
    }
    if !(-53.0..=53.0).contains(&amount) {
        return Err(CellError::Num);
    }
    let shift = amount.abs() as u32;
    // Counts of 49..53 shift the entire 48-bit worksheet word out. A left
    // shift of up to 48 still reports overflow when a set bit passes bit 47.
    let result = if shift > 48 {
        0
    } else if (amount >= 0.) == left {
        if value > (BIT_LIMIT - 1) >> shift {
            return Err(CellError::Num);
        }
        value << shift
    } else {
        value >> shift
    };
    num_val(result as f64)
}
