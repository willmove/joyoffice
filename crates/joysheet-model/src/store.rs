//! Copy-on-write sparse cell storage.
//!
//! Cells live in bands of 64 rows, each band behind an `Arc`. Cloning a store (an undo snapshot)
//! copies only the band pointers; editing a cell copies just its band.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use joysheet_core::{CellRef, RangeRef};
use serde::{Deserialize, Serialize};

use crate::cell::Cell;

const BAND: u32 = 64;

fn has_value(cell: &Cell) -> bool {
    !cell.value.is_empty() || cell.formula.is_some()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Band {
    /// row → (col → cell)
    rows: BTreeMap<u32, BTreeMap<u32, Cell>>,
    /// Derived solely from keys; never trusted from a file or compared as document content.
    #[serde(skip)]
    extent: OnceLock<Option<RangeRef>>,
    #[serde(skip)]
    formulas: OnceLock<bool>,
}
impl Default for Band {
    fn default() -> Self {
        Self { rows: BTreeMap::new(), extent: OnceLock::new(), formulas: OnceLock::from(false) }
    }
}
impl PartialEq for Band {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}
impl Band {
    fn has_formulas(&self) -> bool {
        *self.formulas.get_or_init(|| self.rows.values().any(|cols| cols.values().any(|c| c.formula.is_some())))
    }
    fn extent(&self) -> Option<RangeRef> {
        *self.extent.get_or_init(|| {
            let r0 = *self.rows.keys().next()?;
            let r1 = *self.rows.keys().next_back()?;
            let mut c0 = u32::MAX;
            let mut c1 = 0;
            for cols in self.rows.values() {
                if let (Some(a), Some(b)) = (cols.keys().next(), cols.keys().next_back()) {
                    c0 = c0.min(*a);
                    c1 = c1.max(*b);
                }
            }
            (c0 != u32::MAX).then(|| RangeRef::new(CellRef::new(r0, c0), CellRef::new(r1, c1)))
        })
    }
}

// A one-column range needs one lookup per row, rather than two range-bound searches.
enum Columns<'a> {
    One(std::option::IntoIter<(&'a u32, &'a Cell)>),
    Many(std::collections::btree_map::Range<'a, u32, Cell>),
}
impl<'a> Iterator for Columns<'a> {
    type Item = (&'a u32, &'a Cell);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One(it) => it.next(),
            Self::Many(it) => it.next(),
        }
    }
}
fn columns(cols: &BTreeMap<u32, Cell>, start: u32, end: u32) -> Columns<'_> {
    if start == end { Columns::One(cols.get_key_value(&start).into_iter()) } else { Columns::Many(cols.range(start..=end)) }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellStore {
    bands: BTreeMap<u32, Arc<Band>>,
    len: usize,
}

