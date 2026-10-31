//! Two-dimensional, sparse, f64 worksheet geometry shared by paint and hit testing.
use gridcraft_engine::{
    core::{CellRef, MAX_COLS, MAX_ROWS},
    model::{LineIndex, Sheet},
};
use std::sync::Arc;

pub const ROW_HEADER: f32 = 52.0;
pub const COL_HEADER: f32 = 28.0;
const MAX_VISIBLE_LINES: usize = 4096;

#[derive(Clone)]
pub struct SheetGeometry {
    pub rows: Arc<LineIndex>,
    pub cols: Arc<LineIndex>,
    /// Computed once per worksheet revision, shared by paint and scrollbar hit tests.
    pub used_end: CellRef,
}
impl SheetGeometry {
    pub fn new(sheet: &Sheet) -> Self {
        Self::with_extent(sheet, sheet.used_range().map(|r| r.end).unwrap_or_default())
    }
    /// The extent must come from the same immutable sheet snapshot.
    pub fn with_extent(sheet: &Sheet, used_end: CellRef) -> Self {
        Self { rows: sheet.rows.index(sheet.default_row_height, MAX_ROWS), cols: sheet.cols.index(sheet.default_col_width, MAX_COLS), used_end }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AxisLine {
    pub index: u32,
    pub start: f32,
    pub size: f32,
    pub frozen: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Debug, Default)]
pub struct GridLayout {
    pub rows: Vec<AxisLine>,
    pub cols: Vec<AxisLine>,
    pub width: f32,
    pub height: f32,
    pub frozen_width: f32,
    pub frozen_height: f32,
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub zoom: f64,
}
fn axis(index: &LineIndex, scroll: f64, extent: f32, header: f32, freeze: u32, max: u32, zoom: f64) -> Vec<AxisLine> {
    let mut result = Vec::new();
    for (frozen, from, until, offset) in [(true, 0, freeze, 0.0), (false, freeze, max, scroll)] {
        if from >= until {
            continue;
        }
        let mut line = if frozen { from } else { index.at(index.start(from) + scroll / zoom).max(from) };
        while line < until && result.len() < MAX_VISIBLE_LINES {
            let start = index.start(line);
            let next = index.start(line.saturating_add(1));
            let screen = header as f64 + start * zoom - offset;
            if screen >= extent as f64 {
                break;
            }
            let size = (next - start) * zoom;
            if size > 0.0 && screen + size > header as f64 {
                result.push(AxisLine { index: line, start: screen as f32, size: size as f32, frozen });
            }
            // `at` skips a contiguous run of hidden rows/columns without scanning it.
            line = index.at(next + f64::EPSILON * next.abs().max(1.)).max(line.saturating_add(1));
        }
    }
    result
}
impl GridLayout {
    pub fn new(sheet: &Sheet, geo: &SheetGeometry, x: f64, y: f64, width: f32, height: f32) -> Self {
        let zoom = (sheet.zoom as f64 / 100.0).clamp(0.1, 4.0);
        let (fr, fc) = sheet.freeze.unwrap_or((0, 0));
        let fr = fr.min(MAX_ROWS);
        let fc = fc.min(MAX_COLS);
        let fw = (geo.cols.start(fc) * zoom) as f32;
        let fh = (geo.rows.start(fr) * zoom) as f32;
        let max_x = (geo.cols.start(MAX_COLS) * zoom - (width - ROW_HEADER).max(0.0) as f64).max(0.0);
        let max_y = (geo.rows.start(MAX_ROWS) * zoom - (height - COL_HEADER).max(0.0) as f64).max(0.0);
        let x = if x.is_finite() { x.clamp(0.0, max_x) } else { 0.0 };
        let y = if y.is_finite() { y.clamp(0.0, max_y) } else { 0.0 };
        Self {
            rows: axis(&geo.rows, y, height, COL_HEADER, fr, MAX_ROWS, zoom),
            cols: axis(&geo.cols, x, width, ROW_HEADER, fc, MAX_COLS, zoom),
            width,
            height,
            frozen_width: fw,
            frozen_height: fh,
            scroll_x: x,
            scroll_y: y,
            zoom,
        }
    }
    pub fn hit(&self, x: f32, y: f32) -> Option<CellRef> {
        if x < ROW_HEADER || y < COL_HEADER || x >= self.width || y >= self.height {
            return None;
        }
        let col = self.cols.iter().find(|l| x >= l.start && x < l.start + l.size && (l.frozen || x >= ROW_HEADER + self.frozen_width))?;
        let row = self.rows.iter().find(|l| y >= l.start && y < l.start + l.size && (l.frozen || y >= COL_HEADER + self.frozen_height))?;
        Some(CellRef::new(row.index, col.index))
    }
    pub fn cell_rect(&self, cell: CellRef, geo: &SheetGeometry, freeze: (u32, u32)) -> Rect {
        let x = geo.cols.start(cell.col) * self.zoom - if cell.col < freeze.1 { 0.0 } else { self.scroll_x };
        let y = geo.rows.start(cell.row) * self.zoom - if cell.row < freeze.0 { 0.0 } else { self.scroll_y };
        Rect {
            x: ROW_HEADER + x as f32,
            y: COL_HEADER + y as f32,
            width: ((geo.cols.start(cell.col.saturating_add(1)) - geo.cols.start(cell.col)) * self.zoom) as f32,
            height: ((geo.rows.start(cell.row.saturating_add(1)) - geo.rows.start(cell.row)) * self.zoom) as f32,
        }
    }
    pub fn reveal(&self, cell: CellRef, geo: &SheetGeometry, freeze: (u32, u32)) -> (f64, f64) {
        let rect = self.cell_rect(cell, geo, freeze);
        let mut x = self.scroll_x;
        let mut y = self.scroll_y;
        if cell.col >= freeze.1 {
            if rect.x < ROW_HEADER + self.frozen_width {
                x -= (ROW_HEADER + self.frozen_width - rect.x) as f64;
            } else if rect.x + rect.width > self.width {
                x += (rect.x + rect.width - self.width) as f64;
            }
        }
        if cell.row >= freeze.0 {
            if rect.y < COL_HEADER + self.frozen_height {
                y -= (COL_HEADER + self.frozen_height - rect.y) as f64;
            } else if rect.y + rect.height > self.height {
                y += (rect.y + rect.height - self.height) as f64;
            }
        }
        (x.max(0.0), y.max(0.0))
    }
    pub fn painted_cell_bound(&self) -> usize {
        self.rows.len().saturating_mul(self.cols.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn used_extent_follows_expansion_clear_and_undo() {
        let mut s = gridcraft_engine::Session::new();
        s.execute("file.new", serde_json::json!({})).unwrap();
        for (command, params, expected) in [
            ("cell.set", serde_json::json!({"cell":"B2","input":"1"}), CellRef::new(1, 1)),
            ("cell.set", serde_json::json!({"cell":"T1000000","input":"2"}), CellRef::new(999_999, 19)),
            ("cell.set", serde_json::json!({"cell":"T1000000","input":""}), CellRef::new(1, 1)),
            ("edit.undo", serde_json::json!({}), CellRef::new(999_999, 19)),
        ] {
            s.execute(command, params).unwrap();
            let sheet = &s.active().unwrap().wb.sheets[0];
            assert_eq!(SheetGeometry::new(sheet).used_end, expected);
        }
    }
    #[test]
    fn millionth_row_is_precise_and_bounded() {
        let sh = Sheet::new("Large");
        let geo = SheetGeometry::new(&sh);
        let y = geo.rows.start(999_999) + 0.25;
        let view = GridLayout::new(&sh, &geo, 0.0, y, 1280.0, 720.0);
        assert_eq!(view.rows.first().unwrap().index, 999_999);
        assert!((view.rows.first().unwrap().start - (COL_HEADER - 0.25)).abs() < 0.01);
        assert!(view.painted_cell_bound() < 1000);
    }
    #[test]
    fn frozen_cells_win_hit_testing() {
        let mut sh = Sheet::new("Freeze");
        sh.freeze = Some((1, 1));
        sh.zoom = 125;
        let geo = SheetGeometry::new(&sh);
        let view = GridLayout::new(&sh, &geo, 300.0, 500.0, 1000.0, 600.0);
        assert_eq!(view.hit(ROW_HEADER + 4.0, COL_HEADER + 4.0), Some(CellRef::new(0, 0)));
        for row in view.rows.iter().filter(|r| !r.frozen && r.start >= COL_HEADER + view.frozen_height) {
            for col in view.cols.iter().filter(|c| !c.frozen && c.start >= ROW_HEADER + view.frozen_width) {
                let point = (col.start + 1.0, row.start + 1.0);
                assert_eq!(view.hit(point.0, point.1), Some(CellRef::new(row.index, col.index)));
            }
        }
    }
    #[test]
    fn invalid_scroll_and_far_selection_are_safe() {
        let sh = Sheet::new("Sheet");
        let geo = SheetGeometry::new(&sh);
        let view = GridLayout::new(&sh, &geo, f64::NAN, f64::INFINITY, 800.0, 600.0);
        assert_eq!(view.scroll_y, 0.0);
        let (x, y) = view.reveal(CellRef::new(MAX_ROWS - 1, MAX_COLS - 1), &geo, (0, 0));
        let far = GridLayout::new(&sh, &geo, x, y, 800.0, 600.0);
        assert!(far.rows.iter().any(|r| r.index == MAX_ROWS - 1));
        assert!(far.cols.iter().any(|c| c.index == MAX_COLS - 1));
    }
    #[test]
    fn hidden_prefix_and_custom_dimensions_share_hit_geometry() {
        let mut sh = Sheet::new("Hidden");
        for row in 0..128 {
            sh.rows.insert(row, gridcraft_engine::model::LineInfo { hidden: true, ..Default::default() });
        }
        sh.cols.insert(0, gridcraft_engine::model::LineInfo { size: Some(137.), ..Default::default() });
        let geo = SheetGeometry::new(&sh);
        let view = GridLayout::new(&sh, &geo, 0., 0., 800., 600.);
        assert_eq!(view.rows.first().unwrap().index, 128);
        assert_eq!(view.hit(ROW_HEADER + 136., COL_HEADER + 2.), Some(CellRef::new(128, 0)));
        assert_eq!(view.hit(ROW_HEADER + 138., COL_HEADER + 2.), Some(CellRef::new(128, 1)));
    }
}
