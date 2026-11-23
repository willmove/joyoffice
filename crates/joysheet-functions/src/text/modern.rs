//! First-party modern delimiter-splitting and regular-expression text functions:
//! `TEXTBEFORE`, `TEXTAFTER`, `TEXTSPLIT`, `REGEXTEST`, `REGEXEXTRACT`, and `REGEXREPLACE`.

use std::borrow::Cow;

use joysheet_core::{CellError, Value};

use super::classic::check_len;
use crate::util::{MAX_CELLS, MAX_TEXT, R, arg, array_val, has, int, opt_bool, opt_int, scalar, text, text_val};
use crate::{Arg, Ctx};

// ---------------------------------------------------------------------------------------------
// Delimiter matching for TEXTBEFORE / TEXTAFTER / TEXTSPLIT

fn fold_chars(s: &[char]) -> Vec<char> {
    s.iter().map(|ch| ch.to_lowercase().next().unwrap_or(*ch)).collect()
}

fn read_delimiters(v: &Value) -> R<Vec<String>> {
    match v {
        Value::Array(arr) => arr.iter().map(Value::to_text).collect(),
        other => Ok(vec![other.to_text()?]),
    }
}

/// Returns non-overlapping byte-offset match spans `(start, end)` in `hay` when
/// both `hay` and all non-empty delimiters are ASCII, avoiding `Vec<char>` allocation.
fn find_matches_ascii(hay: &str, delims: &[String], insensitive: bool, from_end: bool) -> Vec<(usize, usize)> {
    let ds: Vec<&[u8]> = delims.iter().filter(|d| !d.is_empty()).map(String::as_bytes).collect();
    if ds.is_empty() {
        return (0..=hay.len()).map(|i| (i, i)).collect();
    }
    let hb = hay.as_bytes();
    let eq = |window: &[u8], pat: &[u8]| if insensitive { window.eq_ignore_ascii_case(pat) } else { window == pat };
    let mut out = Vec::new();
    if from_end {
        let mut j = hb.len();
        while j > 0 {
            let best = ds.iter().filter(|d| j.checked_sub(d.len()).and_then(|st| hb.get(st..j)).is_some_and(|w| eq(w, d))).map(|d| d.len()).max();
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
    while i < hb.len() {
        let best = ds.iter().filter(|d| hb.get(i..i + d.len()).is_some_and(|w| eq(w, d))).map(|d| d.len()).max();
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

/// Returns non-overlapping char-index match spans `(start, end)` on arbitrary Unicode text.
fn find_matches_unicode(hay: &[char], delims: &[String], insensitive: bool, from_end: bool) -> Vec<(usize, usize)> {
    let t: Cow<'_, [char]> = if insensitive { Cow::Owned(fold_chars(hay)) } else { Cow::Borrowed(hay) };
    let ds: Vec<Vec<char>> = delims
        .iter()
        .filter(|d| !d.is_empty())
        .map(|d| {
            let raw: Vec<char> = d.chars().collect();
            if insensitive { fold_chars(&raw) } else { raw }
        })
        .collect();
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
    let raw = text(a, 0)?;
    let delims = read_delimiters(&arg(a, 1)?.value)?;
    let inst = if has(a, 2) { int(a, 2)? } else { 1 };
    let mode = opt_int(a, 3, 0)?;
    let match_end = opt_bool(a, 4, false)?;
    let ascii_only = raw.is_ascii() && delims.iter().all(|d| d.is_ascii());
    let char_count = if ascii_only { raw.len() } else { raw.chars().count() };
    if !(0..=1).contains(&mode) || inst == 0 || inst.unsigned_abs() as usize > char_count.max(1) {
        return Err(CellError::Value);
    }
    let insensitive = mode == 1;
    let from_end = inst < 0;
    let k = inst.unsigned_abs() as usize;

    if ascii_only {
        let mut matches = find_matches_ascii(&raw, &delims, insensitive, from_end);
        if match_end {
            if inst > 0 {
                if matches.last().is_none_or(|x| x.0 != raw.len()) {
                    matches.push((raw.len(), raw.len()));
                }
            } else if matches.first().is_none_or(|x| x.0 != 0) {
                matches.insert(0, (0, 0));
            }
        }
        let hit = if inst > 0 { matches.get(k - 1) } else { matches.len().checked_sub(k).and_then(|i| matches.get(i)) };
        return match hit {
            Some(&(st, en)) => text_val(if before { raw.get(..st).unwrap_or("") } else { raw.get(en..).unwrap_or("") }),
            None => match a.get(5) {
                Some(x) => Ok(scalar(x)),
                None => Err(CellError::NA),
            },
        };
    }

    let chars: Vec<char> = raw.chars().collect();
    let mut matches = find_matches_unicode(&chars, &delims, insensitive, from_end);
    if match_end {
        if inst > 0 {
            if matches.last().is_none_or(|x| x.0 != chars.len()) {
                matches.push((chars.len(), chars.len()));
            }
        } else if matches.first().is_none_or(|x| x.0 != 0) {
            matches.insert(0, (0, 0));
        }
    }
    let hit = if inst > 0 { matches.get(k - 1) } else { matches.len().checked_sub(k).and_then(|i| matches.get(i)) };
    match hit {
        Some(&(st, en)) => {
            let slice = if before { chars.get(..st) } else { chars.get(en..) }.unwrap_or(&[]);
            text_val(slice.iter().collect::<String>())
        }
        None => match a.get(5) {
            Some(x) => Ok(scalar(x)),
            None => Err(CellError::NA),
        },
    }
}

pub(super) fn textbefore(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    text_before_after(a, true)
}

pub(super) fn textafter(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    text_before_after(a, false)
}

fn split_ascii<'a>(s: &'a str, delims: &[String], insensitive: bool) -> Vec<&'a str> {
    if delims.iter().all(String::is_empty) {
        return vec![s];
    }
    let mut out = Vec::new();
    let mut last = 0;
    for (st, en) in find_matches_ascii(s, delims, insensitive, false) {
        out.push(s.get(last..st).unwrap_or(""));
        last = en;
    }
    out.push(s.get(last..).unwrap_or(""));
    out
}

fn split_unicode<'a>(s: &'a [char], delims: &[String], insensitive: bool) -> Vec<&'a [char]> {
    if delims.iter().all(String::is_empty) {
        return vec![s];
    }
    let mut out = Vec::new();
    let mut last = 0;
    for (st, en) in find_matches_unicode(s, delims, insensitive, false) {
        out.push(s.get(last..st).unwrap_or(&[]));
        last = en;
    }
    out.push(s.get(last..).unwrap_or(&[]));
    out
}

