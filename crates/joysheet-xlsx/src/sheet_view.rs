//! Worksheet display options, view modes, zoom, pane identifiers, and selected ranges shared by
//! the BIFF8 (`.xls`), BIFF12 (`.xlsb`), and SpreadsheetML (`.xlsx`) readers and writers.
use std::fmt::Write as _;

use joysheet_core::{CellRef, MAX_COLS, MAX_ROWS, RangeRef};
use joysheet_model::{
    Sheet, SheetViewMode,
    sheet::{Pane, PaneSelection},
};

use crate::IoError;
use crate::xml::El;

// Bound view metadata independently of the rectangular area selected. Whole
// rows and columns each use a single RangeRef.
pub(crate) const MAX_VIEW_AREAS: usize = 65_536;

pub(crate) struct DisplayOptions {
    pub show_formulas: bool,
    pub show_gridlines: bool,
    pub show_headings: bool,
    pub show_zeros: bool,
    pub show_outline_symbols: bool,
    pub right_to_left: bool,
    pub view: SheetViewMode,
    pub zoom: Option<u16>,
}

pub(crate) fn clamp_zoom(percent: u32) -> Option<u16> {
    (percent != 0).then(|| percent.clamp(10, 400) as u16)
}

/// Resolves the active worksheet zoom percentage. Zero means "unset / default" in BIFF8, BIFF12,
/// and SpreadsheetML rather than 10%.
pub(crate) fn resolve_zoom(view: SheetViewMode, active: u32, normal: u32, page_break: u32, page_layout: u32) -> Option<u16> {
    let view_scale = match view {
        SheetViewMode::Normal => normal,
        SheetViewMode::PageBreakPreview => page_break,
        SheetViewMode::PageLayout => page_layout,
    };
    clamp_zoom(active).or_else(|| clamp_zoom(view_scale))
}

/// Decodes a BIFF8 `SCL` (`0x00A0`, MS-XLS §2.4.247) rational zoom (`numerator / denominator`).
pub(crate) fn biff8_scl_zoom(numerator: u16, denominator: u16) -> Option<u16> {
    if numerator == 0 || denominator == 0 {
        return None;
    }
    let den = u32::from(denominator);
    let pct = (u32::from(numerator) * 100 + den / 2) / den;
    clamp_zoom(pct)
}

pub(crate) fn apply_display(sheet: &mut Sheet, opts: DisplayOptions) {
    sheet.show_formulas = opts.show_formulas;
    sheet.show_gridlines = opts.show_gridlines;
    sheet.show_headings = opts.show_headings;
    sheet.show_zeros = opts.show_zeros;
    sheet.show_outline_symbols = opts.show_outline_symbols;
    sheet.right_to_left = opts.right_to_left;
    sheet.view = opts.view;
    if let Some(zoom) = opts.zoom {
        sheet.zoom = zoom;
    }
}

/// Decodes the shared BIFF8 `WsBool` (`0x0081`, MS-XLS §2.4.351) and BIFF12 `BrtWsProp`
/// (`0x0093`, MS-XLSB §2.4.854) outline summary bits (`fRowSumsBelow` = `0x0040`,
/// `fColSumsRight` = `0x0080`).
pub(crate) fn apply_outline_summary(sheet: &mut Sheet, flags: u16) {
    sheet.summary_below = flags & 0x0040 != 0;
    sheet.summary_right = flags & 0x0080 != 0;
}

pub(crate) fn cell(row: u32, col: u32) -> Option<CellRef> {
    (row < MAX_ROWS && col < MAX_COLS).then_some(CellRef::new(row, col))
}

/// With one frozen axis Excel stores the other axis on the worksheet view itself.
/// Some older writers put both coordinates on the pane and leave the sheet at A1.
pub(crate) fn freeze(sheet: &mut Sheet, rows: u32, cols: u32, pane: CellRef) {
    let main = sheet.view_top_left;
    sheet.view_top_left = CellRef::new(
        if rows != 0 {
            rows.max(pane.row)
        } else if main.row != 0 {
            main.row
        } else {
            pane.row
        },
        if cols != 0 {
            cols.max(pane.col)
        } else if main.col != 0 {
            main.col
        } else {
            pane.col
        },
    );
    sheet.freeze = Some((rows, cols));
}

pub(crate) fn binary_pane(id: u32) -> Option<Pane> {
    Some(match id {
        0 => Pane::BottomRight,
        1 => Pane::TopRight,
        2 => Pane::BottomLeft,
        3 => Pane::TopLeft,
        _ => return None,
    })
}

