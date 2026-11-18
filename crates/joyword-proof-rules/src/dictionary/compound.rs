use super::FlagMode;
use super::flags::VARIATION_SELECTOR;
use std::borrow::Cow;

pub const MAX_COMPOUND_TOKENS: usize = 32;
const MAX_GROUP_BYTES: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Once,
    Optional,
    Any,
}

pub fn compound_pattern(pattern: &str, mode: FlagMode) -> Result<Vec<(u64, Repeat)>, String> {
    let input = if pattern.contains(VARIATION_SELECTOR) {
        Cow::Owned(pattern.replace(VARIATION_SELECTOR, ""))
    } else {
        Cow::Borrowed(pattern)
    };
    let mut tail: &str = &input;
    let mut tokens: Vec<(u64, Repeat)> = Vec::new();
    let mut ignored_class = false;
    let error = |reason: &str| format!("COMPOUNDRULE `{pattern}`: {reason}");
    while let Some(ch) = tail.chars().next() {
        tail = &tail[ch.len_utf8()..];
        if ch == '*' || ch == '?' {
            if !ignored_class {
                let Some((_, repeat)) = tokens.last_mut().filter(|(_, r)| *r == Repeat::Once)
                else {
                    return Err(error(&format!("`{ch}` follows no flag")));
                };
                *repeat = if ch == '*' {
                    Repeat::Any
                } else {
                    Repeat::Optional
                };
            }
            continue;
        }
        ignored_class = false;
        let flag = match ch {
            '(' => {
                let (group, rest) = tail
                    .split_once(')')
                    .ok_or_else(|| error("`(` is not closed"))?;
                if group
                    .char_indices()
                    .next_back()
                    .is_some_and(|(i, _)| i >= MAX_GROUP_BYTES)
                {
                    return Err(error("`(` is not closed"));
                }
                tail = rest;
                mode.parse_one(group).map_err(|reason| error(&reason))?
            }
            c if mode == FlagMode::Char => u64::from(c),
            '[' => {
                let (_, rest) = tail
                    .split_once(']')
                    .ok_or_else(|| error("`[` is not closed"))?;
                tail = rest;
                ignored_class = true;
                continue;
            }
            ')' => return Err(error("`)` closes no group")),
            // Existing numeric/long dictionaries contain punctuation alternatives
            // which authorize no word. Keep this behavior across compact saves.
            c if !c.is_alphanumeric() => return Ok(vec![(u64::MAX, Repeat::Once)]),
            _ => return Err(error("long and numeric flags are written in ( )")),
        };
        if tokens.len() == MAX_COMPOUND_TOKENS {
            return Err(error(&format!("more than {MAX_COMPOUND_TOKENS} flags")));
        }
        tokens.push((flag, Repeat::Once));
    }
    if tokens.is_empty() {
        Err("COMPOUNDRULE with no flags".into())
    } else {
        Ok(tokens)
    }
}
