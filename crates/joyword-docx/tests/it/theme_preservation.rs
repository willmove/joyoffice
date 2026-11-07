use std::io::{Cursor, Read};

fn theme(bytes: &[u8]) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let name = z
        .file_names()
        .map(Result::unwrap)
        .find(|n| n.starts_with("word/theme/") && n.ends_with(".xml"))
        .unwrap()
        .to_string();
    let mut out = Vec::new();
    z.by_name(&name).unwrap().read_to_end(&mut out).unwrap();
    out
}

#[test]
fn word_themes_survive_two_saves_and_body_edits() {
    let paths = [
        "breaks/word_keepnext.docx",
        "breaks/word_nogrid.docx",
        "breaks/word_widow4.docx",
        "interop/word_edited.docx",
        "interop/wordresaved/DOCX-MIN-001.docx",
        "interop/wordresaved/DOCX-MIN-002.docx",
        "interop/wordresaved/DOCX-MIN-003.docx",
        "interop/wordresaved/DOCX-MIN-004.docx",
        "interop/wordresaved/DOCX-SAMPLE-001.docx",
    ];
    for path in paths {
        let source = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/reference/word2024")
                .join(path),
        )
        .unwrap();
        let expected = theme(&source);
        assert_eq!(
            String::from_utf8_lossy(&expected)
                .matches("<a:font ")
                .count(),
            94,
            "{path}"
        );
        let mut doc = joyword_docx::read(&source).unwrap();
        doc.body.push(joyword_model::para_block(
            joyword_model::Paragraph::with_text(
                "Body edit preserves the theme",
                Default::default(),
            ),
        ));
        let saved = joyword_docx::write(&doc).unwrap();
        assert_eq!(theme(&saved), expected, "{path}");
        let again = joyword_docx::write(&joyword_docx::read(&saved).unwrap()).unwrap();
        assert_eq!(theme(&again), expected, "{path}");
    }
}

fn package(xml: &[u8], dependency: bool) -> Vec<u8> {
    let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let main_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="theme" Type="{rel}/theme" Target="theme/source.xml"/></Relationships>"#
    );
    let theme_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="asset" Type="{rel}/image" Target="../media/texture.bin"/></Relationships>"#
    );
    let mut entries = vec![
        ("word/document.xml", br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/></w:body></w:document>"#.as_slice()),
        ("word/_rels/document.xml.rels", main_rels.as_bytes()),
        ("word/theme/source.xml", xml),
        ("word/theme/_rels/source.xml.rels", theme_rels.as_bytes()),
    ];
    if dependency {
        entries.push(("word/media/texture.bin", b"opaque-image-asset"));
    }
    super::fixtures::zip(&entries)
}

fn custom_theme() -> String {
    // Alternate prefix, per-script/EA/CS fonts, unknown attributes, extension and relationship.
    r#"<d:theme xmlns:d="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:x="urn:test:theme" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" name="Original" x:keep="yes"><d:themeElements><d:clrScheme name="Colors"><d:dk1><d:sysClr val="windowText" lastClr="000000"/></d:dk1><d:accent1><d:srgbClr val="112233" x:retain="color"/></d:accent1></d:clrScheme><d:fontScheme name="Fonts"><d:majorFont><d:latin typeface="Heading" x:retain="font"/><d:ea typeface="EA"/><d:cs typeface="CS"/><d:font script="Hans" typeface="Chinese"/><d:font script="Arab" typeface="Arabic"/></d:majorFont><d:minorFont><d:latin typeface="Body"/><d:font script="Jpan" typeface="Japanese"/></d:minorFont></d:fontScheme><d:fmtScheme name="Effects"><x:unknown r:embed="asset"><x:payload>keep &amp; preserve</x:payload></x:unknown></d:fmtScheme></d:themeElements><!-- retained comment --><d:extLst><d:ext uri="urn:test"><x:extra/></d:ext></d:extLst></d:theme>"#.into()
}

