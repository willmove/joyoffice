#[test]
fn calculation_phases_belong_to_the_current_command_only() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    run(&e, "cell.set", json!({"cell":"A1","input":"2"}));
    let c = run(&e, "cell.set", json!({"cell":"B1","input":"=SUM(A1:A5)"}));
    assert_eq!(c.timing.calculation["formulasEvaluated"], 1);
    assert!(c.timing.calculation["rangesRead"].as_u64().unwrap() >= 1);
    for key in ["recalcMs", "dependencyMs", "evaluationMs", "writebackMs", "rangeReadMs", "usedRangeMs", "rowFitMs"] {
        assert!(c.timing.calculation[key].as_f64().unwrap() >= 0.);
    }
    assert_eq!(run(&e, "cell.get", json!({"cell":"B1"})).timing.calculation["formulasEvaluated"], 0);
    assert_eq!(run(&e, "cell.get", json!({"cell":"B1"})).result.unwrap()["value"].as_f64(), Some(2.));
}
#[test]
fn completion_wakeup_observes_both_reply_and_snapshot() {
    let engine = super::EngineClient::start().unwrap();
    let updates = engine.subscribe();
    updates.try_recv().unwrap();
    let ticket = engine.submit("file.new", serde_json::json!({}), Default::default()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while updates.try_recv().is_err() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let c = ticket.poll().unwrap().unwrap();
    assert!(c.result.is_ok());
    assert_eq!(engine.latest().sequence, c.snapshot.sequence);
    assert!(engine.latest().active().is_some());
}
use super::*;
fn run(e: &EngineClient, c: &str, p: Value) -> Completion {
    let r = e.execute(c, p).unwrap();
    assert!(r.result.is_ok(), "{:?}", r.result);
    r
}
#[test]
fn queued_edits_snapshots_and_undo() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    let before = e.latest();
    let a = e.submit("cell.set", json!({"cell":"A1","input":"10"}), CommandTarget::default()).unwrap();
    let b = e.submit("cell.set", json!({"cell":"B1","input":"=A1*2"}), CommandTarget::default()).unwrap();
    a.wait(Duration::from_secs(5)).unwrap().result.unwrap();
    b.wait(Duration::from_secs(5)).unwrap().result.unwrap();
    assert!(before.active().unwrap().sheet().unwrap().cells.is_empty());
    assert_eq!(run(&e, "cell.get", json!({"cell":"B1"})).result.unwrap()["value"], 20.0);
    run(&e, "edit.undo", json!({}));
    assert!(run(&e, "cell.get", json!({"cell":"B1"})).result.unwrap()["value"].is_null());
}
#[test]
fn guards_and_targets() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    let revision = e.latest().active().unwrap().revision;
    let first = e.latest().active_window.unwrap();
    run(&e, "cell.set", json!({"input":"1"}));
    assert!(
        e.submit_guarded("cell.set", json!({"input":"2"}), CommandTarget::default(), Some(revision))
            .unwrap()
            .wait(Duration::from_secs(5))
            .unwrap()
            .result
            .is_err()
    );
    run(&e, "file.new", json!({}));
    let front = e.latest().active_window;
    e.dispatch("engine.execute", json!({"window":first,"command":"cell.set","params":{"cell":"A1","input":"中文"}})).unwrap();
    assert_eq!(e.latest().active_window, front);
    assert_eq!(e.dispatch("cell.get", json!({"window":first,"cell":"A1"})).unwrap()["value"], "中文");
    assert!(e.dispatch("cell.get", json!({"window":99999})).is_err());
}
#[test]
fn script_task_identity_follows_target_document() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    let (doc, first) = (e.latest().active().unwrap().doc, e.latest().active_window.unwrap());
    run(&e, "automate.newScript", json!({"name":"empty","commands":[]}));
    run(&e, "file.new", json!({}));
    let second = e.latest().active_window.unwrap();
    let ticket = e.submit("automate.runScript", json!({"name":"empty"}), CommandTarget { window: None, doc: Some(doc) }).unwrap();
    let id = ticket.id;
    ticket.wait(Duration::from_secs(5)).unwrap().result.unwrap();
    let tasks = e.dispatch("engine.tasks", json!({"doc":doc})).unwrap();
    assert_eq!(tasks[0]["id"], id);
    assert_eq!(tasks[0]["window"], first);
    assert_eq!(tasks[0]["state"], "completed");
    assert!(e.dispatch("engine.tasks", json!({"window":second})).unwrap().as_array().unwrap().is_empty());
    assert!(e.dispatch("engine.cancel", json!({"window":second,"id":id})).is_err());
    assert_eq!(e.dispatch("engine.cancel", json!({"window":first,"id":id})).unwrap()["cancelled"], false);
    assert_eq!(e.latest().active_window, Some(second));
}
#[test]
fn validated_edit_rejects_without_mutation_and_keeps_undo_semantics() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    run(&e, "data.validation", json!({"range":"A1","type":"whole","formula1":"0","formula2":"100"}));
    assert!(e.execute("__joysheet.validatedSet", json!({"cell":"A1","input":"999"})).unwrap().result.is_err());
    assert!(run(&e, "cell.get", json!({"cell":"A1"})).result.unwrap()["value"].is_null());
    run(&e, "__joysheet.validatedSet", json!({"cell":"A1","input":"50"}));
    run(&e, "edit.undo", json!({}));
    assert!(run(&e, "cell.get", json!({"cell":"A1"})).result.unwrap()["value"].is_null());
}
#[test]
fn warning_confirmation_targets_explicit_sheet_and_keeps_undo() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    run(&e, "data.validation", json!({"range":"A1","type":"whole","formula1":"0","formula2":"100","errorStyle":"warning"}));
    run(&e, "home.insertSheet", json!({"name":"Other"}));
    let revision = e.latest().active().unwrap().revision;
    assert!(e.execute("__joysheet.validatedSet", json!({"sheet":0,"cell":"A1","input":"999"})).unwrap().result.is_err());
    assert_eq!(revision, e.latest().active().unwrap().revision);
    assert!(e.latest().active().unwrap().workbook.sheets[0].cells.is_empty());
    run(&e, "__joysheet.validatedSet", json!({"sheet":0,"cell":"A1","input":"999","validationAccepted":true}));
    let view = e.latest();
    let wb = &view.active().unwrap().workbook;
    assert_eq!(wb.sheets[0].value(joysheet_engine::core::CellRef::new(0, 0)).display(), "999");
    assert!(wb.sheets[1].cells.is_empty());
    run(&e, "edit.undo", json!({}));
    assert!(e.latest().active().unwrap().workbook.sheets[0].cells.is_empty());
}
#[test]
fn object_batch_is_one_undo_step_and_rolls_back_partial_failure() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    let a = run(&e, "insert.shape", json!({"at":"A1"})).result.unwrap()["shape"].as_u64().unwrap();
    let b = run(&e, "insert.shape", json!({"at":"D1"})).result.unwrap()["shape"].as_u64().unwrap();
    let before = e.latest();
    let undo = before.active().unwrap().undo.len();
    let op = |id, dx| json!({"command":"object.move","params":{"kind":"shape","id":id,"dx":dx}});
    assert!(e.execute("__joysheet.objectBatch", json!({"sheet":0,"operations":[op(a,30),op(999999,50)]})).unwrap().result.is_err());
    assert_eq!(e.latest().active().unwrap().workbook.sheets[0].shapes[0].anchor.dx, 0.);
    assert_eq!(e.latest().active().unwrap().undo.len(), undo);
    run(&e, "__joysheet.objectBatch", json!({"sheet":0,"operations":[op(a,30),op(b,50)]}));
    assert_eq!(e.latest().active().unwrap().undo.len(), undo + 1);
    run(&e, "edit.undo", json!({}));
    assert!(e.latest().active().unwrap().workbook.sheets[0].shapes.iter().all(|s| s.anchor.dx == 0.));
    assert!(e.execute("__joysheet.objectBatch", json!({"sheet":1,"operations":[op(a,10)]})).unwrap().result.is_err());
}
#[test]
fn stale_native_activation_cannot_override_a_new_document() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    let first = e.latest().active_window.unwrap();
    run(&e, "file.new", json!({}));
    let second = e.latest().active_window.unwrap();
    assert_eq!(run(&e, "__joysheet.nativeActivate", json!({"window":first,"previousActive":first})).result.unwrap()["ignored"], true);
    assert_eq!(e.latest().active_window, Some(second));
    run(&e, "__joysheet.nativeActivate", json!({"window":first,"previousActive":second}));
    assert_eq!(e.latest().active_window, Some(first));
    run(&e, "cell.set", json!({"cell":"A1","input":"intended document"}));
    assert_eq!(e.latest().active().unwrap().workbook.sheets[0].value(joysheet_engine::core::CellRef::new(0, 0)).display(), "intended document");
    assert!(e.latest().window(second).unwrap().workbook.sheets[0].cells.is_empty());
}
#[test]
fn preview_import_does_not_mutate_the_live_document() {
    let e = EngineClient::start().unwrap();
    run(&e, "file.new", json!({}));
    let before = e.latest().active().unwrap().workbook.clone();
    let result = run(&e, "__joysheet.previewImport", json!({"text":"Name,Amount\nA,42","sheet":"Preview"})).result.unwrap();
    assert_eq!(result["previewOnly"], true);
    assert_eq!(result["rows"], 2);
    assert!(Arc::ptr_eq(&before, &e.latest().active().unwrap().workbook));
}
#[test]
fn failed_save_preserves_document_and_existing_files() {
    let dir = std::env::temp_dir().join(format!("joysheet-save-failure-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("book.xlsx");
    let temporary = dir.join("book.xlsx.sctmp");
    std::fs::write(&temporary, b"occupied").unwrap();
    let engine = EngineClient::start().unwrap();
    run(&engine, "file.new", json!({}));
    run(&engine, "cell.set", json!({"input":"中文未保存"}));
    assert!(engine.execute("file.save", json!({"path":path})).unwrap().result.is_err());
    assert_eq!(std::fs::read(&temporary).unwrap(), b"occupied");
    assert!(!path.exists());
    assert!(engine.latest().active().unwrap().dirty);
    assert_eq!(run(&engine, "cell.get", json!({})).result.unwrap()["value"], "中文未保存");
    std::fs::remove_file(temporary).unwrap();
    run(&engine, "file.save", json!({"path":path}));
    assert!(!engine.latest().active().unwrap().dirty);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
