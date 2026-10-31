use super::*;
use serde_json::json;
#[test]
fn auth_and_remote_commands() {
    let engine = EngineClient::start().unwrap();
    engine.execute("file.new", json!({})).unwrap().result.unwrap();
    let server = ControlServer::start(0, engine).unwrap();
    let addr = format!("127.0.0.1:{}", server.port);
    let token = joysheet_protocol::client_token(&addr).unwrap();
    let mut remote = joysheet_mcp::Remote::connect_with_token(&addr, token).unwrap();
    use joysheet_mcp::Backend;
    remote.call("engine.execute", json!({"command":"cell.set","params":{"input":"=SUM(1,2,3)"}})).unwrap();
    assert_eq!(remote.call("cell.get", json!({})).unwrap()["value"], 6.0);
    assert!(joysheet_mcp::Remote::connect_with_token(&addr, "incorrect-token-123".into()).is_err());
    assert!(remote.call("ui.inspect", json!({"doc":999999})).is_err());
    assert!(remote.call("ui.inspect", json!({"doc":1,"window":1})).is_err());
    assert_eq!(remote.call("cell.get", json!({})).unwrap()["value"], 6.0);
}
#[test]
fn timed_out_pending_calls_are_not_run() {
    let state = Arc::new(AtomicU8::new(2));
    let (reply, _) = mpsc::channel();
    let call = UiCall { method: "ui.text".into(), params: json!({}), window: Some(1), reply, state };
    assert!(!call.claim());
}
