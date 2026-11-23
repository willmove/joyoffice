//! Text functions.

mod classic;

use joysheet_core::parse::parse_number_text_in;
use joysheet_core::{CellError, Value, number_to_text};

use crate::util::{
    A, MAX_CELLS, MAX_TEXT, R, S, arg, array_val, as_array, has, int, num, num_val, opt_bool, opt_int, opt_num, round_half_away, scalar, text,
    text_val,
};
use crate::{Arg, Ctx, FnSpec, VAR};
use classic::{
    char_fn, check_len, clean, code, concat, concatenate, exact, find, identity, left, len, lower, mid, proper, push_capped, replace, rept, right,
    search, substitute, t_fn, textjoin, trim, unichar, unicode, upper,
};

const TJ: &[bool] = &[false, true, false];
const TB: &[bool] = &[true, false, true, true, true, false];
const TS: &[bool] = &[true, false, false, true, true, false];
const ATT: &[bool] = &[false, true];

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// VALUE: a blank cell reads as 0, but empty or blank text is not a number (observed:
/// `VALUE("")` is `#VALUE!`).
fn value(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    match scalar(arg(a, 0)?) {
        Value::Number(n) => num_val(n),
        Value::Empty => num_val(0.0),
        Value::Error(e) => Err(e),
        Value::Text(t) => parse_number_text_in(&t, c.date_system()).ok_or(CellError::Value).and_then(num_val),
        _ => Err(CellError::Value),
    }
}

fn numbervalue(a: &[Arg], c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let dec = if a.len() > 1 { text(a, 1)?.chars().next().ok_or(CellError::Value)? } else { '.' };
    let grp = if a.len() > 2 { text(a, 2)?.chars().next().ok_or(CellError::Value)? } else { ',' };
    if dec == grp {
        return Err(CellError::Value);
    }
    let mut body: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if body.is_empty() {
        return num_val(0.0);
    }
    let mut pct = 0;
    while let Some(b) = body.strip_suffix('%') {
        body = b.to_string();
        pct += 1;
    }
    let mut seen_dec = false;
    let mut out = String::new();
    for c in body.chars() {
        if c == dec {
            if seen_dec {
                return Err(CellError::Value);
            }
            seen_dec = true;
            out.push('.');
        } else if c == grp {
            if seen_dec {
                return Err(CellError::Value);
            }
        } else if c == '.' || c == ',' {
            return Err(CellError::Value);
        } else {
            out.push(c);
        }
    }
    let mut n = parse_number_text_in(&out, c.date_system()).ok_or(CellError::Value)?;
    for _ in 0..pct {
        n /= 100.0;
    }
    num_val(n)
}

