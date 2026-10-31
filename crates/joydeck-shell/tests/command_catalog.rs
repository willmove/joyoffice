use joydeck_engine::command_request::CommandRequest;
use joydeck_shell::{
    automation::{Dispatcher, Request, VERSION},
    edit::{Command, Session},
    workspace::SharedSession,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn dispatch(d: &mut Dispatcher, id: &str, method: &str, params: Value) -> Value {
    d.dispatch(Request {
        protocol_version: VERSION.into(),
        request_id: id.into(),
        method: method.into(),
        target: Some(d.session.id),
        params,
    })
}
fn seed() -> Session {
    let mut s = Session::new();
    s.execute(s.id, Command::AddPage).unwrap();
    for o in [
        joydeck_shell::edit::object("rect"),
        joydeck_shell::edit::object("ellipse"),
        joydeck_shell::table::object(2, 2).unwrap(),
    ] {
        s.execute(s.id, Command::AddObject(0, o)).unwrap();
    }
    s
}
#[test]
fn every_registered_command_validates_and_matches_direct_kernel_with_single_history() {
    let base = seed().deck;
    let mut changed = base.slides[0].objects[0].clone();
    changed.x = 200.;
    changed.fill = "#aabbcc".into();
    let asset = joydeck_shell::playback::Asset {
        extension: "wav".into(),
        bytes: vec![1, 2, 3],
        info: joydeck_shell::playback::Info {
            profiles: vec![],
            fps: 0.,
            duration: 3.,
            codecs: vec!["lpcm".into()],
            width: 0,
            height: 0,
            adapter: "test-model".into(),
        },
    };
    let asset_id = asset.id();
    let cases = vec![
        json!({"command":"page.add"}),
        json!({"command":"page.copy","pages":[0]}),
        json!({"command":"page.delete","pages":[1]}),
        json!({"command":"page.reorder","pages":[0],"target":2}),
        json!({"command":"page.visibility","pages":[0]}),
        json!({"command":"page.size","width":800.,"height":600.}),
        json!({"command":"object.add","page":0,"object":joydeck_shell::edit::object("text")}),
        json!({"command":"object.update","page":0,"objects":[changed]}),
        json!({"command":"object.delete","page":0,"objects":[0,1]}),
        json!({"command":"object.reorder","page":0,"objects":[0],"target":3}),
        json!({"command":"review.edit","action":{"type":"add","id":"thread-one","slide":base.slides[0].id,"object":null,"author":"作者","text":"批注"}}),
        json!({"command":"table.edit","page":0,"object":base.slides[0].objects[2].id,"action":{"type":"setCell","value":[0,0,"表格命令",[]]}}),
        json!({"command":"document.design","action":{"type":"notes","value":[0,"新备注"]}}),
        json!({"command":"presentation.motion","presentation":{"loop_show":true}}),
        json!({"command":"media.edit","playback":{"assets":{asset_id.clone():asset},"clips":[{"id":"clip-one","slide":base.slides[0].id,"asset":asset_id,"start":0.,"end":3.,"volume":0.5,"looping":false,"autoplay":false,"cross_slide":false}]}}),
        json!({"command":"transaction.batch","commands":[{"command":"page.add"},{"command":"document.design","action":{"type":"notes","value":[0,"batch备注"]}}]}),
    ];
    let advertised = joydeck_shell::automation::describe();
    let registered = advertised["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        registered,
        cases
            .iter()
            .map(|v| v["command"].as_str().unwrap().to_owned())
            .collect()
    );
    assert_eq!(
        registered,
        advertised["commandParameters"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    );
    for params in cases {
        let mut d = Dispatcher::new(vec![], vec![]).unwrap();
        d.session = SharedSession::from_deck((*base).clone(), true);
        let mut direct = Session::from_deck((*base).clone(), true);
        let before = d.session.deck.clone();
        let checked = dispatch(&mut d, "validate", "command.validate", params.clone());
        assert_eq!(checked["result"]["valid"], true, "{params}: {checked}");
        assert_eq!(d.session.deck, before);
        assert_eq!(d.session.revision, 0);
        assert!(!d.session.dirty());
        assert!(!d.session.can_undo());
        let decoded: CommandRequest = serde_json::from_value(params.clone()).unwrap();
        let canonical = serde_json::to_value(&decoded).unwrap();
        assert_eq!(
            serde_json::to_value(
                serde_json::from_value::<CommandRequest>(canonical.clone()).unwrap()
            )
            .unwrap(),
            canonical
        );
        direct
            .execute(direct.id, decoded.into_command().unwrap())
            .unwrap();
        let response = dispatch(&mut d, "execute", "command.execute", params.clone());
        assert!(response.get("result").is_some(), "{params}: {response}");
        d.session.deck.validate().unwrap();
        // Independent sessions allocate different new IDs; all persisted values
        // and references to the shared baseline must agree.
        let mut actual = (*d.session.deck).clone();
        let mut expected = (*direct.deck).clone();
        for deck in [&mut actual, &mut expected] {
            for slide in &mut deck.slides {
                for o in &mut slide.objects {
                    o.id.clear();
                }
            }
        }
        assert_eq!(actual, expected, "{params}");
        assert_eq!(d.session.revision, 1, "{params}");
        let after = d.session.deck.clone();
        assert_eq!(
            dispatch(&mut d, "execute", "command.execute", params),
            response
        );
        d.session.undo();
        assert_eq!(d.session.deck, before);
        assert!(!d.session.can_undo());
        d.session.redo();
        assert_eq!(d.session.deck, after);
    }
}

#[test]
fn revision_guard_strict_parameters_and_batch_failure_preserve_state() {
    let mut d = Dispatcher::new(vec![], vec![]).unwrap();
    let before = d.session.deck.clone();
    let cases = vec![
        json!({"command":"page.add","typo":true}),
        json!({"command":"page.add","expectedRevision":1}),
        json!({"command":"page.add","expectedRevision":"0"}),
        json!({"command":"page.delete","pages":[]}),
        json!({"command":"object.delete","page":0,"objects":[0,0]}),
        json!({"command":"object.update","page":0,"objects":[]}),
        json!({"command":"transaction.batch","commands":[]}),
        json!({"command":"transaction.batch","commands":[{"command":"page.add"},{"command":"page.delete","pages":[99]}]}),
    ];
    for (i, params) in cases.into_iter().enumerate() {
        for method in ["command.validate", "command.execute"] {
            let result = dispatch(&mut d, &format!("bad-{i}-{method}"), method, params.clone());
            assert!(result.get("error").is_some(), "{result}");
            assert_eq!(d.session.deck, before);
            assert_eq!(d.session.revision, 0);
            assert!(!d.session.can_undo());
        }
    }
    let checked = dispatch(
        &mut d,
        "reusable-validate",
        "command.validate",
        json!({"command":"page.add","expectedRevision":0}),
    );
    assert_eq!(checked["result"]["valid"], true);
    assert!(
        dispatch(
            &mut d,
            "edit",
            "command.execute",
            json!({"command":"page.add","expectedRevision":0})
        )
        .get("result")
        .is_some()
    );
    assert!(
        dispatch(
            &mut d,
            "reusable-validate",
            "command.validate",
            json!({"command":"page.add","expectedRevision":0})
        )
        .get("error")
        .is_some()
    );
    let mut nested = json!({"command":"page.add"});
    for _ in 0..17 {
        nested = json!({"command":"transaction.batch","commands":[nested]});
    }
    assert!(
        dispatch(&mut d, "deep", "command.execute", nested)
            .get("error")
            .is_some()
    );
    assert_eq!(d.session.deck.slides.len(), 2);
}
