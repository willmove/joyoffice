//! End-to-end engine acceptance shared by xtask and tests.
use crate::EngineClient;
use serde_json::{Value, json};
use std::path::Path;
pub fn run(output: &Path) -> Result<Value, String> {
    std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let path = output.join("joysheet-smoke.xlsx");
    let e = EngineClient::start()?;
    let run = |c: &str, p: Value| e.execute(c, p)?.result;
    run("file.new", json!({}))?;
    run("range.setValues", json!({"range":"A1","values":[["名称","数量","单价","金额"],["苹果",3,5,"=B2*C2"],["梨",4,6,"=B3*C3"]]}))?;
    if run("cell.get", json!({"cell":"D2"}))?["value"] != json!(15.0) {
        return Err("initial formula failed".into());
    }
    run("cell.set", json!({"cell":"B2","input":"10"}))?;
    if run("cell.get", json!({"cell":"D2"}))?["value"] != json!(50.0) {
        return Err("recalculation failed".into());
    }
    run("edit.undo", json!({}))?;
    if run("cell.get", json!({"cell":"D2"}))?["value"] != json!(15.0) {
        return Err("undo failed".into());
    }
    run("edit.redo", json!({}))?;
    run("home.bold", json!({"range":"A1:D1"}))?;
    run("file.save", json!({"path":path}))?;
    run("file.close", json!({"force":true,"document":true}))?;
    run("file.open", json!({"path":path}))?;
    let after = run("cell.get", json!({"cell":"D2"}))?;
    if after["value"] != json!(50.0) {
        return Err(format!("round trip failed: {after}"));
    }
    if run("cell.get", json!({"cell":"A2"}))?["value"] != json!("苹果") {
        return Err("Unicode round trip failed".into());
    }
    Ok(json!({"passed":true,"file":path,"formula":after,"checks":["unicode","formula","recalc","undo","redo","format","xlsx-save-close-reopen"]}))
}
#[cfg(test)]
mod tests {
    #[test]
    fn edit_save_reopen() {
        let dir = std::env::temp_dir().join(format!("joysheet-smoke-{}", std::process::id()));
        assert_eq!(super::run(&dir).unwrap()["passed"], true);
        let _ = std::fs::remove_file(dir.join("joysheet-smoke.xlsx"));
        let _ = std::fs::remove_dir(dir);
    }
}
