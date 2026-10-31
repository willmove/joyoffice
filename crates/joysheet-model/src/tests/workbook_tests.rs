use super::*;

#[test]
fn phonetics_are_additive_in_json() {
    let mut wb = Workbook::new();
    assert!(wb.phonetics.is_empty(), "new workbooks have no phonetic data");
    wb.phonetics.push(PhoneticString {
        text: "東".into(),
        runs: vec![PhoneticRun { start: 0, end: 1, text: "ヒガシ".into() }],
        props: Some(PhoneticProps { font_id: 3, kind: Some("Hiragana".into()), alignment: None }),
    });
    let json = serde_json::to_string(&wb).expect("serialize");
    assert_eq!(serde_json::from_str::<Workbook>(&json).expect("deserialize").phonetics, wb.phonetics);
    // JSON written before the field existed still loads.
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("value");
    value.as_object_mut().expect("an object").remove("phonetics");
    assert!(serde_json::from_value::<Workbook>(value).expect("older JSON").phonetics.is_empty());
}

#[test]
fn sheet_names() {
    let wb = Workbook::new();
    assert_eq!(wb.next_sheet_name(), "Sheet2");
    assert!(wb.check_sheet_name("sheet1", None).is_err());
    assert!(wb.check_sheet_name("sheet1", Some(0)).is_ok());
    assert!(wb.check_sheet_name("a/b", None).is_err());
    assert!(wb.check_sheet_name(&"x".repeat(32), None).is_err());
    assert!(wb.check_sheet_name("Budget 2026", None).is_ok());
}
