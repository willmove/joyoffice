use super::*;

/// CELL("format") code for a number format.
pub(super) fn cell_format_code(code: &str) -> String {
    let c = code.to_ascii_lowercase();
    if c == "general" {
        return "G".into();
    }
    if c.contains('%') {
        return format!("P{}", decimals(&c));
    }
    if c.contains('e') && c.contains('0') {
        return format!("S{}", decimals(&c));
    }
    if c.contains('$') {
        return format!("C{}", decimals(&c));
    }
    if c.contains('y') || c.contains('d') {
        return "D1".into();
    }
    if c.contains('h') {
        return "D9".into();
    }
    if c.contains(',') {
        return format!(",{}", decimals(&c));
    }
    format!("F{}", decimals(&c))
}

fn decimals(c: &str) -> usize {
    c.split_once('.').map_or(0, |(_, d)| d.chars().take_while(|ch| *ch == '0').count())
}

pub(super) fn same_kind(a: &Value, b: &Value) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b) || (a.is_empty() || b.is_empty())
}

/// Converts an R1C1 reference text to A1 (for INDIRECT(…, FALSE)). `Ok(None)` means the text is
/// not R1C1; `Err` means it is R1C1 but points past the sheet.
pub fn r1c1_to_a1(s: &str, at: CellRef) -> Result<Option<String>, CellError> {
    let Some(mut reference) = joysheet_formula::parse_r1c1_reference(s, at)? else { return Ok(None) };
    reference.sheet = SheetSel::Current;
    let address = joysheet_formula::printer::reference_a1(&reference);
    // The public converter preserves existing quoting and spelling of the sheet qualifier.
    Ok(Some(match s.rsplit_once('!') {
        Some((sheet, _)) => format!("{sheet}!{address}"),
        None => address,
    }))
}
