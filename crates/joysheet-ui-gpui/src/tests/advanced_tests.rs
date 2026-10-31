use super::{defaults, parameters};
use serde_json::json;
#[test]
fn pivot_and_solver_adapters_preserve_protocol() {
    let mut p = defaults("insert.pivotTable").unwrap();
    parameters("insert.pivotTable", &mut p).unwrap();
    assert_eq!(p["values"][0]["field"], "金额");
    assert!(p.get("source").is_none());
    let mut p = json!({"constraintLines":" B2 <= 10\n\nB2 int "});
    parameters("data.solver", &mut p).unwrap();
    assert_eq!(p["constraints"], json!(["B2 <= 10", "B2 int"]));
}
