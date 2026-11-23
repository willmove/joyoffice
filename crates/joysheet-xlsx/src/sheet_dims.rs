//! Shared worksheet dimension, hidden-line, and outline-level helpers across
//! SpreadsheetML (`.xlsx`), BIFF8 (`.xls`), and BIFF12 (`.xlsb`).
//!
//! Covers:
//! - Default column width (`defaultColWidth` / `baseColWidth`, BIFF8 `DEFCOLWIDTH`
//!   `0x0055` & `STANDARDWIDTH` `0x0099`, BIFF12 `BrtWsFmtInfo` `485`) and default
//!   row height (`defaultRowHeight`, BIFF8 `DEFAULTROWHEIGHT` `0x0225`, BIFF12
//!   `BrtWsFmtInfo` `485`).
//! - Column spans (`<col>`, BIFF8 `COLINFO` `0x007D`, BIFF12 `BrtColInfo` `60`)
//!   with one shared expansion budget (`MAX_COLUMN_ASSIGNMENTS`).
//! - Row headers (`<row>`, BIFF8 `ROW` `0x0208`, BIFF12 `BrtRowHdr` `0`).

use std::fmt::Write;

use joysheet_core::{MAX_COLS, MAX_ROWS};
use joysheet_model::{LineInfo, Sheet, StyleId};

use crate::IoError;
use crate::read::Ctx;
use crate::tables::{col_width_to_px, pt_to_px, px_to_col_width, px_to_pt};
use crate::xml::El;

/// Maximum expanded column assignments per worksheet across `<cols>`, BIFF8
/// `COLINFO`, and BIFF12 `BrtColInfo` records.
pub(crate) const MAX_COLUMN_ASSIGNMENTS: u64 = 1_000_000;

/// Converts `baseColWidth` / `cchDefColWidth` (character count) to pixels using
/// Excel's standard 7 px digit + 5 px padding rounded up to an 8 px grid.
pub(crate) fn base_col_width_to_px(chars: f64) -> f32 {
    (((chars.clamp(0.0, 255.0) * 7.0 + 5.0) / 8.0).ceil() * 8.0) as f32
}

/// Applies worksheet default column width and row height when valid and positive.
pub(crate) fn apply_sheet_defaults(sheet: &mut Sheet, default_col_chars: Option<f64>, base_col_chars: Option<f64>, default_row_pt: Option<f64>) {
    if let Some(pt) = default_row_pt {
        let px = pt_to_px(pt);
        if px > 0.0 {
            sheet.default_row_height = px;
        }
    }
    if let Some(w) = default_col_chars {
        let px = col_width_to_px(w);
        if px > 0.0 {
            sheet.default_col_width = px;
        }
    } else if let Some(b) = base_col_chars {
        let px = base_col_width_to_px(b);
        if px > 0.0 {
            sheet.default_col_width = px;
        }
    }
}

/// Parsed properties for one column span (`first..=last`, 0-based).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ColumnSpec {
    /// Column width in character units (`width` or `coldx / 256.0`), if present.
    pub width_chars: Option<f64>,
    pub custom_width: bool,
    pub hidden: bool,
    pub outline: u8,
    pub collapsed: bool,
    pub style: Option<StyleId>,
}

/// Applies a 0-based inclusive column span `[first, last]` clipped to `max_col`
/// (inclusive) while charging the shared `remaining` expansion budget.
pub(crate) fn apply_col_span(sheet: &mut Sheet, first: u32, last: u32, max_col: u32, spec: ColumnSpec, remaining: &mut u64) -> Result<(), IoError> {
    if first > max_col || last < first {
        return Ok(());
    }
    let last = last.min(max_col);
    let mut info = LineInfo::default();
    if let Some(w) = spec.width_chars {
        let px = col_width_to_px(w);
        if spec.custom_width || px != sheet.default_col_width {
            info.size = Some(px);
        }
    }
    info.hidden = spec.hidden;
    info.outline = spec.outline.min(7);
    info.collapsed = spec.collapsed;
    if let Some(style) = spec.style
        && style != StyleId::DEFAULT
    {
        info.style = Some(style);
    }
    if info == LineInfo::default() {
        return Ok(());
    }
    *remaining = remaining
        .checked_sub(u64::from(last - first + 1))
        .ok_or_else(|| IoError::TooLarge(format!("sheet \"{}\": more than {MAX_COLUMN_ASSIGNMENTS} expanded column assignments", sheet.name)))?;
    for col in first..=last {
        sheet.cols.insert(col, info);
    }
    Ok(())
}

