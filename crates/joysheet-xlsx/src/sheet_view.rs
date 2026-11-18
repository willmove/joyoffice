//! Worksheet pane identifiers and selected ranges shared by the three Excel readers.
use std::fmt::Write as _;

use joysheet_core::{CellRef, MAX_COLS, MAX_ROWS, RangeRef};
use joysheet_model::{
    Sheet,
    sheet::{Pane, PaneSelection},
};

use crate::IoError;
use crate::xml::El;

// Bound view metadata independently of the rectangular area selected. Whole
// rows and columns each use a single RangeRef.
pub(crate) const MAX_VIEW_AREAS: usize = 65_536;

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
}
