//! Parse the reference text used by INDIRECT in R1C1 mode. Keeping the axes
//! typed avoids formatting A1 text and parsing it again during recalculation.
use crate::lexer::{Tok, tokenize};
use crate::{Anchor, MAX_FORMULA_LEN, RefKind, Reference, SheetSel};
use joysheet_core::{CellError, CellRef, MAX_COLS, MAX_ROWS};

type Coordinate = (u32, bool);

#[derive(Clone, Copy)]
enum Endpoint {
    Cell(Anchor),
    Row(Coordinate),
    Column(Coordinate),
}

/// A bracketed displacement or a one-based absolute coordinate. No digits
/// after the axis letter, and empty brackets, both denote this row or column.
fn coordinate(s: &str, base: u32, maximum: u32) -> Result<Option<Coordinate>, CellError> {
    if s.is_empty() || s == "[]" {
        return Ok(Some((base, false)));
    }
    let (digits, delta, negative) = match s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Some(inner) => (inner.strip_prefix(['-', '+']).unwrap_or(inner), true, inner.starts_with('-')),
        None => (s, false, false),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(None);
    }
    let number = digits.parse::<u32>().map_err(|_| CellError::Ref)?;
    let index = if !delta {
        number.checked_sub(1)
    } else if negative {
        base.checked_sub(number)
    } else {
        base.checked_add(number)
    };
    index.filter(|&v| v < maximum).map(|v| Some((v, !delta))).ok_or(CellError::Ref)
}

fn endpoint(s: &str, at: CellRef) -> Result<Option<Endpoint>, CellError> {
    if let Some(row) = s.strip_prefix(['R', 'r']) {
        if let Some((r, c)) = row.split_once(['C', 'c']) {
            // A name such as R99999999COST is still a name. Establish that
            // both axes are numeric before reporting an out-of-grid result.
            return match (coordinate(r, at.row, MAX_ROWS), coordinate(c, at.col, MAX_COLS)) {
                (Ok(None), _) | (_, Ok(None)) => Ok(None),
                (Ok(Some((row, row_abs))), Ok(Some((col, col_abs)))) => Ok(Some(Endpoint::Cell(Anchor { row, col, row_abs, col_abs }))),
                _ => Err(CellError::Ref),
            };
        }
        return Ok(coordinate(row, at.row, MAX_ROWS)?.map(Endpoint::Row));
    }
    match s.strip_prefix(['C', 'c']) {
        Some(c) => Ok(coordinate(c, at.col, MAX_COLS)?.map(Endpoint::Column)),
        None => Ok(None),
    }
}

/// Resolve a cell, a rectangle or whole axes relative to the caller's cell.
/// A non-reference can still be a table or a defined name. Once coordinates
/// have identified a reference, an out-of-grid target stays a reference error.
pub fn parse_r1c1_reference(text: &str, at: CellRef) -> Result<Option<Reference>, CellError> {
    if text.len() > MAX_FORMULA_LEN && text.chars().take(MAX_FORMULA_LEN + 1).count() > MAX_FORMULA_LEN {
        return Err(CellError::Ref);
    }
    let (prefix, body) = text.rsplit_once('!').map_or((None, text), |(s, b)| (Some(s.len() + 1), b));
    let (left, right) = body.split_once(':').map_or((body, None), |(a, b)| (a, Some(b)));
    let Some(first) = endpoint(left, at)? else { return Ok(None) };
    let last = match right {
        Some(end) => match endpoint(end, at)? {
            Some(e) => e,
            None => return Ok(None),
        },
        None => first,
    };
    let kind = match (first, last) {
        (Endpoint::Cell(a), Endpoint::Cell(b)) if right.is_some() => RefKind::Range(a, b),
        (Endpoint::Cell(a), Endpoint::Cell(_)) => RefKind::Cell(a),
        (Endpoint::Row((a, aa)), Endpoint::Row((b, ba))) => RefKind::Rows(a, aa, b, ba),
        (Endpoint::Column((a, aa)), Endpoint::Column((b, ba))) => RefKind::Cols(a, aa, b, ba),
        _ => return Err(CellError::Ref),
    };
    let sheet = if let Some(end) = prefix {
        let mut tokens = tokenize(&text[..end]).map_err(|_| CellError::Ref)?.into_iter().map(|t| t.tok);
        match (tokens.next(), tokens.next()) {
            (Some(Tok::Sheet(a, None)), Some(Tok::Eof)) => SheetSel::Named(a),
            (Some(Tok::Sheet(a, Some(b))), Some(Tok::Eof)) => SheetSel::Span(a, b),
            _ => return Err(CellError::Ref),
        }
    } else {
        SheetSel::Current
    };
    Ok(Some(Reference { sheet, kind }))
}
