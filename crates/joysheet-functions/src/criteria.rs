//! Excel criteria predicates (COUNTIF, SUMIFS, AVERAGEIFS, MAXIFS, MINIFS, DCOUNT…) and
//! pre-compiled wildcard matching implemented from Microsoft's public Excel specifications.

use std::cmp::Ordering;

use joysheet_core::parse::parse_number_text_in;
use joysheet_core::{CellError, DateSystem, Value, compare_numbers, compare_text};

/// Longest text criterion accepted by single-pair criteria functions, measured in UTF-16 code
/// units (surrogate-pair characters count as 2 units). Multi-pair `*IFS` functions truncate at
/// this boundary instead of returning `#VALUE!`.
const MAX_CRITERION_UTF16_UNITS: usize = 255;

/// Step cap per wildcard match or search call so pathological patterns with many `*` tokens stay
/// strictly bounded rather than going quadratic.
const MAX_STEPS: u64 = 1 << 26;

/// Stack buffer size (in chars) used when folding short non-ASCII text without heap allocation.
const STACK_CHARS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WildTok {
    Char(char),
    Any,
    Star,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AsciiTok {
    Byte(u8),
    Any,
    Star,
}

/// Pre-compiled case-insensitive Excel wildcard pattern (`*`, `?`, and `~` escapes).
#[derive(Clone, Debug)]
pub(crate) struct WildcardPattern(WildKind);

#[derive(Clone, Debug)]
enum WildKind {
    AnyText,
    Exact { chars: Vec<char>, ascii: Option<Box<[u8]>> },
    Prefix { chars: Vec<char>, ascii: Option<Box<[u8]>> },
    Suffix { chars: Vec<char>, ascii: Option<Box<[u8]>> },
    Contains { chars: Vec<char>, ascii: Option<Box<[u8]>> },
    General { tokens: Vec<WildTok>, min_len: usize, ascii_tokens: Option<Vec<AsciiTok>> },
}

impl WildcardPattern {
    pub(crate) fn compile(pattern: &str) -> Self {
        let tokens = compile_tokens(pattern);
        let kind = match tokens.as_slice() {
            [WildTok::Star] => WildKind::AnyText,
            _ => {
                let starts_star = matches!(tokens.first(), Some(WildTok::Star));
                let ends_star = tokens.len() > 1 && matches!(tokens.last(), Some(WildTok::Star));
                let inner_start = usize::from(starts_star);
                let inner_end = tokens.len().saturating_sub(usize::from(ends_star));
                let inner = tokens.get(inner_start..inner_end).unwrap_or(&[]);
                if !inner.is_empty() && inner.iter().all(|t| matches!(t, WildTok::Char(_))) {
                    let chars: Vec<char> = inner
                        .iter()
                        .filter_map(|t| match t {
                            WildTok::Char(c) => Some(*c),
                            _ => None,
                        })
                        .collect();
                    let ascii = to_ascii_bytes(&chars);
                    match (starts_star, ends_star) {
                        (false, false) => WildKind::Exact { chars, ascii },
                        (false, true) => WildKind::Prefix { chars, ascii },
                        (true, false) => WildKind::Suffix { chars, ascii },
                        (true, true) => WildKind::Contains { chars, ascii },
                    }
                } else if tokens.is_empty() {
                    WildKind::Exact { chars: Vec::new(), ascii: Some(Box::new([])) }
                } else {
                    let min_len = tokens.iter().filter(|t| !matches!(t, WildTok::Star)).count();
                    let ascii_tokens = to_ascii_tokens(&tokens);
                    WildKind::General { tokens, min_len, ascii_tokens }
                }
            }
        };
        Self(kind)
    }

    pub(crate) fn matches(&self, text: &str) -> bool {
        match &self.0 {
            WildKind::AnyText => true,
            WildKind::Exact { chars, ascii } => {
                if text.is_ascii()
                    && let Some(pat) = ascii.as_deref()
                {
                    return text.len() == pat.len() && text.as_bytes().iter().zip(pat).all(|(a, b)| a.to_ascii_lowercase() == *b);
                }
                let mut it = text.chars().map(fold_char);
                for &expected in chars {
                    if it.next() != Some(expected) {
                        return false;
                    }
                }
                it.next().is_none()
            }
            WildKind::Prefix { chars, ascii } => {
                if text.is_ascii()
                    && let Some(pat) = ascii.as_deref()
                {
                    return text.as_bytes().get(..pat.len()).is_some_and(|head| head.iter().zip(pat).all(|(a, b)| a.to_ascii_lowercase() == *b));
                }
                let mut it = text.chars().map(fold_char);
                chars.iter().all(|&expected| it.next() == Some(expected))
            }
            WildKind::Suffix { chars, ascii } => {
                if text.is_ascii()
                    && let Some(pat) = ascii.as_deref()
                {
                    return text
                        .len()
                        .checked_sub(pat.len())
                        .and_then(|start| text.as_bytes().get(start..))
                        .is_some_and(|tail| tail.iter().zip(pat).all(|(a, b)| a.to_ascii_lowercase() == *b));
                }
                let mut it = text.chars().rev().map(fold_char);
                chars.iter().rev().all(|&expected| it.next() == Some(expected))
            }
            WildKind::Contains { chars, ascii } => {
                if text.is_ascii()
                    && let Some(pat) = ascii.as_deref()
                {
                    let Some((&first, rest)) = pat.split_first() else { return true };
                    let bytes = text.as_bytes();
                    let Some(last_start) = bytes.len().checked_sub(pat.len()) else { return false };
                    for i in 0..=last_start {
                        if let Some(&b) = bytes.get(i)
                            && b.to_ascii_lowercase() == first
                            && bytes.get(i + 1..i + pat.len()).is_some_and(|tail| tail.iter().zip(rest).all(|(a, c)| a.to_ascii_lowercase() == *c))
                        {
                            return true;
                        }
                    }
                    return false;
                }
                let Some((&first, rest)) = chars.split_first() else { return true };
                with_folded_chars(text, |hay| {
                    let Some(last_start) = hay.len().checked_sub(chars.len()) else { return false };
                    for i in 0..=last_start {
                        if hay.get(i) == Some(&first) && hay.get(i + 1..i + chars.len()) == Some(rest) {
                            return true;
                        }
                    }
                    false
                })
            }
            WildKind::General { tokens, min_len, ascii_tokens } => {
                if text.is_ascii()
                    && let Some(pat) = ascii_tokens.as_deref()
                {
                    if text.len() < *min_len {
                        return false;
                    }
                    let mut budget = MAX_STEPS;
                    return match_ascii(pat, text.as_bytes(), true, &mut budget).is_some();
                }
                with_folded_chars(text, |hay| {
                    if hay.len() < *min_len {
                        return false;
                    }
                    let mut budget = MAX_STEPS;
                    match_chars(tokens, hay, true, &mut budget).is_some()
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
enum Operand {
    Number(f64),
    Bool(bool),
    Error(CellError),
    Pattern(WildcardPattern),
    Text(String),
    /// A criterion truncated at 255 UTF-16 code units in the middle of a surrogate pair: the
    /// complete scalar values before the cut represent a prefix followed by a lone high surrogate
    /// in Excel's UTF-16 buffer. No valid UTF-8 cell text contains a lone surrogate, so `=` matches
    /// nothing, `<>` matches everything, and order comparisons split just above `kept`.
    CutText(String),
    /// Nothing after the operator (`"="`, `"<>"`, `"<="`, …).
    Blank,
}

/// A compiled Excel cell predicate.
#[derive(Clone, Debug)]
pub(crate) struct Criterion {
    op: Op,
    operand: Operand,
    /// Bare `""` (empty text literal or empty-string cell): matches both blank cells and `""`.
    bare_empty: bool,
    date_system: DateSystem,
}

impl Criterion {
    /// Compiles a criterion value using the default 1900 date system.
    #[cfg(test)]
    pub(crate) fn parse(v: &Value) -> Criterion {
        Criterion::parse_in(v, DateSystem::D1900)
    }

    /// Compiles a criterion value using the workbook's date system.
    pub(crate) fn parse_in(v: &Value, date_system: DateSystem) -> Criterion {
        match v {
            Value::Number(n) => Criterion { op: Op::Eq, operand: Operand::Number(*n), bare_empty: false, date_system },
            Value::Bool(b) => Criterion { op: Op::Eq, operand: Operand::Bool(*b), bare_empty: false, date_system },
            Value::Error(e) => Criterion { op: Op::Eq, operand: Operand::Error(*e), bare_empty: false, date_system },
            Value::Empty => Criterion { op: Op::Eq, operand: Operand::Number(0.0), bare_empty: false, date_system },
            Value::Array(a) => Criterion::parse_in(a.data.first().unwrap_or(&Value::Empty), date_system),
            Value::Text(t) => Criterion::parse_text(t, date_system),
        }
    }

    /// Compiles the criterion argument of `COUNTIF`, `SUMIF`, `AVERAGEIF`, or a single-pair
    /// `*IFS` call: text exceeding 255 UTF-16 code units returns `#VALUE!`.
    pub(crate) fn parse_arg_in(v: &Value, date_system: DateSystem) -> Result<Criterion, CellError> {
        match v {
            Value::Array(a) => Criterion::parse_arg_in(a.data.first().unwrap_or(&Value::Empty), date_system),
            Value::Text(t) if exceeds_utf16_limit(t, MAX_CRITERION_UTF16_UNITS) => Err(CellError::Value),
            other => Ok(Criterion::parse_in(other, date_system)),
        }
    }

    /// Compiles a criterion argument of a multi-pair `*IFS` call: text exceeding 255 UTF-16 code
    /// units is truncated at the 255-unit boundary instead of returning `#VALUE!`.
    pub(crate) fn parse_arg_truncated_in(v: &Value, date_system: DateSystem) -> Criterion {
        match v {
            Value::Array(a) => Criterion::parse_arg_truncated_in(a.data.first().unwrap_or(&Value::Empty), date_system),
            Value::Text(t) if t.len() > MAX_CRITERION_UTF16_UNITS => {
                let mut units = 0usize;
                for (byte_idx, ch) in t.char_indices() {
                    let step = ch.len_utf16();
                    if units + step > MAX_CRITERION_UTF16_UNITS {
                        if units == MAX_CRITERION_UTF16_UNITS {
                            return Criterion::parse_text(&t[..byte_idx], date_system);
                        }
                        let (op, rest) = split_op(&t[..byte_idx]);
                        return Criterion { op, operand: Operand::CutText(rest.to_owned()), bare_empty: false, date_system };
                    }
                    units += step;
                }
                Criterion::parse_text(t, date_system)
            }
            other => Criterion::parse_in(other, date_system),
        }
    }

    fn parse_text(t: &str, date_system: DateSystem) -> Criterion {
        if t.is_empty() {
            return Criterion { op: Op::Eq, operand: Operand::Blank, bare_empty: true, date_system };
        }
        let (op, rest) = split_op(t);
        let operand = if rest.is_empty() {
            Operand::Blank
        } else if let Some(n) = parse_number_text_in(rest, date_system) {
            Operand::Number(n)
        } else if rest.eq_ignore_ascii_case("TRUE") {
            Operand::Bool(true)
        } else if rest.eq_ignore_ascii_case("FALSE") {
            Operand::Bool(false)
        } else if let Some(e) = CellError::parse(rest) {
            Operand::Error(e)
        } else if matches!(op, Op::Eq | Op::Ne) {
            Operand::Pattern(WildcardPattern::compile(rest))
        } else {
            Operand::Text(rest.to_owned())
        };
        Criterion { op, operand, bare_empty: false, date_system }
    }

    /// Evaluates the predicate against a borrowed cell value without cloning or heap allocation.
    pub(crate) fn matches(&self, v: &Value) -> bool {
        let cell = match v {
            Value::Array(a) => a.data.first().unwrap_or(&Value::Empty),
            other => other,
        };
        if self.bare_empty {
            return matches!(cell, Value::Empty) || matches!(cell, Value::Text(t) if t.is_empty());
        }
        match &self.operand {
            Operand::Blank => match self.op {
                Op::Eq => matches!(cell, Value::Empty),
                Op::Ne => !matches!(cell, Value::Empty),
                _ => matches!(cell, Value::Text(t) if !t.is_empty() && cmp_op(self.op, compare_text(t, ""))),
            },
            Operand::Number(n) => {
                let candidate = match cell {
                    Value::Number(x) => Some(*x),
                    Value::Text(t) if matches!(self.op, Op::Eq | Op::Ne) => parse_number_text_in(t, self.date_system),
                    _ => None,
                };
                match candidate {
                    Some(x) => cmp_op(self.op, compare_numbers(x, *n)),
                    None => self.op == Op::Ne,
                }
            }
            Operand::Bool(b) => match cell {
                Value::Bool(x) => cmp_op(self.op, x.cmp(b)),
                _ => self.op == Op::Ne,
            },
            Operand::Error(e) => match cell {
                Value::Error(x) => cmp_op(self.op, x.code().cmp(&e.code())),
                _ => self.op == Op::Ne,
            },
            Operand::Pattern(pat) => {
                let matched = matches!(cell, Value::Text(t) if pat.matches(t));
                if self.op == Op::Eq { matched } else { !matched }
            }
            Operand::Text(pat) => matches!(cell, Value::Text(t) if cmp_op(self.op, compare_text(t, pat))),
            Operand::CutText(kept) => match self.op {
                Op::Eq => false,
                Op::Ne => true,
                Op::Lt | Op::Le => matches!(cell, Value::Text(t) if compare_text(t, kept) != Ordering::Greater),
                Op::Gt | Op::Ge => matches!(cell, Value::Text(t) if compare_text(t, kept) == Ordering::Greater),
            },
        }
    }
}

fn exceeds_utf16_limit(s: &str, limit: usize) -> bool {
    // Every UTF-16 code unit needs at least 1 UTF-8 byte, so short strings skip UTF-16 decoding.
    if s.len() <= limit {
        return false;
    }
    let mut units = 0usize;
    for ch in s.chars() {
        units += ch.len_utf16();
        if units > limit {
            return true;
        }
    }
    false
}

fn split_op(t: &str) -> (Op, &str) {
    if let Some(r) = t.strip_prefix(">=") {
        (Op::Ge, r)
    } else if let Some(r) = t.strip_prefix("<=") {
        (Op::Le, r)
    } else if let Some(r) = t.strip_prefix("<>") {
        (Op::Ne, r)
    } else if let Some(r) = t.strip_prefix('=') {
        (Op::Eq, r)
    } else if let Some(r) = t.strip_prefix('>') {
        (Op::Gt, r)
    } else if let Some(r) = t.strip_prefix('<') {
        (Op::Lt, r)
    } else {
        (Op::Eq, t)
    }
}

fn cmp_op(op: Op, ord: Ordering) -> bool {
    match op {
        Op::Eq => ord == Ordering::Equal,
        Op::Ne => ord != Ordering::Equal,
        Op::Lt => ord == Ordering::Less,
        Op::Le => ord != Ordering::Greater,
        Op::Gt => ord == Ordering::Greater,
        Op::Ge => ord != Ordering::Less,
    }
}

/// Folds one Unicode scalar to a single lowercase scalar so character indices stay aligned
/// (e.g. U+0130 `İ` folds to `'i'`).
#[inline]
fn fold_char(c: char) -> char {
    if c.is_ascii() { c.to_ascii_lowercase() } else { c.to_lowercase().next().unwrap_or(c) }
}

fn compile_tokens(pattern: &str) -> Vec<WildTok> {
    let mut out = Vec::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '~' => match chars.next() {
                Some(escaped) => out.push(WildTok::Char(fold_char(escaped))),
                None => out.push(WildTok::Char('~')),
            },
            '*' => {
                if !matches!(out.last(), Some(WildTok::Star)) {
                    out.push(WildTok::Star);
                }
            }
            '?' => out.push(WildTok::Any),
            other => out.push(WildTok::Char(fold_char(other))),
        }
    }
    out
}

fn to_ascii_bytes(chars: &[char]) -> Option<Box<[u8]>> {
    if !chars.iter().all(char::is_ascii) {
        return None;
    }
    let bytes: Vec<u8> = chars.iter().map(|&c| c as u8).collect();
    Some(bytes.into_boxed_slice())
}

fn to_ascii_tokens(tokens: &[WildTok]) -> Option<Vec<AsciiTok>> {
    let mut out = Vec::with_capacity(tokens.len());
    for &tok in tokens {
        match tok {
            WildTok::Char(c) if c.is_ascii() => out.push(AsciiTok::Byte(c as u8)),
            WildTok::Char(_) => return None,
            WildTok::Any => out.push(AsciiTok::Any),
            WildTok::Star => out.push(AsciiTok::Star),
        }
    }
    Some(out)
}

fn with_folded_chars<R>(text: &str, f: impl FnOnce(&[char]) -> R) -> R {
    if text.len() <= STACK_CHARS {
        let mut buf = ['\0'; STACK_CHARS];
        let mut len = 0usize;
        for ch in text.chars() {
            if let Some(slot) = buf.get_mut(len) {
                *slot = fold_char(ch);
                len += 1;
            }
        }
        f(buf.get(..len).unwrap_or(&[]))
    } else {
        let folded: Vec<char> = text.chars().map(fold_char).collect();
        f(&folded)
    }
}

fn match_ascii(p: &[AsciiTok], t: &[u8], whole: bool, budget: &mut u64) -> Option<usize> {
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    loop {
        *budget = budget.checked_sub(1)?;
        if pi == p.len() && (!whole || ti == t.len()) {
            return Some(ti);
        }
        let step = match (p.get(pi), t.get(ti)) {
            (Some(AsciiTok::Star), _) => {
                star = Some((pi, ti));
                pi += 1;
                continue;
            }
            (Some(AsciiTok::Any), Some(_)) => true,
            (Some(AsciiTok::Byte(c)), Some(&x)) => *c == x.to_ascii_lowercase(),
            _ => false,
        };
        if step {
            pi += 1;
            ti += 1;
            continue;
        }
        match star {
            Some((sp, st)) if st < t.len() => {
                star = Some((sp, st + 1));
                pi = sp + 1;
                ti = st + 1;
            }
            _ => return None,
        }
    }
}

fn match_chars(p: &[WildTok], t: &[char], whole: bool, budget: &mut u64) -> Option<usize> {
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    loop {
        *budget = budget.checked_sub(1)?;
        if pi == p.len() && (!whole || ti == t.len()) {
            return Some(ti);
        }
        let step = match (p.get(pi), t.get(ti)) {
            (Some(WildTok::Star), _) => {
                star = Some((pi, ti));
                pi += 1;
                continue;
            }
            (Some(WildTok::Any), Some(_)) => true,
            (Some(WildTok::Char(c)), Some(x)) => c == x,
            _ => false,
        };
        if step {
            pi += 1;
            ti += 1;
            continue;
        }
        match star {
            Some((sp, st)) if st < t.len() => {
                star = Some((sp, st + 1));
                pi = sp + 1;
                ti = st + 1;
            }
            _ => return None,
        }
    }
}

/// Whether `pattern` contains a wildcard or tilde escape character.
pub(crate) fn has_wildcards(pattern: &str) -> bool {
    pattern.contains(['*', '?', '~'])
}

/// Case-insensitive whole-text match with `*`, `?`, and `~` escapes.
pub(crate) fn wildcard_match(pattern: &str, text: &str) -> bool {
    WildcardPattern::compile(pattern).matches(text)
}

/// `SEARCH`-style find: 0-based character index of the first position at or after `start` where
/// the wildcard pattern matches case-insensitively.
pub(crate) fn wildcard_find(pattern: &str, text: &str, start: usize) -> Option<usize> {
    let tokens = compile_tokens(pattern);
    let p = match tokens.first() {
        Some(WildTok::Star) => tokens.get(1..).unwrap_or(&[]),
        _ => tokens.as_slice(),
    };
    if text.is_ascii()
        && let Some(ap) = to_ascii_tokens(p)
    {
        let bytes = text.as_bytes();
        if ap.is_empty() {
            return (start <= bytes.len()).then_some(start);
        }
        let min_len = ap.iter().filter(|t| !matches!(t, AsciiTok::Star)).count();
        let anys = ap.iter().take_while(|t| matches!(t, AsciiTok::Any)).count();
        let anchor = ap.get(anys).copied();
        let mut budget = MAX_STEPS;
        let mut i = start;
        while i.saturating_add(min_len) <= bytes.len() {
            let hit = match (anchor, bytes.get(i + anys)) {
                (Some(AsciiTok::Byte(c)), Some(&b)) => b.to_ascii_lowercase() == c && match_ascii(&ap, &bytes[i..], false, &mut budget).is_some(),
                (Some(AsciiTok::Byte(_)), None) => break,
                _ => match_ascii(&ap, &bytes[i..], false, &mut budget).is_some(),
            };
            if hit {
                return Some(i);
            }
            i += 1;
        }
        return None;
    }

    with_folded_chars(text, |hay| {
        if p.is_empty() {
            return (start <= hay.len()).then_some(start);
        }
        let min_len = p.iter().filter(|t| !matches!(t, WildTok::Star)).count();
        let anys = p.iter().take_while(|t| matches!(t, WildTok::Any)).count();
        let anchor = p.get(anys).copied();
        let mut budget = MAX_STEPS;
        let mut i = start;
        while i.saturating_add(min_len) <= hay.len() {
            let hit = match (anchor, hay.get(i + anys)) {
                (Some(WildTok::Char(c)), Some(&ch)) => ch == c && match_chars(p, &hay[i..], false, &mut budget).is_some(),
                (Some(WildTok::Char(_)), None) => break,
                _ => match_chars(p, &hay[i..], false, &mut budget).is_some(),
            };
            if hit {
                return Some(i);
            }
            i += 1;
        }
        None
    })
}

/// Exact-or-wildcard equality used by `MATCH`/`VLOOKUP`/`XLOOKUP` exact modes: numbers compare
/// bit-for-bit (unlike `=` and criteria functions which use 15-significant-digit comparison).
pub(crate) fn lookup_equal(needle: &Value, hay: &Value, wild: bool) -> bool {
    match (needle, hay) {
        (Value::Text(p), Value::Text(t)) => {
            if wild {
                wildcard_match(p, t)
            } else {
                compare_text(p, t) == Ordering::Equal
            }
        }
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Error(a), Value::Error(b)) => a == b,
        (Value::Empty, Value::Empty) => true,
        _ => false,
    }
}

/// Pre-compiled lookup equality matcher so exact wildcard searches do not recompile patterns per
/// row.
pub(crate) enum LookupMatcher<'a> {
    Wildcard(WildcardPattern),
    Exact(&'a Value),
}

impl<'a> LookupMatcher<'a> {
    pub(crate) fn new(needle: &'a Value, wild: bool) -> Self {
        if wild
            && let Value::Text(pattern) = needle
            && has_wildcards(pattern)
        {
            Self::Wildcard(WildcardPattern::compile(pattern))
        } else {
            Self::Exact(needle)
        }
    }

    pub(crate) fn matches(&self, hay: &Value) -> bool {
        match self {
            Self::Wildcard(pat) => matches!(hay, Value::Text(t) if pat.matches(t)),
            Self::Exact(needle) => lookup_equal(needle, hay, false),
        }
    }
}

#[cfg(test)]
#[path = "tests/criteria_tests.rs"]
mod tests;
