use joyword_model::props::{CharProps, ThemeFont};
use joyword_model::{Document, Paragraph, para_block};

fn bound(ascii: bool) -> Document {
    let mut doc = Document::new();
    doc.settings.major_font = "Original Heading".into();
    doc.body = vec![para_block(Paragraph::with_text(
        "Bound text",
        CharProps {
            font: Some("Original Heading".into()),
            theme_font: Some(ThemeFont {
                ascii: ascii.then(|| "majorAscii".into()),
                h_ansi: Some("majorHAnsi".into()),
                resolved: "Original Heading".into(),
            }),
            ..Default::default()
        },
    ))]
    .into();
    doc
}

fn reopen(doc: &Document) -> Document {
    joyword_docx::read(&joyword_docx::write(doc).unwrap()).unwrap()
}

#[test]
fn theme_font_references_survive_two_saves_and_follow_new_theme() {
    for ascii in [false, true] {
        let original = bound(ascii);
        let once = reopen(&original);
        let mut twice = reopen(&once);
        assert_eq!(
            twice.body[0].as_para().unwrap().runs[0].props.theme_font,
            original.body[0].as_para().unwrap().runs[0].props.theme_font
        );
        twice.settings.major_font = "New Heading".into();
        assert_eq!(
            reopen(&twice).body[0].as_para().unwrap().runs[0]
                .props
                .font
                .as_deref(),
            Some("New Heading")
        );
    }
}

#[test]
fn explicit_same_font_stays_fixed_after_theme_change() {
    let mut doc = reopen(&bound(true));
    let start = doc.start_of(joyword_model::StoryRef::Body);
    doc.para_mut(start.story, &start.path)
        .unwrap()
        .format(0, 10, &|p| {
            p.set_fixed_font(Some("Original Heading".into()))
        })
        .unwrap();
    doc.settings.major_font = "New Heading".into();
    let saved = reopen(&doc);
    let props = &saved.body[0].as_para().unwrap().runs[0].props;
    assert_eq!(props.font.as_deref(), Some("Original Heading"));
    assert!(props.theme_font.is_none());
}

#[test]
fn later_fixed_font_declaration_does_not_reuse_previous_binding() {
    let source = reopen(&bound(true));
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
                .replace(
                    "</w:rPr>",
                    "<w:rFonts w:ascii=\"Original Heading\" w:hAnsi=\"Original Heading\"/></w:rPr>",
                )
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
            .theme_font
            .is_none()
    );
}
