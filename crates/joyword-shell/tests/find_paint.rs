//! Find, replace and the format painter through the command registry.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::{Method, write_archive};
use joyword_shell::command::{
    ArgValue, EDIT_BOLD, FILE_OPEN, FIND_NEXT, FIND_REPLACE, FIND_REPLACE_ALL, FORMAT_COPY,
    FORMAT_PAINT,
};
use joyword_shell::host::Office;

fn fixture(id: u8) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn two_paragraphs() -> Vec<u8> {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let package_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>甲甲</w:t></w:r></w:p><w:p><w:r><w:t>乙乙</w:t></w:r></w:p></w:body></w:document>"#;
    let items = [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", package_rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    let stored: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, *data, Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

fn width_and_regex_doc() -> Vec<u8> {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let package_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>ＡＢＣ 12 dog 34</w:t></w:r></w:p></w:body></w:document>"#;
    let items = [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", package_rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    let stored: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, *data, Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

fn open_bytes(bytes: &[u8], name: &str) -> (tempfile::TempDir, Office) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join(name);
    fs::write(&source, bytes).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
        .unwrap();
    (dir, office)
}

fn paragraph(office: &Office, index: usize) -> &joyword_model::Paragraph {
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .get(index)
        .unwrap()
        .as_paragraph()
        .unwrap()
}

#[test]
fn find_replace_and_format_painter_go_through_the_command_registry() {
    let (_contract_dir, mut office) = open_bytes(&fixture(1), "contract.docx");
    office
        .invoke(FIND_NEXT, &[("query", ArgValue::Text("甲方".into()))])
        .unwrap();
    assert_eq!(office.active().unwrap().selected_text().unwrap(), "甲方");
    office
        .invoke(
            FIND_REPLACE_ALL,
            &[
                ("query", ArgValue::Text("方".into())),
                ("replacement", ArgValue::Text("侧".into())),
            ],
        )
        .unwrap();
    assert_eq!(
        paragraph(&office, 0).plain_text(|_| String::new()),
        "合作协议：甲侧与乙侧于 2026 年签署。"
    );
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(
        paragraph(&office, 0).plain_text(|_| String::new()),
        "合作协议：甲方与乙方于 2026 年签署。"
    );

    let (_heading_dir, styled) = open_bytes(&fixture(2), "heading.docx");
    let rows = styled.active().unwrap().style_rows();
    let heading = rows
        .iter()
        .find(|row| row.id == "1")
        .expect("heading style");
    assert!(heading.preview.contains("bold"), "{}", heading.preview);
    assert!(heading.preview.contains("32hp"), "{}", heading.preview);
    assert!(heading.preview.contains("宋体"), "{}", heading.preview);
    assert_eq!(
        styled.active().unwrap().caret_styles().0.as_deref(),
        Some("1")
    );

    let (_plain_dir, mut painter) = open_bytes(&two_paragraphs(), "plain.docx");
    let joined = painter.active().unwrap().view().index().text().to_owned();
    let split_byte = joined.find('\n').unwrap();
    let split = joined[..split_byte].encode_utf16().count();
    painter
        .active_mut()
        .unwrap()
        .select(0..split, false)
        .unwrap();
    painter.invoke(EDIT_BOLD, &[]).unwrap();
    painter.invoke(FORMAT_COPY, &[]).unwrap();
    assert!(painter.active().unwrap().format_armed());
    let end = painter.active().unwrap().view().index().len_utf16();
    painter
        .active_mut()
        .unwrap()
        .select(split + 1..end, false)
        .unwrap();
    painter.invoke(FORMAT_PAINT, &[]).unwrap();
    assert!(!painter.active().unwrap().format_armed());
    assert!(
        paragraph(&painter, 1)
            .runs()
            .iter()
            .any(|run| run.props.bold == Some(true))
    );
    painter.active_mut().unwrap().undo().unwrap();
    assert!(
        paragraph(&painter, 1)
            .runs()
            .iter()
            .all(|run| run.props.bold.is_none())
    );
    assert!(
        paragraph(&painter, 0)
            .runs()
            .iter()
            .any(|run| run.props.bold == Some(true))
    );
}

#[test]
fn find_width_and_regex_flags_go_through_the_command_registry() {
    let (_dir, mut office) = open_bytes(&width_and_regex_doc(), "wide.docx");
    office
        .invoke(
            FIND_NEXT,
            &[
                ("query", ArgValue::Text("ABC".into())),
                ("width", ArgValue::Bool(true)),
            ],
        )
        .unwrap();
    assert_eq!(office.active().unwrap().selected_text().unwrap(), "ＡＢＣ");

    // Without the width flag the same half-width query finds nothing and the
    // selection stays put.
    office
        .invoke(FIND_NEXT, &[("query", ArgValue::Text("ABC".into()))])
        .unwrap();
    assert_eq!(office.active().unwrap().selected_text().unwrap(), "ＡＢＣ");

    office
        .invoke(
            FIND_REPLACE_ALL,
            &[
                ("query", ArgValue::Text(r"\d+".into())),
                ("replacement", ArgValue::Text("#".into())),
                ("regex", ArgValue::Bool(true)),
            ],
        )
        .unwrap();
    assert_eq!(
        paragraph(&office, 0).plain_text(|_| String::new()),
        "ＡＢＣ # dog #"
    );
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(
        paragraph(&office, 0).plain_text(|_| String::new()),
        "ＡＢＣ 12 dog 34"
    );

    let failed = office.invoke(
        FIND_NEXT,
        &[
            ("query", ArgValue::Text("(".into())),
            ("regex", ArgValue::Bool(true)),
        ],
    );
    assert!(failed.is_err(), "an invalid pattern is rejected");
}

fn capture_doc() -> Vec<u8> {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let package_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>aaabbb</w:t></w:r></w:p></w:body></w:document>"#;
    let items = [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", package_rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    let stored: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, *data, Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

#[test]
fn regex_capture_replacement_goes_through_the_command_registry() {
    let (_dir, mut office) = open_bytes(&capture_doc(), "groups.docx");
    office
        .invoke(
            FIND_REPLACE_ALL,
            &[
                ("query", ArgValue::Text("(a+)(b+)".into())),
                ("replacement", ArgValue::Text("$2-$1".into())),
                ("regex", ArgValue::Bool(true)),
            ],
        )
        .unwrap();
    assert_eq!(
        paragraph(&office, 0).plain_text(|_| String::new()),
        "bbb-aaa"
    );
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(
        paragraph(&office, 0).plain_text(|_| String::new()),
        "aaabbb"
    );

    office
        .invoke(
            FIND_REPLACE_ALL,
            &[
                ("query", ArgValue::Text("aaabbb".into())),
                ("replacement", ArgValue::Text("$1".into())),
            ],
        )
        .unwrap();
    assert_eq!(paragraph(&office, 0).plain_text(|_| String::new()), "$1");
    office.active_mut().unwrap().undo().unwrap();

    office
        .invoke(
            FIND_REPLACE,
            &[
                ("query", ArgValue::Text("(a+)(b+)".into())),
                ("replacement", ArgValue::Text("$$".into())),
                ("regex", ArgValue::Bool(true)),
            ],
        )
        .unwrap();
    assert_eq!(office.active().unwrap().selected_text().unwrap(), "$");
    assert_eq!(paragraph(&office, 0).plain_text(|_| String::new()), "$");
}
