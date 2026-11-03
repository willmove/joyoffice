use super::automation::*;
use super::helpers::Scratch;
use serde_json::{Value, json};
fn request(
    id: &str,
    method: &str,
    target: Option<u64>,
    revision: Option<u64>,
    params: Value,
) -> Request {
    Request {
        protocol_version: VERSION.into(),
        request_id: id.into(),
        method: method.into(),
        target,
        expected_revision: revision,
        params,
    }
}
#[test]
fn revision_includes_selection_and_replays_do_not_execute_twice() {
    let root = Scratch::new().unwrap();
    let mut d = Dispatcher::new(&root.0).unwrap();
    let new = d.dispatch(request("new", "document.new", None, None, json!({})));
    let target = new["result"]["documentId"].as_u64().unwrap();
    assert!(target < (1 << 53));
    let select = request(
        "select",
        "command.execute",
        Some(target),
        Some(0),
        json!({"command":{"command":"select","page":1,"shapes":[]}}),
    );
    assert_eq!(d.dispatch(select.clone())["result"]["revision"], 1);
    assert_eq!(d.dispatch(select)["result"]["revision"], 1);
    let stale = d.dispatch(request(
        "stale",
        "command.execute",
        Some(target),
        Some(0),
        json!({"command":{"command":"insert-text","x":10.,"y":10.,"text":"must not be inserted"}}),
    ));
    assert!(stale.get("error").is_some());
    let insert = request(
        "insert",
        "command.execute",
        Some(target),
        Some(1),
        json!({"command":{"command":"insert-text","x":10.,"y":10.,"text":"中文😀"}}),
    );
    assert_eq!(d.dispatch(insert.clone())["result"]["revision"], 2);
    assert_eq!(d.dispatch(insert)["result"]["revision"], 2);
    let conflict = d.dispatch(request(
        "insert",
        "command.execute",
        Some(target),
        Some(2),
        json!({"command":{"command":"delete"}}),
    ));
    assert_eq!(conflict["error"]["code"], "id-conflict");
    let inspected = d.dispatch(request(
        "inspect",
        "document.inspect",
        Some(target),
        None,
        json!({}),
    ));
    let objects = inspected["result"]["slides"][0]["objects"]
        .as_array()
        .unwrap();
    assert_eq!(objects.iter().filter(|o| o["text"] == "中文😀").count(), 1);
}
#[test]
fn paths_and_dirty_replacement_are_guarded() {
    let root = Scratch::new().unwrap();
    let mut d = Dispatcher::new(&root.0).unwrap();
    assert!(d.path("relative.pptx", false).is_err());
    assert!(
        d.path(&root.0.join("../escape.pptx").to_string_lossy(), true)
            .is_err()
    );
    let elsewhere = Scratch::new().unwrap();
    assert!(
        d.path(&elsewhere.0.join("new.pdf").to_string_lossy(), true)
            .is_err()
    );
    assert!(
        d.path(&root.0.join("new.pdf:hidden").to_string_lossy(), true)
            .is_err()
    );
    d.dispatch(request("new", "document.new", None, None, json!({})));
    let status = d.dispatch(request("status", "document.status", None, None, json!({})));
    let target = status["result"]["documentId"].as_u64().unwrap();
    d.dispatch(request(
        "edit",
        "command.execute",
        Some(target),
        Some(0),
        json!({"command":{"command":"insert-rect","x":10.,"y":10.,"width":100.,"height":50.}}),
    ));
    assert!(
        d.dispatch(request("replace", "document.new", None, None, json!({})))
            .get("error")
            .is_some()
    );
    assert!(
        d.dispatch(request(
            "close",
            "document.close",
            Some(target),
            Some(1),
            json!({})
        ))
        .get("error")
        .is_some()
    );
    assert!(
        d.dispatch(request(
            "discard",
            "document.close",
            Some(target),
            Some(1),
            json!({"dirtyPolicy":"discard"})
        ))
        .get("result")
        .is_some()
    );
}

