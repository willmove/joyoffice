//! W12: importer → text input → export → importer for formatted paragraphs
//! whose properties are fully mapped.

use std::path::PathBuf;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{ExportMode, ImportResult, W, import};
use joyword_model::Justification;
use joyword_shell::docx::DocxSession;
use joyword_shell::input::{Direction, InputLimits, TextInput};

const SECT: &str = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/></w:sectPr>"#;

fn fixture(id: u8) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn package(body: &str) -> Vec<u8> {
    let source = fixture(1);
    let package = Package::read(&source, &Budget::default()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}{SECT}</w:body></w:document>"#
    );
    let mut items = vec![(
        "[Content_Types].xml",
        package.content_types_xml(),
        Method::Deflated,
    )];
    for part in package.parts() {
        let data = if part.name.as_str() == "/word/document.xml" {
            xml.as_bytes()
        } else {
            &part.data
        };
        items.push((part.name.item_name(), data, part.method));
    }
    write_archive(&items).unwrap()
}

fn open(bytes: &[u8]) -> TextInput<DocxSession> {
    let session = DocxSession::open(bytes, &Budget::default()).unwrap();
    TextInput::new(session, InputLimits::default()).unwrap()
}

fn flat(saved: &ImportResult) -> String {
    saved
        .document
        .body()
        .blocks
        .iter()
        .map(|block| {
            block
                .as_paragraph()
                .unwrap()
                .plain_text(|_| "\u{FFFC}".into())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn check(input: &TextInput<DocxSession>, original: &[u8], expected: &str) -> Vec<u8> {
    assert_eq!(input.view().index().text(), expected);
    let plan = input.backend().export_plan().unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    assert_eq!(plan.version(), input.backend().session().version());
    assert_eq!(plan.origin_bytes(), Some(original));
    assert_ne!(plan.bytes(), original);
    let saved = import(plan.bytes(), &Budget::default()).unwrap();
    assert_eq!(flat(&saved), expected);
    assert!(!saved.document.validate().has_errors());
    plan.bytes().to_vec()
}

fn caret(input: &mut TextInput<DocxSession>, offset: usize) {
    input.select(offset..offset, false).unwrap();
}

#[test]
fn enter_and_backspace_keep_justification_and_bold() {
    let body = r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>甲乙丙</w:t></w:r></w:p><w:p><w:r><w:rPr><w:i/></w:rPr><w:t>丁戊</w:t></w:r></w:p>"#;
    let bytes = package(body);
    let mut input = open(&bytes);
    caret(&mut input, 1);
    input.replace(None, "\n").unwrap();
    let saved = check(&input, &bytes, "甲\n乙丙\n丁戊");
    let again = import(&saved, &Budget::default()).unwrap();
    let paragraphs: Vec<_> = again
        .document
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph())
        .collect();
    assert_eq!(
        paragraphs[0].props.justification,
        Some(Justification::Center)
    );
    assert_eq!(
        paragraphs[1].props.justification,
        Some(Justification::Center)
    );
    assert_eq!(paragraphs[0].runs()[0].props.bold, Some(true));
    assert_eq!(paragraphs[1].runs()[0].props.bold, Some(true));
    assert_eq!(paragraphs[2].runs()[0].props.italic, Some(true));
    caret(&mut input, 2);
    input.delete(Direction::Backward).unwrap();
    let saved = check(&input, &bytes, "甲乙丙\n丁戊");
    let again = import(&saved, &Budget::default()).unwrap();
    let first = again.document.body().blocks.get(0).unwrap();
    let first = first.as_paragraph().unwrap();
    assert_eq!(first.props.justification, Some(Justification::Center));
    assert_eq!(first.runs()[0].props.bold, Some(true));
    assert_eq!(input.backend().backing().original_bytes(), bytes);
}
