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

#[test]
fn remote_dispatcher_tracks_replaced_document_identity_and_replays() {
    if !cfg!(unix) {
        return;
    }
    let d = Dispatcher::new(vec![], vec![]).unwrap();
    let old_id = d.session.id;
    let descriptor = std::env::temp_dir().join(format!("joydeck-control-replace-{old_id}.json"));
    let service = Service::start(d, &descriptor, false).unwrap();
    let mut client = Dispatcher::new(vec![], vec![]).unwrap();
    client.connect(descriptor.clone()).unwrap();
    assert_eq!(client.session.id, old_id);
    let request = Request {
        protocol_version: VERSION.into(),
        request_id: "new-document".into(),
        method: "document.new".into(),
        target: None,
        params: json!({}),
    };
    let response = client.dispatch(request.clone());
    let new_id = response["result"]["documentId"].as_u64().unwrap();
    assert_ne!(new_id, old_id);
    assert_eq!(client.session.id, new_id);
    assert_eq!(client.dispatch(request), response);
    let add = Request {
        protocol_version: VERSION.into(),
        request_id: "add".into(),
        method: "command.execute".into(),
        target: Some(client.session.id),
        params: json!({"command":"page.add"}),
    };
    assert!(client.dispatch(add).get("result").is_some());
    let rejected = Request {
        protocol_version: VERSION.into(),
        request_id: "dirty-new".into(),
        method: "document.new".into(),
        target: None,
        params: json!({}),
    };
    assert!(client.dispatch(rejected).get("error").is_some());
    assert_eq!(client.session.id, new_id);
    let mut fresh = Dispatcher::new(vec![], vec![]).unwrap();
    fresh.connect(descriptor).unwrap();
    assert_eq!(fresh.session.id, new_id);
    let inspect = Request {
        protocol_version: VERSION.into(),
        request_id: "fresh-inspect".into(),
        method: "document.inspect".into(),
        target: Some(fresh.session.id),
        params: json!({}),
    };
    assert_eq!(
        fresh.dispatch(inspect)["result"]["deck"]["slides"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    drop(service);
}