fn xml_pane(s: Option<&str>) -> Option<Pane> {
    Some(match s.unwrap_or("topLeft") {
        "topLeft" => Pane::TopLeft,
        "bottomLeft" => Pane::BottomLeft,
        "topRight" => Pane::TopRight,
        "bottomRight" => Pane::BottomRight,
        _ => return None,
    })
}

fn name(pane: Pane) -> &'static str {
    match pane {
        Pane::BottomRight => "bottomRight",
        Pane::TopRight => "topRight",
        Pane::BottomLeft => "bottomLeft",
        Pane::TopLeft => "topLeft",
    }
}

pub(crate) fn restore(sheet: &mut Sheet, pane: Pane, active_pane: Pane, selection: PaneSelection) {
    if pane == active_pane {
        sheet.view_active = selection.active;
    }
    if selection.valid_for(selection.active) {
        // A split-only view has one editable viewport. Keep the selection of
        // its active pane independently of its measurements in twips.
        if sheet.freeze.is_some() || pane == active_pane {
            let key = if sheet.freeze.is_none() { Pane::TopLeft } else { pane };
            sheet.view_selections.insert(key, selection);
        }
    }
}

pub(crate) fn read_xml_views(e: &El, sheet: &mut Sheet) -> Result<(), IoError> {
    let Some(v) = e.kids("sheetView").find(|sv| sv.attr_u32("workbookViewId").unwrap_or(0) == 0).or_else(|| e.child("sheetView")) else {
        return Ok(());
    };
    let view = v.attr("view").and_then(SheetViewMode::parse).unwrap_or_default();
    let zoom = resolve_zoom(
        view,
        v.attr_u32("zoomScale").unwrap_or(0),
        v.attr_u32("zoomScaleNormal").unwrap_or(0),
        v.attr_u32("zoomScaleSheetLayoutView").unwrap_or(0),
        v.attr_u32("zoomScalePageLayoutView").unwrap_or(0),
    );
    apply_display(
        sheet,
        DisplayOptions {
            show_formulas: v.flag("showFormulas", false),
            show_gridlines: v.flag("showGridLines", true),
            show_headings: v.flag("showRowColHeaders", true),
            show_zeros: v.flag("showZeros", true),
            show_outline_symbols: v.flag("showOutlineSymbols", true),
            right_to_left: v.flag("rightToLeft", false),
            view,
            zoom,
        },
    );
    if let Some(c) = v.attr("topLeftCell").and_then(CellRef::parse) {
        sheet.view_top_left = c;
    }
    let mut active_pane = None;
    if let Some(p) = v.child("pane") {
        if matches!(p.attr("state"), Some("frozen") | Some("frozenSplit")) {
            let cols = p.attr_f64("xSplit").unwrap_or(0.0).clamp(0.0, (MAX_COLS - 1) as f64) as u32;
            let rows = p.attr_f64("ySplit").unwrap_or(0.0).clamp(0.0, (MAX_ROWS - 1) as f64) as u32;
            if rows > 0 || cols > 0 {
                let pane = p.attr("topLeftCell").and_then(CellRef::parse).unwrap_or_default();
                freeze(sheet, rows, cols, pane);
            }
        }
        active_pane = p.attr("activePane");
    }
    read_selections(v, sheet, active_pane)
}