impl CellStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn get(&self, c: CellRef) -> Option<&Cell> {
        self.bands.get(&(c.row / BAND))?.rows.get(&c.row)?.get(&c.col)
    }
    /// Mutable access (copies the band if shared).
    pub fn get_mut(&mut self, c: CellRef) -> Option<&mut Cell> {
        let band = self.bands.get_mut(&(c.row / BAND))?;
        if !band.rows.get(&c.row).is_some_and(|r| r.contains_key(&c.col)) {
            return None;
        }
        let band = Arc::make_mut(band);
        // A caller can replace the formula through this reference. Extents depend only on keys.
        if let Some(flag) = band.formulas.get_mut() {
            *flag = true;
        } else {
            let _ = band.formulas.set(true);
        }
        band.rows.get_mut(&c.row)?.get_mut(&c.col)
    }
    /// Calculation results in row-major order. Detaches each touched band once and preserves
    /// formula/extent indexes: only values change. Unknown cells are ignored, like get_mut.
    pub fn write_values(&mut self, values: impl Iterator<Item = (CellRef, joysheet_core::Value)>) {
        let mut values = values.peekable();
        while let Some((at, _)) = values.peek() {
            let key = at.row / BAND;
            if let Some(band) = self.bands.get_mut(&key) {
                let band = Arc::make_mut(band);
                while values.peek().is_some_and(|(c, _)| c.row / BAND == key) {
                    if let Some((at, value)) = values.next()
                        && let Some(cell) = band.rows.get_mut(&at.row).and_then(|r| r.get_mut(&at.col))
                        && cell.value != value
                    {
                        cell.value = value;
                    }
                }
            } else {
                while values.peek().is_some_and(|(c, _)| c.row / BAND == key) {
                    values.next();
                }
            }
        }
    }
    /// Stores a cell; a default (empty, unstyled) cell is removed instead.
    pub fn set(&mut self, c: CellRef, cell: Cell) {
        if cell.is_blank() {
            self.remove(c);
            return;
        }
        let band = Arc::make_mut(self.bands.entry(c.row / BAND).or_default());
        let formula = cell.formula.is_some();
        let prior = band.rows.entry(c.row).or_default().insert(c.col, cell);
        if formula {
            if let Some(flag) = band.formulas.get_mut() {
                *flag = true;
            } else {
                let _ = band.formulas.set(true);
            }
        } else if prior.as_ref().is_some_and(|c| c.formula.is_some()) {
            band.formulas.take();
        }
        if prior.is_none() {
            self.len += 1;
            band.extent.take();
        }
    }
    /// Puts every cell in the band its row says and recounts the length — serialized band keys
    /// and the cached count are derived state, and the ones a file carries are not trusted. Cells
    /// outside the sheet grid are dropped; returns how many (#1008).
    pub fn rebuild(&mut self) -> usize {
        let mut out = CellStore::new();
        let mut dropped = 0;
        for (_, band) in std::mem::take(&mut self.bands) {
            for (row, cols) in band.rows.iter() {
                for (col, cell) in cols.iter() {
                    let at = CellRef::new(*row, *col);
                    if at.is_valid() {
                        out.set(at, cell.clone());
                    } else {
                        dropped += 1;
                    }
                }
            }
        }
        *self = out;
        dropped
    }

    pub fn remove(&mut self, c: CellRef) -> Option<Cell> {
        let key = c.row / BAND;
        let band = self.bands.get_mut(&key)?;
        if !band.rows.get(&c.row).is_some_and(|r| r.contains_key(&c.col)) {
            return None;
        }
        let b = Arc::make_mut(band);
        b.extent.take();
        let row = b.rows.get_mut(&c.row)?;
        let old = row.remove(&c.col);
        if old.as_ref().is_some_and(|c| c.formula.is_some()) {
            b.formulas.take();
        }
        if row.is_empty() {
            b.rows.remove(&c.row);
        }
        if b.rows.is_empty() {
            self.bands.remove(&key);
        }
        if old.is_some() {
            self.len -= 1;
        }
        old
    }
    /// Cells in `r`, row-major. Efficient for sparse data: skips empty bands and rows.
    pub fn iter_range(&self, r: RangeRef) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        let (b0, b1) = (r.start.row / BAND, r.end.row / BAND);
        self.bands.range(b0..=b1).flat_map(move |(_, band)| {
            band.rows
                .range(r.start.row..=r.end.row)
                .flat_map(move |(row, cols)| columns(cols, r.start.col, r.end.col).map(move |(col, cell)| (CellRef::new(*row, *col), cell)))
        })
    }
    /// Formula cells, in exactly the same row-major order as `iter_range().filter(...)`.
    /// Bands known to contain no formulas are skipped. Mutable access invalidates this index.
    pub fn iter_formulas(&self, r: RangeRef) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        self.bands.range(r.start.row / BAND..=r.end.row / BAND).filter(|(_, band)| band.formulas.get() != Some(&false)).flat_map(move |(_, band)| {
            band.rows.range(r.start.row..=r.end.row).flat_map(move |(row, cols)| {
                columns(cols, r.start.col, r.end.col).filter(|(_, c)| c.formula.is_some()).map(move |(col, cell)| (CellRef::new(*row, *col), cell))
            })
        })
    }
    pub fn iter(&self) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        self.bands
            .values()
            .flat_map(|band| band.rows.iter().flat_map(|(row, cols)| cols.iter().map(move |(col, cell)| (CellRef::new(*row, *col), cell))))
    }
    /// All stored formulas, including unsanitized positions, in the order of `iter`.
    pub fn formulas(&self) -> impl Iterator<Item = (CellRef, &Cell)> + '_ {
        self.bands.values().filter(|band| band.has_formulas()).flat_map(|band| {
            band.rows
                .iter()
                .flat_map(|(row, cols)| cols.iter().filter(|(_, c)| c.formula.is_some()).map(move |(col, cell)| (CellRef::new(*row, *col), cell)))
        })
    }
    /// Cells of one row in `c0..=c1`.
    pub fn row(&self, row: u32, c0: u32, c1: u32) -> impl Iterator<Item = (u32, &Cell)> + '_ {
        self.bands
            .get(&(row / BAND))
            .and_then(|b| b.rows.get(&row))
            .into_iter()
            .flat_map(move |cols| columns(cols, c0, c1).map(|(c, cell)| (*c, cell)))
    }
    /// Smallest range containing every stored cell, `None` when empty.
    pub fn used_range(&self) -> Option<RangeRef> {
        let first_band = self.bands.values().next()?;
        let last_band = self.bands.values().next_back()?;
        let r0 = *first_band.rows.keys().next()?;
        let r1 = *last_band.rows.keys().next_back()?;
        let mut c0 = u32::MAX;
        let mut c1 = 0;
        for band in self.bands.values() {
            if let Some(range) = band.extent() {
                c0 = c0.min(range.start.col);
                c1 = c1.max(range.end.col);
            }
        }
        if c0 == u32::MAX {
            return None;
        }
        Some(RangeRef::new(CellRef::new(r0, c0), CellRef::new(r1, c1)))
    }
    /// Last stored row in column `col` at or above `below_or_at` (for Ctrl+arrow navigation).
    pub fn has(&self, c: CellRef) -> bool {
        self.get(c).is_some_and(has_value)
    }
    /// Last row (inclusive, capped at `cap`) of the maximal run of contiguous rows at or after
    /// `from.row` that store a cell in `from.col` with a non-empty *value* in `from.col`. `None`
    /// when `from` itself doesn't qualify.
    ///
    /// Deliberately narrower than `has`/`get` (which also count a formula with no cached value
    /// yet, for Ctrl+arrow navigation): this matches `Sheet::value_ref`, whose callers test the
    /// `Value` it returns with `!v.is_empty()` regardless of whether a formula produced it — a
    /// formula cell with an empty cached value (`=""`, or not yet recalculated) is "empty" to
    /// them. A stored cell with no value and no formula (for example a formatted spill child) does
    /// fall through to `spill` in `value_ref`, so `last_contiguous_nonempty` asks `value_ref`
    /// about the row where this run stops.
    ///
    /// Walks each band's row map with `BTreeMap::range` (one in-order pass per band) instead of
    /// one `get` per row, so a long, densely-populated run costs roughly one tree-traversal step
    /// per row rather than one binary search per row.
    pub fn contiguous_value_run(&self, from: CellRef, cap: u32) -> Option<u32> {
        let mut last = None;
        let mut expect = from.row;
        while expect <= cap {
            let band_key = expect / BAND;
            let Some(band) = self.bands.get(&band_key) else { break };
            let band_hi = ((band_key + 1) * BAND - 1).min(cap);
            let mut advanced = false;
            for (r, cols) in band.rows.range(expect..=band_hi) {
                if *r != expect || cols.get(&from.col).is_none_or(|c| c.value.is_empty()) {
                    return last;
                }
                last = Some(expect);
                expect += 1;
                advanced = true;
            }
            if !advanced {
                break;
            }
        }
        last
    }
    /// Positions whose cells differ between `self` and `other` (stored on one side only, or not
    /// equal). Bands both stores share are skipped, so comparing an edited copy with the store
    /// it was cloned from only visits the bands the edit touched.
    pub fn diff(&self, other: &CellStore) -> Vec<CellRef> {
        let empty = Band::default();
        let mut keys: Vec<u32> = self.bands.keys().chain(other.bands.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        let mut out = Vec::new();
        for k in keys {
            let (a, b) = (self.bands.get(&k), other.bands.get(&k));
            if let (Some(a), Some(b)) = (a, b)
                && Arc::ptr_eq(a, b)
            {
                continue;
            }
            let (a, b) = (a.map_or(&empty, |x| x), b.map_or(&empty, |x| x));
            for (row, cols) in &a.rows {
                for (col, cell) in cols {
                    if b.rows.get(row).and_then(|r| r.get(col)) != Some(cell) {
                        out.push(CellRef::new(*row, *col));
                    }
                }
            }
            for (row, cols) in &b.rows {
                for col in cols.keys() {
                    if !a.rows.get(row).is_some_and(|r| r.contains_key(col)) {
                        out.push(CellRef::new(*row, *col));
                    }
                }
            }
        }
        out
    }
    /// Approximate heap storage kept alive by this snapshot's bands that `newer` no longer
    /// shares. This is for bounding Undo history, not for reporting process memory: BTreeMap
    /// nodes and allocator overhead vary by platform. Shared formulas and values are counted
    /// only when the newer band no longer holds the same Arc.
    pub fn retained_bytes_against(&self, newer: &CellStore) -> usize {
        use std::mem::size_of;

        let mut bytes = 0usize;
        for (key, band) in &self.bands {
            let next = newer.bands.get(key);
            if next.is_some_and(|b| Arc::ptr_eq(b, band)) {
                continue;
            }
            bytes = bytes.saturating_add(size_of::<Band>());
            for (row, cols) in &band.rows {
                bytes = bytes.saturating_add(size_of::<u32>() + 3 * size_of::<usize>());
                for (col, cell) in cols {
                    // Cells and BTreeMap nodes are duplicated with a cloned band even when
                    // their formula/value Arcs are still shared with the next snapshot.
                    bytes = bytes.saturating_add(size_of::<u32>() + size_of::<Cell>() + 3 * size_of::<usize>());
                    let next_cell = next.and_then(|b| b.rows.get(row)).and_then(|r| r.get(col));
                    if let Some(formula) = &cell.formula
                        && !next_cell.and_then(|c| c.formula.as_ref()).is_some_and(|f| Arc::ptr_eq(f, formula))
                    {
                        bytes = bytes.saturating_add(size_of::<crate::cell::Formula>() + formula.text.len());
                        if let Some(expr) = &formula.expr {
                            expr.walk(&mut |_| bytes = bytes.saturating_add(size_of::<joysheet_formula::Expr>()));
                        }
                    }
                    match &cell.value {
                        joysheet_core::Value::Text(text)
                            if !next_cell.and_then(|c| c.value.as_text()).is_some_and(|t| std::ptr::eq(t, text.as_ref())) =>
                        {
                            bytes = bytes.saturating_add(text.len());
                        }
                        joysheet_core::Value::Array(array)
                            if !next_cell.is_some_and(|c| matches!(&c.value, joysheet_core::Value::Array(a) if Arc::ptr_eq(a, array))) =>
                        {
                            bytes = bytes.saturating_add(
                                size_of::<joysheet_core::Array>() + array.data.len().saturating_mul(size_of::<joysheet_core::Value>()),
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        bytes
    }
    /// Removes every cell in `r` and returns them.
    pub fn take_range(&mut self, r: RangeRef) -> Vec<(CellRef, Cell)> {
        let keys: Vec<CellRef> = self.iter_range(r).map(|(c, _)| c).collect();
        keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect()
    }
    /// Rows strictly greater than or equal to `from` moved by `delta` (positive = down). Rows that
    /// would land before `from`... callers delete the vacated block first.
    pub fn shift_rows(&mut self, from: u32, delta: i64) {
        let moved: Vec<(CellRef, Cell)> = {
            let keys: Vec<CellRef> = self.iter().filter(|(c, _)| c.row >= from).map(|(c, _)| c).collect();
            keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect()
        };
        for (c, cell) in moved {
            let nr = c.row as i64 + delta;
            if (0..joysheet_core::MAX_ROWS as i64).contains(&nr) {
                self.set(CellRef::new(nr as u32, c.col), cell);
            }
        }
    }
    pub fn shift_cols(&mut self, from: u32, delta: i64) {
        let keys: Vec<CellRef> = self.iter().filter(|(c, _)| c.col >= from).map(|(c, _)| c).collect();
        let moved: Vec<(CellRef, Cell)> = keys.into_iter().filter_map(|c| self.remove(c).map(|cell| (c, cell))).collect();
        for (c, cell) in moved {
            let nc = c.col as i64 + delta;
            if (0..joysheet_core::MAX_COLS as i64).contains(&nc) {
                self.set(CellRef::new(c.row, nc as u32), cell);
            }
        }
    }
    /// Shifts cells within rows `r0..=r1` and columns `>= from` horizontally (Insert/Delete Cells
    /// with shift right/left).
    pub fn shift_cols_in_rows(&mut self, r0: u32, r1: u32, from: u32, delta: i64) {
        let range = RangeRef::new(CellRef::new(r0, from), CellRef::new(r1, joysheet_core::MAX_COLS - 1));
        let moved = self.take_range(range);
        for (c, cell) in moved {
            let nc = c.col as i64 + delta;
            if (0..joysheet_core::MAX_COLS as i64).contains(&nc) {
                self.set(CellRef::new(c.row, nc as u32), cell);
            }
        }
    }
    /// Shifts cells within columns `c0..=c1` and rows `>= from` vertically.
    pub fn shift_rows_in_cols(&mut self, c0: u32, c1: u32, from: u32, delta: i64) {
        let range = RangeRef::new(CellRef::new(from, c0), CellRef::new(joysheet_core::MAX_ROWS - 1, c1));
        let moved = self.take_range(range);
        for (c, cell) in moved {
            let nr = c.row as i64 + delta;
            if (0..joysheet_core::MAX_ROWS as i64).contains(&nr) {
                self.set(CellRef::new(nr as u32, c.col), cell);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/store_tests.rs"]
mod tests;
