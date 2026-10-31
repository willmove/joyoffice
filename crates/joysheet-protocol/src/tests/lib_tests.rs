use super::*;
#[test]
fn validates_envelopes_and_preserves_id() {
    let r = ControlRequest::parse(r#"{"id":"a","method":"cell.get"}"#).unwrap();
    assert_eq!(r.params, json!({}));
    assert_eq!(response(r.id, Ok(json!(42)))["id"], "a");
    assert!(ControlRequest::parse(r#"{"method":"cell.get","params":[]}"#).is_err());
    assert!(ControlRequest::parse(&" ".repeat(MAX_LINE_BYTES + 1)).is_err());
    assert!(CommandTarget::from_params(&json!({"doc":"1"})).is_err());
    assert!(CommandTarget::from_params(&json!({"doc":1,"window":2})).is_err());
}
