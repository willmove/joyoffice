use super::*;

#[test]
fn merge_across_keeps_the_first_value_of_each_row() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[1, 2], [3, 4]]})).unwrap();
    s.execute("cell.set", json!({"cell": "D1", "input": "=SUM(A1:B2)"})).unwrap();
    s.execute("home.mergeAcross", json!({"range": "A1:B2"})).unwrap();
    let v = |s: &mut crate::Session, a: &str| s.execute("cell.get", json!({"cell": a})).unwrap()["value"].clone();
    assert_eq!(v(&mut s, "B1"), Json::Null);
    assert_eq!(v(&mut s, "B2"), Json::Null);
    assert_eq!(v(&mut s, "D1"), json!(4.0));
}

#[test]
fn the_default_symbol_reproduces_the_accounting_format() {
    assert_eq!(accounting_format("$"), ACCOUNTING);
}

#[test]
fn decimal_steps() {
    let v = joysheet_core::Value::Number(1.5);
    assert_eq!(adjust_decimals("General", &v, 1), "0.00");
    assert_eq!(adjust_decimals("General", &v, -1), "0");
    assert_eq!(adjust_decimals("0", &v, 1), "0.0");
    assert_eq!(adjust_decimals("0.00", &v, -1), "0.0");
    assert_eq!(adjust_decimals("0.0", &v, -1), "0");
    assert_eq!(adjust_decimals("#,##0.00;[Red]-#,##0.00", &v, 1), "#,##0.000;[Red]-#,##0.000");
    assert_eq!(adjust_decimals("0%", &v, 1), "0.0%");
}

#[test]
fn format_cells_takes_the_documented_keys() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("cell.set", json!({"cell": "A1", "input": "5"})).unwrap();
    s.execute("home.formatCells", json!({"range": "A1", "style": {"numFmt": "0.0", "borders": {"diagUp": {"style": "Thin"}}}})).unwrap();
    let st = &s.execute("cell.get", json!({"cell": "A1"})).unwrap()["style"];
    assert_eq!(st["num_fmt"], "0.0");
    assert_eq!(st["borders"]["diag_up"]["style"], "Thin");
    // The style's own snake_case keys keep working.
    s.execute("home.formatCells", json!({"range": "A1", "style": {"num_fmt": "0.00"}})).unwrap();
    assert_eq!(s.execute("cell.get", json!({"cell": "A1"})).unwrap()["style"]["num_fmt"], "0.00");
}

#[test]
fn width_units() {
    assert_eq!(chars_to_points(8.43), 64.0);
    assert!((points_to_chars(64.0) - 8.43).abs() < 0.01);
}
