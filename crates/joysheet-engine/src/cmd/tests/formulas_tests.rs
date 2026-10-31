use super::*;

#[test]
fn names_with_relative_references() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1", "values": [[1], [2], [3]]})).unwrap();
    // Defined from B2, `=Sheet1!A2` is "the cell to the left".
    s.execute("selection.set", json!({"range": "B2"})).unwrap();
    let r = s.execute("formulas.defineName", json!({"name": "LeftCell", "refersTo": "=Sheet1!A2"})).unwrap();
    assert_eq!(r["refersTo"], "=Sheet1!XFD1");
    s.execute("cell.set", json!({"cell": "B1", "input": "=LeftCell"})).unwrap();
    s.execute("cell.set", json!({"cell": "B3", "input": "=LeftCell*10"})).unwrap();
    let v = |s: &mut Session, a: &str| s.execute("cell.get", json!({"cell": a})).unwrap()["value"].clone();
    assert_eq!(v(&mut s, "B1"), json!(1.0));
    assert_eq!(v(&mut s, "B3"), json!(30.0));
    // Absolute references are kept as they are.
    let r = s.execute("formulas.defineName", json!({"name": "First", "refersTo": "=Sheet1!$A$1"})).unwrap();
    assert_eq!(r["refersTo"], "=Sheet1!$A$1");
}

#[test]
fn the_name_commands_leave_joysheet_data_alone() {
    // Defining, renaming or deleting a name of a stored item's shape would replace or remove
    // one of its chunks and leave the rest behind: refused unless `force` is true.
    let mut s = Session::new();
    s.new_workbook();
    let wb = std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().wb);
    let text = "m".repeat(9_000);
    stash::set(wb, "macro_1", &text).unwrap();
    wb.names.push(DefinedName { name: "_sc_watch".into(), scope: None, formula: "\"[]\"".into(), comment: String::new(), hidden: true });
    // A legacy item is read whether or not it is hidden, so a visible one is data too.
    wb.names.push(DefinedName { name: "_sc_old".into(), scope: None, formula: "\"[]\"".into(), comment: String::new(), hidden: false });
    let names = |s: &Session| s.doc().unwrap().wb.names.clone();
    let before = names(&s);
    assert!(before.len() >= 3, "the item takes two chunks");
    for params in [
        json!({"name": "_gc_macro_1_0", "refersTo": "=1"}),
        json!({"name": "_SC_Watch", "refersTo": "=1"}),
        json!({"name": "Mine", "refersTo": "=1", "oldName": "_gc_macro_1_1"}),
        json!({"name": "_gc_macro_1_0", "refersTo": "=1", "force": false}),
        json!({"name": "Mine2", "refersTo": "=1", "oldName": "_SC_old"}),
    ] {
        let err = s.execute("formulas.defineName", params.clone()).unwrap_err().to_string();
        assert!(err.contains("force"), "{params}: {err}");
    }
    for name in ["_gc_macro_1_1", "_sc_watch", "_sc_old"] {
        let err = s.execute("formulas.deleteName", json!({"name": name})).unwrap_err().to_string();
        assert!(err.contains("force"), "{name}: {err}");
    }
    s.execute("range.setValues", json!({"range": "A1:B2", "values": [["_gc_macro_1_0", "Rate"], [1, 2]]})).unwrap();
    let r = s.execute("formulas.createFromSelection", json!({"range": "A1:B2", "top": true, "replace": true})).unwrap();
    assert_eq!((r["created"].clone(), r["skipped"].clone()), (json!(1), json!(["_gc_macro_1_0"])));
    let after = names(&s);
    assert_eq!(after.iter().filter(|n| n.name != "Rate").cloned().collect::<Vec<_>>(), before);
    assert_eq!(stash::get(&s.doc().unwrap().wb, "macro_1").as_deref(), Some(text.as_str()));
    // A visible name of the shape is the person's own (the stash never reads it): the name
    // commands change and remove it without `force`, as Name Manager cannot pass one.
    let own = DefinedName { name: "_gc_rate_0".into(), scope: None, formula: "=1".into(), comment: String::new(), hidden: false };
    std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().wb).names.push(own.clone());
    s.execute("formulas.defineName", json!({"name": "Rate0", "refersTo": "=2", "oldName": "_gc_rate_0"})).unwrap();
    s.execute("formulas.deleteName", json!({"name": "Rate0"})).unwrap();
    std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().wb).names.push(own);
    s.execute("formulas.deleteName", json!({"name": "_gc_rate_0"})).unwrap();
    assert_eq!(stash::get(&s.doc().unwrap().wb, "macro_1").as_deref(), Some(text.as_str()), "the hidden item is untouched");
    // Ordinary names and forced calls go ahead.
    s.execute("formulas.deleteName", json!({"name": "Rate"})).unwrap();
    s.execute("formulas.defineName", json!({"name": "_gc_macro_1_1", "refersTo": "=1", "force": true})).unwrap();
    s.execute("formulas.deleteName", json!({"name": "_gc_macro_1_1", "force": true})).unwrap();
    assert!(!names(&s).iter().any(|n| n.name == "_gc_macro_1_1"));
}

