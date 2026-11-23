//! First-party classic text functions.
//!
//! Covers concatenation (`CONCATENATE`, `CONCAT`, `TEXTJOIN`), slicing and length
//! (`LEFT`/`LEFTB`, `RIGHT`/`RIGHTB`, `MID`/`MIDB`, `LEN`/`LENB`), case and cleanup
//! (`LOWER`, `UPPER`, `PROPER`, `TRIM`, `CLEAN`), substitution and repetition
//! (`SUBSTITUTE`, `REPLACE`/`REPLACEB`, `REPT`), searching and comparison
//! (`FIND`/`FINDB`, `SEARCH`/`SEARCHB`, `EXACT`), character codes (`CHAR`, `CODE`,
//! `UNICHAR`, `UNICODE`), and `T`.
//!
//! Uses UTF-8 byte-offset scanning and ASCII fast paths to avoid `Vec<char>`
//! allocations, and streams `TRIM` and `TEXTJOIN` directly into output buffers
//! within the 32,767-character cell text budget.

use joysheet_core::{CellError, Value};

use crate::criteria::wildcard_find;
use crate::util::{MAX_TEXT, R, arg, boolean, has, num, num_val, opt_num, scalar, text, text_val};
use crate::{Arg, Ctx};

/// Windows-1252 code points 128–159.
const CP1252: [u32; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019,
    0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

/// Validates that `s` fits within Excel's 32,767-character cell limit.
pub(super) fn check_len(s: String) -> R<Value> {
    if s.len() > MAX_TEXT && s.chars().count() > MAX_TEXT {
        return Err(CellError::Value);
    }
    text_val(s)
}

/// Appends `piece` to `out`, failing early before allocation grows beyond the
/// maximum UTF-8 byte size of a valid cell string (`MAX_TEXT * 4`).
pub(super) fn push_capped(out: &mut String, piece: &str) -> R<()> {
    if out.len().saturating_add(piece.len()) > MAX_TEXT * 4 {
        return Err(CellError::Value);
    }
    out.push_str(piece);
    Ok(())
}

/// Appends the text representation of a cell value without cloning existing `Value::Text`.
fn push_value_capped(out: &mut String, v: &Value) -> R<bool> {
    match v {
        Value::Empty => Ok(false),
        Value::Text(t) => {
            if t.is_empty() {
                return Ok(false);
            }
            push_capped(out, t)?;
            Ok(true)
        }
        Value::Bool(b) => {
            push_capped(out, if *b { "TRUE" } else { "FALSE" })?;
            Ok(true)
        }
        Value::Error(e) => Err(*e),
        other => {
            let s = other.to_text()?;
            if s.is_empty() {
                return Ok(false);
            }
            push_capped(out, &s)?;
            Ok(true)
        }
    }
}

fn char_len(s: &str) -> usize {
    if s.is_ascii() { s.len() } else { s.chars().count() }
}

/// Byte offset of the `n`-th character in `s` (0-based), or `s.len()` when `n >= char_len(s)`.
fn char_byte_offset(s: &str, n: usize) -> usize {
    if s.is_ascii() {
        return n.min(s.len());
    }
    s.char_indices().nth(n).map_or(s.len(), |(b, _)| b)
}

fn count_arg(a: &[Arg], i: usize, default: f64) -> R<usize> {
    let n = opt_num(a, i, default)?;
    if n < 0.0 || n.is_nan() {
        return Err(CellError::Value);
    }
    Ok(if n > 1e9 { 1_000_000_000 } else { n.trunc() as usize })
}

fn start_arg(a: &[Arg], i: usize) -> R<usize> {
    let start = num(a, i)?;
    if start.is_nan() || start < 1.0 {
        return Err(CellError::Value);
    }
    Ok(if start > 1e9 { 1_000_000_000 } else { start.trunc() as usize })
}

pub(super) fn concatenate(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let mut out = String::new();
    for x in a {
        push_value_capped(&mut out, &scalar(x))?;
    }
    check_len(out)
}

pub(super) fn concat(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let mut out = String::new();
    for x in a {
        match &x.value {
            Value::Array(arr) => {
                for v in arr.iter() {
                    push_value_capped(&mut out, v)?;
                }
            }
            v => {
                push_value_capped(&mut out, v)?;
            }
        }
    }
    check_len(out)
}

fn value_is_empty_text(v: &Value) -> R<bool> {
    match v {
        Value::Empty => Ok(true),
        Value::Text(t) => Ok(t.is_empty()),
        Value::Error(e) => Err(*e),
        _ => Ok(false),
    }
}

pub(super) fn textjoin(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let delim_val = &arg(a, 0)?.value;
    let delims: Vec<String> = match delim_val {
        Value::Array(arr) => arr.iter().map(Value::to_text).collect::<R<_>>()?,
        v => vec![v.to_text()?],
    };
    let ignore = boolean(a, 1)?;
    let mut out = String::new();
    let mut emitted = 0usize;
    let mut emit = |v: &Value| -> R<()> {
        if ignore && value_is_empty_text(v)? {
            return Ok(());
        }
        if emitted > 0
            && !delims.is_empty()
            && let Some(d) = delims.get((emitted - 1) % delims.len())
        {
            push_capped(&mut out, d)?;
        }
        push_value_capped(&mut out, v)?;
        emitted += 1;
        Ok(())
    };
    for x in a.iter().skip(2) {
        match &x.value {
            Value::Array(arr) => {
                for v in arr.iter() {
                    emit(v)?;
                }
            }
            v => emit(v)?,
        }
    }
    check_len(out)
}

pub(super) fn left(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let n = count_arg(a, 1, 1.0)?;
    if n == 0 || s.is_empty() {
        return text_val("");
    }
    let end = char_byte_offset(&s, n);
    text_val(s.get(..end).unwrap_or(""))
}

pub(super) fn right(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let n = count_arg(a, 1, 1.0)?;
    if n == 0 || s.is_empty() {
        return text_val("");
    }
    let start = if s.is_ascii() { s.len().saturating_sub(n) } else { s.char_indices().rev().nth(n - 1).map_or(0, |(b, _)| b) };
    text_val(s.get(start..).unwrap_or(""))
}

pub(super) fn mid(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let start = start_arg(a, 1)? - 1;
    let n = count_arg(a, 2, 0.0)?;
    if n == 0 || s.is_empty() {
        return text_val("");
    }
    if s.is_ascii() {
        let b0 = start.min(s.len());
        let b1 = b0.saturating_add(n).min(s.len());
        return text_val(s.get(b0..b1).unwrap_or(""));
    }
    let mut it = s.char_indices();
    let Some((b0, _)) = it.nth(start) else {
        return text_val("");
    };
    let b1 = it.nth(n - 1).map_or(s.len(), |(b, _)| b);
    text_val(s.get(b0..b1).unwrap_or(""))
}

pub(super) fn len(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    num_val(char_len(&text(a, 0)?) as f64)
}

pub(super) fn lower(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    if s.is_ascii() { text_val(s.to_ascii_lowercase()) } else { text_val(s.to_lowercase()) }
}

pub(super) fn upper(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    if s.is_ascii() { text_val(s.to_ascii_uppercase()) } else { text_val(s.to_uppercase()) }
}

pub(super) fn proper(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let mut out = String::with_capacity(s.len());
    let mut prev_letter = false;
    for ch in s.chars() {
        if ch.is_alphabetic() {
            if prev_letter {
                if ch.is_ascii() {
                    out.push(ch.to_ascii_lowercase());
                } else {
                    out.extend(ch.to_lowercase());
                }
            } else if ch.is_ascii() {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.extend(ch.to_uppercase());
            }
            prev_letter = true;
        } else {
            out.push(ch);
            prev_letter = false;
        }
    }
    text_val(out)
}

/// Excel `TRIM`: strips leading and trailing ASCII spaces (U+0020) and collapses
/// internal runs of ASCII spaces into a single space, leaving all other whitespace
/// untouched.
pub(super) fn trim(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let mut out = String::with_capacity(s.len());
    let mut first = true;
    for word in s.split(' ') {
        if word.is_empty() {
            continue;
        }
        if !first {
            out.push(' ');
        }
        out.push_str(word);
        first = false;
    }
    text_val(out)
}

/// Excel `CLEAN`: strips ASCII control characters 0x00–0x1F.
pub(super) fn clean(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    if s.bytes().all(|b| b >= 32) {
        return text_val(s);
    }
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if (ch as u32) >= 32 {
            out.push(ch);
        }
    }
    text_val(out)
}

