#![allow(clippy::unwrap_used, reason = "Test assertions")]
use joydeck_engine::{Command, ReviewAction, Session, WorkspaceAction, original_bindings};
use joydeck_model::{Presentation, ShapeKind};
use serde_json::{Value, json};
fn legacy(session: &mut Session, value: Value) {
    let command = joydeck_shell::legacy_commands::parse(&value, session).unwrap();
    session.execute(command).unwrap();
}
#[test]
fn advertised_legacy_commands_edit_the_real_model_and_keep_review_references_valid() {
    let model = Presentation::default();
    let mut s = Session::new(model.clone(), original_bindings(&model), true);
    legacy(&mut s, json!({"command":"page.add"}));
    legacy(&mut s, json!({"command":"page.copy","pages":[1]}));
    let ids: Vec<_> = s.model().slides.iter().map(|s| s.id).collect();
    legacy(&mut s, json!({"command":"page.visibility","pages":[0,1]}));
    assert!(s.model().slides[0].hidden && s.model().slides[1].hidden);
    legacy(
        &mut s,
        json!({"command":"page.reorder","pages":[2],"target":0}),
    );
    assert_eq!(s.model().slides[0].id, ids[2]);
    legacy(
        &mut s,
        json!({"command":"page.size","width":800.,"height":450.}),
    );
    assert_eq!(s.model().slide_size.width, 800.);
    legacy(
        &mut s,
        json!({"command":"document.design","action":{"type":"notes","value":[0,"notes 😀"]}}),
    );
    assert_eq!(s.model().slides[0].notes.text(), "notes 😀");
    s.execute(Command::InsertTable {
        x: 20.,
        y: 20.,
        rows: 2,
        cols: 2,
        width: 300.,
        row_height: 40.,
    })
    .unwrap();
    let id = s.selection().shapes[0];
    for action in [
        json!({"type":"rowHeight","value":[0,60.]}),
        json!({"type":"columnWidth","value":[1,120.]}),
        json!({"type":"paste","value":[0,0,"first\tsecond"]}),
        json!({"type":"insertRow","value":2}),
        json!({"type":"deleteRow","value":2}),
        json!({"type":"insertColumn","value":2}),
        json!({"type":"deleteColumn","value":2}),
        json!({"type":"merge","value":{"top":0,"left":0,"bottom":0,"right":1}}),
        json!({"type":"split","value":[0,0]}),
    ] {
        legacy(
            &mut s,
            json!({"command":"table.edit","page":0,"object":id.0,"action":action}),
        );
    }
    let ShapeKind::Table(table) = &s.model().slides[0].shape(id).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(table.rows[0].height, 60.);
    assert_eq!(table.cols[1], 120.);
    assert_eq!((table.n_rows(), table.n_cols()), (2, 2));
    assert!(table.rows[0].cells[0].text.text().contains("second"));
    for action in [
        json!({"type":"add","id":"thread","slide":ids[2].0.to_string(),"object":id.0.to_string(),"author":"A","text":"comment"}),
        json!({"type":"reply","id":"thread","author":"B","text":"reply"}),
        json!({"type":"resolve","id":"thread","resolved":true}),
        json!({"type":"accessibility","object":id.0.to_string(),"alt":"table description","decorative":false}),
    ] {
        legacy(&mut s, json!({"command":"review.edit","action":action}));
    }
    let slide = &s.model().slides[0];
    assert!(slide.comments[0].resolved);
    assert_eq!(slide.comments[0].replies[0].text, "reply");
    assert_eq!(slide.comments[0].shape, Some(id));
    assert_eq!(slide.shape(id).unwrap().descr, "table description");
    s.execute(Command::Review {
        action: ReviewAction::ReadingOrder { shapes: vec![id] },
    })
    .unwrap();
    assert!(
        s.model().slides[0]
            .shape(id)
            .unwrap()
            .tags
            .contains(&("JoyOffice.ReadingOrder".into(), "0".into()))
    );
    let before = s.snapshot();
    legacy(
        &mut s,
        json!({"command":"object.delete","page":0,"objects":[0]}),
    );
    assert!(s.model().slides[0].comments.is_empty());
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    s.execute(Command::Workspace {
        action: WorkspaceAction::Section {
            name: "section".into(),
            slides: ids.clone(),
        },
    })
    .unwrap();
    legacy(&mut s, json!({"command":"page.delete","pages":[1]}));
    assert_eq!(s.model().slides.len(), 2);
    assert!(!s.model().sections[0].slides.contains(&ids[0]));
    legacy(
        &mut s,
        json!({"command":"transaction.batch","commands":[{"command":"page.add"},{"command":"page.size","width":900.,"height":500.}]}),
    );
    assert_eq!(s.model().slides.len(), 3);
    assert_eq!(s.model().slide_size.width, 900.);
    for name in [
        "object.add",
        "object.update",
        "object.reorder",
        "presentation.motion",
        "media.edit",
    ] {
        let before = s.snapshot();
        assert!(joydeck_shell::legacy_commands::parse(&json!({"command":name}), &s).is_err());
        assert_eq!(s.model(), before.model.as_ref());
    }
}

