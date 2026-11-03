#![allow(clippy::unwrap_used, clippy::expect_used, reason = "Test assertions")]
use joydeck_engine::Command;
use joydeck_shell::{Cancellation, Document, editor::Editor};
#[test]
fn fresh_document_is_clean_but_first_edit_is_recoverable_and_saveable() {
    let root = joydeck_shell::platform::private_directory("r8-new").unwrap();
    let mut e = Editor::new(root.join("recovery")).unwrap();
    assert!(!e.session.dirty());
    e.session
        .execute(Command::InsertShape {
            preset: "ellipse".into(),
            x: 50.,
            y: 70.,
            width: 160.,
            height: 100.,
        })
        .unwrap();
    assert!(e.session.dirty());
    e.checkpoint_job().checkpoint().unwrap();
    let output = root.join("新文稿 test.joydeck");
    let saved = e.save_job(output.clone()).unwrap().run().unwrap();
    e.apply_saved(saved);
    let reopened = Editor::open(&output, root.join("reopened")).unwrap();
    assert_eq!(reopened.session.model(), e.session.model());
    assert!(!reopened.session.dirty());
    e.session.execute(Command::Undo).unwrap();
    assert!(e.session.dirty());
}
#[test]
fn copied_objects_survive_native_and_public_pptx_exchange() {
    let root = joydeck_shell::platform::private_directory("r8-clipboard").unwrap();
    let mut e = Editor::new(root.join("recovery")).unwrap();
    e.session
        .execute(Command::InsertShape {
            preset: "triangle".into(),
            x: 50.,
            y: 80.,
            width: 100.,
            height: 90.,
        })
        .unwrap();
    let transfer =
        joydeck_engine::transfer::capture(e.session.model(), 0, &e.session.selection().shapes)
            .unwrap();
    let encoded = serde_json::to_string(&transfer).unwrap();
    e.session
        .execute(Command::PasteObjects {
            transfer: Box::new(serde_json::from_str(&encoded).unwrap()),
        })
        .unwrap();
    let expected = e.session.model().slides[0].shapes.len();
    for extension in ["joydeck", "pptx"] {
        let output = root.join(format!("copied.{extension}"));
        e.save_job(output.clone()).unwrap().run().unwrap();
        let read = Document::open(&output, &Cancellation::default()).unwrap();
        assert_eq!(read.presentation.slides[0].shapes.len(), expected);
    }
}
#[test]
fn clipboard_image_uses_file_import_limits() {
    let data = include_bytes!("../../../tests/fixtures/joydeck/r4-r5/shapes.png").to_vec();
    assert!(matches!(
        joydeck_shell::platform::picture_bytes_command(data, false).unwrap(),
        Command::InsertPicture { .. }
    ));
    assert!(
        joydeck_shell::platform::picture_bytes_command(vec![0; 16 * 1024 * 1024 + 1], false)
            .is_err()
    );
    assert!(
        joydeck_shell::platform::picture_bytes_command(b"malformed image".to_vec(), false).is_err()
    );
}