pub(super) fn textsplit(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let raw = text(a, 0)?;
    let col_d = if has(a, 1) { read_delimiters(&arg(a, 1)?.value)? } else { Vec::new() };
    let row_d = if has(a, 2) { read_delimiters(&arg(a, 2)?.value)? } else { Vec::new() };
    if col_d.iter().all(String::is_empty) && row_d.iter().all(String::is_empty) {
        return Err(CellError::Value);
    }
    let ignore = opt_bool(a, 3, false)?;
    let mode = opt_int(a, 4, 0)?;
    if !(0..=1).contains(&mode) {
        return Err(CellError::Value);
    }
    let insensitive = mode == 1;
    let pad = if has(a, 5) { scalar(arg(a, 5)?) } else { Value::Error(CellError::NA) };

    let ascii_only = raw.is_ascii() && col_d.iter().all(|d| d.is_ascii()) && row_d.iter().all(|d| d.is_ascii());
    let rows: Vec<Vec<String>> = if ascii_only {
        let mut rows = Vec::new();
        for r in split_ascii(&raw, &row_d, insensitive) {
            if ignore && r.is_empty() {
                continue;
            }
            let cells: Vec<String> =
                split_ascii(r, &col_d, insensitive).into_iter().filter(|c| !(ignore && c.is_empty())).map(str::to_string).collect();
            if ignore && cells.is_empty() {
                continue;
            }
            rows.push(cells);
        }
        rows
    } else {
        let chars: Vec<char> = raw.chars().collect();
        let mut rows = Vec::new();
        for r in split_unicode(&chars, &row_d, insensitive) {
            if ignore && r.is_empty() {
                continue;
            }
            let cells: Vec<String> =
                split_unicode(r, &col_d, insensitive).into_iter().filter(|c| !(ignore && c.is_empty())).map(|c| c.iter().collect()).collect();
            if ignore && cells.is_empty() {
                continue;
            }
            rows.push(cells);
        }
        rows
    };

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

// ---------------------------------------------------------------------------------------------
// Regular expressions: REGEXTEST / REGEXEXTRACT / REGEXREPLACE

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

pub(super) fn regextest(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let re = build_regex(&text(a, 1)?, case_arg(a, 2)?)?;
    Ok(Value::Bool(re.is_match(&s)))
}

pub(super) fn regexextract(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let mode = opt_int(a, 2, 0)?;
    let re = build_regex(&text(a, 1)?, case_arg(a, 3)?)?;
    match mode {
        0 => re.find(&s).map(|m| Value::from(m.as_str())).ok_or(CellError::NA),
        1 => {
            let all: Vec<Value> = re.find_iter(&s).take(MAX_CELLS).map(|m| Value::from(m.as_str())).collect();
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

/// Converts Excel-style `$1` references into unambiguous `${1}` syntax on UTF-8 slices.
fn normalize_excel_backrefs(rep: &str) -> Cow<'_, str> {
    if !rep.contains('$') {
        return Cow::Borrowed(rep);
    }
    let bytes = rep.as_bytes();
    let mut out = String::with_capacity(rep.len() + 4);
    let mut i = 0;
    while i < bytes.len() {
        if bytes.get(i) == Some(&b'$') {
            if bytes.get(i + 1) == Some(&b'$') {
                out.push_str("$$");
                i += 2;
                continue;
            }
            if bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
                let mut j = i + 1;
                while bytes.get(j).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                }
                out.push_str("${");
                out.push_str(rep.get(i + 1..j).unwrap_or(""));
                out.push('}');
                i = j;
                continue;
            }
        }
        if let Some(ch) = rep.get(i..).and_then(|tail| tail.chars().next()) {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    Cow::Owned(out)
}

/// Piece of a compiled regex replacement template.
pub(super) enum Piece {
    Text(String),
    Index(usize),
    Name(String),
}

/// Compiles a replacement template following `regex::Captures::expand` rules:
/// `$$` emits `$`, `${name}` spans until `}`, `$name` spans `[_0-9A-Za-z]+`,
/// purely numeric names index capture groups, and a bare `$` stays literal.
pub(super) fn template(rep: &str) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut rest = rep;
    while let Some(i) = rest.find('$') {
        lit.push_str(rest.get(..i).unwrap_or(""));
        rest = rest.get(i + 1..).unwrap_or("");
        if let Some(after) = rest.strip_prefix('$') {
            lit.push('$');
            rest = after;
            continue;
        }
        let braced = rest.strip_prefix('{').and_then(|b| b.find('}').map(|j| (b.get(..j).unwrap_or(""), b.get(j + 1..).unwrap_or(""))));
        let (name, after) = braced.unwrap_or_else(|| {
            let j = rest.find(|c: char| !(c == '_' || c.is_ascii_alphanumeric())).unwrap_or(rest.len());
            (rest.get(..j).unwrap_or(""), rest.get(j..).unwrap_or(""))
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

/// Appends one match expansion to `out`, returning `None` before exceeding `limit` bytes.
pub(super) fn expand_capped(cap: &regex::Captures, pieces: &[Piece], out: &mut String, limit: usize) -> Option<()> {
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

pub(super) fn regexreplace(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let s = text(a, 0)?;
    let raw_rep = text(a, 2)?;
    let normalized = normalize_excel_backrefs(&raw_rep);
    let rep = template(&normalized);
    let occ = opt_int(a, 3, 0)?;
    let re = build_regex(&text(a, 1)?, case_arg(a, 4)?)?;
    let limit = MAX_TEXT * 4;
    let mut out = String::new();
    if occ == 0 {
        let mut last = 0;
        for cap in re.captures_iter(&s) {
            let Some(m) = cap.get(0) else { continue };
            let prefix = s.get(last..m.start()).unwrap_or_default();
            if out.len().saturating_add(prefix.len()) > limit {
                return Err(CellError::Value);
            }
            out.push_str(prefix);
            expand_capped(&cap, &rep, &mut out, limit).ok_or(CellError::Value)?;
            last = m.end();
        }
        let tail = s.get(last..).unwrap_or_default();
        if out.len().saturating_add(tail.len()) > limit {
            return Err(CellError::Value);
        }
        out.push_str(tail);
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
