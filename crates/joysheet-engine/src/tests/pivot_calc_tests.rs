use super::*;

fn fields(n: &str) -> Option<String> {
    ["Sales", "Units", "Unit Price", "Q1"].iter().find(|f| f.eq_ignore_ascii_case(n)).map(|f| f.to_string())
}

#[test]
fn compiles_field_names() {
    let c = compile("= Sales*0.1 + 'Unit Price'/units - Sales", &fields).unwrap();
    assert_eq!(c.refs, vec!["Sales", "Unit Price", "Units"]);
    // A field named like a cell reference is still a field; functions stay functions.
    let c = compile("=ROUND(Q1*1.5E2,0)", &fields).unwrap();
    assert_eq!(c.refs, vec!["Q1"]);
    let wb = Workbook::new();
    assert_eq!(eval(&wb, &c, &[Value::Number(2.0)]), Value::Number(300.0));
    assert!(compile("=Sales*Bogus", &fields).unwrap_err().contains("Bogus"));
    assert!(compile("=A1*2", &fields).is_err());
    assert!(compile("='Nope'*2", &fields).is_err());
    assert!(compile("=Sales*(", &fields).is_err());
    assert!(compile("=", &fields).is_err());
    assert!(compile(&format!("={}", "1+".repeat(MAX_FORMULA_LEN)), &fields).is_err());
}

#[test]
fn deep_and_cyclic_chains_are_bounded() {
    // 300 calculated fields, each using the previous one, and a two-field cycle.
    let mut pt = PivotTable::default();
    pt.calculated_fields.push(joysheet_model::PivotCalcField { name: "C0".into(), formula: "=Sales+1".into() });
    for i in 1..300 {
        pt.calculated_fields.push(joysheet_model::PivotCalcField { name: format!("C{i}"), formula: format!("=C{}+1", i - 1) });
    }
    pt.calculated_fields.push(joysheet_model::PivotCalcField { name: "P".into(), formula: "=Q".into() });
    pt.calculated_fields.push(joysheet_model::PivotCalcField { name: "Q".into(), formula: "=P*2".into() });
    let set = CalcSet::new(&pt, &["Sales".to_string()]);
    let wb = Workbook::new();
    let sum = |_: &str| Value::Number(10.0);
    assert_eq!(set.value(&wb, 0, &sum), Value::Number(11.0));
    assert_eq!(set.value(&wb, 10, &sum), Value::Number(21.0));
    // Past the depth bound (the list itself is capped at MAX_CALC_FIELDS).
    assert_eq!(set.value(&wb, 200, &sum), Value::Error(CellError::Ref));
    assert_eq!(set.index("P"), None, "P is past the cap of {MAX_CALC_FIELDS} fields");
    // A small cycle within the cap.
    let mut small = PivotTable::default();
    small.calculated_fields.push(joysheet_model::PivotCalcField { name: "P".into(), formula: "=Q".into() });
    small.calculated_fields.push(joysheet_model::PivotCalcField { name: "Q".into(), formula: "=P*2".into() });
    let set = CalcSet::new(&small, &["Sales".to_string()]);
    assert!(set.is_cyclic(0));
    assert_eq!(set.value(&wb, 0, &sum), Value::Error(CellError::Ref));
    assert_eq!(set.source_refs(0), Vec::<String>::new());
}
