use crate::{ribbon_defaults, ribbon_menus, workflow};
use joysheet_engine::Session;
use serde_json::{Value, json};

fn run_choice(s: &mut Session, menu: &str, id: &str) -> Result<Value, String> {
    let t = ribbon_menus::tools(menu).into_iter().find(|t| t.id == id).ok_or("missing menu choice")?;
    s.execute(t.command, serde_json::from_str(t.params).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
#[test]
fn paste_values_and_formats_keep_their_distinct_semantics() -> Result<(), String> {
    let mut s = Session::new();
    s.execute("file.new", json!({})).map_err(|e| e.to_string())?;
    s.execute("cell.set", json!({"cell":"A1","input":"=2+3"})).map_err(|e| e.to_string())?;
    s.execute("home.bold", json!({"range":"A1"})).map_err(|e| e.to_string())?;
    s.execute("edit.copy", json!({"range":"A1"})).map_err(|e| e.to_string())?;
    s.execute("selection.set", json!({"range":"B1"})).map_err(|e| e.to_string())?;
    run_choice(&mut s, "paste", "paste-values")?;
    let d = s.active().ok_or("document")?;
    let sh = d.wb.active().ok_or("sheet")?;
    let b = joysheet_engine::core::CellRef::parse("B1").ok_or("cell")?;
    let c = sh.cell(b).ok_or("pasted cell")?;
    assert!(c.formula.is_none());
    assert_eq!(c.value, joysheet_engine::core::Value::Number(5.));
    assert!(!d.wb.styles.get(c.style).font.bold);
    run_choice(&mut s, "paste", "paste-formats")?;
    let d = s.active().ok_or("document")?;
    let sh = d.wb.active().ok_or("sheet")?;
    assert_eq!(sh.value(b), joysheet_engine::core::Value::Number(5.));
    assert!(d.wb.styles.get(sh.style_id(b)).font.bold);
    s.execute("edit.undo", json!({})).map_err(|e| e.to_string())?;
    let d = s.active().ok_or("document")?;
    assert!(!d.wb.styles.get(d.wb.active().ok_or("sheet")?.style_id(b)).font.bold);
    Ok(())
}
#[test]
fn ribbon_forms_drive_real_engine_operations() -> Result<(), String> {
    let mut s = Session::new();
    s.execute("file.new", json!({})).map_err(|e| e.to_string())?;
    let mut p = ribbon_defaults::defaults("insert.symbol").ok_or("symbol defaults")?;
    p["char"] = json!("Ω");
    s.execute("insert.symbol", p).map_err(|e| e.to_string())?;
    run_choice(&mut s, "page-orientation", "landscape")?;
    run_choice(&mut s, "page-size", "paper-a3")?;
    run_choice(&mut s, "margins", "margins-narrow")?;
    let d = s.active().ok_or("document")?;
    let sh = d.wb.active().ok_or("sheet")?;
    assert_eq!(sh.print.paper, "A3");
    assert_eq!(sh.print.orientation, joysheet_engine::model::Orientation::Landscape);
    assert_eq!(sh.value(joysheet_engine::core::CellRef::new(0, 0)), joysheet_engine::core::Value::Text("Ω".into()));
    let p = workflow::parameters("data.consolidate", json!({"sourcesText":"Sheet1!A1:B2; Sheet2!A1:B2"}))?;
    assert_eq!(p["sources"], json!(["Sheet1!A1:B2", "Sheet2!A1:B2"]));
    assert!(workflow::parameters("data.subtotal", json!({"columnsText":" , "})).is_err());
    Ok(())
}
#[test]
fn every_enabled_ribbon_leaf_has_a_registered_command_and_valid_parameters() -> Result<(), String> {
    let mut pending: Vec<_> =
        (1..crate::ribbon_catalog::TABS.len()).flat_map(crate::ribbon_catalog::groups).flat_map(|g| g.tools.into_iter().flatten()).collect();
    while let Some(t) = pending.pop() {
        let p: Value = serde_json::from_str(t.params).map_err(|e| format!("{}: {e}", t.id))?;
        assert!(p.is_object());
        if t.menu {
            let choices = ribbon_menus::tools(t.command);
            assert!(!choices.is_empty(), "{}", t.id);
            pending.extend(choices);
        } else if !t.command.starts_with('@') {
            assert!(joysheet_engine::find_command(t.command).is_some(), "{}: {}", t.id, t.command);
        }
    }
    Ok(())
}