/// Formats `x` (already rounded) with `decimals` places and optional thousands separators.
fn format_fixed(x: f64, decimals: i64, commas: bool) -> String {
    let d = decimals.max(0) as usize;
    let s = format!("{:.*}", d, x.abs());
    let (int_part, frac) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (s.clone(), None),
    };
    let int_part = if commas {
        let b = int_part.as_bytes();
        let mut o = String::new();
        for (i, ch) in b.iter().enumerate() {
            if i > 0 && (b.len() - i) % 3 == 0 {
                o.push(',');
            }
            o.push(char::from(*ch));
        }
        o
    } else {
        int_part
    };
    let mut out = int_part;
    if let Some(f) = frac {
        out.push('.');
        out.push_str(&f);
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

fn fixed(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (r, d) = rounded(a, 2.0)?;
    let no_commas = opt_bool(a, 2, false)?;
    let body = format_fixed(r, d, !no_commas);
    text_val(if r < 0.0 { format!("-{body}") } else { body })
}

fn dollar(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let (r, d) = rounded(a, 2.0)?;
    let body = format_fixed(r, d, true);
    text_val(if r < 0.0 { format!("(${body})") } else { format!("${body}") })
}

// ---------------------------------------------------------------------------------------------
// BAHTTEXT

const THAI_DIGITS: [&str; 10] = ["ศูนย์", "หนึ่ง", "สอง", "สาม", "สี่", "ห้า", "หก", "เจ็ด", "แปด", "เก้า"];
/// Units, tens, hundreds, thousands, ten thousands, hundred thousands.
const THAI_PLACES: [&str; 6] = ["", "สิบ", "ร้อย", "พัน", "หมื่น", "แสน"];

/// Thai words for a group of up to six decimal digits (most significant first); empty for zero.
/// One in the tens is สิบ, two is ยี่สิบ, and a final one after higher digits is เอ็ด; `after`
/// says the number already has a non-zero digit in a higher group (so 1,000,001 ends in เอ็ด).
fn thai_group(digits: &[u8], after: bool) -> String {
    let n = digits.len();
    let higher = after || digits.iter().take(n.saturating_sub(1)).any(|d| *d != 0);
    let mut out = String::new();
    for (i, &d) in digits.iter().enumerate() {
        let place = n - 1 - i;
        match (place, d) {
            (_, 0) => {}
            (0, 1) if higher => out.push_str("เอ็ด"),
            (1, 1) => out.push_str("สิบ"),
            (1, 2) => out.push_str("ยี่สิบ"),
            _ => {
                out.push_str(THAI_DIGITS.get(d as usize).copied().unwrap_or_default());
                out.push_str(THAI_PLACES.get(place).copied().unwrap_or_default());
            }
        }
    }
    out
}

/// Thai words for a whole number written in decimal digits: groups of six digits, each but the
/// last followed by ล้าน (so 10^12 is หนึ่งล้านล้าน).
fn thai_number(digits: &str) -> String {
    let d: Vec<u8> = digits.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').skip_while(|x| *x == 0).collect();
    if d.is_empty() {
        return "ศูนย์".to_string();
    }
    let first = match d.len() % 6 {
        0 => 6,
        r => r,
    };
    let mut out = String::new();
    let mut start = 0;
    let mut len = first;
    while let Some(group) = d.get(start..start + len) {
        out.push_str(&thai_group(group, start > 0));
        start += len;
        len = 6;
        if start < d.len() {
            out.push_str("ล้าน");
        }
    }
    out
}

/// BAHTTEXT(number): the amount in Thai words with บาท, rounded to satang (สตางค์); a whole
/// number of baht ends in ถ้วน and a negative amount starts with ลบ.
fn bahttext(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = round_half_away(num(a, 0)?, 2);
    if !x.is_finite() {
        return Err(CellError::Num);
    }
    let fixed = format!("{:.2}", x.abs());
    let (whole, frac) = fixed.split_once('.').unwrap_or((fixed.as_str(), "00"));
    let baht = whole.bytes().any(|b| b != b'0');
    let satang = frac.bytes().any(|b| b != b'0');
    let mut out = String::new();
    if x < 0.0 && (baht || satang) {
        out.push_str("ลบ");
    }
    if baht || !satang {
        out.push_str(&thai_number(whole));
        out.push_str("บาท");
    }
    if satang {
        out.push_str(&thai_number(frac));
        out.push_str("สตางค์");
    } else {
        out.push_str("ถ้วน");
    }
    text_val(out)
}

/// Lower-cased char sequence with one char per input char (keeps positions aligned).
fn fold(c: &[char]) -> Vec<char> {
    c.iter().map(|ch| ch.to_lowercase().next().unwrap_or(*ch)).collect()
}

/// Delimiter list from a scalar or array argument.
fn delimiters(v: &Value) -> R<Vec<Vec<char>>> {
    as_array(v).iter().map(|d| d.to_text().map(|s| chars(&s))).collect()
}

/// Non-overlapping delimiter matches (start, end) in char positions, in ascending order. Scans left
/// to right preferring the longest delimiter starting at each position, or (`from_end`) right to
/// left preferring the longest delimiter ending at each position. Empty delimiters are ignored unless all
/// delimiters are empty, in which case every position matches with zero length.
fn find_matches(text: &[char], delims: &[Vec<char>], insensitive: bool, from_end: bool) -> Vec<(usize, usize)> {
    let t = if insensitive { fold(text) } else { text.to_vec() };
    let ds: Vec<Vec<char>> = delims.iter().filter(|d| !d.is_empty()).map(|d| if insensitive { fold(d) } else { d.clone() }).collect();
    if ds.is_empty() {
        return (0..=t.len()).map(|i| (i, i)).collect();
    }
    let mut out = Vec::new();
    if from_end {
        let mut j = t.len();
        while j > 0 {
            let best =
                ds.iter().filter(|d| j.checked_sub(d.len()).and_then(|st| t.get(st..j)).is_some_and(|w| w == d.as_slice())).map(Vec::len).max();
            match best {
                Some(l) => {
                    out.push((j - l, j));
                    j -= l;
                }
                None => j -= 1,
            }
        }
        out.reverse();
        return out;
    }
    let mut i = 0;
    while i < t.len() {
        let best = ds.iter().filter(|d| t.get(i..i + d.len()).is_some_and(|w| w == d.as_slice())).map(Vec::len).max();
        match best {
            Some(l) => {
                out.push((i, i + l));
                i += l;
            }
            None => i += 1,
        }
    }
    out
}

fn text_before_after(a: &[Arg], before: bool) -> R<Value> {
    let s = chars(&text(a, 0)?);
    let delims = delimiters(&arg(a, 1)?.value)?;
    let inst = if has(a, 2) { int(a, 2)? } else { 1 };
    let mode = opt_int(a, 3, 0)?;
    let match_end = opt_bool(a, 4, false)?;
    if !(0..=1).contains(&mode) || inst == 0 || inst.unsigned_abs() as usize > s.len().max(1) {
        return Err(CellError::Value);
    }
    let mut m = find_matches(&s, &delims, mode == 1, inst < 0);
    if match_end {
        if inst > 0 {
            if m.last().is_none_or(|x| x.0 != s.len()) {
                m.push((s.len(), s.len()));
            }
        } else if m.first().is_none_or(|x| x.0 != 0) {
            m.insert(0, (0, 0));
        }
    }
    let k = inst.unsigned_abs() as usize;
    let hit = if inst > 0 { m.get(k - 1) } else { m.len().checked_sub(k).and_then(|i| m.get(i)) };
    match hit {
        Some(&(st, en)) => text_val(if before { s[..st].iter().collect::<String>() } else { s[en..].iter().collect::<String>() }),
        None => match a.get(5) {
            Some(x) => Ok(scalar(x)),
            None => Err(CellError::NA),
        },
    }
}

fn textbefore(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    text_before_after(a, true)
}

fn textafter(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    text_before_after(a, false)
}

fn split_by(s: &[char], delims: &[Vec<char>], insensitive: bool) -> Vec<Vec<char>> {
    if delims.iter().all(Vec::is_empty) {
        return vec![s.to_vec()];
    }
    let mut out = Vec::new();
    let mut last = 0;
    for (st, en) in find_matches(s, delims, insensitive, false) {
        out.push(s[last..st].to_vec());
        last = en;
    }
    out.push(s[last..].to_vec());
    out
}

fn textsplit(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = chars(&text(a, 0)?);
    let col_d = if has(a, 1) { delimiters(&arg(a, 1)?.value)? } else { vec![] };
    let row_d = if has(a, 2) { delimiters(&arg(a, 2)?.value)? } else { vec![] };
    if col_d.iter().all(|d| d.is_empty()) && row_d.iter().all(|d| d.is_empty()) {
        return Err(CellError::Value);
    }
    let ignore = opt_bool(a, 3, false)?;
    let mode = opt_int(a, 4, 0)?;
    if !(0..=1).contains(&mode) {
        return Err(CellError::Value);
    }
    let pad = if has(a, 5) { scalar(arg(a, 5)?) } else { Value::Error(CellError::NA) };
    let mut rows: Vec<Vec<String>> = Vec::new();
    for r in split_by(&s, &row_d, mode == 1) {
        if ignore && r.is_empty() {
            continue;
        }
        let cells: Vec<String> =
            split_by(&r, &col_d, mode == 1).into_iter().filter(|c| !(ignore && c.is_empty())).map(|c| c.into_iter().collect()).collect();
        if ignore && cells.is_empty() {
            continue;
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err(CellError::Calc);
    }
    let ncols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let nrows = rows.len();
    if nrows.saturating_mul(ncols) > MAX_CELLS {
        return Err(CellError::Num);
    }
    let mut data = Vec::with_capacity(nrows * ncols);
    for r in rows {
        let n = r.len();
        data.extend(r.into_iter().map(Value::from));
        data.extend(std::iter::repeat_n(pad.clone(), ncols - n));
    }
    array_val(nrows, ncols, data)
}

fn value_text(v: &Value, strict: bool) -> String {
    match v {
        Value::Text(t) if strict => format!("\"{}\"", t.replace('"', "\"\"")),
        Value::Text(t) => t.to_string(),
        Value::Error(e) => e.as_str().to_string(),
        Value::Number(n) => number_to_text(*n),
        other => other.to_text().unwrap_or_default(),
    }
}

fn format_arg(a: &[Arg], i: usize) -> R<bool> {
    match opt_int(a, i, 0)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CellError::Value),
    }
}

