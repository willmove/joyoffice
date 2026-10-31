use super::*;

#[test]
fn manage_rules_delete_then_move() {
    let mut s = Session::new();
    s.new_workbook();
    for v in ["1", "2"] {
        let rule = json!({"type": "cellIs", "operator": "greaterThan", "value": v, "preset": "lightRedFill"});
        s.execute("home.conditionalFormat", json!({"range": "A1:A3", "rule": rule})).unwrap();
    }
    // Deleting first left two rules' worth of indices for the move: it panicked.
    s.execute("home.manageRules", json!({"delete": 0, "moveUp": 1})).unwrap();
    s.execute("home.manageRules", json!({"delete": 0, "moveDown": 0})).unwrap();
    let rules = s.execute("home.manageRules", json!({})).unwrap();
    assert_eq!(rules["rules"].as_array().map(Vec::len), Some(0));
}

#[test]
fn table_without_header_keeps_outside_references() {
    // #1030: the data moved down for the new header row, but formulas outside kept the old addresses.
    let mut s = Session::new();
    s.new_workbook();
    s.execute("home.insertSheet", json!({"name": "Two"})).unwrap();
    s.execute("sheet.activate", json!({"sheet": 0})).unwrap();
    s.execute("cell.set", json!({"cell": "A1", "input": "10"})).unwrap();
    s.execute("cell.set", json!({"cell": "A2", "input": "20"})).unwrap();
    s.execute("cell.set", json!({"cell": "B1", "input": "=SUM(A1:A2)"})).unwrap();
    s.execute("cell.set", json!({"cell": "C1", "input": "=A2"})).unwrap();
    s.execute("cell.set", json!({"sheet": "Two", "cell": "A1", "input": "=Sheet1!A2"})).unwrap();
    s.execute("insert.table", json!({"range": "A1:A2", "header": false})).unwrap();
    let get = |s: &mut Session, a: &str| s.execute("cell.get", json!({"cell": a})).unwrap();
    assert_eq!(get(&mut s, "A1")["value"], "Column1");
    assert_eq!(get(&mut s, "A2")["value"], json!(10.0));
    assert_eq!(get(&mut s, "B1")["formula"], "=SUM(A2:A3)");
    assert_eq!(get(&mut s, "B1")["value"], json!(30.0));
    assert_eq!(get(&mut s, "C1")["formula"], "=A3");
    assert_eq!(get(&mut s, "C1")["value"], json!(20.0));
    let other = s.execute("cell.get", json!({"sheet": "Two", "cell": "A1"})).unwrap();
    assert_eq!(other["formula"], "=Sheet1!A3");
    assert_eq!(other["value"], json!(20.0));
}

#[test]
fn table_names_refuse_defined_names_and_r1c1() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1:A2", "values": [["h"], ["1"]]})).unwrap();
    s.execute("formulas.defineName", json!({"name": "Rate", "refersTo": "=0.2"})).unwrap();
    s.execute("insert.table", json!({"range": "A1:A2"})).unwrap();
    let e = s.execute("table.rename", json!({"table": "Table1", "name": "Rate"})).unwrap_err();
    assert!(e.to_string().contains("already exists"), "{e}");
    let e = s.execute("table.rename", json!({"table": "Table1", "name": "rate"})).unwrap_err();
    assert!(e.to_string().contains("already exists"), "{e}");
    for n in ["R2C3", "c", "r", "RC", "C5"] {
        let e = s.execute("table.rename", json!({"table": "Table1", "name": n})).unwrap_err();
        assert!(e.to_string().contains("not valid"), "{n}: {e}");
        let e = s.execute("insert.table", json!({"range": "C1:C2", "name": n})).unwrap_err();
        assert!(e.to_string().contains("not valid"), "{n}: {e}");
    }
    s.execute("table.rename", json!({"table": "Table1", "name": "Table1"})).unwrap();
    s.execute("table.rename", json!({"table": "Table1", "name": "Rates"})).unwrap();
}
