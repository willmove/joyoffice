#![allow(clippy::unwrap_used, clippy::expect_used, reason = "Test assertions")]
use joydeck_shell::{
    automation::{Dispatcher, Request, VERSION},
    editor::Editor,
    platform,
};
use serde_json::json;
#[test]
fn preserving_save_accepts_reassigned_resource_ids_but_rejects_media_edits() {
    let root = platform::private_directory("r9-media-save").unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/joydeck/r6-r7/delivery-media.pptx");
    let mut editor = Editor::open(&source, root.join("recovery")).unwrap();
    editor
        .session
        .execute(joydeck_engine::Command::InsertText {
            x: 20.,
            y: 20.,
            text: "new object shifts package-local IDs".into(),
        })
        .unwrap();
    editor
        .save_job(root.join("saved.pptx"))
        .unwrap()
        .run()
        .unwrap();
    let output = root.join("must-not-exist.pptx");
    let mut job = editor.save_job(output.clone()).unwrap();
    let model = std::sync::Arc::make_mut(&mut job.snapshot.model);
    let slide = std::sync::Arc::make_mut(&mut model.slides[0]);
    let media = slide
        .shapes
        .iter_mut()
        .find_map(|s| match &mut s.kind {
            joydeck_model::ShapeKind::Media(m) => Some(m),
            _ => None,
        })
        .unwrap();
    media.volume = 0.123;
    assert!(job.run().err().unwrap().contains("media parameters/bytes"));
    assert!(!output.exists());
}
fn request(
    id: &str,
    method: &str,
    target: Option<u64>,
    revision: Option<u64>,
    params: serde_json::Value,
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
fn attached_control_edits_the_live_session_and_rejects_stale_gui_versions() {
    let root = platform::private_directory("r9-attached").unwrap();
    let mut live = Some(Editor::new(root.join("recovery")).unwrap());
    let mut d = Dispatcher::new(&root).unwrap();
    let cap = d.dispatch_attached(
        &mut live,
        1,
        request("cap", "engine.commands", None, None, json!({})),
    );
    assert_eq!(cap["result"]["attachedDesktop"], true);
    let target = cap["result"]["documentId"].as_u64().unwrap();
    let insert = request(
        "insert",
        "command.execute",
        Some(target),
        Some(0),
        json!({"command":{"command":"insert-text","x":10.,"y":10.,"text":"attached"}}),
    );
    let result = d.dispatch_attached(&mut live, 1, insert.clone());
    assert!(result.get("error").is_none(), "{result}");
    assert!(
        live.as_ref().unwrap().session.model().slides[0]
            .shapes
            .iter()
            .any(|s| s.text.as_ref().is_some_and(|b| b.text() == "attached"))
    );
    assert_eq!(d.dispatch_attached(&mut live, 1, insert), result);
    live.as_mut()
        .unwrap()
        .session
        .execute(joydeck_engine::Command::Undo)
        .unwrap();
    let stale = d.dispatch_attached(
        &mut live,
        1,
        request("stale", "document.redo", Some(target), Some(1), json!({})),
    );
    assert!(stale.get("error").is_some());
    assert!(live.as_ref().unwrap().session.can_redo());
    let redo = d.dispatch_attached(
        &mut live,
        1,
        request("redo", "document.redo", Some(target), Some(2), json!({})),
    );
    assert!(redo.get("error").is_none(), "{redo}");
    let replaced = d.dispatch_attached(
        &mut live,
        2,
        request(
            "new-target",
            "document.redo",
            Some(target),
            Some(3),
            json!({}),
        ),
    );
    assert!(replaced.get("error").is_some());
}
#[test]
fn migrated_review_media_links_and_original_bytes_survive_native_reopen() {
    let bytes =
        include_bytes!("../../../tests/fixtures/joydeck/r9-ui2/legacy-review-media.joydeck");
    let root = platform::private_directory("r9-migration").unwrap();
    let doc = joydeck_shell::Document::from_bytes(
        "legacy.joydeck".into(),
        bytes.to_vec(),
        &Default::default(),
    )
    .unwrap();
    let mut editor = Editor::from_document(doc, root.join("recovery")).unwrap();
    let p = editor.session.model();
    assert_eq!(p.slides[0].comments[0].text, "Original comment");
    assert_eq!(p.slides[0].comments[0].replies[0].text, "Original reply");
    assert!(p.slides[0].comments[0].resolved);
    assert_eq!(p.slides[0].shapes[0].descr, "Original triangle description");
    assert!(
        matches!(p.slides[0].shapes[0].click.as_ref().unwrap().action, joydeck_model::text::Action::Slide { slide } if slide == p.slides[1].id)
    );
    assert!(p.show.loop_until_esc);
    assert_eq!(p.custom_shows[0].slides, [p.slides[1].id, p.slides[0].id]);
    let media = p.slides[0]
        .shapes
        .iter()
        .find_map(|s| {
            if let joydeck_model::ShapeKind::Media(m) = &s.kind {
                Some(m)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(media.volume, 0.5);
    assert!(media.loop_play);
    assert!(!media.autoplay);
    assert_eq!(
        p.media(media.media).unwrap().data.as_slice(),
        include_bytes!("../../../tests/fixtures/joydeck/media/original-tone.wav")
    );
    let expected = p.clone();
    let public = root.join("migrated.pptx");
    assert!(
        editor
            .save_job(public.clone())
            .unwrap()
            .run()
            .err()
            .unwrap()
            .contains(".joydeck")
    );
    assert!(!public.exists());
    let public = root.join("regenerated.pptx");
    joydeck_shell::exchange::export(&editor, &public, "regenerate", &Default::default()).unwrap();
    let read = joydeck_shell::Document::open(&public, &Default::default()).unwrap();
    assert_eq!(
        read.presentation.slides[0].comments[0].text,
        "Original comment"
    );
    assert_eq!(
        read.presentation.slides[0].comments[0].replies[0].text,
        "Original reply"
    );
    assert!(read.presentation.media.iter().any(|m| m.data.as_slice()
        == include_bytes!("../../../tests/fixtures/joydeck/media/original-tone.wav")));
    assert!(read.presentation.slides[0].comments[0].resolved);
    assert_eq!(
        read.presentation.slides[0].shapes[0].descr,
        "Original triangle description"
    );
    let output = root.join("migrated.joydeck");
    let saved = editor.save_job(output.clone()).unwrap().run().unwrap();
    editor.apply_saved(saved);
    let reopened = Editor::open(&output, root.join("reopened")).unwrap();
    assert_eq!(reopened.session.model(), &expected);
    let parts = joydeck_shell::native::parts(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(parts["migration/legacy-original.joydeck"], bytes);
}
#[test]
fn comparison_accepts_one_object_atomically_and_refuses_stale_results() {
    let root = platform::private_directory("r9-compare").unwrap();
    let mut editor = Editor::new(root.join("recovery")).unwrap();
    editor
        .session
        .execute(joydeck_engine::Command::InsertText {
            x: 20.,
            y: 50.,
            text: "before".into(),
        })
        .unwrap();
    let before = editor.session.model().clone();
    let id = editor.session.selection().shapes[0];
    let mut reference = before.clone();
    std::sync::Arc::make_mut(&mut reference.slides[0])
        .shape_mut(id)
        .unwrap()
        .text = Some(joydeck_model::TextBody::from_text("after"));
    let reference = joydeck_shell::Document::from_model("reference".into(), reference).unwrap();
    let comparison = joydeck_shell::comparison::Comparison::new(&editor, reference).unwrap();
    assert_eq!(comparison.report.differences.len(), 1);
    let command = comparison
        .command(
            &editor,
            &comparison.report.base_sha256,
            &comparison.report.reference_sha256,
            1,
            id,
        )
        .unwrap();
    let revision = editor.session.revision();
    editor.session.execute(command).unwrap();
    assert_eq!(editor.session.revision(), revision + 1);
    assert_eq!(
        editor.session.model().slides[0]
            .shape(id)
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .text(),
        "after"
    );
    assert!(
        comparison
            .command(
                &editor,
                &comparison.report.base_sha256,
                &comparison.report.reference_sha256,
                1,
                id
            )
            .is_err()
    );
    editor
        .session
        .execute(joydeck_engine::Command::Undo)
        .unwrap();
    assert_eq!(editor.session.model(), &before);
    editor
        .session
        .execute(joydeck_engine::Command::Redo)
        .unwrap();
    assert_eq!(
        editor.session.model().slides[0]
            .shape(id)
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .text(),
        "after"
    );
}
#[test]
fn documented_legacy_parameters_are_atomic_and_unknown_fields_are_refused() {
    let root = platform::private_directory("r9-legacy-control").unwrap();
    let mut d = Dispatcher::new(&root).unwrap();
    let new = d.dispatch(request("new", "document.new", None, None, json!({})));
    let target = new["result"]["documentId"].as_u64().unwrap();
    let params = json!({"command":"transaction.batch","expectedRevision":0,"commands":[{"command":"page.add"},{"command":"page.size","width":800.,"height":450.}]});
    let result = d.dispatch(request(
        "legacy-batch",
        "command.execute",
        Some(target),
        None,
        params,
    ));
    assert_eq!(result["result"]["pages"], 2, "{result}");
    let undo = d.dispatch(request(
        "undo",
        "document.undo",
        Some(target),
        Some(1),
        json!({}),
    ));
    assert_eq!(undo["result"]["pages"], 1);
    let unknown = d.dispatch(request(
        "unknown",
        "command.execute",
        Some(target),
        Some(2),
        json!({"command":"page.add","extra":1}),
    ));
    assert!(unknown.get("error").is_some());
    let inspect = d.dispatch(request(
        "status",
        "document.status",
        Some(target),
        None,
        json!({}),
    ));
    assert_eq!(inspect["result"]["revision"], 2);
}