fn valuetotext(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let strict = format_arg(a, 1)?;
    check_len(value_text(&scalar(arg(a, 0)?), strict))
}

fn arraytotext(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let strict = format_arg(a, 1)?;
    let arr = as_array(&arg(a, 0)?.value);
    let mut out = String::new();
    if strict {
        out.push('{');
    }
    for r in 0..arr.rows {
        for c in 0..arr.cols {
            if r > 0 && c == 0 {
                out.push_str(if strict { ";" } else { ", " });
            } else if c > 0 {
                out.push_str(if strict { "," } else { ", " });
            }
            push_capped(&mut out, &value_text(arr.get(r, c).unwrap_or(&Value::Empty), strict))?;
        }
    }
    if strict {
        out.push('}');
    }
    check_len(out)
}

fn build_regex(pattern: &str, insensitive: bool) -> R<regex::Regex> {
    if pattern.len() > MAX_TEXT {
        return Err(CellError::Value);
    }
    regex::RegexBuilder::new(pattern)
        .case_insensitive(insensitive)
        .size_limit(1 << 22)
        .dfa_size_limit(1 << 22)
        .nest_limit(100)
        .build()
        .map_err(|_| CellError::Value)
}

fn case_arg(a: &[Arg], i: usize) -> R<bool> {
    match opt_int(a, i, 0)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(CellError::Value),
    }
}

