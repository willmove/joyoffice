use super::*;

fn protected_book() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("review.allowEditRanges", json!({"action": "add", "title": "Open", "range": "A1:A3"})).unwrap();
    s.execute("review.allowEditRanges", json!({"action": "add", "title": "Budget", "range": "B1:B3,D1", "password": "pa55"})).unwrap();
    s.execute("review.protectSheet", json!({})).unwrap();
    s
}

#[test]
fn edit_ranges_open_locked_cells_and_passwords_really_verify() {
    let mut s = protected_book();
    s.execute("cell.set", json!({"cell": "A2", "input": "1"})).unwrap();
    assert!(s.execute("cell.set", json!({"cell": "C1", "input": "1"})).is_err());
    let err = s.execute("cell.set", json!({"cell": "D1", "input": "1"})).unwrap_err().to_string();
    assert!(err.contains("Budget"), "{err}");
    // A paste or fill is checked the same way.
    assert!(s.execute("range.setValues", json!({"range": "B2", "values": [[5]]})).is_err());
    assert!(s.execute("review.allowEditRanges", json!({"action": "unlock", "title": "Budget", "password": "pass"})).is_err());
    assert!(s.execute("review.allowEditRanges", json!({"action": "unlock", "title": "Budget"})).is_err());
    let list = s.execute("review.allowEditRanges", json!({"action": "unlock", "title": "budget", "password": "pa55"})).unwrap();
    assert_eq!(list[1]["unlocked"], true);
    s.execute("range.setValues", json!({"range": "B2", "values": [[5]]})).unwrap();
    s.execute("cell.set", json!({"cell": "D1", "input": "2"})).unwrap();
    // Ranges can't change while the sheet is protected.
    assert!(s.execute("review.allowEditRanges", json!({"action": "delete", "title": "Open"})).is_err());
}

#[test]
fn edit_ranges_add_modify_delete_list_and_undo() {
    let mut s = Session::new();
    s.new_workbook();
    // Empty params list; nothing to list yet.
    assert_eq!(s.execute("review.allowEditRanges", json!({})).unwrap(), json!([]));
    s.execute("selection.set", json!({"range": "C3:D4"})).unwrap();
    let list = s.execute("review.allowEditRanges", json!({"action": "add"})).unwrap();
    assert_eq!(list, json!([{"title": "Range1", "range": "C3:D4", "password": false, "unlocked": true}]));
    assert!(s.execute("review.allowEditRanges", json!({"action": "add", "title": "range1", "range": "A1"})).is_err());
    let list = s
        .execute("review.allowEditRanges", json!({"action": "modify", "title": "Range1", "newTitle": "Mine", "range": "E5", "password": "x"}))
        .unwrap();
    assert_eq!((list[0]["title"].as_str(), list[0]["range"].as_str(), list[0]["password"].as_bool()), (Some("Mine"), Some("E5"), Some(true)));
    // Inserting a row above moves the range with its cells.
    s.execute("home.insertRows", json!({"rows": "1:1"})).unwrap();
    assert_eq!(s.execute("review.allowEditRanges", json!({})).unwrap()[0]["range"], "E6");
    s.execute("review.allowEditRanges", json!({"action": "delete", "title": "Mine"})).unwrap();
    assert_eq!(s.execute("review.allowEditRanges", json!({})).unwrap(), json!([]));
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.execute("review.allowEditRanges", json!({})).unwrap()[0]["title"], "Mine");
    assert!(s.execute("review.allowEditRanges", json!({"action": "nope"})).is_err());
    assert!(s.execute("review.allowEditRanges", json!({"action": "delete"})).is_err());
}
