use super::*;
#[test]
fn new_spill_declines_a_previously_cached_empty_area() {
    let mut wb = Workbook::new();
    let anchor = CellRef::new(0, 1);
    wb.sheet_mut(0).unwrap().cells.set(anchor, joysheet_model::Cell::formula(joysheet_model::Formula::new("=SEQUENCE(3)")));
    let mut exprs = HashMap::default();
    exprs.insert((0, anchor), std::sync::Arc::new(joysheet_formula::parse("SEQUENCE(3)").unwrap()));
    let mut rng = 1;
    let mut host = PassHost::new(&wb, &exprs, &mut rng, 0.);
    let expression = joysheet_formula::parse("SUM(B2:B3)").unwrap();
    assert_eq!(Evaluator::new(&mut host, 0, CellRef::new(0, 25)).value(&expression), Value::Number(0.));
    assert_eq!(host.sums.len(), 1);
    host.calculate((0, anchor));
    assert_eq!(Evaluator::new(&mut host, 0, CellRef::new(1, 25)).value(&expression), Value::Number(5.));
}
#[test]
fn pass_memo_is_bounded_and_never_caches_error_results() {
    let wb = Workbook::new();
    let exprs = HashMap::default();
    let mut rng = 1;
    let mut host = PassHost::new(&wb, &exprs, &mut rng, 0.);
    for row in 0..100 {
        let range = RangeRef::new(CellRef::new(row, 0), CellRef::new(row, 1));
        assert!(host.visit_range(0, range, &mut |_| true));
        host.remember_sum(0, range, Value::Number(row as f64));
    }
    assert_eq!(host.sums.len(), 64);
    host.sums.clear();
    host.remember_sum(0, RangeRef::new(CellRef::new(0, 0), CellRef::new(1, 0)), Value::Error(CellError::Div0));
    assert!(host.sums.is_empty());
}