pub(crate) fn write_xml_views(out: &mut String, sheet: &Sheet, selected: bool) {
    out.push_str("<sheetViews><sheetView");
    if selected {
        out.push_str(" tabSelected=\"1\"");
    }
    if !sheet.show_gridlines {
        out.push_str(" showGridLines=\"0\"");
    }
    if !sheet.show_headings {
        out.push_str(" showRowColHeaders=\"0\"");
    }
    if sheet.show_formulas {
        out.push_str(" showFormulas=\"1\"");
    }
    if !sheet.show_zeros {
        out.push_str(" showZeros=\"0\"");
    }
    if !sheet.show_outline_symbols {
        out.push_str(" showOutlineSymbols=\"0\"");
    }
    if sheet.right_to_left {
        out.push_str(" rightToLeft=\"1\"");
    }
    if sheet.view != SheetViewMode::Normal {
        let _ = write!(out, " view=\"{}\"", sheet.view.as_str());
    }
    if sheet.zoom != 100 && (10..=400).contains(&sheet.zoom) {
        let view_attr = match sheet.view {
            SheetViewMode::Normal => "zoomScaleNormal",
            SheetViewMode::PageBreakPreview => "zoomScaleSheetLayoutView",
            SheetViewMode::PageLayout => "zoomScalePageLayoutView",
        };
        let _ = write!(out, " zoomScale=\"{0}\" {view_attr}=\"{0}\"", sheet.zoom);
    }
    let (rows, cols) = sheet.freeze.unwrap_or_default();
    let top = sheet.view_top_left;
    let main_origin = CellRef::new(if rows == 0 { top.row } else { 0 }, if cols == 0 { top.col } else { 0 });
    if main_origin != CellRef::default() {
        let _ = write!(out, " topLeftCell=\"{}\"", main_origin.a1());
    }
    out.push_str(" workbookViewId=\"0\">");
    if let Some((rows, cols)) = sheet.freeze
        && (rows > 0 || cols > 0)
    {
        let pane = active_name(sheet);
        out.push_str("<pane");
        if cols > 0 {
            let _ = write!(out, " xSplit=\"{cols}\"");
        }
        if rows > 0 {
            let _ = write!(out, " ySplit=\"{rows}\"");
        }
        let _ = write!(
            out,
            " topLeftCell=\"{}\" activePane=\"{pane}\" state=\"frozen\"/>",
            CellRef::new(
                if rows == 0 { 0 } else { rows.max(top.row).min(MAX_ROWS - 1) },
                if cols == 0 { 0 } else { cols.max(top.col).min(MAX_COLS - 1) }
            )
            .a1()
        );
    }
    write_selections(out, sheet);
    out.push_str("</sheetView></sheetViews>");
}

pub(crate) fn read_selections(view: &El, sheet: &mut Sheet, active: Option<&str>) -> Result<(), IoError> {
    let active = xml_pane(active).unwrap_or(Pane::TopLeft);
    for sel in view.kids("selection") {
        let Some(pane) = xml_pane(sel.attr("pane")) else { continue };
        let at = match sel.attr("activeCell") {
            Some(value) => match CellRef::parse(value) {
                Some(cell) => cell,
                None => continue,
            },
            None => CellRef::default(),
        };
        let text = sel.attr("sqref").unwrap_or("");
        if text.split_ascii_whitespace().take(MAX_VIEW_AREAS + 1).count() > MAX_VIEW_AREAS {
            return Err(IoError::TooLarge("worksheet selection contains too many areas".into()));
        }
        let ranges = text.split_ascii_whitespace().map(RangeRef::parse).collect::<Option<Vec<_>>>().unwrap_or_default();
        let index = sel.attr_u32("activeCellId").unwrap_or(0) as usize;
        restore(sheet, pane, active, PaneSelection { active: at, active_area: index, areas: ranges });
    }
    Ok(())
}

pub(crate) fn write_selections(out: &mut String, sheet: &Sheet) {
    let current = sheet.pane_at(sheet.view_active);
    let (rows, cols) = sheet.freeze.unwrap_or_default();
    for pane in [Pane::TopLeft, Pane::BottomLeft, Pane::TopRight, Pane::BottomRight] {
        if (matches!(pane, Pane::BottomRight | Pane::BottomLeft) && rows == 0) || (matches!(pane, Pane::BottomRight | Pane::TopRight) && cols == 0) {
            continue;
        }
        let saved = sheet.view_selections.get(&pane).filter(|v| v.valid_for(v.active) && (pane != current || v.active == sheet.view_active));
        if saved.is_none() && pane != current {
            continue;
        }
        let active = saved.map_or(sheet.view_active, |s| s.active);
        out.push_str("<selection");
        if pane != Pane::TopLeft {
            let _ = write!(out, " pane=\"{}\"", name(pane));
        }
        let _ = write!(out, " activeCell=\"{}\"", active.a1());
        if let Some(s) = saved
            && s.active_area > 0
        {
            let _ = write!(out, " activeCellId=\"{}\"", s.active_area);
        }
        out.push_str(" sqref=\"");
        if let Some(s) = saved {
            for (i, r) in s.areas.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&r.start.a1());
                if r.start != r.end {
                    out.push(':');
                    out.push_str(&r.end.a1());
                }
            }
        } else {
            out.push_str(&active.a1());
        }
        out.push_str("\"/>");
    }
}

pub(crate) fn active_name(sheet: &Sheet) -> &'static str {
    name(sheet.pane_at(sheet.view_active))
}

/// Only the first worksheet window participates in the primary workbook view.
/// BIFF8 may continue one pane's range list across contiguous Selection records.
pub(crate) struct BinaryView {
    seen: bool,
    pub reading: bool,
    pub frozen: bool,
    pub active: Pane,
    selections: std::collections::BTreeMap<Pane, PaneSelection>,
}

