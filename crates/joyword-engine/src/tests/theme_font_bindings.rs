use crate::Session;
use joyword_model::props::{CharProps, ThemeFont};
use joyword_model::{Document, Paragraph, StoryRef, para_block};
use serde_json::json;

fn session() -> Session {
    let mut doc = Document::new();
    doc.settings.major_font = "Original Heading".into();
    doc.body = vec![para_block(Paragraph::with_text(
        "Bound text",
        CharProps {
            font: Some("Original Heading".into()),
            theme_font: Some(ThemeFont {
                ascii: Some("majorAscii".into()),
                h_ansi: Some("majorHAnsi".into()),
                resolved: "Original Heading".into(),
            }),
            ..Default::default()
        },
    ))]
    .into();
    Session::new(doc)
}

#[test]
fn theme_command_refreshes_binding_and_undo_restores_it() {
    let mut s = session();
    let original = s.doc.clone();
    s.pending = Some(s.doc.body[0].as_para().unwrap().runs[0].props.clone());
    let original_pending = s.pending.clone();
    s.run(
        "design.themeFonts",
        &json!({"heading":"New Heading","body":"New Body"}),
    )
    .unwrap();
    let props = &s.doc.body[0].as_para().unwrap().runs[0].props;
    assert_eq!(props.font.as_deref(), Some("New Heading"));
    assert!(props.theme_font.is_some());
    assert_eq!(
        s.pending.as_ref().and_then(|p| p.font.as_deref()),
        Some("New Heading")
    );
    let reopened = joyword_docx::read(&joyword_docx::write(&s.doc).unwrap()).unwrap();
    assert_eq!(
        reopened.body[0].as_para().unwrap().runs[0]
            .props
            .font
            .as_deref(),
        Some("New Heading")
    );
    s.undo();
    assert_eq!(s.doc, original);
    assert_eq!(s.pending, original_pending);
}

#[test]
fn font_command_detaches_same_value_and_undo_restores_binding() {
    let mut s = session();
    s.sel.anchor = s.doc.start_of(StoryRef::Body);
    s.sel.focus = s.doc.end_of(StoryRef::Body);
    s.run("format.font", &json!({"name":"Original Heading"}))
        .unwrap();
    assert!(
        s.doc.body[0].as_para().unwrap().runs[0]
            .props
            .theme_font
            .is_none()
    );
    s.undo();
    assert!(
        s.doc.body[0].as_para().unwrap().runs[0]
            .props
            .theme_font
            .is_some()
    );
}
