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
mod tests {
    use super::*;
    use joysheet_core::Value;

    fn c(r: u32, col: u32) -> CellRef {
        CellRef::new(r, col)
    }
    fn slow_extent(s: &CellStore) -> Option<RangeRef> {
        s.iter().map(|(c, _)| RangeRef::new(c, c)).reduce(|a, b| a.union(&b))
    }
    fn formula_positions(s: &CellStore, range: RangeRef) -> Vec<CellRef> {
        s.iter().filter(|(c, cell)| range.contains(*c) && cell.formula.is_some()).map(|(c, _)| c).collect()
    }
    #[test]
    fn formula_index_matches_full_walk_after_mutation_and_cow() {
        use crate::Formula;
        let all = RangeRef::new(c(0, 0), c(1000, 6));
        let mut s = CellStore::new();
        for row in 0..256 {
            for col in 0..4 {
                s.set(c(row, col), Cell::value(1.0.into()));
            }
        }
        for i in 0..350 {
            let snapshot = s.clone();
            let before = formula_positions(&snapshot, all);
            let at = c((i * 71) % 330, (i * 11) % 6);
            match i % 5 {
                0 => s.set(at, Cell::formula(Formula::new("=1"))),
                1 => s.set(at, Cell::value(2.0.into())),
                2 => {
                    s.set(at, Cell::value(3.0.into()));
                    s.get_mut(at).unwrap().formula = Some(Formula::new("=4").into());
                }
                3 => {
                    if let Some(cell) = s.get_mut(at) {
                        cell.formula = None;
                    }
                }
                _ => {
                    s.remove(at);
                }
            }
            for range in [all, RangeRef::new(c(12, i % 6), c(300, i % 6)), RangeRef::new(c(64, 0), c(127, 3))] {
                assert_eq!(s.iter_formulas(range).map(|(c, _)| c).collect::<Vec<_>>(), formula_positions(&s, range));
            }
            assert_eq!(snapshot.iter_formulas(all).map(|(c, _)| c).collect::<Vec<_>>(), before);
            if i % 53 == 0 {
                let mut json = serde_json::to_value(&s).unwrap();
                json["bands"]["0"]["formulas"] = serde_json::json!(false);
                let mut restored: CellStore = serde_json::from_value(json).unwrap();
                assert_eq!(restored.iter_formulas(all).map(|(c, _)| c).collect::<Vec<_>>(), formula_positions(&s, all));
                restored.rebuild();
                // Formula ASTs are intentionally not serialized; compare persisted content.
                assert_eq!(serde_json::to_value(&restored).unwrap(), serde_json::to_value(&s).unwrap());
            }
        }
    }
    #[test]
    fn narrow_and_wide_range_walks_keep_identical_cells_and_order() {
        let mut s = CellStore::new();
        for row in [0, 1, 63, 64, 127, 128, 999_999] {
            for col in [0, 2, 4, 8] {
                s.set(c(row, col), Cell::value((row as f64 + col as f64).into()));
            }
        }
        for start in 0..10 {
            for end in start..10 {
                let range = RangeRef::new(c(1, start), c(999_999, end));
                let expected: Vec<_> = s.iter().filter(|(at, _)| range.contains(*at)).collect();
                assert_eq!(s.iter_range(range).collect::<Vec<_>>(), expected);
                for row in [0, 1, 62, 63, 64, 999_999] {
                    let expected: Vec<_> =
                        s.iter().filter(|(at, _)| at.row == row && at.col >= start && at.col <= end).map(|(at, cell)| (at.col, cell)).collect();
                    assert_eq!(s.row(row, start, end).collect::<Vec<_>>(), expected);
                }
            }
        }
        s.set(c(joysheet_core::MAX_ROWS + 1, joysheet_core::MAX_COLS + 1), Cell::formula(crate::Formula::new("=1")));
        assert_eq!(
            s.formulas().map(|(c, _)| c).collect::<Vec<_>>(),
            s.iter().filter(|(_, cell)| cell.formula.is_some()).map(|(c, _)| c).collect::<Vec<_>>()
        );
        s.get_mut(c(1, 0)).unwrap().style = crate::StyleId(1);
        assert_eq!(s.bands.get(&0).unwrap().formulas.get(), Some(&true));
        assert_eq!(s.iter_formulas(RangeRef::new(c(0, 0), c(999_999, 0))).count(), 0);
    }
    #[test]
    fn cached_extents_match_sparse_edits_shifts_snapshots_and_json() {
        let mut s = CellStore::new();
        let mut random = 71u64;
        for i in 0..1200 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let cell = c((random >> 32) as u32 % 1024, (random >> 16) as u32 % 41);
            let prior = s.clone();
            let prior_range = slow_extent(&prior);
            match i % 4 {
                0 => {
                    s.remove(cell);
                }
                1 => {
                    s.set(cell, Cell::default());
                }
                _ => {
                    s.set(cell, Cell::value(Value::Number(i as f64)));
                }
            }
            assert_eq!(s.used_range(), slow_extent(&s));
            assert_eq!(prior.used_range(), prior_range);
            if i % 137 == 0 {
                let mut restored: CellStore = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
                assert_eq!(s, restored);
                assert_eq!(restored.used_range(), s.used_range());
                assert_eq!(restored.rebuild(), 0);
                assert_eq!(restored.used_range(), slow_extent(&restored));
            }
        }
        // Structural movement goes through the same mutators as ordinary edits.
        s.shift_rows(32, 3);
        s.shift_cols(2, 1);
        assert_eq!(s.used_range(), slow_extent(&s));
    }
    #[test]
    fn extent_cache_is_not_content_and_survives_value_edits() {
        let mut s = CellStore::new();
        s.set(c(0, 0), Cell::value(1.0.into()));
        s.set(c(999_999, 19), Cell::value(2.0.into()));
        let cold = serde_json::to_string(&s).unwrap();
        let snapshot = s.clone();
        assert_eq!(s.used_range(), slow_extent(&s));
        assert_eq!(serde_json::to_string(&s).unwrap(), cold);
        assert_eq!(s, snapshot);
        s.set(c(999_999, 19), Cell::value(3.0.into()));
        assert!(s.bands.values().all(|b| b.extent.get().is_some()));
        *s.get_mut(c(999_999, 19)).unwrap() = Cell::default();
        assert_eq!(s.used_range(), slow_extent(&s)); // Mutable values do not remove stored keys.
        s.remove(c(999_999, 19));
        assert_eq!(s.used_range().unwrap().end, c(0, 0));
        assert_eq!(snapshot.used_range().unwrap().end, c(999_999, 19));
        let mut json: serde_json::Value = serde_json::from_str(&cold).unwrap();
        json["bands"]["0"]["extent"] = serde_json::json!("untrusted derived cache");
        let restored: CellStore = serde_json::from_value(json).unwrap();
        assert_eq!(restored.used_range(), snapshot.used_range());
    }

    #[test]
    fn set_get_remove() {
        let mut s = CellStore::new();
        s.set(c(0, 0), Cell::value(1.0.into()));
        s.set(c(100, 5), Cell::value("x".into()));
        assert_eq!(s.len(), 2);
        assert_eq!(s.get(c(100, 5)).map(|x| x.value.clone()), Some(Value::from("x")));
        assert_eq!(s.used_range().unwrap().a1(), "A1:F101");
        s.set(c(0, 0), Cell::default());
        assert_eq!(s.len(), 1);
        assert!(s.remove(c(100, 5)).is_some());
        assert!(s.is_empty());
        assert!(s.used_range().is_none());
    }

    #[test]
    fn cow_snapshots() {
        let mut s = CellStore::new();
        for r in 0..1000 {
            s.set(c(r, 0), Cell::value((r as f64).into()));
        }
        let snap = s.clone();
        s.set(c(5, 0), Cell::value(99.0.into()));
        assert_eq!(snap.get(c(5, 0)).unwrap().value, Value::Number(5.0));
        assert_eq!(s.get(c(5, 0)).unwrap().value, Value::Number(99.0));
    }

    #[test]
    fn diff_visits_changed_bands() {
        let mut s = CellStore::new();
        for r in 0..1000 {
            s.set(c(r, 0), Cell::value((r as f64).into()));
        }
        let snap = s.clone();
        assert!(s.diff(&snap).is_empty());
        s.set(c(5, 0), Cell::value(99.0.into()));
        s.set(c(700, 3), Cell::value("new".into()));
        s.remove(c(900, 0));
        assert_eq!(snap.diff(&s), [c(5, 0), c(700, 3), c(900, 0)]);
    }

    #[test]
    fn range_iteration_order() {
        let mut s = CellStore::new();
        s.set(c(70, 2), Cell::value(3.0.into()));
        s.set(c(1, 1), Cell::value(1.0.into()));
        s.set(c(1, 3), Cell::value(2.0.into()));
        s.set(c(500, 0), Cell::value(4.0.into()));
        let v: Vec<f64> = s.iter_range(RangeRef::parse("A1:D100").unwrap()).map(|(_, x)| x.value.as_f64().unwrap()).collect();
        assert_eq!(v, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn shifting() {
        let mut s = CellStore::new();
        s.set(c(2, 0), Cell::value(1.0.into()));
        s.set(c(5, 0), Cell::value(2.0.into()));
        s.shift_rows(3, 2);
        assert!(s.get(c(7, 0)).is_some());
        assert!(s.get(c(2, 0)).is_some());
        s.shift_cols(0, 1);
        assert!(s.get(c(7, 1)).is_some());
    }

    #[test]
    fn calculated_writeback_keeps_snapshots_formulas_and_sparse_extents() {
        let mut s = CellStore::new();
        for row in 0..200 {
            s.set(c(row, 1), Cell::formula(crate::Formula::new("=1+2")));
        }
        let original = s.clone();
        let range = RangeRef::parse("A1:D300").unwrap();
        let before: Vec<_> = s.iter_formulas(range).map(|(c, _)| c).collect();
        s.write_values((0..300).map(|r| (c(r, 1), joysheet_core::Value::Number(r as f64))));
        assert_eq!(s.len(), 200);
        assert_eq!(s.iter_formulas(range).map(|(c, _)| c).collect::<Vec<_>>(), before);
        assert_eq!(s.get(c(199, 1)).unwrap().value, joysheet_core::Value::Number(199.));
        assert_eq!(original.get(c(199, 1)).unwrap().value, joysheet_core::Value::Empty);
        assert_eq!(s.used_range(), original.used_range());
    }
}