/// Parsed properties for one 0-based row.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RowSpec {
    /// Explicit row height in points (`ht` or `miyRw / 20.0`), if present.
    pub height_pt: Option<f64>,
    pub custom_height: bool,
    pub hidden: bool,
    pub outline: u8,
    pub collapsed: bool,
    pub style: Option<StyleId>,
}

/// Applies row formatting, height, hidden state, and outline metadata for row `r` (0-based).
pub(crate) fn apply_row_info(sheet: &mut Sheet, row: u32, spec: RowSpec) {
    if row >= MAX_ROWS {
        return;
    }
    let mut info =
        LineInfo { custom: spec.custom_height, hidden: spec.hidden, outline: spec.outline.min(7), collapsed: spec.collapsed, ..LineInfo::default() };
    if let Some(pt) = spec.height_pt {
        let px = pt_to_px(pt);
        if info.custom || (px - sheet.default_row_height).abs() > 0.5 {
            info.size = Some(px);
        }
    }
    if let Some(style) = spec.style {
        info.style = Some(style);
    }
    if info != LineInfo::default() {
        sheet.rows.insert(row, info);
    }
}

/// Decodes BIFF8 `COLINFO` (`0x007D`) or BIFF12 `BrtColInfo` (`60`) flags into a `ColumnSpec`.
/// Both specifications share the exact same 16-bit flag layout:
/// bit 0 = `fHidden`, bit 1 = `fUserSet`, bits 8–10 = `iOutLevel`, bit 12 = `fCollapsed`.
pub(crate) fn binary_col_spec(coldx: u32, flags: u16, style: Option<StyleId>) -> ColumnSpec {
    ColumnSpec {
        width_chars: Some(f64::from(coldx) / 256.0),
        custom_width: flags & 0x0002 != 0,
        hidden: flags & 0x0001 != 0,
        outline: ((flags >> 8) & 7) as u8,
        collapsed: flags & 0x1000 != 0,
        style,
    }
}

/// Reads `<sheetFormatPr>` default dimensions into `sheet`.
pub(crate) fn read_xml_format_pr(e: &El, sheet: &mut Sheet) {
    apply_sheet_defaults(sheet, e.attr_f64("defaultColWidth"), e.attr_f64("baseColWidth"), e.attr_f64("defaultRowHeight"));
}

/// Reads `<cols>` into `sheet` using the shared column-expansion budget.
pub(crate) fn read_xml_cols(cx: &Ctx<'_>, e: &El, sheet: &mut Sheet, remaining: &mut u64) -> Result<(), IoError> {
    for c in e.kids("col") {
        let (Some(min), Some(max)) = (c.attr_u32("min"), c.attr_u32("max")) else { continue };
        if min == 0 || min > MAX_COLS || max < min {
            continue;
        }
        let spec = ColumnSpec {
            width_chars: c.attr_f64("width"),
            custom_width: c.flag("customWidth", false),
            hidden: c.flag("hidden", false),
            outline: c.attr_u32("outlineLevel").unwrap_or(0).min(7) as u8,
            collapsed: c.flag("collapsed", false),
            style: c.attr_u32("style").map(|s| cx.style(Some(s))),
        };
        apply_col_span(sheet, min - 1, max - 1, MAX_COLS - 1, spec, remaining)?;
    }
    Ok(())
}

/// Writes `<sheetFormatPr .../>` with default dimensions and maximum outline levels.
pub(crate) fn write_xml_format_pr(out: &mut String, sheet: &Sheet, fmt_num: impl Fn(f64) -> String) {
    let max_row_outline = sheet.rows.values().map(|r| r.outline.min(7)).max().unwrap_or(0);
    let max_col_outline = sheet.cols.values().map(|c| c.outline.min(7)).max().unwrap_or(0);
    out.push_str("<sheetFormatPr");
    if (sheet.default_col_width - joysheet_model::DEFAULT_COL_WIDTH).abs() > 0.01 {
        let _ = write!(out, " defaultColWidth=\"{}\"", fmt_num(px_to_col_width(sheet.default_col_width)));
    }
    let _ = write!(out, " defaultRowHeight=\"{}\"", fmt_num(px_to_pt(sheet.default_row_height)));
    if (sheet.default_row_height - joysheet_model::DEFAULT_ROW_HEIGHT).abs() > 0.01 {
        out.push_str(" customHeight=\"1\"");
    }
    if max_row_outline > 0 {
        let _ = write!(out, " outlineLevelRow=\"{max_row_outline}\"");
    }
    if max_col_outline > 0 {
        let _ = write!(out, " outlineLevelCol=\"{max_col_outline}\"");
    }
    out.push_str("/>");
}

