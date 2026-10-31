//! Two-dimensional, sparse, f64 worksheet geometry shared by paint and hit testing.
use joysheet_engine::{
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
#[path = "tests/geometry_tests.rs"]
mod tests;
