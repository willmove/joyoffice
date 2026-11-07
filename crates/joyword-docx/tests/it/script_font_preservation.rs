use joyword_model::props::{CharProps, ScriptFontRef};
use joyword_model::{Document, Paragraph, StoryRef, para_block};
fn bound() -> Document {
    let mut d = Document::new();
    d.body = vec![para_block(Paragraph::with_text(
        "Script fonts",
        CharProps {
            font_ea: Some("EA fixed".into()),
            font_cs: Some("CS fixed".into()),
            font_ea_theme: Some(ScriptFontRef {
                value: "majorEastAsia".into(),
                fixed: Some("EA fixed".into()),
            }),
            font_cs_theme: Some(ScriptFontRef {
                value: "minorBidi".into(),
                fixed: Some("CS fixed".into()),
            }),
            font_hint: Some("eastAsia".into()),
            ..Default::default()
        },
    ))]
    .into();
    d
}
fn reopen(d: &Document) -> Document {
    joyword_docx::read(&joyword_docx::write(d).unwrap()).unwrap()
}
fn props(d: &Document) -> &CharProps {
    &d.body[0].as_para().unwrap().runs[0].props
}
fn replace_main(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut parts = Vec::new();
    for i in 0..z.len() {
        let mut p = z.by_index(i).unwrap();
        let name = p.name().unwrap().to_string();
        let mut b = Vec::new();
        std::io::Read::read_to_end(&mut p, &mut b).unwrap();
        if name == "word/document.xml" {
            b = String::from_utf8(b).unwrap().replace(from, to).into_bytes();
        }
        parts.push((name, b));
    }
    super::fixtures::zip(
        &parts
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect::<Vec<_>>(),
    )
}
#[test]
fn script_references_hint_and_unknown_values_survive_twice() {
    for unusual in [false, true] {
        let mut d = bound();
        if unusual {
            let start = d.start_of(StoryRef::Body);
            let p = &mut d.para_mut(start.story, &start.path).unwrap().runs[0].props;
            p.font_ea_theme.as_mut().unwrap().value = "future & slot".into();
            p.font_hint = Some("future-hint".into());
        }
        let saved = reopen(&reopen(&d));
        assert_eq!(props(&saved).font_ea_theme, props(&d).font_ea_theme);
        assert_eq!(props(&saved).font_cs_theme, props(&d).font_cs_theme);
        assert_eq!(props(&saved).font_hint, props(&d).font_hint);
    }
}
#[test]
fn same_value_explicit_script_edit_detaches_only_edited_binding() {
    let mut d = reopen(&bound());
    let start = d.start_of(StoryRef::Body);
    d.para_mut(start.story, &start.path)
        .unwrap()
        .format(0, 12, &|p| p.set_fixed_ea_font(Some("EA fixed".into())))
        .unwrap();
    let saved = reopen(&d);
    assert!(props(&saved).font_ea_theme.is_none());
    assert!(props(&saved).font_cs_theme.is_some());
    assert_eq!(props(&saved).font_hint.as_deref(), Some("eastAsia"));
}
#[test]
fn unknown_rfonts_attributes_or_children_refuse_save() {
    let bytes = joyword_docx::write(&bound()).unwrap();
    for (from, to) in [
        ("<w:rFonts ", "<w:rFonts w:future=\"keep\" "),
        (
            "<w:rFonts ",
            "<w:rFonts xmlns:x=\"urn:future\" x:keep=\"yes\" ",
        ),
        ("</w:rPr>", "<w:rFonts><w:future/></w:rFonts></w:rPr>"),
    ] {
        let source = replace_main(&bytes, from, to);
        let d = joyword_docx::read(&source).unwrap();
        assert!(joyword_docx::write(&d).is_err());
    }
}
#[test]
fn later_font_declaration_replaces_opaque_bindings_and_hint() {
    let bytes = joyword_docx::write(&bound()).unwrap();
    let source = replace_main(
        &bytes,
        "</w:rPr>",
        "<w:rFonts w:eastAsia=\"New\" w:cs=\"Other\"/></w:rPr>",
    );
    let d = reopen(&joyword_docx::read(&source).unwrap());
    assert!(props(&d).font_ea_theme.is_none());
    assert!(props(&d).font_cs_theme.is_none());
    assert!(props(&d).font_hint.is_none());
}
#[test]
fn unresolved_script_fonts_transferred_to_another_theme_refuse_save() {
    let mut dest = Document::new();
    let _fragment = dest.adopt(bound());
    assert!(joyword_docx::write(&dest).is_err());
}
