//! The selection stored for a particular pane of a worksheet window.
use super::Sheet;
use joysheet_core::{CellRef, RangeRef};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pane {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneSelection {
    pub active: CellRef,
    pub areas: Vec<RangeRef>,
    pub active_area: usize,
}

impl PaneSelection {
    pub fn valid_for(&self, active: CellRef) -> bool {
        self.active == active && self.areas.get(self.active_area).is_some_and(|r| r.contains(active))
    }

    /// The editor's active area comes last. Preserve the original file order on disk.
    pub fn editor_areas(&self) -> impl Iterator<Item = RangeRef> + '_ {
        self.areas.iter().enumerate().filter_map(|(i, &r)| (i != self.active_area).then_some(r)).chain(self.areas.get(self.active_area).copied())
    }
}

impl Sheet {
    pub fn pane_at(&self, cell: CellRef) -> Pane {
        let (rows, cols) = self.freeze.unwrap_or_default();
        match (rows > 0 && cell.row >= rows, cols > 0 && cell.col >= cols) {
            (true, true) => Pane::BottomRight,
            (true, false) => Pane::BottomLeft,
            (false, true) => Pane::TopRight,
            (false, false) => Pane::TopLeft,
        }
    }

    pub fn stored_selection(&self) -> Option<&PaneSelection> {
        self.view_selections.get(&self.pane_at(self.view_active)).filter(|s| s.valid_for(self.view_active))
    }
}
