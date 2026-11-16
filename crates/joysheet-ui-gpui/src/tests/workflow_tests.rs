use super::*;

#[test]
fn format_dialog_changes_only_edited_properties_and_keeps_mixed_styles() -> Result<(), String> {
    let mut initial = defaults("home.formatCells").ok_or("defaults")?;
    let mut edited = initial.clone();
    edited["bold"] = json!(true);
    edited["_original"] = initial.clone();
    let params = parameters("home.formatCells", edited)?;
    assert_eq!(params["style"], json!({"font":{"bold":true}}));
    initial["_original"] = initial.clone();
    assert_eq!(parameters("home.formatCells", initial)?["style"], json!({}));
    let mut s = joysheet_engine::Session::new();
    s.execute("file.new", json!({})).map_err(|e| e.to_string())?;
    s.execute("home.fontColor", json!({"range":"B1","color":{"theme":4,"tint":0.25}})).map_err(|e| e.to_string())?;
    s.execute("home.fontSize", json!({"range":"B1","size":24})).map_err(|e| e.to_string())?;
    let before = s.active().ok_or("document")?.wb.clone();
    let mut params = params;
    params["range"] = json!("A1:B1");
    s.execute("home.formatCells", params).map_err(|e| e.to_string())?;
    let after = &s.active().ok_or("document")?.wb;
    let b = joysheet_engine::core::CellRef::new(0, 1);
    let style = after.styles.get(after.active().ok_or("sheet")?.style_id(b));
    assert!(style.font.bold);
    assert_eq!(style.font.size, 24.);
    assert_eq!(style.font.color, before.styles.get(before.active().ok_or("sheet")?.style_id(b)).font.color);
    Ok(())
}

#[test]
fn page_dialog_keeps_header_text_separate_from_header_margin_and_scale_mode() -> Result<(), String> {
    let p = parameters(
        "pageLayout.pageSetup",
        json!({"header":"&CReport","footer":"&P","headerMargin":0.4,"footerMargin":0.3,"left":0.7,"fitMode":false,"scale":85,"fitWidth":1,"fitHeight":1}),
    )?;
    assert_eq!(p["header"], "&CReport");
    assert_eq!(p["margins"]["header"], 0.4);
    assert_eq!(p["scale"], 85);
    assert!(p.get("fitWidth").is_none());
    let p = parameters("pageLayout.pageSetup", json!({"fitMode":true,"scale":85,"fitWidth":2,"fitHeight":0}))?;
    assert!(p.get("scale").is_none());
    assert_eq!(p["fitWidth"], 2);
    Ok(())
}

#[test]
fn page_dialog_converts_centimeters_and_only_sends_changed_settings() -> Result<(), String> {
    let mut p = crate::dialogs::page_setup_values(&joysheet_engine::model::PrintSettings::default(), true);
    let original = p.clone();
    p["left"] = json!(2.54);
    p["_original"] = original.clone();
    let result = parameters("pageLayout.pageSetup", p)?;
    assert_eq!(result, json!({"margins":{"left":1.0}}));
    let mut p = original.clone();
    p["_original"] = original;
    assert_eq!(parameters("pageLayout.pageSetup", p)?, json!({}));
    Ok(())
}
#[test]
fn sort_retains_region_and_rejects_bad_order() {
    let p = parameters("data.sort", json!({"column":"C","order":"desc","range":"A1:C8"})).unwrap();
    assert_eq!(p["keys"][0]["column"], "C");
    assert_eq!(p["range"], "A1:C8");
    assert!(parameters("data.sort", json!({"order":"bad"})).is_err());
}
#[test]
fn multiple_sort_keys_keep_priority_and_reject_invalid_directions() {
    let p = parameters("data.sort", json!({"column":"A","order":"asc","keysText":"B:desc, C:asc","range":"A1:C8","header":true})).unwrap();
    assert_eq!(p["keys"], json!([{"column":"A","order":"asc"},{"column":"B","order":"desc"},{"column":"C","order":"asc"}]));
    assert_eq!(p["range"], "A1:C8");
    assert!(parameters("data.sort", json!({"keysText":"B:sideways"})).is_err());
    assert!(p.get("keysText").is_none());
}
#[test]
fn columns_trim_and_keep_engine_arrays() {
    assert_eq!(parameters("data.removeDuplicates", json!({"columns":" A, C "})).unwrap()["columns"], json!(["A", "C"]));
    assert_eq!(parameters("data.removeDuplicates", json!({"columns":["B"]})).unwrap()["columns"], json!(["B"]));
}
