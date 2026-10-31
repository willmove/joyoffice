//! GridCraft data kept in hidden workbook-scope defined names, so that it travels inside the xlsx
//! file.
//!
//! An item `key` is stored as names `_gc_<key>_0`, `_gc_<key>_1`, …, each a formula of string
//! literals joined with `&`: `"…"&"…"`. Excel limits a string literal to 255 characters and a
//! formula to 8,192, so a long value is split over as many names as it needs and the file stays
//! one that Excel accepts. Older items (custom views, scripts, the watch list and scenarios) keep
//! one name each, `_sc_<key>`; reading accepts that form as well, so those items can move to this
//! format later, and a write of the same key replaces it.
//!
//! XML can't carry every character (controls other than tab, line feed and carriage return, and
//! U+FFFE and U+FFFF), and the xlsx writer drops those. In `_gc_` items each of them, and the
//! marker itself, is written as [`ESCAPE`] followed by its code as four upper-case hex digits
//! (`U+0001` becomes `\u{FDD0}0001`), and reading turns them back. The marker is a Unicode
//! noncharacter, which XML carries but real text does not use, so the escape costs nothing on
//! ordinary text. Legacy `_sc_` items are read as they are.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use joysheet_model::{DefinedName, Workbook};
use serde_json::Value as Json;

use crate::{EngineError, Result};

const PREFIX: &str = "_gc_";
const LEGACY_PREFIX: &str = "_sc_";
/// Characters (UTF-16 units, as Excel counts them) in one string literal.
const LITERAL_UNITS: usize = 255;
/// Characters in one name's formula.
const FORMULA_UNITS: usize = 8_192;
/// The longest key: `_gc_<key>_<k>` stays well inside Excel's 255-character names.
const MAX_KEY: usize = 200;
/// The longest item: 4,194,304 characters.
pub const MAX_ITEM_CHARS: usize = 4 * 1024 * 1024;
/// The most bytes the stored (escaped) form of an item can take: an escaped one-byte character
/// takes seven (the three-byte marker and four digits).
const MAX_STORED_BYTES: usize = MAX_ITEM_CHARS * 7;
/// Starts an escaped character in a `_gc_` item: U+FDD0, a noncharacter.
pub const ESCAPE: char = '\u{FDD0}';
/// The most items [`items`] and [`keys`] return: the first this many keys in sorted order. A
/// file can carry any number of hidden names; the rest are not listed.
pub const MAX_ITEMS: usize = 10_000;

/// The names that hold one item: its `_gc_` chunks by number (the first of each number) and its
/// legacy `_sc_` name.
#[derive(Default)]
struct Parts<'a> {
    chunks: BTreeMap<u64, &'a str>,
    legacy: Option<&'a str>,
}

impl<'a> Parts<'a> {
    fn add(&mut self, chunk: Option<u64>, formula: &'a str) {
        match chunk {
            Some(i) => {
                self.chunks.entry(i).or_insert(formula);
            }
            None => self.legacy = self.legacy.or(Some(formula)),
        }
    }

    /// Whether the names make an item: a later chunk on its own does not.
    fn is_item(&self) -> bool {
        self.chunks.contains_key(&0) || self.legacy.is_some()
    }
}

#[cfg(test)]
thread_local! {
    /// Names [`group`] has looked at on this thread, so a test can bound the work of a listing.
    static NAME_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The workbook's items whose lower-cased key starts with `prefix` (lower-cased), with the names
/// that hold them, in one pass over the names.
fn group<'a>(wb: &'a Workbook, prefix: &str) -> BTreeMap<String, Parts<'a>> {
    let prefix = prefix.to_ascii_lowercase();
    let mut items: BTreeMap<String, Parts<'a>> = BTreeMap::new();
    for n in wb.names.iter() {
        #[cfg(test)]
        NAME_VISITS.with(|v| v.set(v.get().saturating_add(1)));
        let Some((k, chunk)) = item_of(n) else { continue };
        let k = k.to_ascii_lowercase();
        if k.starts_with(&prefix) {
            items.entry(k).or_default().add(chunk, &n.formula);
        }
    }
    items
}

/// The item text stored under `key`, or `None` when there is none (or it is over the size limit).
pub fn get(wb: &Workbook, key: &str) -> Option<String> {
    let mut parts = Parts::default();
    for n in wb.names.iter() {
        if let Some((k, chunk)) = item_of(n)
            && k.eq_ignore_ascii_case(key)
        {
            parts.add(chunk, &n.formula);
        }
    }
    assemble(&parts)
}

