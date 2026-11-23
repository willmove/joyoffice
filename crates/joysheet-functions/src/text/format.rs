//! First-party text/number formatting and conversion functions:
//! `VALUE`, `NUMBERVALUE`, `FIXED`, `DOLLAR`, `USDOLLAR`, `BAHTTEXT`,
//! `VALUETOTEXT`, and `ARRAYTOTEXT`.

use joysheet_core::parse::parse_number_text_in;
use joysheet_core::{CellError, Value, number_to_text};

use super::classic::{check_len, push_capped};
use crate::util::{R, arg, num, num_val, opt_bool, opt_int, opt_num, round_half_away, scalar, text, text_val};
use crate::{Arg, Ctx};

/// `VALUE`: a blank cell coerces to 0, while empty or blank text returns `#VALUE!`.
pub(super) fn value(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    match scalar(arg(a, 0)?) {
        Value::Number(n) => num_val(n),
        Value::Empty => num_val(0.0),
        Value::Error(e) => Err(e),
        Value::Text(t) => parse_number_text_in(&t, c.date_system()).ok_or(CellError::Value).and_then(num_val),
        _ => Err(CellError::Value),
    }
}

/// `NUMBERVALUE(text, [decimal_separator], [group_separator])`.
pub(super) fn numbervalue(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let dec = if a.len() > 1 { text(a, 1)?.chars().next().ok_or(CellError::Value)? } else { '.' };
    let grp = if a.len() > 2 { text(a, 2)?.chars().next().ok_or(CellError::Value)? } else { ',' };
    if dec == grp {
        return Err(CellError::Value);
    }
    let mut normalized = String::with_capacity(s.len());
    let mut pct = 0usize;
    let mut seen_dec = false;
    // Strip all whitespace, count trailing `%` signs, and validate separator placement in one pass.
    let mut non_ws = String::with_capacity(s.len());
    for ch in s.chars() {
        if !ch.is_whitespace() {
            non_ws.push(ch);
        }
    }
    if non_ws.is_empty() {
        return num_val(0.0);
    }
    let mut body = non_ws.as_str();
    while let Some(stripped) = body.strip_suffix('%') {
        body = stripped;
        pct += 1;
    }
    for ch in body.chars() {
        if ch == dec {
            if seen_dec {
                return Err(CellError::Value);
            }
            seen_dec = true;
            normalized.push('.');
        } else if ch == grp {
            if seen_dec {
                return Err(CellError::Value);
            }
        } else if ch == '.' || ch == ',' {
            return Err(CellError::Value);
        } else {
            normalized.push(ch);
        }
    }
    let mut n = parse_number_text_in(&normalized, c.date_system()).ok_or(CellError::Value)?;
    for _ in 0..pct {
        n /= 100.0;
    }
    num_val(n)
}