/// Writes `<cols>...</cols>` coalescing adjacent columns with identical settings.
pub(crate) fn write_xml_cols(out: &mut String, sheet: &Sheet, sid: impl Fn(StyleId) -> u32, fmt_num: impl Fn(f64) -> String) {
    if sheet.cols.is_empty() {
        return;
    }
    out.push_str("<cols>");
    let mut run: Option<(u32, u32, LineInfo)> = None;
    let flush = |s: &mut String, (a, b, i): (u32, u32, LineInfo)| {
        let w = i.size.unwrap_or(sheet.default_col_width);
        let _ = write!(s, "<col min=\"{}\" max=\"{}\" width=\"{}\"", a + 1, b + 1, fmt_num(px_to_col_width(w)));
        if let Some(st) = i.style {
            let _ = write!(s, " style=\"{}\"", sid(st));
        }
        if i.hidden {
            s.push_str(" hidden=\"1\"");
        }
        if i.size.is_some() {
            s.push_str(" customWidth=\"1\"");
        }
        if i.outline > 0 {
            let _ = write!(s, " outlineLevel=\"{}\"", i.outline.min(7));
        }
        if i.collapsed {
            s.push_str(" collapsed=\"1\"");
        }
        s.push_str("/>");
    };
    for (&c, info) in &sheet.cols {
        if c >= MAX_COLS {
            continue;
        }
        run = match run {
            Some((a, b, i)) if b + 1 == c && i == *info => Some((a, c, i)),
            Some(r) => {
                flush(out, r);
                Some((c, c, *info))
            }
            None => Some((c, c, *info)),
        };
    }
    if let Some(r) = run {
        flush(out, r);
    }
    out.push_str("</cols>");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_col_spec_decodes_shared_biff_flags_and_enforces_budget() -> Result<(), IoError> {
        let mut sheet = Sheet::new("Dims");
        let mut rem = 5u64;
        let spec = binary_col_spec(16 * 256, 0x0001 | 0x0002 | (2 << 8) | 0x1000, None);
        apply_col_span(&mut sheet, 1, 3, 255, spec, &mut rem)?;
        assert_eq!(rem, 2);
        let Some(info) = sheet.cols.get(&2) else {
            return Err(IoError::Format("missing column 2".into()));
        };
        assert!(info.hidden);
        assert!(info.collapsed);
        assert_eq!(info.outline, 2);
        assert!(info.size.is_some());
        assert!(matches!(apply_col_span(&mut sheet, 4, 10, 255, spec, &mut rem), Err(IoError::TooLarge(_))));
        Ok(())
    }

    #[test]
    fn row_info_uses_sheet_default_row_height_threshold() {
        let mut sheet = Sheet::new("Rows");
        apply_sheet_defaults(&mut sheet, Some(12.0), None, Some(24.0));
        assert_eq!(sheet.default_row_height, pt_to_px(24.0));
        // Automatic row equal to default height (24 pt) is noise and omitted.
        apply_row_info(&mut sheet, 0, RowSpec { height_pt: Some(24.0), ..RowSpec::default() });
        assert!(!sheet.rows.contains_key(&0));
        // Automatic row at 15 pt differs from 24 pt default and is kept.
        apply_row_info(&mut sheet, 1, RowSpec { height_pt: Some(15.0), outline: 3, collapsed: true, ..RowSpec::default() });
        let r1 = sheet.rows.get(&1).copied().unwrap_or_default();
        assert_eq!(r1.size, Some(pt_to_px(15.0)));
        assert_eq!(r1.outline, 3);
        assert!(r1.collapsed);
    }
}
