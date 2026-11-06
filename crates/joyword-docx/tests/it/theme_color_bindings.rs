use joyword_model::props::{CharProps, Rgb, TextColor, ThemeColor};
use joyword_model::{Document, Paragraph, StoryRef, para_block};

fn bound(tint: Option<&str>, shade: Option<&str>) -> Document {
    let mut doc = Document::new();
    let mut binding = ThemeColor {
        name: "accent1".into(),
        slot: 4,
        tint: tint.map(str::to_string),
        shade: shade.map(str::to_string),
        resolved: Rgb::BLACK,
    };
    binding.resolved = binding.resolve(&doc.settings.theme_colors).unwrap();
    doc.body = vec![para_block(Paragraph::with_text(
        "Bound text",
        CharProps {
            color: Some(TextColor::Rgb(binding.resolved)),
            theme_color: Some(binding),
            ..Default::default()
        },
    ))]
    .into();
    doc
}
fn reopen(doc: &Document) -> Document {
    joyword_docx::read(&joyword_docx::write(doc).unwrap()).unwrap()
}
fn props(doc: &Document) -> &CharProps {
    &doc.body[0].as_para().unwrap().runs[0].props
}
#[test]
fn references_and_modifiers_survive_two_saves_and_theme_replacement() {
    for (tint, shade) in [
        (None, None),
        (Some("00"), None),
        (Some("ff"), None),
        (Some("99"), None),
        (None, Some("00")),
        (None, Some("FF")),
        (None, Some("BF")),
        (Some("99"), Some("BF")),
        (Some("invalid"), None),
    ] {
        let original = bound(tint, shade);
        let mut twice = reopen(&reopen(&original));
        assert_eq!(props(&twice).theme_color, props(&original).theme_color);
        twice.settings.theme_colors[4] = Rgb(255, 0, 0);
        let expected = props(&twice)
            .theme_color
            .as_ref()
            .unwrap()
            .resolve(&twice.settings.theme_colors);
        assert_eq!(props(&reopen(&twice)).color, expected.map(TextColor::Rgb));
    }
}
#[test]
fn explicit_same_color_and_auto_remain_fixed() {
    for auto in [false, true] {
        let mut doc = reopen(&bound(None, Some("BF")));
        let value = if auto {
            Some(TextColor::Auto)
        } else {
            props(&doc).color
        };
        let start = doc.start_of(StoryRef::Body);
        doc.para_mut(start.story, &start.path)
            .unwrap()
            .format(0, 10, &|p| p.set_fixed_color(value))
            .unwrap();
        doc.settings.theme_colors[4] = Rgb(255, 0, 0);
        let saved = reopen(&doc);
        assert_eq!(props(&saved).color, value);
        assert!(props(&saved).theme_color.is_none());
    }
}
#[test]
fn mapped_color_retains_logical_reference_and_mapping() {
    let bytes = joyword_docx::write(&bound(None, None)).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut parts = Vec::new();
    for i in 0..zip.len() {
        let mut part = zip.by_index(i).unwrap();
        let name = part.name().unwrap().to_string();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut part, &mut bytes).unwrap();
        if name == "word/settings.xml" {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace(
                    "</w:settings>",
                    "<w:clrSchemeMapping w:accent1=\"accent6\"/></w:settings>",
                )
                .into_bytes();
        }
        parts.push((name, bytes));
    }
    let refs = parts
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect::<Vec<_>>();
    let original = joyword_docx::read(&super::fixtures::zip(&refs)).unwrap();
    assert_eq!(props(&original).theme_color.as_ref().unwrap().slot, 9);
    let mut saved = reopen(&reopen(&original));
    assert_eq!(props(&saved).theme_color, props(&original).theme_color);
    saved.settings.theme_colors[9] = Rgb(255, 0, 0);
    assert_eq!(
        props(&reopen(&saved)).color,
        Some(TextColor::Rgb(Rgb(255, 0, 0)))
    );
}

#[test]
fn later_fixed_color_declaration_does_not_reuse_previous_binding() {
    let source = reopen(&bound(None, None));
    let bytes = joyword_docx::write(&source).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut parts = Vec::new();
    for index in 0..zip.len() {
        let mut part = zip.by_index(index).unwrap();
        let name = part.name().unwrap().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut part, &mut data).unwrap();
        if name == "word/document.xml" {
            let xml = String::from_utf8(data).unwrap();
            data = xml
                .replace("</w:rPr>", "<w:color w:val=\"auto\"/></w:rPr>")
                .into_bytes();
        }
        parts.push((name, data));
    }
    let refs = parts
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect::<Vec<_>>();
    let doc = joyword_docx::read(&super::fixtures::zip(&refs)).unwrap();
    assert!(
        doc.body[0].as_para().unwrap().runs[0]
            .props
            .theme_color
            .is_none()
    );
}
