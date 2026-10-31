use joysheet_core::Value;
use serde_json::json;

use crate::Session;

#[test]
fn editing_a_header_renames_the_column() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["a", "b", "c"], [1, 2, 3]]})).unwrap();
    s.execute("insert.table", json!({"range": "A1:C2", "header": true})).unwrap();
    s.execute("cell.set", json!({"cell": "E1", "input": "=SUM(Table1[a])"})).unwrap();
    s.execute("cell.set", json!({"cell": "A1", "input": "zz"})).unwrap();
    s.execute("edit.clearContents", json!({"range": "B1"})).unwrap();
    s.execute("cell.set", json!({"cell": "C1", "input": "ZZ"})).unwrap();
    let get = |s: &mut Session, a: &str| s.execute("cell.get", json!({"cell": a})).unwrap();
    assert_eq!(get(&mut s, "E1")["formula"], "=SUM(Table1[zz])");
    assert_eq!(get(&mut s, "E1")["value"], json!(1.0));
    let d = s.doc().unwrap();
    let sh = d.wb.sheet(0).unwrap();
    let names: Vec<&str> = sh.tables[0].columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["zz", "Column2", "ZZ2"]);
    assert_eq!(sh.value(joysheet_core::CellRef::new(0, 1)), Value::from("Column2"));
    assert_eq!(sh.value(joysheet_core::CellRef::new(0, 2)), Value::from("ZZ2"));
    s.execute("range.setValues", json!({"range": "A1", "values": [["x", "x", 5]]})).unwrap();
    let sh = s.doc().unwrap().wb.sheet(0).unwrap();
    let names: Vec<&str> = sh.tables[0].columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["x", "x2", "5"]);
    assert_eq!(sh.value(joysheet_core::CellRef::new(0, 2)), Value::from("5"));
}

#[test]
fn go_to_a_table_selects_its_data() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [["a", "b"], [1, 2], [3, 4]]})).unwrap();
    s.execute("insert.table", json!({"range": "A1:B3", "header": true})).unwrap();
    assert_eq!(s.execute("edit.goTo", json!({"reference": "Table1"})).unwrap()["selection"], "A2:B3");
}