#[test]
fn theme_edits_preserve_unknown_markup_fonts_and_dependencies() {
    let original = custom_theme();
    for utf16 in [false, true] {
        let xml = if utf16 {
            [
                vec![0xff, 0xfe],
                original.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            ]
            .concat()
        } else {
            original.as_bytes().to_vec()
        };
        let mut doc = joyword_docx::read(&package(&xml, true)).unwrap();
        assert_eq!(theme(&joyword_docx::write(&doc).unwrap()), xml);
        doc.settings.major_font = "New & Heading".into();
        doc.settings.minor_font = "New Body".into();
        doc.settings.theme_name = "New Theme".into();
        doc.settings.theme_colors[0] = joyword_model::props::Rgb(1, 2, 3);
        doc.settings.theme_colors[4] = joyword_model::props::Rgb(4, 5, 6);
        let saved = joyword_docx::write(&doc).unwrap();
        let raw = theme(&saved);
        let text = if utf16 {
            String::from_utf16(
                &raw[2..]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        } else {
            String::from_utf8(raw).unwrap()
        };
        for fragment in [
            "<d:font script=\"Hans\" typeface=\"Chinese\"/>",
            "<d:font script=\"Arab\" typeface=\"Arabic\"/>",
            "<d:font script=\"Jpan\" typeface=\"Japanese\"/>",
            "<d:ea typeface=\"EA\"/>",
            "<d:cs typeface=\"CS\"/>",
            "<x:unknown r:embed=\"asset\"><x:payload>keep &amp; preserve</x:payload></x:unknown>",
            "<!-- retained comment -->",
            "x:retain=\"font\"",
            "x:retain=\"color\"",
            "<x:extra/>",
        ] {
            assert!(text.contains(fragment), "{fragment}");
        }
        let reopened = joyword_docx::read(&saved).unwrap();
        assert_eq!(reopened.settings.major_font, "New & Heading");
        assert_eq!(reopened.settings.minor_font, "New Body");
        assert_eq!(reopened.settings.theme_name, "New Theme");
        assert_eq!(
            reopened.settings.theme_colors[0],
            doc.settings.theme_colors[0]
        );
        assert_eq!(
            reopened.settings.theme_colors[4],
            doc.settings.theme_colors[4]
        );
        let mut z = zip::ZipArchive::new(Cursor::new(&saved)).unwrap();
        let mut asset = Vec::new();
        z.by_name("word/media/texture.bin")
            .unwrap()
            .read_to_end(&mut asset)
            .unwrap();
        assert_eq!(asset, b"opaque-image-asset");
        assert_eq!(
            theme(&joyword_docx::write(&reopened).unwrap()),
            theme(&saved)
        );
    }
}

#[test]
fn incomplete_theme_dependency_refuses_save() {
    let doc = joyword_docx::read(&package(custom_theme().as_bytes(), false)).unwrap();
    assert!(joyword_docx::write(&doc).is_err());
}

#[test]
fn ambiguous_theme_edit_refuses_without_losing_original() {
    let xml = custom_theme().replace(
        "<d:latin typeface=\"Heading\" x:retain=\"font\"/>",
        "<d:latin typeface=\"Heading\"/><d:latin typeface=\"Duplicate\"/>",
    );
    let mut doc = joyword_docx::read(&package(xml.as_bytes(), true)).unwrap();
    doc.settings.major_font = "Changed".into();
    assert!(joyword_docx::write(&doc).is_err());
    doc.settings.major_font = "Heading".into();
    assert_eq!(theme(&joyword_docx::write(&doc).unwrap()), xml.as_bytes());
}

#[test]
fn missing_retained_resource_and_complex_color_edits_refuse() {
    let xml = custom_theme();
    let mut doc = joyword_docx::read(&package(xml.as_bytes(), true)).unwrap();
    doc.passthrough.remove("word/media/texture.bin");
    assert!(joyword_docx::write(&doc).is_err());
    let xml = xml.replace(
        "<d:srgbClr val=\"112233\" x:retain=\"color\"/>",
        "<d:srgbClr val=\"112233\"><d:tint val=\"20000\"/></d:srgbClr>",
    );
    let mut doc = joyword_docx::read(&package(xml.as_bytes(), true)).unwrap();
    assert_eq!(theme(&joyword_docx::write(&doc).unwrap()), xml.as_bytes());
    doc.settings.theme_colors[4] = joyword_model::props::Rgb(4, 5, 6);
    assert!(joyword_docx::write(&doc).is_err());
}
