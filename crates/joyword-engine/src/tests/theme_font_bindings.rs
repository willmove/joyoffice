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

#[test]
fn fixed_style_fonts_and_defaults_survive_theme_commands_and_undo() {
    let mut doc = Document::new();
    doc.styles
        .default_chr
        .set_fixed_font(Some("Fixed body".into()));
    let heading = doc
        .styles
        .styles
        .iter_mut()
        .find(|s| s.id == "Heading1")
        .unwrap();
    // Even the exact old built-in name does not establish theme intent.
    heading
        .chr
        .set_fixed_font(Some(joyword_model::styles::HEADING_FONT.into()));
    let original = doc.clone();
    let mut s = Session::new(doc);
    s.run(
        "design.themeFonts",
        &json!({"heading":"New heading","body":"New body"}),
    )
    .unwrap();
    assert_eq!(s.doc.styles.default_chr.font.as_deref(), Some("Fixed body"));
    assert_eq!(
        s.doc
            .styles
            .styles
            .iter()
            .find(|s| s.id == "Heading1")
            .unwrap()
            .chr
            .font
            .as_deref(),
        Some(joyword_model::styles::HEADING_FONT)
    );
    let saved = joyword_docx::read(&joyword_docx::write(&s.doc).unwrap()).unwrap();
    assert_eq!(saved.styles.default_chr.font.as_deref(), Some("Fixed body"));
    s.undo();
    assert_eq!(s.doc, original);
}

#[test]
fn new_document_theme_fonts_follow_declared_roles_after_roundtrip() {
    let doc = Document::new();
    let doc = joyword_docx::read(&joyword_docx::write(&doc).unwrap()).unwrap();
    let mut s = Session::new(doc);
    s.run(
        "design.themeFonts",
        &json!({"heading":"New heading","body":"New body"}),
    )
    .unwrap();
    assert_eq!(s.doc.styles.default_chr.font.as_deref(), Some("New body"));
    for (id, expected) in [
        ("Heading1", "New heading"),
        ("Heading3", "New body"),
        ("Title", "New heading"),
    ] {
        assert_eq!(
            s.doc
                .styles
                .styles
                .iter()
                .find(|s| s.id == id)
                .unwrap()
                .chr
                .font
                .as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn word_corpus_fixed_latin_and_explicit_script_fonts_are_not_replaced() {
    for name in [
        "breaks/word_keepnext.docx",
        "breaks/word_nogrid.docx",
        "breaks/word_widow4.docx",
        "interop/word_edited.docx",
        "interop/wordresaved/DOCX-MIN-001.docx",
        "interop/wordresaved/DOCX-MIN-002.docx",
        "interop/wordresaved/DOCX-MIN-003.docx",
        "interop/wordresaved/DOCX-MIN-004.docx",
        "interop/wordresaved/DOCX-SAMPLE-001.docx",
    ] {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/reference/word2024")
                .join(name),
        )
        .unwrap();
        let doc = joyword_docx::read(&bytes).unwrap();
        let before = doc.styles.clone();
        let mut s = Session::new(doc);
        s.run(
            "design.themeFonts",
            &json!({"heading":"New heading","body":"New body"}),
        )
        .unwrap();
        for (old, new) in std::iter::once((&before.default_chr, &s.doc.styles.default_chr)).chain(
            before
                .styles
                .iter()
                .zip(&s.doc.styles.styles)
                .map(|(a, b)| (&a.chr, &b.chr)),
        ) {
            if old.theme_font.is_none() {
                assert_eq!(old.font, new.font, "{name}");
            }
            assert_eq!(old.font_ea, new.font_ea, "{name}");
            assert_eq!(old.font_cs, new.font_cs, "{name}");
        }
    }
}
