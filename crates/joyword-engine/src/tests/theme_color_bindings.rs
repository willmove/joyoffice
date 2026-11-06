use crate::Session;
use joyword_model::props::{CharProps, Rgb, TextColor, ThemeColor};
use joyword_model::{Document, Paragraph, StoryRef, para_block};
use serde_json::json;

fn session() -> Session {
    let mut doc = Document::new();
    doc.settings.theme_colors[4] = Rgb(255, 0, 0);
    doc.body = vec![para_block(Paragraph::with_text(
        "Bound text",
        CharProps {
            color: Some(TextColor::Rgb(Rgb(191, 0, 0))),
            theme_color: Some(ThemeColor {
                name: "accent1".into(),
                slot: 4,
                tint: None,
                shade: Some("BF".into()),
                resolved: Rgb(191, 0, 0),
            }),
            ..Default::default()
        },
    ))]
    .into();
    Session::new(doc)
}

#[test]
fn theme_command_updates_colors_pending_and_undo() {
    let mut s = session();
    let original = s.doc.clone();
    s.pending = Some(s.doc.body[0].as_para().unwrap().runs[0].props.clone());
    let pending = s.pending.clone();
    s.run("design.theme", &json!({"name":"Studio"})).unwrap();
    let props = &s.doc.body[0].as_para().unwrap().runs[0].props;
    let expected = props
        .theme_color
        .as_ref()
        .unwrap()
        .resolve(&s.doc.settings.theme_colors)
        .map(TextColor::Rgb);
    assert_eq!(props.color, expected);
    assert_ne!(props.color, Some(TextColor::Rgb(Rgb(191, 0, 0))));
    assert_eq!(s.pending.as_ref().unwrap().color, expected);
    let saved = joyword_docx::read(&joyword_docx::write(&s.doc).unwrap()).unwrap();
    assert_eq!(
        saved.body[0].as_para().unwrap().runs[0].props.color,
        expected
    );
    s.undo();
    assert_eq!(s.doc, original);
    assert_eq!(s.pending, pending);
}
#[test]
fn explicit_color_command_detaches_same_value_and_undo_restores() {
    let mut s = session();
    s.sel.anchor = s.doc.start_of(StoryRef::Body);
    s.sel.focus = s.doc.end_of(StoryRef::Body);
    s.run("format.color", &json!({"color":"BF0000"})).unwrap();
    assert!(
        s.doc.body[0].as_para().unwrap().runs[0]
            .props
            .theme_color
            .is_none()
    );
    s.undo();
    assert!(
        s.doc.body[0].as_para().unwrap().runs[0]
            .props
            .theme_color
            .is_some()
    );
}
