use super::*;
use crate::cell::Formula;

#[test]
fn line_index_matches_linear() {
    let mut s = Sheet::new("S");
    for r in (0..5000).step_by(3) {
        s.rows.insert(r, LineInfo { size: Some(10.0 + (r % 7) as f32), hidden: r % 11 == 0, ..Default::default() });
    }
    let idx = LineIndex::new(&s.rows, s.default_row_height, joysheet_core::MAX_ROWS);
    for r in [0u32, 1, 2, 3, 4, 100, 999, 4998, 5000, 6000] {
        assert!((idx.start(r) - s.row_top(r)).abs() < 1e-6, "start {r}");
    }
    for y in [0.0, 5.0, 33.3, 1000.0, 47_000.0, 90_000.0] {
        assert_eq!(idx.at(y), s.row_at(y), "at {y}");
    }
}

/// Reference implementation: one `value_ref` call per row, exactly the loop
/// `last_contiguous_nonempty` replaces (from `grid.rs`'s fill-handle double-click).
fn last_contiguous_nonempty_naive(s: &Sheet, col: u32, from_row: u32, cap: u32) -> u32 {
    let mut row = from_row;
    while row <= cap && s.value_ref(CellRef::new(row, col)).is_some_and(|v| !v.is_empty()) {
        row += 1;
    }
    row.saturating_sub(1).max(from_row.saturating_sub(1))
}

/// Deterministic xorshift32 so failures reproduce without pulling in a `rand` dependency
/// just for this test.
struct Xorshift32(u32);
impl Xorshift32 {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n.max(1)
    }
}

#[test]
fn last_contiguous_nonempty_matches_naive() {
    let mut rng = Xorshift32(0x9e3779b9);
    for trial in 0..300u32 {
        let mut s = Sheet::new("S");
        let rows = rng.below(400);
        // A mix of directly-stored values, formulas (value empty but formula present),
        // spill-only cells, blank-but-stored cells, and gaps, like a real sheet.
        for r in 0..rows {
            match rng.below(5) {
                0 => s.set_value(CellRef::new(r, 0), Value::Number(r as f64)),
                1 => s.set_cell(CellRef::new(r, 0), Cell { formula: Some(Arc::new(Formula::new("1"))), ..Default::default() }),
                2 => {
                    s.spill.insert(CellRef::new(r, 0), Value::Number(r as f64));
                }
                3 => {
                    // Stored but blank: does not count (matches `has_value`/`value_ref`).
                    s.set_value(CellRef::new(r, 0), Value::Text("".into()));
                }
                4 if rng.below(2) == 0 => {
                    // A style-only stored cell over a spill value: `value_ref` falls through
                    // to the spill value, so the row counts.
                    s.spill.insert(CellRef::new(r, 0), Value::Number(r as f64));
                    s.set_cell(CellRef::new(r, 0), Cell { style: StyleId(1), ..Default::default() });
                }
                _ => {} // gap
            }
        }
        for _ in 0..5 {
            let from_row = rng.below(420);
            let cap = from_row.saturating_add(rng.below(450));
            let got = s.last_contiguous_nonempty(0, from_row, cap);
            let want = last_contiguous_nonempty_naive(&s, 0, from_row, cap);
            assert_eq!(got, want, "trial {trial}: col 0, from_row {from_row}, cap {cap}, rows up to {rows}");
        }
    }
}

#[test]
fn last_contiguous_nonempty_dense_run() {
    let mut s = Sheet::new("S");
    for r in 0..2000u32 {
        s.set_value(CellRef::new(r, 2), Value::Number(r as f64));
    }
    assert_eq!(s.last_contiguous_nonempty(2, 0, 1_999), 1999);
    assert_eq!(s.last_contiguous_nonempty(2, 0, 1_000), 1000); // capped
    assert_eq!(s.last_contiguous_nonempty(2, 500, 1_999), 1999);
    assert_eq!(s.last_contiguous_nonempty(2, 2000, 3000), 1999); // nothing at from_row
    assert_eq!(s.last_contiguous_nonempty(2, 2001, 3000), 2000); // from_row > cap: from_row - 1
}

#[test]
fn last_contiguous_nonempty_formula_with_empty_value_ends_the_run() {
    let mut s = Sheet::new("S");
    s.set_value(CellRef::new(0, 1), Value::Number(1.0));
    s.set_value(CellRef::new(1, 1), Value::Number(2.0));
    // row 2: a formula (e.g. `=""`) whose cached value is empty. It does not count, same as
    // `value_ref(...).is_some_and(|v| !v.is_empty())` says for it, even though it has a
    // formula (unlike `CellStore::has`, used for Ctrl+arrow navigation, which counts it).
    s.set_cell(CellRef::new(2, 1), Cell { formula: Some(Arc::new(Formula::new("\"\""))), ..Default::default() });
    s.set_value(CellRef::new(3, 1), Value::Number(4.0)); // would extend the run if row 2 didn't stop it
    assert_eq!(s.last_contiguous_nonempty(1, 0, 100), 1);
    assert_eq!(s.last_contiguous_nonempty(1, 2, 100), 1); // from_row itself is the stop
}

#[test]
fn last_contiguous_nonempty_spill_extends_a_style_only_stored_row() {
    let mut s = Sheet::new("S");
    s.set_value(CellRef::new(0, 1), Value::Number(1.0));
    // row 1: a formatted spill child: stored with a style only, value from `spill`.
    s.spill.insert(CellRef::new(1, 1), Value::Number(2.0));
    s.set_cell(CellRef::new(1, 1), Cell { style: StyleId(1), ..Default::default() });
    s.set_value(CellRef::new(2, 1), Value::Number(3.0));
    assert_eq!(s.last_contiguous_nonempty(1, 0, 100), 2);
    assert_eq!(s.last_contiguous_nonempty(1, 1, 100), 2);
}

#[test]
fn last_contiguous_nonempty_spill_extends_an_unstored_row() {
    let mut s = Sheet::new("S");
    s.set_value(CellRef::new(0, 1), Value::Number(1.0));
    // row 1: nothing stored, but a spill value from an array formula anchored elsewhere.
    s.spill.insert(CellRef::new(1, 1), Value::Number(2.0));
    s.set_value(CellRef::new(2, 1), Value::Number(3.0));
    // row 3: a genuine gap (nothing stored, no spill).
    assert_eq!(s.last_contiguous_nonempty(1, 0, 100), 2);
    assert_eq!(s.last_contiguous_nonempty(1, 3, 100), 2); // from_row itself is the gap
}

#[test]
fn geometry() {
    let mut s = Sheet::new("S");
    assert_eq!(s.row_top(3), 60.0);
    s.rows.insert(1, LineInfo { size: Some(40.0), ..Default::default() });
    s.rows.insert(2, LineInfo { hidden: true, ..Default::default() });
    assert_eq!(s.row_top(1), 20.0);
    assert_eq!(s.row_top(2), 60.0);
    assert_eq!(s.row_top(3), 60.0);
    assert_eq!(s.row_top(4), 80.0);
    assert_eq!(s.row_at(10.0), 0);
    assert_eq!(s.row_at(30.0), 1);
    assert_eq!(s.row_at(59.0), 1);
    assert_eq!(s.row_at(61.0), 3);
    assert_eq!(s.row_at(85.0), 4);
    assert_eq!(s.col_at(1e12), joysheet_core::MAX_COLS - 1);
    assert_eq!(s.col_at(-5.0), 0);
    assert_eq!(s.col_left(2), 128.0);
}