impl Default for BinaryView {
    fn default() -> Self {
        Self { seen: false, reading: false, frozen: false, active: Pane::TopLeft, selections: Default::default() }
    }
}

impl BinaryView {
    pub fn begin(&mut self) -> bool {
        self.reading = !std::mem::replace(&mut self.seen, true);
        self.reading
    }

    pub fn pane(&mut self, sheet: &mut Sheet, id: u32, split: (u32, u32), top: Option<CellRef>) {
        if let Some(active) = binary_pane(id) {
            self.active = active;
        }
        if self.frozen && (split.0 > 0 || split.1 > 0) && split.0 < MAX_ROWS && split.1 < MAX_COLS {
            freeze(sheet, split.0, split.1, top.unwrap_or_default());
        }
    }

    pub fn select(&mut self, id: u32, selection: PaneSelection) -> Result<(), IoError> {
        if let Some(pane) = binary_pane(id) {
            match self.selections.entry(pane) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(selection);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let old = entry.get_mut();
                    if old.active == selection.active && old.active_area == selection.active_area {
                        if old.areas.len() + selection.areas.len() > MAX_VIEW_AREAS {
                            return Err(IoError::TooLarge("worksheet selection contains too many areas".into()));
                        }
                        old.areas.extend(selection.areas);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn finish(self, sheet: &mut Sheet) {
        for (pane, selection) in self.selections {
            restore(sheet, pane, self.active, selection);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_first_area_and_implicit_a1_keep_the_document_order() {
        let view = crate::xml::parse(br#"<sheetView><selection sqref="A1:B2 D4:E5"/></sheetView>"#).unwrap();
        let mut sheet = Sheet::new("View");
        read_selections(&view, &mut sheet, None).unwrap();
        let stored = sheet.stored_selection().unwrap();
        assert_eq!(stored.editor_areas().map(|r| r.a1()).collect::<Vec<_>>(), ["D4:E5", "A1:B2"]);
        let mut xml = String::new();
        write_selections(&mut xml, &sheet);
        assert_eq!(xml, r#"<selection activeCell="A1" sqref="A1:B2 D4:E5"/>"#);
    }

    #[test]
    fn continued_binary_ranges_enforce_the_combined_budget() {
        let at = CellRef::new(3, 4);
        let area = RangeRef::cell(at);
        let mut view = BinaryView::default();
        view.select(3, PaneSelection { active: at, active_area: 0, areas: vec![area; MAX_VIEW_AREAS] }).unwrap();
        assert!(matches!(view.select(3, PaneSelection { active: at, active_area: 0, areas: vec![area] }), Err(IoError::TooLarge(_))));
    }

    #[test]
    fn xml_views_resolve_zero_zoom_and_write_mode_specific_scale() -> Result<(), IoError> {
        let views = crate::xml::parse(
            br#"<sheetViews><sheetView view="pageBreakPreview" zoomScale="0" zoomScaleSheetLayoutView="85" showGridLines="0" showOutlineSymbols="0" workbookViewId="0"/></sheetViews>"#,
        )?;
        let mut sheet = Sheet::new("Preview");
        read_xml_views(&views, &mut sheet)?;
        assert_eq!(sheet.view, SheetViewMode::PageBreakPreview);
        assert_eq!(sheet.zoom, 85);
        assert!(!sheet.show_gridlines);
        assert!(!sheet.show_outline_symbols);
        let mut xml = String::new();
        write_xml_views(&mut xml, &sheet, true);
        assert!(xml.contains(r#"view="pageBreakPreview""#));
        assert!(xml.contains(r#"zoomScale="85" zoomScaleSheetLayoutView="85""#));
        assert!(xml.contains(r#"showOutlineSymbols="0""#));
        // Zero zoom without a fallback keeps the default 100% rather than clamping to 10%.
        let zero = crate::xml::parse(br#"<sheetViews><sheetView zoomScale="0" workbookViewId="0"/></sheetViews>"#)?;
        let mut default_sheet = Sheet::new("Default");
        read_xml_views(&zero, &mut default_sheet)?;
        assert_eq!(default_sheet.zoom, 100);
        Ok(())
    }

    #[test]
    fn biff8_scl_rounds_and_clamps_rational_zoom() {
        assert_eq!(biff8_scl_zoom(77, 50), Some(154));
        assert_eq!(biff8_scl_zoom(1, 20), Some(10));
        assert_eq!(biff8_scl_zoom(9, 1), Some(400));
        assert_eq!(biff8_scl_zoom(0, 100), None);
        assert_eq!(biff8_scl_zoom(100, 0), None);
    }
}
