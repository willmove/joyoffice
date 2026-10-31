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