#[test]
fn arabic_defined_names_count_characters_not_bytes() {
    let mut s = Session::new();
    s.new_workbook();
    let arabic = |n: usize| "س".repeat(n);
    for ok in [200, 255] {
        s.execute("formulas.defineName", json!({"name": arabic(ok), "refersTo": "=1"})).unwrap();
    }
    assert!(s.execute("formulas.defineName", json!({"name": arabic(256), "refersTo": "=1"})).is_err());
    s.execute("range.setValues", json!({"range": "A1", "values": [[format!("ع{}", arabic(199))], [1]]})).unwrap();
    let r = s.execute("formulas.createFromSelection", json!({"range": "A1:A2", "top": true})).unwrap();
    assert_eq!(r["created"], 1);
}

#[test]
fn names_referring_to_several_ranges() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1:C2", "values": [[1, 0, 5], [2, 0, 7]]})).unwrap();
    // Excel writes and accepts a union in a name without parentheses.
    let r = s.execute("formulas.defineName", json!({"name": "u", "refersTo": "=Sheet1!$A$1:$A$2,Sheet1!$C$1:$C$2"})).unwrap();
    assert_eq!(r["refersTo"], "=Sheet1!$A$1:$A$2,Sheet1!$C$1:$C$2");
    for (cell, input) in [("E1", "=SUM(u)"), ("E2", "=AREAS(u)"), ("E3", "=MIN(u)"), ("E4", "=INDEX(u,2,1,2)")] {
        s.execute("cell.set", json!({"cell": cell, "input": input})).unwrap();
    }
    let v = |s: &mut Session, a: &str| s.execute("cell.get", json!({"cell": a})).unwrap()["value"].clone();
    assert_eq!(v(&mut s, "E1"), json!(15.0));
    assert_eq!(v(&mut s, "E2"), json!(2.0));
    assert_eq!(v(&mut s, "E3"), json!(1.0));
    assert_eq!(v(&mut s, "E4"), json!(7.0));
    // Renaming the sheet follows every area, and the definition keeps Excel's form.
    s.execute("sheet.rename", json!({"name": "Data"})).unwrap();
    let name = s.doc().unwrap().wb.names.iter().find(|n| n.name == "u").map(|n| n.formula.clone());
    assert_eq!(name.as_deref(), Some("Data!$A$1:$A$2,Data!$C$1:$C$2"));
    assert_eq!(v(&mut s, "E1"), json!(15.0));
}

#[test]
fn name_manager_previews_a_name_over_several_ranges() {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("range.setValues", json!({"range": "A1:C2", "values": [[1, 0, 5], [2, 0, 7]]})).unwrap();
    s.execute("formulas.defineName", json!({"name": "u", "refersTo": "=Sheet1!$A$1:$A$2,Sheet1!$C$1:$C$2"})).unwrap();
    let list = s.execute("formulas.nameManager", json!({})).unwrap();
    let u = list.as_array().and_then(|l| l.iter().find(|n| n["name"] == "u")).cloned().unwrap();
    // The union parses as one, so the preview is what the union gives in a cell, not #NAME?.
    assert_eq!(u["value"], json!({"error": "#VALUE!"}));
}

