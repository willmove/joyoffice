#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test assertions"
)]
use joydeck_engine::{Command, Session, original_bindings, transfer};
use joydeck_model::{Presentation, Shape, ShapeId, ShapeKind, Xfrm, style::PictureFill};
use std::sync::Arc;
fn session(p: Presentation) -> Session {
    Session::new(p.clone(), original_bindings(&p), true)
}
#[test]
fn preset_insertion_is_atomic_and_undoable() {
    let mut s = session(Presentation::default());
    let before = s.snapshot();
    assert!(
        s.execute(Command::InsertShape {
            preset: "unsupported".into(),
            x: 0.,
            y: 0.,
            width: 100.,
            height: 100.
        })
        .is_err()
    );
    assert_eq!(s.model(), before.model.as_ref());
    assert!(!s.can_undo());
    s.execute(Command::InsertShape {
        preset: "ellipse".into(),
        x: 20.,
        y: 30.,
        width: 160.,
        height: 90.,
    })
    .unwrap();
    assert_eq!(
        s.model().slides[0].shapes.len(),
        before.model.slides[0].shapes.len() + 1
    );
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    s.execute(Command::Redo).unwrap();
    assert!(s.dirty());
}
#[test]
fn clipboard_remaps_colliding_media_and_undo_keeps_destination() {
    let mut source = Presentation::default();
    let media = source
        .add_media("source.png", "image/png", vec![1, 2, 3])
        .unwrap();
    let id = ShapeId(source.alloc_id().unwrap());
    Arc::make_mut(&mut source.slides[0]).shapes.push(Shape {
        id,
        name: "picture".into(),
        xfrm: Some(Xfrm::new(10., 20., 100., 60.)),
        kind: ShapeKind::Picture {
            fill: PictureFill {
                media,
                ..Default::default()
            },
        },
        ..Default::default()
    });
    let transfer = transfer::capture(&source, 0, &[id]).unwrap();
    let encoded = serde_json::to_string(&transfer).unwrap();
    let transfer = serde_json::from_str(&encoded).unwrap();
    let mut destination = Presentation::default();
    destination
        .add_media("destination.png", "image/png", vec![9, 8, 7])
        .unwrap();
    let mut s = session(destination);
    let before = s.snapshot();
    s.execute(Command::PasteObjects {
        transfer: Box::new(transfer),
    })
    .unwrap();
    let pasted = s.model().slides[0].shapes.last().unwrap();
    assert_ne!(pasted.id, id);
    let ShapeKind::Picture { fill } = &pasted.kind else {
        panic!("picture missing")
    };
    assert_eq!(
        s.model().media(fill.media).unwrap().data.as_ref(),
        &vec![1, 2, 3]
    );
    assert!(
        s.model()
            .media
            .iter()
            .any(|m| m.data.as_ref() == &vec![9, 8, 7])
    );
    s.execute(Command::Undo).unwrap();
    assert_eq!(s.model(), before.model.as_ref());
    s.execute(Command::Redo).unwrap();
    assert_eq!(s.model().media.len(), 2);
}
#[test]
fn clipboard_missing_resources_and_duplicate_ids_do_not_change_history() {
    let mut s = session(Presentation::default());
    s.execute(Command::InsertShape {
        preset: "rect".into(),
        x: 20.,
        y: 20.,
        width: 100.,
        height: 60.,
    })
    .unwrap();
    let mut transfer = transfer::capture(s.model(), 0, &s.selection().shapes).unwrap();
    transfer.shapes.push(transfer.shapes[0].clone());
    let before = s.snapshot();
    let revision = s.revision();
    assert!(
        s.execute(Command::PasteObjects {
            transfer: Box::new(transfer)
        })
        .is_err()
    );
    assert_eq!(s.model(), before.model.as_ref());
    assert_eq!(s.revision(), revision);
}
#[test]
fn clipboard_keeps_resolved_placeholder_position_as_regular_shape() {
    let p = Presentation::default();
    let id = p.slides[0].shapes[0].id;
    let clip = transfer::capture(&p, 0, &[id]).unwrap();
    assert!(clip.shapes[0].ph.is_none());
    assert!(clip.shapes[0].xfrm.is_some());
    assert!(clip.shapes[0].text.is_some());
}