#[test]
fn r9_legacy_command_schema_fails_without_mutating_the_document() -> Result<(), String> {
    let root = Scratch::new()?;
    let mut d = Dispatcher::new(&root.0)?;
    let commands = d.dispatch(request("schema", "engine.commands", None, None, json!({})));
    assert_eq!(commands["result"]["commandSchemaVersion"], 2);
    assert_eq!(commands["result"]["legacyCommandSchemaSupported"], false);
    let new = d.dispatch(request("new", "document.new", None, None, json!({})));
    let target = new["result"]["documentId"]
        .as_u64()
        .ok_or("missing document ID")?;
    let before = d.dispatch(request(
        "before",
        "document.status",
        Some(target),
        None,
        json!({}),
    ));
    let legacy = d.dispatch(request(
        "legacy",
        "command.execute",
        Some(target),
        Some(0),
        json!({"command":{"schemaVersion":1,"commandId":"page.add","arguments":{}}}),
    ));
    assert_eq!(legacy["error"]["code"], "operation-failed");
    let after = d.dispatch(request(
        "after",
        "document.status",
        Some(target),
        None,
        json!({}),
    ));
    assert_eq!(before["result"], after["result"]);
    Ok(())
}

#[test]
fn validation_and_restored_history_methods_preserve_live_state() -> Result<(), String> {
    fn status(d: &mut Dispatcher, id: &str) -> Value {
        d.dispatch(request(id, "document.status", None, None, json!({})))["result"].clone()
    }
    let root = Scratch::new()?;
    let mut d = Dispatcher::new(&root.0)?;
    let new = d.dispatch(request("new", "document.new", None, None, json!({})));
    let target = new["result"]["documentId"].as_u64().ok_or("document ID")?;
    let before = status(&mut d, "before");
    let valid = d.dispatch(request(
        "valid",
        "command.validate",
        Some(target),
        Some(0),
        json!({"command":{"command":"insert-text","x":10.,"y":10.,"text":"预检😀"}}),
    ));
    assert_eq!(valid["result"]["valid"], true);
    assert_eq!(valid["result"]["wouldChange"], true);
    assert_eq!(valid["result"]["sideEffects"], false);
    assert_eq!(before, status(&mut d, "after-valid"));
    let invalid = d.dispatch(request(
        "invalid",
        "command.validate",
        Some(target),
        Some(0),
        json!({"command":{"command":"select","page":99,"shapes":[]}}),
    ));
    assert!(invalid.get("error").is_some());
    let stale = d.dispatch(request(
        "stale",
        "command.validate",
        Some(target),
        Some(99),
        json!({"command":{"command":"new-slide"}}),
    ));
    assert!(stale.get("error").is_some());
    let legacy = d.dispatch(request(
        "legacy",
        "command.validate",
        Some(target),
        Some(0),
        json!({"schemaVersion":1,"commandId":"page.add","arguments":{}}),
    ));
    assert!(legacy.get("error").is_some());
    assert_eq!(before, status(&mut d, "after-invalid"));
    assert_eq!(before["canUndo"], false);
    let edit = d.dispatch(request(
        "edit",
        "command.execute",
        Some(target),
        Some(0),
        json!({"command":{"command":"new-slide"}}),
    ));
    assert_eq!(edit["result"]["pages"], 2);
    let preview_undo = d.dispatch(request(
        "preview-undo",
        "command.validate",
        Some(target),
        Some(1),
        json!({"command":{"command":"undo"}}),
    ));
    assert_eq!(preview_undo["result"]["wouldChange"], true);
    assert_eq!(status(&mut d, "after-preview")["pages"], 2);
    let undo = request("undo", "document.undo", Some(target), Some(1), json!({}));
    let undone = d.dispatch(undo.clone());
    assert_eq!(undone["result"]["pages"], 1);
    assert_eq!(undone["result"]["dirty"], false);
    assert_eq!(d.dispatch(undo), undone);
    let stale_redo = d.dispatch(request(
        "stale-redo",
        "document.redo",
        Some(target),
        Some(1),
        json!({}),
    ));
    assert!(stale_redo.get("error").is_some());
    let redone = d.dispatch(request(
        "redo",
        "document.redo",
        Some(target),
        Some(2),
        json!({}),
    ));
    assert_eq!(redone["result"]["pages"], 2);
    assert_eq!(redone["result"]["revision"], 3);
    assert_eq!(redone["result"]["canUndo"], true);
    Ok(())
}
