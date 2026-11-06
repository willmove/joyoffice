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

#[test]
fn theme_colors_preserve_fixed_style_text_and_borders() {
    use joyword_model::props::{Border, BorderStyle, Borders};
    for command in ["design.themeColors", "design.theme"] {
        let mut doc = Document::new();
        for id in [
            "Heading1",
            "IntenseQuote",
            "IntenseEmphasis",
            "IntenseReference",
        ] {
            let style = doc.styles.styles.iter_mut().find(|s| s.id == id).unwrap();
            style
                .chr
                .set_fixed_color(Some(TextColor::Rgb(Rgb(255, 0, 0))));
            style.para.borders = Some(Borders {
                top: Some(Border {
                    style: BorderStyle::Single,
                    width: 1.0,
                    color: Some(Rgb(0, 255, 0)),
                    space: 0.0,
                }),
                ..Default::default()
            });
        }
        let before = doc.clone();
        let mut s = Session::new(doc);
        s.run(command, &json!({"name":"Studio"})).unwrap();
        for (old, new) in before.styles.styles.iter().zip(&s.doc.styles.styles) {
            if old.chr.theme_color.is_none() {
                assert_eq!(old.chr.color, new.chr.color, "{command}: {}", old.id);
            }
            assert_eq!(old.para.borders, new.para.borders, "{command}: {}", old.id);
        }
        let reopened = joyword_docx::read(&joyword_docx::write(&s.doc).unwrap()).unwrap();
        for id in ["Heading1", "IntenseQuote"] {
            let style = reopened.styles.styles.iter().find(|s| s.id == id).unwrap();
            assert_eq!(style.chr.color, Some(TextColor::Rgb(Rgb(255, 0, 0))));
            assert_eq!(
                style.para.borders.as_ref().unwrap().top.unwrap().color,
                Some(Rgb(0, 255, 0))
            );
        }
        s.undo();
        assert_eq!(s.doc, before);
    }
}

#[test]
fn bound_style_color_follows_theme_while_same_value_fixed_style_stays_fixed() {
    let mut doc = Document::new();
    let original = doc.settings.theme_colors[4];
    let style = doc
        .styles
        .styles
        .iter_mut()
        .find(|s| s.id == "Heading1")
        .unwrap();
    style.chr.color = Some(TextColor::Rgb(original));
    style.chr.theme_color = Some(ThemeColor {
        name: "accent1".into(),
        slot: 4,
        tint: None,
        shade: None,
        resolved: original,
    });
    let fixed = doc
        .styles
        .styles
        .iter_mut()
        .find(|s| s.id == "IntenseQuote")
        .unwrap();
    fixed.chr.set_fixed_color(Some(TextColor::Rgb(original)));
    let mut s = Session::new(doc);
    let before = s.doc.clone();
    s.run("design.themeColors", &json!({"name":"Studio"}))
        .unwrap();
    let bound = s
        .doc
        .styles
        .styles
        .iter()
        .find(|s| s.id == "Heading1")
        .unwrap();
    assert_eq!(
        bound.chr.color,
        Some(TextColor::Rgb(s.doc.settings.theme_colors[4]))
    );
    assert_ne!(bound.chr.color, Some(TextColor::Rgb(original)));
    let fixed = s
        .doc
        .styles
        .styles
        .iter()
        .find(|s| s.id == "IntenseQuote")
        .unwrap();
    assert_eq!(fixed.chr.color, Some(TextColor::Rgb(original)));
    let saved = joyword_docx::read(&joyword_docx::write(&s.doc).unwrap()).unwrap();
    assert_eq!(
        saved
            .styles
            .styles
            .iter()
            .find(|s| s.id == "Heading1")
            .unwrap()
            .chr
            .theme_color,
        bound.chr.theme_color
    );
    assert_eq!(
        saved
            .styles
            .styles
            .iter()
            .find(|s| s.id == "IntenseQuote")
            .unwrap()
            .chr
            .color,
        fixed.chr.color
    );
    s.undo();
    assert_eq!(s.doc, before);
}

#[test]
fn theme_color_commands_preserve_unbound_word_style_colors_and_borders() {
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
        s.run("design.themeColors", &json!({"name":"Studio"}))
            .unwrap();
        for (old, new) in before.styles.iter().zip(&s.doc.styles.styles) {
            if old.chr.theme_color.is_none() {
                assert_eq!(old.chr.color, new.chr.color, "{name}: {}", old.id);
            } else {
                assert_eq!(
                    new.chr.color,
                    new.chr
                        .theme_color
                        .as_ref()
                        .unwrap()
                        .resolve(&s.doc.settings.theme_colors)
                        .map(TextColor::Rgb),
                    "{name}: {}",
                    old.id
                );
            }
            assert_eq!(old.para.borders, new.para.borders, "{name}: {}", old.id);
        }
    }
}

#[test]
fn style_sets_keep_the_fixed_colors_they_explicitly_choose() {
    for (name, expected) in [
        ("basic", Rgb(0x26, 0x26, 0x26)),
        ("shaded", Rgb::WHITE),
        ("minimalist", Rgb(0x40, 0x40, 0x40)),
    ] {
        let mut s = Session::new(Document::new());
        s.run("design.styleSet", &json!({"name":name})).unwrap();
        let heading = s
            .doc
            .styles
            .styles
            .iter()
            .find(|s| s.id == "Heading1")
            .unwrap();
        assert_eq!(heading.chr.color, Some(TextColor::Rgb(expected)), "{name}");
    }
}
