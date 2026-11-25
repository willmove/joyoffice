//! Repair the numeric nulls emitted by older model writers.

use std::collections::VecDeque;

use serde_json::Error;

use crate::{FormatError, Result, project::Manifest};

const MAX_PATCHES: usize = 256;
const MAX_CANDIDATES: usize = 32;
const REPARSE_BUDGET: usize = 256 << 20;

pub(super) fn decode(json: &[u8]) -> Result<Manifest> {
    let mut error = match serde_json::from_slice(json) {
        Ok(manifest) => return Ok(manifest),
        Err(error) => error,
    };
    if !numeric_null(&error) {
        return Err(FormatError::Damaged(error.to_string()));
    }
    let mut json = json.to_vec();
    let mut budget = REPARSE_BUDGET;
    'patch: for _ in 0..MAX_PATCHES {
        if !numeric_null(&error) {
            break;
        }
        let limit = error_offset(&json, &error).saturating_add(4);
        for offset in candidates_before(&json, limit).into_iter().rev() {
            let Some(remaining) = budget.checked_sub(json.len()) else {
                break 'patch;
            };
            budget = remaining;
            // Padding leaves the offsets of the other tokens and error positions unchanged.
            let token = offset..offset + 4;
            json[token.clone()].copy_from_slice(b"0   ");
            match serde_json::from_slice(&json) {
                Ok(manifest) => return Ok(manifest),
                Err(next) if (next.line(), next.column()) > (error.line(), error.column()) => {
                    error = next;
                    continue 'patch;
                }
                Err(_) => json[token].copy_from_slice(b"null"),
            }
        }
        break;
    }
    Err(FormatError::Damaged(error.to_string()))
}

fn numeric_null(error: &Error) -> bool {
    let message = error.to_string();
    message.starts_with("invalid type: null, expected f")
        || message.starts_with("invalid type: null, expected a number")
}

fn error_offset(json: &[u8], error: &Error) -> usize {
    json.split_inclusive(|byte| *byte == b'\n')
        .take(error.line().saturating_sub(1))
        .map(<[u8]>::len)
        .sum::<usize>()
        .saturating_add(error.column())
}

fn candidates_before(json: &[u8], limit: usize) -> VecDeque<usize> {
    let mut quoted = false;
    let mut previous = b' ';
    let mut cursor = 0;
    let mut positions = VecDeque::with_capacity(MAX_CANDIDATES);
    let end = limit.min(json.len());
    while cursor < end {
        let byte = json[cursor];
        if quoted {
            match byte {
                b'"' => {
                    quoted = false;
                    previous = b'"';
                }
                b'\\' => cursor += 1,
                _ => {}
            }
        } else if !byte.is_ascii_whitespace() {
            quoted = byte == b'"';
            if byte == b'n'
                && matches!(previous, b':' | b'[' | b',')
                && json.get(cursor..cursor + 4) == Some(b"null")
                && json.get(cursor + 4).is_none_or(|next| {
                    next.is_ascii_whitespace() || matches!(next, b',' | b']' | b'}')
                })
            {
                if positions.len() == MAX_CANDIDATES {
                    positions.pop_front();
                }
                positions.push_back(cursor);
            }
            previous = byte;
        }
        cursor += 1;
    }
    positions
}