/// A workbook with `Sales` and `rate`, a hidden name, a name of Sheet1 that hides a workbook
/// `Tax`, and a name of another sheet.
fn named_workbook() -> Session {
    let mut s = Session::new();
    s.new_workbook();
    s.execute("formulas.defineName", json!({"name": "Sales", "refersTo": "=Sheet1!$A$1:$A$3"})).unwrap();
    s.execute("formulas.defineName", json!({"name": "rate", "refersTo": "=0.07"})).unwrap();
    s.execute("formulas.defineName", json!({"name": "Tax", "refersTo": "=0.2"})).unwrap();
    let d = s.doc_mut().unwrap();
    let wb = std::sync::Arc::make_mut(&mut d.wb);
    let name =
        |name: &str, scope, formula: &str, hidden| DefinedName { name: name.into(), scope, formula: formula.into(), comment: String::new(), hidden };
    wb.names.push(name("_Filter", None, "Sheet1!$A$1:$B$9", true));
    wb.names.push(name("TAX", Some(0), "0.25", false));
    wb.names.push(name("Other", Some(1), "1", false));
    s
}

fn text_at(s: &Session, cell: &str) -> Value {
    s.doc().unwrap().wb.active().unwrap().value(CellRef::parse(cell).unwrap())
}

#[test]
fn paste_names_lists_the_names_the_active_sheet_can_use() {
    let mut s = named_workbook();
    let r = s.execute("formulas.pasteName", json!({})).unwrap();
    assert_eq!(
        r["names"],
        json!([
            {"name": "rate", "refersTo": "=0.07"},
            {"name": "Sales", "refersTo": "=Sheet1!$A$1:$A$3"},
            {"name": "TAX", "refersTo": "=0.25"},
        ])
    );
    assert_eq!(text_at(&s, "A1"), Value::Empty, "listing the names writes nothing");
}

#[test]
fn paste_names_and_use_in_formula_ask_the_ui_to_insert_the_name() {
    let mut s = named_workbook();
    s.take_ui_requests();
    s.execute("formulas.pasteName", json!({"name": "Sales"})).unwrap();
    s.execute("formulas.useInFormula", json!({"name": "rate"})).unwrap();
    assert_eq!(s.take_ui_requests(), vec![crate::UiRequest::InsertName("Sales".into()), crate::UiRequest::InsertName("rate".into())]);
    assert_eq!(text_at(&s, "A1"), Value::Empty, "the cell changes only when the edit is entered");
    // A name Paste Name doesn't list is refused; one typed in another case is pasted as listed.
    assert!(s.execute("formulas.pasteName", json!({"name": "NoSuchName"})).is_err());
    s.execute("formulas.pasteName", json!({"name": "SALES"})).unwrap();
    assert_eq!(s.take_ui_requests(), vec![crate::UiRequest::InsertName("Sales".into())]);
}

#[test]
fn paste_list_writes_each_name_and_what_it_refers_to() {
    let mut s = named_workbook();
    s.execute("selection.set", json!({"range": "D2"})).unwrap();
    let r = s.execute("formulas.pasteName", json!({"list": true})).unwrap();
    assert_eq!(r["range"], json!("D2:E4"));
    let rows = [("D2", "rate"), ("E2", "=0.07"), ("D3", "Sales"), ("E3", "=Sheet1!$A$1:$A$3"), ("D4", "TAX"), ("E4", "=0.25")];
    for (cell, text) in rows {
        assert_eq!(text_at(&s, cell), Value::text(text), "{cell}");
    }
    assert!(s.doc().unwrap().wb.active().unwrap().cell(CellRef::parse("E3").unwrap()).is_some_and(|c| c.formula.is_none()), "refers-to is text");
    s.execute("edit.undo", json!({})).unwrap();
    for (cell, _) in rows {
        assert_eq!(text_at(&s, cell), Value::Empty, "{cell} after Undo");
    }
    s.execute("formulas.pasteName", json!({"list": true, "at": "B7"})).unwrap();
    assert_eq!(text_at(&s, "B7"), Value::text("rate"));
}

#[test]
fn paste_list_needs_names_and_room() {
    let mut s = named_workbook();
    let last = format!("A{}", joysheet_core::MAX_ROWS - 1);
    assert!(s.execute("formulas.pasteName", json!({"list": true, "at": last})).is_err(), "three rows don't fit below the second-to-last row");
    let last_col = format!("{}1", joysheet_core::col_to_letters(joysheet_core::MAX_COLS - 1));
    assert!(s.execute("formulas.pasteName", json!({"list": true, "at": last_col})).is_err(), "two columns don't fit in the last column");
    let mut empty = Session::new();
    empty.new_workbook();
    assert!(empty.execute("formulas.pasteName", json!({"list": true})).is_err());
    assert_eq!(empty.execute("formulas.pasteName", json!({})).unwrap()["names"], json!([]));
}
