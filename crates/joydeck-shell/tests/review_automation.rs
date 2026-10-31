extern crate joydeck_shell as joydeck;
use joydeck::{
    automation::{Dispatcher, Request, VERSION},
    edit::{Command, Session},
    review::{self, Action},
};
use serde_json::json;
fn add(s: &mut Session) {
    let page = s.deck.slides[0].id.clone();
    s.execute(
        s.id,
        Command::Review(Action::Add {
            id: "thread-1".into(),
            slide: page,
            object: None,
            author: "测试作者".into(),
            text: "请核查图表".into(),
        }),
    )
    .unwrap();
}
#[test]
fn review_transaction_undo_anchor_cleanup_and_reopen() {
    let mut s = Session::new();
    add(&mut s);
    let baseline = s.deck.clone();
    s.execute(
        s.id,
        Command::Review(Action::Reply {
            id: "thread-1".into(),
            author: "另一作者".into(),
            text: "已核查".into(),
        }),
    )
    .unwrap();
    s.undo();
    assert_eq!(s.deck, baseline);
    s.redo();
    assert_eq!(s.deck.review.threads[0].messages.len(), 2);
    let before = s.deck.clone();
    assert!(
        s.execute(
            s.id,
            Command::Review(Action::Reply {
                id: "thread-1".into(),
                author: "".into(),
                text: "x".into()
            })
        )
        .is_err()
    );
    assert_eq!(s.deck, before);
    let file = std::env::temp_dir().join(format!("joydeck-review-{}.joydeck", s.id));
    joydeck::edit::save_new(&file, &s.deck).unwrap();
    let d = joydeck::edit::open_editable(&file).unwrap();
    assert_eq!(d.review, s.deck.review);
    std::fs::remove_file(file).unwrap();
}
#[test]
fn reading_order_is_independent_and_delete_undo_restores_access() {
    let mut s = Session::new();
    s.execute(s.id, Command::AddObject(0, joydeck::edit::object("rect")))
        .unwrap();
    let id = s.deck.slides[0].objects[0].id.clone();
    s.execute(
        s.id,
        Command::Review(Action::Accessibility {
            object: id.clone(),
            alt: "销售图".into(),
            decorative: false,
        }),
    )
    .unwrap();
    s.execute(
        s.id,
        Command::Review(Action::Order {
            slide: s.deck.slides[0].id.clone(),
            objects: vec![id],
        }),
    )
    .unwrap();
    let before = s.deck.clone();
    s.execute(s.id, Command::DeleteObject(0, 0)).unwrap();
    assert!(s.deck.review.accessibility.is_empty());
    s.undo();
    assert_eq!(s.deck, before);
    let findings = review::check(&s.deck);
    assert_eq!(findings[0].code, "missing-title");
}
fn req(d: &Dispatcher, id: &str, method: &str, params: serde_json::Value) -> Request {
    Request {
        protocol_version: VERSION.into(),
        request_id: id.into(),
        method: method.into(),
        target: Some(d.session.id),
        params,
    }
}
#[test]
fn dispatcher_idempotence_conflict_target_and_dirty_open() {
    let mut d = Dispatcher::new(vec![], vec![]).unwrap();
    let r = req(&d, "add", "command.execute", json!({"command":"page.add"}));
    let response = d.dispatch(r.clone());
    assert!(response.get("result").is_some());
    assert_eq!(d.dispatch(r), response);
    assert_eq!(d.session.deck.slides.len(), 2);
    let conflict = req(&d, "add", "document.inspect", json!({}));
    assert_eq!(d.dispatch(conflict)["error"]["code"], "id-conflict");
    let r = req(&d, "new", "document.new", json!({}));
    assert!(d.dispatch(r).get("error").is_some());
    let mut r = req(&d, "wrong", "document.inspect", json!({}));
    r.target = Some(0);
    assert!(d.dispatch(r).get("error").is_some());
}
#[test]
fn canonical_roots_reject_symlink_escape_and_existing_write() {
    let base = std::env::temp_dir().join(format!("joydeck-roots-{}", Session::new().id));
    let root = base.join("root");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(base.join("outside"), b"x").unwrap();
    std::fs::write(root.join("existing"), b"x").unwrap();
    let d = Dispatcher::new(vec![root.clone()], vec![root.clone()]).unwrap();
    assert!(d.path(root.join("new").to_str().unwrap(), true).is_ok());
    assert!(
        d.path(root.join("existing").to_str().unwrap(), true)
            .is_err()
    );
    assert!(
        d.path(base.join("outside").to_str().unwrap(), false)
            .is_err()
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&base, root.join("escape")).unwrap();
        assert!(
            d.path(root.join("escape/outside").to_str().unwrap(), false)
                .is_err()
        );
        assert!(
            d.path(root.join("escape/new").to_str().unwrap(), true)
                .is_err()
        );
    }
    std::fs::remove_dir_all(base).unwrap();
}