/// Formats `x` (already rounded) with `decimals` places and optional thousands commas
/// using a single pre-sized allocation.
fn format_fixed(x: f64, decimals: i64, commas: bool) -> String {
    let d = decimals.max(0) as usize;
    let raw = format!("{:.*}", d, x.abs());
    if !commas {
        return raw;
    }
    let (int_part, frac) = match raw.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (raw.as_str(), None),
    };
    let ib = int_part.as_bytes();
    let comma_count = ib.len().saturating_sub(1) / 3;
    let mut out = String::with_capacity(raw.len() + comma_count);
    for (idx, &ch) in ib.iter().enumerate() {
        if idx > 0 && (ib.len() - idx).is_multiple_of(3) {
            out.push(',');
        }
        out.push(char::from(ch));
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

fn rounded(a: &[Arg], default_dec: f64) -> R<(f64, i64)> {
    let x = num(a, 0)?;
    let d = opt_num(a, 1, default_dec)?.trunc();
    if d > 127.0 {
        return Err(CellError::Value);
    }
    let d = d.max(-308.0) as i64;
    let r = round_half_away(x, d as i32);
    if !r.is_finite() || r.abs() >= 1e300 {
        return Err(CellError::Num);
    }
    Ok((r, d))
}

pub(super) fn fixed(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (r, d) = rounded(a, 2.0)?;
    let no_commas = opt_bool(a, 2, false)?;
    let body = format_fixed(r, d, !no_commas);
    text_val(if r < 0.0 { format!("-{body}") } else { body })
}

pub(super) fn dollar(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (r, d) = rounded(a, 2.0)?;
    let body = format_fixed(r, d, true);
    text_val(if r < 0.0 { format!("(${body})") } else { format!("${body}") })
}

// ---------------------------------------------------------------------------------------------
// BAHTTEXT (Thai currency spelling, rounded half-away to satang)

const THAI_DIGITS: [&str; 10] = ["ศูนย์", "หนึ่ง", "สอง", "สาม", "สี่", "ห้า", "หก", "เจ็ด", "แปด", "เก้า"];
const THAI_PLACES: [&str; 6] = ["", "สิบ", "ร้อย", "พัน", "หมื่น", "แสน"];

fn push_thai_group(out: &mut String, digits: &[u8], after_higher_group: bool) {
    let n = digits.len();
    let has_higher = after_higher_group || digits.iter().take(n.saturating_sub(1)).any(|&d| d != 0);
    for (i, &d) in digits.iter().enumerate() {
        let place = n - 1 - i;
        match (place, d) {
            (_, 0) => {}
            (0, 1) if has_higher => out.push_str("เอ็ด"),
            (1, 1) => out.push_str("สิบ"),
            (1, 2) => out.push_str("ยี่สิบ"),
            _ => {
                out.push_str(THAI_DIGITS.get(usize::from(d)).copied().unwrap_or_default());
                out.push_str(THAI_PLACES.get(place).copied().unwrap_or_default());
            }
        }
    }
}

fn push_thai_number(out: &mut String, digits: &str) {
    let d: Vec<u8> = digits.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').skip_while(|&x| x == 0).collect();
    if d.is_empty() {
        out.push_str("ศูนย์");
        return;
    }
    let first = match d.len() % 6 {
        0 => 6,
        r => r,
    };
    let mut start = 0;
    let mut len = first;
    while let Some(group) = d.get(start..start + len) {
        push_thai_group(out, group, start > 0);
        start += len;
        len = 6;
        if start < d.len() {
            out.push_str("ล้าน");
        }
    }
}

pub(super) fn bahttext(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = round_half_away(num(a, 0)?, 2);
    if !x.is_finite() {
        return Err(CellError::Num);
    }
    let fixed = format!("{:.2}", x.abs());
    let (whole, frac) = fixed.split_once('.').unwrap_or((fixed.as_str(), "00"));
    let has_baht = whole.bytes().any(|b| b != b'0');
    let has_satang = frac.bytes().any(|b| b != b'0');
    let mut out = String::new();
    if x < 0.0 && (has_baht || has_satang) {
        out.push_str("ลบ");
    }
    if has_baht || !has_satang {
        push_thai_number(&mut out, whole);
        out.push_str("บาท");
    }
    if has_satang {
        push_thai_number(&mut out, frac);
        out.push_str("สตางค์");
    } else {
        out.push_str("ถ้วน");
    }
    text_val(out)
}

// ---------------------------------------------------------------------------------------------
// VALUETOTEXT / ARRAYTOTEXT

fn push_value_text(out: &mut String, v: &Value, strict: bool) -> R<()> {
    match v {
        Value::Text(t) if strict => {
            push_capped(out, "\"")?;
            if t.contains('"') {
                push_capped(out, &t.replace('"', "\"\""))?;
            } else {
                push_capped(out, t)?;
            }
            push_capped(out, "\"")
        }
        Value::Text(t) => push_capped(out, t),
        Value::Error(e) => push_capped(out, e.as_str()),
        Value::Number(n) => push_capped(out, &number_to_text(*n)),
        Value::Bool(b) => push_capped(out, if *b { "TRUE" } else { "FALSE" }),
        Value::Empty => Ok(()),
        other => push_capped(out, &other.to_text().unwrap_or_default()),
    }
}

fn format_arg(a: &[Arg], i: usize) -> R<bool> {
    match opt_int(a, i, 0)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CellError::Value),
    }
}

pub(super) fn valuetotext(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let strict = format_arg(a, 1)?;
    let mut out = String::new();
    push_value_text(&mut out, &scalar(arg(a, 0)?), strict)?;
    check_len(out)
}

pub(super) fn arraytotext(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let strict = format_arg(a, 1)?;
    let mut out = String::new();
    if strict {
        out.push('{');
    }
    match &arg(a, 0)?.value {
        Value::Array(arr) => {
            for r in 0..arr.rows {
                for c in 0..arr.cols {
                    if r > 0 && c == 0 {
                        push_capped(&mut out, if strict { ";" } else { ", " })?;
                    } else if c > 0 {
                        push_capped(&mut out, if strict { "," } else { ", " })?;
                    }
                    push_value_text(&mut out, arr.get(r, c).unwrap_or(&Value::Empty), strict)?;
                }
            }
        }
        v => push_value_text(&mut out, v, strict)?,
    }
    if strict {
        push_capped(&mut out, "}")?;
    }
    check_len(out)
}
