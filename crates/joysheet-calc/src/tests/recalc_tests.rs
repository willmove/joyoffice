use std::time::{Duration, Instant};

use joysheet_core::{CellRef, RangeRef, Value};
use joysheet_model::{Cell, Formula, Workbook};

use super::{Calc, HashMap, Host, Intervals, Key, PassHost};

/// xorshift64 for repeatable pseudo-random cases.
fn rng(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn found(iv: &Intervals<u32>, a: u32, b: u32) -> Vec<u32> {
    let mut v = Vec::new();
    iv.query(a, b, &mut |t| v.push(*t));
    v.sort_unstable();
    v
}

#[test]
fn intervals_match_a_linear_scan() {
    let mut iv: Intervals<u32> = Intervals::default();
    let mut model: Vec<(u32, u32, u32)> = Vec::new();
    let mut s = 0x9E37_79B9_7F4A_7C15;
    for step in 0..20_000u32 {
        let roll = rng(&mut s) % 10;
        if roll < 6 || model.is_empty() {
            let r0 = (rng(&mut s) % 500) as u32;
            let r1 = r0 + (rng(&mut s) % 40) as u32;
            if !model.contains(&(r0, r1, step)) {
                iv.insert(r0, r1, step);
                model.push((r0, r1, step));
            }
        } else {
            let i = (rng(&mut s) % model.len() as u64) as usize;
            let (r0, r1, t) = model.swap_remove(i);
            iv.remove(r0, r1, t);
            if roll == 9 {
                // Removed and put back before the next merge.
                iv.insert(r0, r1, t);
                model.push((r0, r1, t));
            }
        }
        if rng(&mut s).is_multiple_of(50) {
            iv.settle();
        }
        if step.is_multiple_of(97) {
            let a = (rng(&mut s) % 560) as u32;
            let b = a + (rng(&mut s) % 30) as u32;
            let mut want: Vec<u32> = model.iter().filter(|(r0, r1, _)| *r0 <= b && *r1 >= a).map(|(_, _, t)| *t).collect();
            want.sort_unstable();
            assert_eq!(found(&iv, a, b), want, "rows {a}..={b} at step {step}");
        }
    }
    iv.settle();
    for row in 0..560 {
        let mut want: Vec<u32> = model.iter().filter(|(r0, r1, _)| *r0 <= row && row <= *r1).map(|(_, _, t)| *t).collect();
        want.sort_unstable();
        assert_eq!(found(&iv, row, row), want, "row {row}");
    }
    for (r0, r1, t) in std::mem::take(&mut model) {
        iv.remove(r0, r1, t);
    }
    iv.settle();
    assert!(iv.is_empty());
}

#[test]
fn interval_queries_cost_log_n_plus_matches() {
    // 40k one-row ranges (like =SUM(E1:J1) down a column) and 1k ranges over everything:
    // a point query visits O(log n) tree nodes per match, not every range in the column.
    let n = 40_000u32;
    let mut iv: Intervals<u32> = Intervals::default();
    for r in 0..n {
        iv.insert(r, r, r);
    }
    for t in 0..1_000 {
        iv.insert(0, n, n + t);
    }
    iv.settle();
    let mut worst = 0;
    for row in (0..n).step_by(7) {
        let mut k = 0;
        let visited = iv.query(row, row, &mut |_| k += 1);
        assert_eq!(k, 1_001);
        worst = worst.max(visited);
    }
    // 1,001 matches in a tree of depth 16: well under 4 nodes per match plus the path.
    assert!(worst < 4 * 1_001 + 64, "visited {worst} nodes");
    let mut k = 0;
    let visited = iv.query(n + 1, n + 5, &mut |_| k += 1);
    assert_eq!(k, 0);
    assert!(visited < 64, "visited {visited} nodes for no match");
}

/// `rows` rows of six numbers (E:J) and `=SUM(E{r}:J{r})` in K.
fn sum_rows(rows: u32) -> Workbook {
    let mut wb = Workbook::new();
    if let Some(sh) = wb.sheet_mut(0) {
        for r in 0..rows {
            for c in 4..10 {
                sh.cells.set(CellRef::new(r, c), Cell::value(Value::Number(f64::from(r % 97 + c))));
            }
            sh.cells.set(CellRef::new(r, 10), Cell::formula(Formula::new(&format!("=SUM(E{0}:J{0})", r + 1))));
        }
    }
    wb
}

/// Opening (full recalc), changing every value of a column, and re-entering every formula.
#[allow(clippy::disallowed_methods)] // a native-only timing test: the clock is read off wasm
fn open_and_bulk_edit(rows: u32) -> Duration {
    let mut wb = sum_rows(rows);
    let mut calc = Calc::new();
    let t = Instant::now();
    calc.recalc_all(&mut wb);
    let values: Vec<Key> = (0..rows).map(|r| (0, CellRef::new(r, 4))).collect();
    if let Some(sh) = wb.sheet_mut(0) {
        for (_, c) in &values {
            sh.cells.set(*c, Cell::value(Value::Number(1.0)));
        }
    }
    calc.cells_changed(&mut wb, &values);
    let formulas: Vec<Key> = (0..rows).map(|r| (0, CellRef::new(r, 10))).collect();
    calc.cells_changed(&mut wb, &formulas);
    let elapsed = t.elapsed();
    let last = wb.sheet(0).map(|s| s.value(CellRef::new(rows - 1, 10)));
    let r = rows - 1;
    assert_eq!(last, Some(Value::Number(f64::from(1 + (5..10).map(|c| r % 97 + c).sum::<u32>()))));
    elapsed
}

#[test]
fn range_formulas_recalc_and_bulk_edit_correctly() {
    // The values behind the timing test below, checked on every run.
    open_and_bulk_edit(500);
}

#[test]
fn range_seeds_match_coordinate_seeds_including_blocking_formulas() {
    let mut wb = Workbook::new();
    let sh = wb.sheet_mut(0).unwrap();
    for (address, formula) in [
        ("A1", "=SEQUENCE(5000)"),
        ("A200", "=2"),
        ("B1", "=SUM(A1:A5000)"),
        ("C1", "=A200"),
        ("D1", "=SUM(A1:C5000)"),
        ("E1", "=B1+C1+D1"),
        ("F1", "=INDIRECT(\"A3\")"),
    ] {
        sh.cells.set(CellRef::parse(address).unwrap(), joysheet_model::Cell::formula(joysheet_model::Formula::new(formula)));
    }
    let mut calc = Calc::new();
    calc.rebuild(&wb);
    for range in ["A1:A3000", "A1:C5000", "A1:ALL5", "Z4000:Z5000"] {
        let r = RangeRef::parse(range).unwrap();
        let coordinates: Vec<_> = r.iter().map(|c| (0, c)).collect();
        let expected: super::HashSet<_> = calc.dirty_closure(&wb, &coordinates).into_iter().collect();
        let mut seeds = Vec::new();
        calc.seed_range(&wb, 0, r, &mut seeds);
        let actual: super::HashSet<_> = calc.dirty_closure(&wb, &seeds).into_iter().collect();
        assert_eq!(actual, expected, "{range}");
        assert!(seeds.len() < 40);
    }
}

#[test]
#[ignore = "timing-sensitive; run with --ignored in release"]
#[allow(clippy::disallowed_methods)]
fn recalc_with_range_formulas_scales_linearly() {
    // Each row's SUM reads a range in the same columns. Recalculating on open used to walk
    // every row per range read (the used range), and finding a cell's dependents scanned
    // every range in its column: 4x the rows took 16x the time. Linear is 4x; allow 8x.
    let best = |rows| (0..3).map(|_| open_and_bulk_edit(rows)).min().unwrap_or_default();
    let small = best(5_000);
    let large = best(20_000);
    assert!(large < small * 8 + Duration::from_millis(100), "5k rows {small:?}, 20k rows {large:?}");
}

#[test]
fn range_dependents_after_edits() {
    // Formulas that read ranges, wide ranges and whole columns, found again after edits.
    let mut wb = Workbook::new();
    let mut calc = Calc::new();
    let set = |wb: &mut Workbook, calc: &mut Calc, at: &str, input: &str| {
        let c = CellRef::parse(at).unwrap();
        let cell = if input.starts_with('=') { Cell::formula(Formula::new(input)) } else { Cell::value(Value::Number(input.parse().unwrap())) };
        wb.sheet_mut(0).unwrap().cells.set(c, cell);
        calc.cells_changed(wb, &[(0, c)]);
    };
    let num = |wb: &Workbook, at: &str| {
        let v = wb.sheet(0).unwrap().value(CellRef::parse(at).unwrap());
        v.as_f64().unwrap_or_else(|| panic!("{at} = {v:?}"))
    };
    set(&mut wb, &mut calc, "A1", "1");
    set(&mut wb, &mut calc, "B2", "2");
    set(&mut wb, &mut calc, "CZ3", "3");
    set(&mut wb, &mut calc, "DA5", "=SUM(A1:B2)");
    set(&mut wb, &mut calc, "DA6", "=SUM(A1:CZ3)");
    set(&mut wb, &mut calc, "DA7", "=SUM(B:B)");
    set(&mut wb, &mut calc, "DA8", "=SUM(2:2)");
    assert_eq!((num(&wb, "DA5"), num(&wb, "DA6"), num(&wb, "DA7"), num(&wb, "DA8")), (3.0, 6.0, 2.0, 2.0));
    set(&mut wb, &mut calc, "B2", "20");
    assert_eq!((num(&wb, "DA5"), num(&wb, "DA6"), num(&wb, "DA7"), num(&wb, "DA8")), (21.0, 24.0, 20.0, 20.0));
    // Re-entering a formula with another range drops the old one.
    set(&mut wb, &mut calc, "DA5", "=SUM(A1:A2)");
    set(&mut wb, &mut calc, "B2", "5");
    assert_eq!(num(&wb, "DA5"), 1.0);
    let mut deps = Vec::new();
    calc.graph.dependents(0, CellRef::parse("B2").unwrap(), &mut deps);
    deps.sort_unstable();
    let at = |s: &str| (0, CellRef::parse(s).unwrap());
    assert_eq!(deps, vec![at("DA6"), at("DA7"), at("DA8")]);
    deps.clear();
    calc.graph.dependents_of_range(0, RangeRef::new(CellRef::parse("A1").unwrap(), CellRef::parse("A9").unwrap()), &mut deps);
    deps.sort_unstable();
    deps.dedup();
    assert_eq!(deps, vec![at("DA5"), at("DA6"), at("DA8")]);
    set(&mut wb, &mut calc, "CZ3", "30");
    assert_eq!(num(&wb, "DA6"), 36.0);
    // Ranges on another sheet.
    wb.sheets.push(std::sync::Arc::new(joysheet_model::Sheet::new("Data")));
    set(&mut wb, &mut calc, "DA9", "=SUM(Data!A1:A3)+SUM(Data!A:CZ)");
    assert_eq!(num(&wb, "DA9"), 0.0);
    wb.sheet_mut(1).unwrap().cells.set(CellRef::parse("A2").unwrap(), Cell::value(Value::Number(4.0)));
    calc.cells_changed(&mut wb, &[(1, CellRef::parse("A2").unwrap())]);
    assert_eq!(num(&wb, "DA9"), 8.0);
    deps.clear();
    calc.graph.dependents(1, CellRef::parse("A2").unwrap(), &mut deps);
    assert_eq!(deps, vec![at("DA9"), at("DA9")]);
}

/// A formula at the nesting limit reading thousands of formulas not calculated yet suspends the
/// evaluations in progress once, not once per formula read.
#[test]
fn wide_read_at_the_nesting_limit_suspends_once() {
    const WIDTH: u32 = 3000;
    let mut wb = Workbook::new();
    let mut exprs = HashMap::default();
    let mut put = |wb: &mut Workbook, at: String, text: String| {
        let at = CellRef::parse(&at).unwrap();
        wb.sheet_mut(0).unwrap().cells.set(at, Cell::formula(Formula::new(&text)));
        exprs.insert((0, at), std::sync::Arc::new(joysheet_formula::parse(&text[1..]).unwrap()));
    };
    // A1 reads A2, ..., A64 (the 64th formula deep) reads a wide range of formulas.
    for r in 1..64 {
        put(&mut wb, format!("A{r}"), format!("=A{}+1", r + 1));
    }
    put(&mut wb, "A64".to_string(), format!("=SUM(B1:B{WIDTH})"));
    for r in 1..=WIDTH {
        put(&mut wb, format!("B{r}"), format!("={r}*2"));
    }
    let mut rng = 1u64;
    let mut host = PassHost::new(&wb, &exprs, &mut rng, 0.0);
    host.calculate((0, CellRef::parse("A1").unwrap()));
    let total = (WIDTH * (WIDTH + 1)) as f64;
    assert_eq!(host.results.get(&(0, CellRef::parse("A1").unwrap())), Some(&Value::Number(total + 63.0)));
    assert!(host.cycles.is_empty());
    assert!(host.suspensions <= 3, "{} suspensions", host.suspensions);
}

/// For tests of `PassHost::visit_range`, which can't be reached from outside this module: the
/// values it hands over for `range` while the formulas in `pending` (cell, formula) have not been
/// calculated yet, or `None` when it declines the block.
pub(crate) fn probe_visit_range(wb: &Workbook, pending: &[(&str, &str)], range: RangeRef) -> Option<Vec<Value>> {
    let mut exprs = HashMap::default();
    for (at, formula) in pending {
        let expr = joysheet_formula::parse(formula.strip_prefix('=').unwrap_or(formula)).ok()?;
        exprs.insert((0, CellRef::parse(at)?), std::sync::Arc::new(expr));
    }

    // A cell in `pending` is calculated the first time something reads it, which is the state
    // of a real pass partway through; `visit_range` has to cope with the formulas it reads
    // being calculated under it.
    let mut rng = 1u64;
    let mut host = PassHost::new(wb, &exprs, &mut rng, 0.0);

    let mut seen = Vec::new();
    host.visit_range(0, range, &mut |v| {
        seen.push(v.clone());
        true
    })
    .then_some(seen)
}
