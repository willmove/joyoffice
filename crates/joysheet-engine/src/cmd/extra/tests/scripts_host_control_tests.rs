use super::*;
#[test]
fn cancellation_before_first_step_preserves_workbook() {
    let mut s = Session::new();
    s.execute("file.new", json!({})).unwrap();
    s.execute("automate.newScript", json!({"name":"cancel","commands":[{"command":"cell.set","params":{"cell":"A1","input":"changed"}}]})).unwrap();
    let control = crate::ScriptControl::default();
    control.cancelled.store(true, std::sync::atomic::Ordering::Release);
    s.script_control = Some(control.clone());
    assert!(s.execute("automate.runScript", json!({"name":"cancel"})).unwrap_err().to_string().contains("cancelled"));
    assert_eq!(control.completed.load(std::sync::atomic::Ordering::Acquire), 0);
    assert!(s.doc().unwrap().wb.active().unwrap().value(joysheet_core::CellRef::default()).is_empty());
    s.script_control = None;
    s.execute("automate.runScript", json!({"name":"cancel"})).unwrap();
    assert_eq!(s.doc().unwrap().wb.active().unwrap().value(joysheet_core::CellRef::default()).display(), "changed");
}