fn regextest(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let re = build_regex(&text(a, 1)?, case_arg(a, 2)?)?;
    Ok(Value::Bool(re.is_match(&s)))
}

fn regexextract(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let mode = opt_int(a, 2, 0)?;
    let re = build_regex(&text(a, 1)?, case_arg(a, 3)?)?;
    match mode {
        0 => re.find(&s).map(|m| Value::from(m.as_str())).ok_or(CellError::NA),
        1 => {
            let all: Vec<Value> = re.find_iter(&s).take(1_000_000).map(|m| Value::from(m.as_str())).collect();
            if all.is_empty() {
                return Err(CellError::NA);
            }
            array_val(all.len(), 1, all)
        }
        2 => {
            let caps = re.captures(&s).ok_or(CellError::NA)?;
            if caps.len() <= 1 {
                return Ok(Value::from(caps.get(0).map_or("", |m| m.as_str())));
            }
            let groups: Vec<Value> = (1..caps.len()).map(|i| Value::from(caps.get(i).map_or("", |m| m.as_str()))).collect();
            array_val(1, groups.len(), groups)
        }
        _ => Err(CellError::Value),
    }
}

/// Converts Excel-style `$1` references to the regex crate's unambiguous `${1}` form.
fn convert_replacement(r: &str) -> String {
    let mut out = String::with_capacity(r.len() + 4);
    let c: Vec<char> = r.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '$' && c.get(i + 1).is_some_and(char::is_ascii_digit) {
            let mut j = i + 1;
            while c.get(j).is_some_and(char::is_ascii_digit) {
                j += 1;
            }
            out.push_str("${");
            out.extend(c[i + 1..j].iter());
            out.push('}');
            i = j;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

/// A piece of a replacement: literal text or a capture group (by number or name).
enum Piece {
    Text(String),
    Index(usize),
    Name(String),
}

/// A replacement split into pieces as `regex::Captures::expand` reads it: `$$` is `$`, `$name` takes
/// the longest run of `[_0-9A-Za-z]`, `${name}` ends at `}` (`${}` is empty), a name of digits is a
/// group number, and a `$` that starts no reference is itself.
fn template(rep: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut rest = rep;
    while let Some(i) = rest.find('$') {
        lit.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        if let Some(r) = rest.strip_prefix('$') {
            lit.push('$');
            rest = r;
            continue;
        }
        // A braced name is a reference even when empty (it expands to nothing).
        let braced = rest.strip_prefix('{').and_then(|b| b.find('}').map(|j| (&b[..j], &b[j + 1..])));
        let (name, after) = braced.unwrap_or_else(|| {
            let j = rest.find(|c: char| !(c == '_' || c.is_ascii_alphanumeric())).unwrap_or(rest.len());
            (&rest[..j], &rest[j..])
        });
        if braced.is_none() && name.is_empty() {
            lit.push('$');
            continue;
        }
        if !lit.is_empty() {
            out.push(Piece::Text(std::mem::take(&mut lit)));
        }
        out.push(match name.parse::<usize>() {
            Ok(n) => Piece::Index(n),
            Err(_) => Piece::Name(name.to_string()),
        });
        rest = after;
    }
    lit.push_str(rest);
    if !lit.is_empty() {
        out.push(Piece::Text(lit));
    }
    out
}

/// Append a match's replacement to `out`, piece by piece; `None` once `out` would pass `limit`
/// bytes, before anything that long is built (#2162).
fn expand_capped(cap: &regex::Captures, pieces: &[Piece], out: &mut String, limit: usize) -> Option<()> {
    for p in pieces {
        let s = match p {
            Piece::Text(t) => t.as_str(),
            Piece::Index(i) => cap.get(*i).map_or("", |m| m.as_str()),
            Piece::Name(n) => cap.name(n).map_or("", |m| m.as_str()),
        };
        if out.len().saturating_add(s.len()) > limit {
            return None;
        }
        out.push_str(s);
    }
    Some(())
}

fn regexreplace(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let rep = template(&convert_replacement(&text(a, 2)?));
    let occ = opt_int(a, 3, 0)?;
    let re = build_regex(&text(a, 1)?, case_arg(a, 4)?)?;
    // Built match by match within a bound past which the text can only be too long (a character
    // is at most four bytes), instead of building every replacement first (#2162).
    let limit = MAX_TEXT * 4;
    let mut out = String::new();
    if occ == 0 {
        let mut last = 0;
        for cap in re.captures_iter(&s) {
            let Some(m) = cap.get(0) else { continue };
            out.push_str(s.get(last..m.start()).unwrap_or_default());
            expand_capped(&cap, &rep, &mut out, limit).ok_or(CellError::Value)?;
            last = m.end();
        }
        out.push_str(s.get(last..).unwrap_or_default());
        return check_len(out);
    }
    let caps: Vec<regex::Captures> = re.captures_iter(&s).collect();
    let k = occ.unsigned_abs() as usize;
    let idx = if occ > 0 { k.checked_sub(1) } else { caps.len().checked_sub(k) };
    let Some(cap) = idx.and_then(|i| caps.get(i)) else { return text_val(s) };
    let Some(m) = cap.get(0) else { return text_val(s) };
    out.push_str(s.get(..m.start()).unwrap_or_default());
    expand_capped(cap, &rep, &mut out, limit).ok_or(CellError::Value)?;
    out.push_str(s.get(m.end()..).unwrap_or_default());
    check_len(out)
}

pub(crate) fn specs() -> Vec<FnSpec> {
    vec![
        f!("CONCATENATE", 1, VAR, Text, S, "CONCATENATE(text1, [text2], ...)", "Joins several text values into one.", concatenate),
        f!("CONCAT", 1, VAR, Text, A, "CONCAT(text1, [text2], ...)", "Joins text from values and every cell of ranges, without a separator.", concat),
        f!(
            "TEXTJOIN",
            3,
            VAR,
            Text,
            TJ,
            "TEXTJOIN(delimiter, ignore_empty, text1, [text2], ...)",
            "Joins text from values and ranges with a delimiter, optionally skipping empty items.",
            textjoin
        ),
        f!("LEFT", 1, 2, Text, S, "LEFT(text, [num_chars])", "Returns the first characters of a text.", left),
        f!("LEFTB", 1, 2, Text, S, "LEFTB(text, [num_bytes])", "Byte version of LEFT (same as LEFT for single-byte text).", left),
        f!("RIGHT", 1, 2, Text, S, "RIGHT(text, [num_chars])", "Returns the last characters of a text.", right),
        f!("RIGHTB", 1, 2, Text, S, "RIGHTB(text, [num_bytes])", "Byte version of RIGHT (same as RIGHT for single-byte text).", right),
        f!("MID", 3, 3, Text, S, "MID(text, start_num, num_chars)", "Returns characters from the middle of a text.", mid),
        f!("MIDB", 3, 3, Text, S, "MIDB(text, start_num, num_bytes)", "Byte version of MID (same as MID for single-byte text).", mid),
        f!("LEN", 1, 1, Text, S, "LEN(text)", "Counts the characters in a text.", len),
        f!("LENB", 1, 1, Text, S, "LENB(text)", "Byte version of LEN (same as LEN for single-byte text).", len),
        f!("LOWER", 1, 1, Text, S, "LOWER(text)", "Converts text to lower case.", lower),
        f!("UPPER", 1, 1, Text, S, "UPPER(text)", "Converts text to upper case.", upper),
        f!("PROPER", 1, 1, Text, S, "PROPER(text)", "Capitalises the first letter of each word and lower-cases the rest.", proper),
        f!("TRIM", 1, 1, Text, S, "TRIM(text)", "Removes leading and trailing spaces and collapses runs of spaces to one.", trim),
        f!("CLEAN", 1, 1, Text, S, "CLEAN(text)", "Removes non-printable control characters from text.", clean),
        f!(
            "SUBSTITUTE",
            3,
            4,
            Text,
            S,
            "SUBSTITUTE(text, old_text, new_text, [instance_num])",
            "Replaces occurrences of one text with another, optionally only a given occurrence.",
            substitute
        ),
        f!("REPLACE", 4, 4, Text, S, "REPLACE(old_text, start_num, num_chars, new_text)", "Replaces part of a text by position.", replace),
        f!("REPLACEB", 4, 4, Text, S, "REPLACEB(old_text, start_num, num_bytes, new_text)", "Byte version of REPLACE.", replace),
        f!("REPT", 2, 2, Text, S, "REPT(text, number_times)", "Repeats a text a given number of times.", rept),
        f!("FIND", 2, 3, Text, S, "FIND(find_text, within_text, [start_num])", "Finds the position of one text in another (case-sensitive).", find),
        f!("FINDB", 2, 3, Text, S, "FINDB(find_text, within_text, [start_num])", "Byte version of FIND.", find),
        f!(
            "SEARCH",
            2,
            3,
            Text,
            S,
            "SEARCH(find_text, within_text, [start_num])",
            "Finds the position of one text in another, ignoring case and allowing wildcards.",
            search
        ),
        f!("SEARCHB", 2, 3, Text, S, "SEARCHB(find_text, within_text, [start_num])", "Byte version of SEARCH.", search),
        f!("EXACT", 2, 2, Text, S, "EXACT(text1, text2)", "TRUE when two texts are identical, including case.", exact),
        f!("CHAR", 1, 1, Text, S, "CHAR(number)", "Returns the character for a code from the Windows-1252 character set.", char_fn),
        f!("CODE", 1, 1, Text, S, "CODE(text)", "Returns the Windows-1252 code of the first character of a text.", code),
        f!("UNICHAR", 1, 1, Text, S, "UNICHAR(number)", "Returns the character for a Unicode code point.", unichar),
        f!("UNICODE", 1, 1, Text, S, "UNICODE(text)", "Returns the Unicode code point of the first character of a text.", unicode),
        f!("VALUE", 1, 1, Text, S, "VALUE(text)", "Converts text that looks like a number, date or time into a number.", value),
        f!(
            "NUMBERVALUE",
            1,
            3,
            Text,
            S,
            "NUMBERVALUE(text, [decimal_separator], [group_separator])",
            "Converts text to a number using the given decimal and group separators.",
            numbervalue
        ),
        f!(
            "FIXED",
            1,
            3,
            Text,
            S,
            "FIXED(number, [decimals], [no_commas])",
            "Rounds a number and formats it as text with fixed decimals and optional thousands separators.",
            fixed
        ),
        f!("DOLLAR", 1, 2, Text, S, "DOLLAR(number, [decimals])", "Formats a number as currency text.", dollar),
        f!("USDOLLAR", 1, 2, Compatibility, S, "USDOLLAR(number, [decimals])", "Formats a number as currency text, like DOLLAR.", dollar),
        f!("BAHTTEXT", 1, 1, Text, S, "BAHTTEXT(number)", "Spells out a number in Thai words with Baht.", bahttext),
        f!("T", 1, 1, Text, S, "T(value)", "Returns the value if it is text, otherwise empty text.", t_fn),
        f!(
            "TEXTBEFORE",
            2,
            6,
            Text,
            TB,
            "TEXTBEFORE(text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found])",
            "Returns the text that comes before a delimiter.",
            textbefore
        ),
        f!(
            "TEXTAFTER",
            2,
            6,
            Text,
            TB,
            "TEXTAFTER(text, delimiter, [instance_num], [match_mode], [match_end], [if_not_found])",
            "Returns the text that comes after a delimiter.",
            textafter
        ),
        f!(
            "TEXTSPLIT",
            2,
            6,
            Text,
            TS,
            "TEXTSPLIT(text, col_delimiter, [row_delimiter], [ignore_empty], [match_mode], [pad_with])",
            "Splits text into a grid of columns and rows at the given delimiters.",
            textsplit
        ),
        f!(
            "VALUETOTEXT",
            1,
            2,
            Text,
            S,
            "VALUETOTEXT(value, [format])",
            "Converts any value to text, optionally in strict (quoted) form.",
            valuetotext
        ),
        f!(
            "ARRAYTOTEXT",
            1,
            2,
            Text,
            ATT,
            "ARRAYTOTEXT(array, [format])",
            "Converts an array to text, either as a comma list or as a strict array constant.",
            arraytotext
        ),
        f!(
            "REGEXTEST",
            2,
            3,
            Text,
            S,
            "REGEXTEST(text, pattern, [case_sensitivity])",
            "TRUE when a regular expression matches somewhere in the text.",
            regextest
        ),
        f!(
            "REGEXEXTRACT",
            2,
            4,
            Text,
            S,
            "REGEXEXTRACT(text, pattern, [return_mode], [case_sensitivity])",
            "Extracts the first match, all matches or the capture groups of a regular expression.",
            regexextract
        ),
        f!(
            "REGEXREPLACE",
            3,
            5,
            Text,
            S,
            "REGEXREPLACE(text, pattern, replacement, [occurrence], [case_sensitivity])",
            "Replaces matches of a regular expression, all of them or a chosen occurrence.",
            regexreplace
        ),
        f!("ASC", 1, 1, Text, S, "ASC(text)", "Converts full-width characters to half-width (no change for single-byte text).", identity),
        f!("DBCS", 1, 1, Text, S, "DBCS(text)", "Converts half-width characters to full-width (no change in this locale).", identity),
        f!("JIS", 1, 1, Text, S, "JIS(text)", "Converts half-width characters to full-width (no change in this locale).", identity),
        f!("PHONETIC", 1, 1, Text, S, "PHONETIC(reference)", "Returns the phonetic (furigana) text; here the text itself.", identity),
    ]
}

#[cfg(test)]
#[path = "tests/text_tests.rs"]
mod tests;