/// The items whose key starts with `prefix` (`"macro_"`), by lower-cased key, read in one pass
/// over the names: the first [`MAX_ITEMS`] of [`keys`], without those over the size limit.
pub fn items(wb: &Workbook, prefix: &str) -> BTreeMap<String, String> {
    group(wb, prefix)
        .into_iter()
        .filter(|(_, parts)| parts.is_item())
        .take(MAX_ITEMS)
        .filter_map(|(k, parts)| assemble(&parts).map(|text| (k, text)))
        .collect()
}

/// The text of the item held by `parts`, as [`get`] reads it.
fn assemble(parts: &Parts) -> Option<String> {
    let chunks = &parts.chunks;
    let mut out = String::new();
    if !chunks.contains_key(&0) {
        out = string_literals(parts.legacy?);
    } else {
        // Chunks run 0, 1, 2, …; anything after a gap isn't part of the item.
        for (want, (i, f)) in chunks.iter().enumerate() {
            if *i != want as u64 {
                break;
            }
            out.push_str(&string_literals(f));
            if out.len() > MAX_STORED_BYTES {
                return None;
            }
        }
        out = unescape(&out);
    }
    (out.chars().count() <= MAX_ITEM_CHARS).then_some(out)
}

/// Whether `c` is stored escaped: XML can't carry it, or it is the marker.
fn needs_escape(c: char) -> bool {
    c == ESCAPE || !matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

/// Turns [`ESCAPE`] and four hex digits back into the character. A marker not followed by four
/// hex digits naming a character (a hand-made name) stays as it is.
fn unescape(s: &str) -> String {
    if !s.contains(ESCAPE) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find(ESCAPE) {
        out.push_str(rest.get(..at).unwrap_or(""));
        let after = rest.get(at + ESCAPE.len_utf8()..).unwrap_or("");
        let code = after.get(..4).filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit())).and_then(|h| u32::from_str_radix(h, 16).ok());
        match code.and_then(char::from_u32) {
            Some(c) => {
                out.push(c);
                rest = after.get(4..).unwrap_or("");
            }
            None => {
                out.push(ESCAPE);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Stores `text` under `key`, replacing what was there (in either format). Empty text removes it.
pub fn set(wb: &mut Workbook, key: &str, text: &str) -> Result<()> {
    check_key(key)?;
    if text.chars().count() > MAX_ITEM_CHARS {
        return Err(EngineError::Other(format!("The data is too large to keep in the workbook ({} MB at most).", MAX_ITEM_CHARS / (1024 * 1024))));
    }
    let formulas = chunk_formulas(text);
    // A visible name of the item's shape is someone's own (see `item_of`); a hidden name beside
    // it with the same name would be the one an xlsx file drops. Refuse before changing anything,
    // in one pass over the names (a file can carry many thousands).
    let chunks = formulas.len() as u64;
    let taken = |n: &&DefinedName| {
        n.scope.is_none() && !n.hidden && parse_name(&n.name).is_some_and(|(k, i)| k.eq_ignore_ascii_case(key) && i.is_some_and(|i| i < chunks))
    };
    if let Some(n) = wb.names.iter().find(taken) {
        return Err(EngineError::Other(format!(
            "The workbook has a defined name '{}', which GridCraft needs for its own data. Rename it first.",
            n.name
        )));
    }
    remove(wb, key);
    for (i, formula) in formulas.into_iter().enumerate() {
        wb.names.push(DefinedName { name: format!("{PREFIX}{key}_{i}"), scope: None, formula, comment: "GridCraft data".into(), hidden: true });
    }
    Ok(())
}

/// Removes the item stored under `key`.
pub fn remove(wb: &mut Workbook, key: &str) {
    wb.names.retain(|n| !item_of(n).is_some_and(|(k, _)| k.eq_ignore_ascii_case(key)));
}

/// The item under `key` as JSON (`None` when missing or not JSON).
pub fn get_json(wb: &Workbook, key: &str) -> Option<Json> {
    serde_json::from_str(&get(wb, key)?).ok()
}

pub fn set_json(wb: &mut Workbook, key: &str, v: &Json) -> Result<()> {
    set(wb, key, &v.to_string())
}

/// The keys of the items stored in the workbook that start with `prefix` (`"macro_"`), sorted
/// and lower-cased: the first [`MAX_ITEMS`] of them. To read them all, [`items`] takes one pass
/// where a [`get`] per key would take one each.
pub fn keys(wb: &Workbook, prefix: &str) -> Vec<String> {
    group(wb, prefix).into_iter().filter(|(_, parts)| parts.is_item()).map(|(k, _)| k).take(MAX_ITEMS).collect()
}

/// Whether the workbook holds an item under `name` (compared without case): a hidden workbook-scope
/// `_gc_<key>_<k>` name, or a workbook-scope legacy `_sc_<key>` name, which is read whether or not
/// it is hidden. A visible `_gc_` name is the person's own.
pub fn holds_item(wb: &Workbook, name: &str) -> bool {
    wb.names.iter().any(|n| n.name.eq_ignore_ascii_case(name) && item_of(n).is_some())
}

/// The item a hidden name belongs to, lower-cased: `_gc_<key>_<k>` and `_sc_<key>` give `<key>`.
pub fn item_key(name: &str) -> Option<String> {
    parse_name(name).map(|(k, _)| k.to_ascii_lowercase())
}

/// The item a name holds a part of: a hidden workbook-scope `_gc_<key>_<k>` name, or a legacy
/// `_sc_<key>` one. A visible name of the `_gc_` shape is someone's own name, not an item: it is
/// neither read, nor listed, nor removed on write.
fn item_of(n: &DefinedName) -> Option<(&str, Option<u64>)> {
    if n.scope.is_some() {
        return None;
    }
    parse_name(&n.name).filter(|(_, chunk)| chunk.is_none() || n.hidden)
}

/// `_gc_<key>_<k>` → (`key`, `Some(k)`); `_sc_<key>` → (`key`, `None`).
fn parse_name(name: &str) -> Option<(&str, Option<u64>)> {
    if let Some(rest) = strip_prefix_ci(name, PREFIX) {
        let (key, k) = rest.rsplit_once('_')?;
        // Only the chunk number as `set` writes it: `_gc_watch_00` is someone else's name, not
        // chunk 0 under another spelling, so it is neither read nor in the way of `_gc_watch_0`.
        let canonical = !k.is_empty() && k.len() <= 9 && k.bytes().all(|b| b.is_ascii_digit()) && (k == "0" || !k.starts_with('0'));
        if key.is_empty() || !canonical {
            return None;
        }
        return k.parse().ok().map(|i| (key, Some(i)));
    }
    strip_prefix_ci(name, LEGACY_PREFIX).filter(|k| !k.is_empty()).map(|k| (k, None))
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| s.get(prefix.len()..)).flatten()
}

fn check_key(key: &str) -> Result<()> {
    let ok = !key.is_empty() && key.len() <= MAX_KEY && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.');
    if ok { Ok(()) } else { Err(EngineError::Other(format!("`{key}` can't name workbook data: use letters, digits, `_` and `.`"))) }
}

/// `text` as name formulas: string literals of at most 255 characters joined with `&`, each formula
/// at most 8,192 characters, with the characters XML can't carry escaped (see [`ESCAPE`]).
/// Concatenating [`string_literals`] of each and turning the escapes back gives `text` back.
pub fn chunk_formulas(text: &str) -> Vec<String> {
    let mut formulas = Vec::new();
    let (mut formula, mut units) = (String::new(), 0usize);
    let (mut literal, mut lit_units) = (String::from("\""), 0usize);
    let mut flush_literal = |literal: &mut String, lit_units: &mut usize, formula: &mut String, units: &mut usize| {
        literal.push('"');
        let cost = literal.encode_utf16().count() + usize::from(!formula.is_empty());
        if *units + cost > FORMULA_UNITS && !formula.is_empty() {
            formulas.push(std::mem::take(formula));
            *units = 0;
        }
        if !formula.is_empty() {
            formula.push('&');
            *units += 1;
        }
        *units += literal.encode_utf16().count();
        formula.push_str(literal);
        *literal = String::from("\"");
        *lit_units = 0;
    };
    for ch in text.chars() {
        // An escaped character is the marker and four digits, kept in one literal.
        let escaped = needs_escape(ch);
        let cost = if escaped { ESCAPE.len_utf16() + 4 } else { ch.len_utf16() };
        if lit_units + cost > LITERAL_UNITS {
            flush_literal(&mut literal, &mut lit_units, &mut formula, &mut units);
        }
        lit_units += cost;
        if escaped {
            literal.push(ESCAPE);
            let _ = write!(literal, "{:04X}", u32::from(ch));
        } else {
            if ch == '"' {
                literal.push('"');
            }
            literal.push(ch);
        }
    }
    if lit_units > 0 {
        flush_literal(&mut literal, &mut lit_units, &mut formula, &mut units);
    }
    if !formula.is_empty() {
        formulas.push(formula);
    }
    formulas
}

/// Concatenates the string literals of a `"a"&"b""c"` formula (anything outside them is ignored).
pub fn string_literals(f: &str) -> String {
    let mut out = String::new();
    let mut it = f.chars().peekable();
    let mut in_str = false;
    while let Some(c) = it.next() {
        if !in_str {
            if c == '"' {
                in_str = true;
            }
            continue;
        }
        if c == '"' {
            if it.peek() == Some(&'"') {
                it.next();
                out.push('"');
            } else {
                in_str = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
#[path = "tests/stash_tests.rs"]
mod tests;