pub(super) fn substitute(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let old = text(a, 1)?;
    let new = text(a, 2)?;
    if old.is_empty() {
        return text_val(s);
    }
    if has(a, 3) {
        let inst = start_arg(a, 3)?;
        match s.match_indices(old.as_str()).nth(inst - 1) {
            Some((pos, _)) => {
                let mut out = String::with_capacity(s.len().saturating_sub(old.len()).saturating_add(new.len()));
                out.push_str(s.get(..pos).unwrap_or(""));
                out.push_str(&new);
                out.push_str(s.get(pos + old.len()..).unwrap_or(""));
                check_len(out)
            }
            None => text_val(s),
        }
    } else {
        if new.len() > old.len() {
            let count = s.matches(old.as_str()).count();
            if s.len().saturating_add(count.saturating_mul(new.len() - old.len())) > MAX_TEXT * 4 {
                return Err(CellError::Value);
            }
        }
        check_len(s.replace(old.as_str(), &new))
    }
}

pub(super) fn replace(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let start = start_arg(a, 1)? - 1;
    let n = count_arg(a, 2, 0.0)?;
    let new = text(a, 3)?;
    let (b0, b1) = if s.is_ascii() {
        let b0 = start.min(s.len());
        let b1 = b0.saturating_add(n).min(s.len());
        (b0, b1)
    } else {
        let b0 = char_byte_offset(&s, start);
        let tail = s.get(b0..).unwrap_or("");
        let b1 = b0 + char_byte_offset(tail, n);
        (b0, b1)
    };
    let out_bytes = b0.saturating_add(new.len()).saturating_add(s.len().saturating_sub(b1));
    if out_bytes > MAX_TEXT * 4 {
        return Err(CellError::Value);
    }
    let mut out = String::with_capacity(out_bytes);
    out.push_str(s.get(..b0).unwrap_or(""));
    out.push_str(&new);
    out.push_str(s.get(b1..).unwrap_or(""));
    check_len(out)
}

