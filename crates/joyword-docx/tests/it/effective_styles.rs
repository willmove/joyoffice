//! Resolve paragraph and character style cascades before and after a real-Word round trip.
use joyword_model::props::{CharProps, ParaProps};
use joyword_model::styles::StyleKind;

#[test]
fn word_styles_keep_current_effective_formatting_after_save() {
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
    let mut checked = 0;
    for path in paths {
        let source = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/reference/word2024")
                .join(path),
        )
        .unwrap();
        let before = joyword_docx::read(&source).unwrap();
        let after = joyword_docx::read(&joyword_docx::write(&before).unwrap()).unwrap();
        for style in &before.styles.styles {
            let mut direct = CharProps::default();
            let para_style = match style.kind {
                StyleKind::Paragraph => Some(style.id.as_str()),
                StyleKind::Character => {
                    direct.style = Some(style.id.clone());
                    None
                }
                // Table conditional regions and numbering require their own contextual audit.
                _ => continue,
            };
            assert_eq!(
                before.styles.resolve_char(para_style, &direct),
                after.styles.resolve_char(para_style, &direct),
                "{path}: {}",
                style.id
            );
            let para = ParaProps {
                style: para_style.map(str::to_string),
                ..Default::default()
            };
            assert_eq!(
                before.styles.resolve_para(&para),
                after.styles.resolve_para(&para),
                "{path}: {}",
                style.id
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 106);
}
