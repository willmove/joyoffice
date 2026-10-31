use super::*;

#[test]
fn flash_fill_reads_shown_text() {
    let mut s = crate::Session::new();
    s.new_workbook();
    let values = serde_json::json!([["2024-01-15", "15/01/2024"], ["2023-12-03", null]]);
    s.execute("range.setValues", serde_json::json!({"range": "A1", "values": values})).unwrap();
    s.execute("selection.set", serde_json::json!({"cell": "B2"})).unwrap();
    s.execute("data.flashFill", serde_json::json!({})).unwrap();
    let sh = s.doc().unwrap().wb.active().unwrap();
    assert_eq!(sh.value(CellRef::new(1, 1)), Value::from("03/12/2023"));
}

#[test]
fn flash_programs() {
    let p = learn(&[("John Smith".into(), "Smith, John".into())]).unwrap();
    assert_eq!(p.apply("Ada Lovelace").as_deref(), Some("Lovelace, Ada"));
    let p = learn(&[("jane doe".into(), "JD".into())]).unwrap();
    assert_eq!(p.apply("alan turing").as_deref(), Some("AT"));
    let p = learn(&[("alice@example.com".into(), "alice".into())]).unwrap();
    assert_eq!(p.apply("bob@example.com").as_deref(), Some("bob"));
}

#[test]
fn split_numbers() {
    assert_eq!(split_num("Item 7"), Some(("Item ".into(), 7, "".into(), 1)));
    assert_eq!(split_num("Q003x"), Some(("Q".into(), 3, "x".into(), 3)));
    assert_eq!(split_num("abc"), None);
}

#[test]
fn times_quarters_ordinals_and_mixed_sources() {
    let mut s = crate::Session::new();
    s.new_workbook();
    let mut fill = |values: serde_json::Value, source: &str, target: &str| {
        s.execute("range.setValues", serde_json::json!({"range": source.get(..2).unwrap(), "values": values})).unwrap();
        s.execute("edit.autoFill", serde_json::json!({"source": source, "target": target})).unwrap();
        let sh = s.doc().unwrap().wb.active().unwrap();
        let r = RangeRef::parse(target).unwrap();
        r.iter().map(|c| crate::display::cell_text(&s.doc().unwrap().wb, sh, c)).collect::<Vec<_>>()
    };
    assert_eq!(fill(serde_json::json!([["9:00"]]), "A1", "A1:A3"), ["9:00", "10:00", "11:00"]);
    assert_eq!(fill(serde_json::json!([["Qtr3"]]), "B1", "B1:B3"), ["Qtr3", "Qtr4", "Qtr1"]);
    assert_eq!(fill(serde_json::json!([["Quarter 4"]]), "C1", "C1:C2"), ["Quarter 4", "Quarter 1"]);
    assert_eq!(fill(serde_json::json!([["1st Period"]]), "D1", "D1:D4"), ["1st Period", "2nd Period", "3rd Period", "4th Period"]);
    assert_eq!(
        fill(serde_json::json!([["Product 1"], ["On backorder"]]), "E1:E2", "E1:E6"),
        ["Product 1", "On backorder", "Product 2", "On backorder", "Product 3", "On backorder"]
    );
}

#[test]
fn falling_growth_series_stops_at_its_stop_value() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("cell.set", serde_json::json!({"cell": "A1", "input": "5"})).unwrap();
    s.execute("edit.fillSeries", serde_json::json!({"range": "A1:A6", "type": "growth", "step": 0.5, "stop": 1})).unwrap();
    let d = s.doc().unwrap();
    let sh = d.wb.active().unwrap();
    let growth: Vec<String> = (1..6).map(|r| crate::display::cell_text(&d.wb, sh, CellRef::new(r, 0))).collect();
    assert_eq!(growth, ["2.5", "1.25", "", "", ""]);
}

#[test]
fn text_series_near_i64_max_neither_wraps_nor_panics() {
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("cell.set", serde_json::json!({"cell": "A1", "input": "X 0"})).unwrap();
    s.execute("cell.set", serde_json::json!({"cell": "A2", "input": "X 9223372036854775807"})).unwrap();
    // The detected step is i64::MAX, so the next value overflows i64. The cell keeps its
    // source value instead of panicking in debug or wrapping in release.
    s.execute("edit.autoFill", serde_json::json!({"source": "A1:A2", "target": "A1:A3"})).unwrap();
    let d = s.doc().unwrap();
    let sh = d.wb.active().unwrap();
    assert_eq!(crate::display::cell_text(&d.wb, sh, CellRef::new(2, 0)), "X 0");
}

#[test]
fn autofill_refuses_targets_that_do_not_extend_the_source() {
    // Whole rows into whole columns once filled every source column down the whole sheet
    // (~17 billion writes) and aborted the process on a 24-51 GB allocation (fuzz seeds 1064,
    // 1114, 1775). Excel refuses every one of these shapes.
    let mut s = crate::Session::new();
    s.new_workbook();
    s.execute("range.setValues", serde_json::json!({"range": "A2", "values": [[1, "Item 1"], [2, "Item 2"]]})).unwrap();
    s.execute("range.setValues", serde_json::json!({"range": "H4", "values": [[10], [20]]})).unwrap();
    let before = s.doc().unwrap().wb.active().unwrap().cells.len();
    for (mode, source, target) in [
        ("series", "2:3", "A:A"),
        ("alternate", "2:3", "A:A"),
        ("series", "8:10", "C:E"),
        ("copy", "24:24", "B:D"),
        ("values", "H4:H9", "D:F"),
        ("series", "H4:H9", "H:H"),
        ("series", "A2:A3", "A1:A5"),
        ("series", "A2:A6", "A2:A4"),
        ("series", "B2:B3", "A2:A3"),
    ] {
        let r = s.execute("edit.autoFill", serde_json::json!({"mode": mode, "source": source, "target": target}));
        assert!(r.is_err(), "{source} -> {target} should be refused");
    }
    assert_eq!(s.doc().unwrap().wb.active().unwrap().cells.len(), before, "a refused fill writes nothing");
    // Targets holding the source in a corner still fill, whole rows and diagonal drags included.
    s.execute("edit.autoFill", serde_json::json!({"source": "2:3", "target": "2:6"})).unwrap();
    s.execute("edit.autoFill", serde_json::json!({"source": "A2:A3", "target": "A1:A3"})).unwrap();
    let sh = s.doc().unwrap().wb.active().unwrap();
    let text = |a: &str| crate::display::cell_text(&s.doc().unwrap().wb, sh, CellRef::parse(a).unwrap());
    assert_eq!([text("A1"), text("A6"), text("B6")], ["0", "5", "Item 5"]);
    assert!(holds_source_in_corner(RangeRef::parse("B5").unwrap(), RangeRef::parse("B3:B5").unwrap()));
    assert!(holds_source_in_corner(RangeRef::parse("A:A").unwrap(), RangeRef::parse("A:C").unwrap()));
    assert!(holds_source_in_corner(RangeRef::parse("B5:C9").unwrap(), RangeRef::parse("B5:D13").unwrap()));
    assert!(holds_source_in_corner(RangeRef::parse("B1:B2").unwrap(), RangeRef::parse("A1:B4").unwrap()));
    assert!(!holds_source_in_corner(RangeRef::parse("A:A").unwrap(), RangeRef::parse("1:1").unwrap()));
}