pub(super) fn rept(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let n = num(a, 1)?;
    if n.is_nan() || n < 0.0 {
        return Err(CellError::Value);
    }
    let n = n.trunc();
    let total = char_len(&s) as f64 * n;
    if total > MAX_TEXT as f64 {
        return Err(CellError::Value);
    }
    if n == 0.0 || s.is_empty() {
        return text_val("");
    }
    text_val(s.repeat(n as usize))
}

fn find_impl(a: &[Arg], wild: bool) -> R<Value> {
    let needle = text(a, 0)?;
    let hay = text(a, 1)?;
    let start_f = opt_num(a, 2, 1.0)?;
    let hay_chars = char_len(&hay);
    if start_f.is_nan() || start_f < 1.0 || start_f > hay_chars.max(1) as f64 {
        return Err(CellError::Value);
    }
    let start = start_f.trunc() as usize - 1;
    if needle.is_empty() {
        return num_val((start + 1) as f64);
    }
    let pos = if wild {
        wildcard_find(&needle, &hay, start)
    } else {
        let start_byte = char_byte_offset(&hay, start);
        hay.get(start_byte..).and_then(|slice| {
            let rel = slice.find(needle.as_str())?;
            if hay.is_ascii() { Some(start + rel) } else { Some(start + slice.get(..rel).map_or(0, |p| p.chars().count())) }
        })
    };
    match pos {
        Some(p) => num_val((p + 1) as f64),
        None => Err(CellError::Value),
    }
}

pub(super) fn find(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    find_impl(a, false)
}

pub(super) fn search(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    find_impl(a, true)
}

pub(super) fn exact(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    Ok(Value::Bool(text(a, 0)? == text(a, 1)?))
}

pub(super) fn char_fn(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let n = num(a, 0)?.trunc();
    if !(1.0..=255.0).contains(&n) {
        return Err(CellError::Value);
    }
    let n = n as u32;
    let cp = if (128..160).contains(&n) { CP1252.get((n - 128) as usize).copied().unwrap_or(n) } else { n };
    text_val(char::from_u32(cp).map(String::from).unwrap_or_default())
}

pub(super) fn code(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let ch = s.chars().next().ok_or(CellError::Value)?;
    let cp = ch as u32;
    let code = if cp < 128 || (160..256).contains(&cp) {
        cp
    } else if let Some(i) = CP1252.iter().position(|&x| x == cp) {
        128 + i as u32
    } else {
        63
    };
    num_val(code as f64)
}

pub(super) fn unichar(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let n = num(a, 0)?.trunc();
    if !(1.0..=1_114_111.0).contains(&n) {
        return Err(CellError::Value);
    }
    match char::from_u32(n as u32) {
        Some(c) => text_val(c.to_string()),
        None => Err(CellError::NA),
    }
}

pub(super) fn unicode(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let ch = s.chars().next().ok_or(CellError::Value)?;
    num_val(ch as u32 as f64)
}

pub(super) fn t_fn(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    match scalar(arg(a, 0)?) {
        Value::Text(t) => Ok(Value::Text(t)),
        Value::Error(e) => Err(e),
        _ => text_val(""),
    }
}

pub(super) fn identity(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    text_val(text(a, 0)?)
}