#[test]
fn legacy_batch_uses_updated_indices_and_undoes_as_one_edit() {
    let model = Presentation::default();
    let mut session = Session::new(model.clone(), original_bindings(&model), true);
    let first = model.slides[0].id;
    let invalid = json!({"command":"transaction.batch","commands":[
        {"command":"page.add"},
        {"command":"document.design","action":{"type":"notes","value":[2,"invalid"]}}
    ]});
    assert!(joydeck_shell::legacy_commands::parse(&invalid, &session).is_err());
    assert_eq!(session.revision(), 0);
    assert_eq!(session.model(), &model);
    let batch = json!({"command":"transaction.batch","commands":[
        {"command":"page.add"},
        {"command":"transaction.batch","commands":[
            {"command":"page.reorder","pages":[1],"target":0},
            {"command":"page.visibility","pages":[0]},
            {"command":"document.design","action":{"type":"notes","value":[0,"moved page"]}}
        ]}
    ]});
    let command = joydeck_shell::legacy_commands::parse(&batch, &session).unwrap();
    assert!(session.validate(command.clone()).unwrap());
    assert_eq!(session.model(), &model);
    assert!(session.execute(command).unwrap());
    assert_eq!(session.revision(), 1);
    assert_eq!(session.model().slides[1].id, first);
    assert!(session.model().slides[0].hidden);
    assert_eq!(session.model().slides[0].notes.text(), "moved page");
    assert!(session.model().slides[1].notes.text().is_empty());
    session.execute(Command::Undo).unwrap();
    assert_eq!(session.model(), &model);
    assert!(!session.can_undo());
}

#[test]
fn legacy_table_string_ids_and_out_of_range_pages_are_checked() {
    let model = Presentation::default();
    let mut session = Session::new(model.clone(), original_bindings(&model), true);
    session
        .execute(Command::InsertTable {
            x: 20.,
            y: 20.,
            rows: 2,
            cols: 2,
            width: 200.,
            row_height: 40.,
        })
        .unwrap();
    let id = session.selection().shapes[0];
    let mut saved_model = session.model().clone();
    std::sync::Arc::make_mut(&mut saved_model.slides[0])
        .shape_mut(id)
        .unwrap()
        .tags
        .push(("JoyOffice.LegacyId".into(), "table-old".into()));
    let mut session = Session::new(saved_model.clone(), original_bindings(&saved_model), true);
    for object in [json!("table-old"), json!(id.0.to_string()), json!(id.0)] {
        legacy(
            &mut session,
            json!({"command":"table.edit","page":0,"object":object,
            "action":{"type":"rowHeight","value":[0,70.]}}),
        );
    }
    let ShapeKind::Table(table) = &session.model().slides[0].shape(id).unwrap().kind else {
        unreachable!()
    };
    assert_eq!(table.rows[0].height, 70.);
    for input in [
        json!({"command":"table.edit","page":usize::MAX,"object":"table-old",
            "action":{"type":"rowHeight","value":[0,80.]}}),
        json!({"command":"document.design","action":{"type":"notes","value":[usize::MAX,"overflow"]}}),
    ] {
        assert!(joydeck_shell::legacy_commands::parse(&input, &session).is_err());
    }
}
