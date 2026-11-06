#![allow(clippy::unwrap_used, clippy::panic, reason = "Test assertions")]
use joydeck_engine::{Command, Session, TextTarget, original_bindings, text_resources};
use joydeck_model::{Fill, Presentation, ShapeId, TextBody, style::PictureFill, text::Bullet};

fn setup() -> (Session, TextBody, Vec<joydeck_engine::transfer::Resource>) {
    let mut source = Presentation::default();
    let image = source
        .add_media("synthetic.png", "image/png", vec![1, 2, 3])
        .unwrap();
    let mut body = TextBody::from_text("image formatting");
    body.paragraphs[0].props.bullet = Some(Bullet::Picture { media: image });
    body.paragraphs[0].runs[0].props.fill = Some(Fill::Picture(PictureFill {
        media: image,
        ..Default::default()
    }));
    let resources = text_resources::capture(&source, &body).unwrap();
    let mut destination = Presentation::default();
    destination
        .add_media("existing.png", "image/png", vec![9, 8, 7])
        .unwrap();
    let s = Session::new(destination.clone(), original_bindings(&destination), true);
    (s, body, resources)
}

#[test]
fn staging_cancel_and_atomic_commit_remap_picture_fill_and_bullet() {
    let (mut s, body, resources) = setup();
    let before = s.snapshot();
    let revision = s.revision();
    let pending =
        text_resources::prepare(&before, TextTarget::Notes, body.clone(), resources.clone())
            .unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    drop(pending); // User cancels before the transaction is executed.
    assert_eq!(s.revision(), revision);
    assert!(!s.can_undo());
    let command =
        text_resources::prepare(&before, TextTarget::Notes, body, resources.clone()).unwrap();
    s.execute(command).unwrap();
    let notes = &s.model().slides[0].notes;
    let Some(Bullet::Picture { media }) = notes.paragraphs[0].props.bullet else {
        panic!("bullet missing")
    };
    assert_ne!(media, resources[0].id);
    let Some(Fill::Picture(fill)) = &notes.paragraphs[0].runs[0].props.fill else {
        panic!("fill missing")
    };
    assert_eq!(fill.media, media);
    assert_eq!(
        s.model().media(media).unwrap().data.as_ref(),
        &vec![1, 2, 3]
    );
    assert!(s.model().media(media).unwrap().link.is_none());
    let after = s.model().clone();
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    assert!(!s.can_undo(), "one undo must remove both text and resource");
    s.execute(Command::Redo).unwrap();
    assert_eq!(s.model(), &after);
}

#[test]
fn missing_duplicate_or_unused_resources_do_not_mutate_model() {
    let (mut s, body, resources) = setup();
    let before = s.snapshot();
    for mode in 0..4 {
        let mut command =
            text_resources::prepare(&before, TextTarget::Notes, body.clone(), resources.clone())
                .unwrap();
        if let Command::SetTextWithResources { resources, .. } = &mut command {
            match mode {
                0 => resources.clear(),
                1 => resources.push(resources[0].clone()),
                2 => {
                    let mut unused = resources[0].clone();
                    unused.id = joydeck_model::MediaId(u32::MAX);
                    resources.push(unused);
                }
                _ => resources[0].content_type = "application/octet-stream".into(),
            }
        }
        assert!(s.execute(command).is_err());
        assert_eq!(s.model(), before.model.as_ref());
        assert!(!s.can_undo());
    }
}

#[test]
fn stale_draft_and_failure_after_resource_allocation_roll_back() {
    let (mut s, body, resources) = setup();
    let before = s.snapshot();
    let command =
        text_resources::prepare(&before, TextTarget::Notes, body.clone(), resources.clone())
            .unwrap();
    assert!(
        s.execute(Command::Batch {
            commands: vec![
                command,
                Command::SetText {
                    target: TextTarget::Shape {
                        shape: ShapeId(u32::MAX)
                    },
                    body: Box::new(TextBody::from_text("invalid target")),
                }
            ]
        })
        .is_err()
    );
    assert_eq!(s.model(), before.model.as_ref());
    assert!(!s.can_undo());
    let pending = text_resources::prepare(&before, TextTarget::Notes, body, resources).unwrap();
    s.execute(Command::SetText {
        target: TextTarget::Notes,
        body: Box::new(TextBody::from_text("newer edit")),
    })
    .unwrap();
    let newer = s.snapshot();
    let revision = s.revision();
    assert!(s.execute(pending).is_err());
    assert_eq!(s.model(), newer.model.as_ref());
    assert_eq!(s.revision(), revision);
}

#[test]
fn source_link_refusal_budget_and_changed_slide_are_safe() {
    let (mut s, body, mut resources) = setup();
    let mut linked_source = Presentation::default();
    linked_source
        .add_media("linked.png", "image/png", vec![1, 2, 3])
        .unwrap();
    linked_source.media[0].link = Some("file:///not-opened/synthetic.png".into());
    assert!(text_resources::capture(&linked_source, &body).is_err());
    let before = s.snapshot();
    resources[0].data = vec![0; 16 * 1024 * 1024 + 1];
    assert!(text_resources::prepare(&before, TextTarget::Notes, body.clone(), resources).is_err());
    assert_eq!(s.model(), before.model.as_ref());
    let (_, _, resources) = setup();
    let pending = text_resources::prepare(&before, TextTarget::Notes, body, resources).unwrap();
    s.execute(Command::NewSlide).unwrap();
    let after = s.snapshot();
    assert!(s.execute(pending).is_err());
    assert_eq!(s.model(), after.model.as_ref());
}

#[test]
fn raw_set_text_does_not_remap_foreign_resources() {
    // Demonstrates why a text-only command cannot implement cross-document media paste.
    let (mut s, body, resources) = setup();
    s.execute(Command::SetText {
        target: TextTarget::Notes,
        body: Box::new(body),
    })
    .unwrap();
    let Some(Bullet::Picture { media }) = s.model().slides[0].notes.paragraphs[0].props.bullet
    else {
        panic!("bullet")
    };
    assert_eq!(media, resources[0].id);
    assert_ne!(
        s.model().media(media).unwrap().data.as_ref(),
        &resources[0].data
    );
}

#[test]
fn shape_and_cell_transactions_roundtrip_command_and_history() {
    for table in [false, true] {
        let (mut s, body, resources) = setup();
        s.execute(if table {
            Command::InsertTable {
                x: 0.,
                y: 0.,
                rows: 1,
                cols: 1,
                width: 100.,
                row_height: 40.,
            }
        } else {
            Command::InsertText {
                x: 0.,
                y: 0.,
                text: "before".into(),
            }
        })
        .unwrap();
        let shape = s.selection().shapes[0];
        let target = if table {
            TextTarget::Cell {
                shape,
                row: 0,
                col: 0,
            }
        } else {
            TextTarget::Shape { shape }
        };
        let before = s.snapshot();
        let pending = text_resources::prepare(&before, target, body, resources).unwrap();
        let encoded = serde_json::to_vec(&pending).unwrap();
        let decoded: Command = serde_json::from_slice(&encoded).unwrap();
        s.execute(decoded).unwrap();
        assert_eq!(s.model().media.len(), before.model.media.len() + 1);
        let after = s.model().clone();
        s.execute(Command::Undo).unwrap();
        assert_eq!(s.model(), before.model.as_ref());
        s.execute(Command::Redo).unwrap();
        assert_eq!(s.model(), &after);
    }
}
