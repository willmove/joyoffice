extern crate joydeck_shell as joydeck;
use joydeck::{
    automation::{Dispatcher, Request, VERSION},
    control::Service,
};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpStream,
};
#[test]
fn authenticated_service_shares_desktop_history_and_rejects_unauthenticated() {
    if !cfg!(unix) {
        return;
    }
    let d = Dispatcher::new(vec![], vec![]).unwrap();
    let mut view = d.session.clone();
    let descriptor = std::env::temp_dir().join(format!("joydeck-control-test-{}.json", view.id));
    let service = Service::start(d, &descriptor, true).unwrap();
    let mut stream = TcpStream::connect(service.address).unwrap();
    writeln!(stream, "{}", json!({"token":"wrong","request":{}})).unwrap();
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&line).unwrap()["error"]["code"],
        "unauthenticated"
    );
    let req = Request {
        protocol_version: VERSION.into(),
        request_id: "once".into(),
        method: "command.execute".into(),
        target: Some(view.id),
        params: json!({"command":"page.add"}),
    };
    let first = joydeck::control::remote(&descriptor, &req).unwrap();
    assert!(first.get("result").is_some(), "{first}");
    assert_eq!(joydeck::control::remote(&descriptor, &req).unwrap(), first);
    view.sync();
    assert_eq!(view.deck.slides.len(), 2);
    view.undo();
    assert_eq!(view.deck.slides.len(), 1);
    drop(service);
    assert!(!descriptor.exists());
}
