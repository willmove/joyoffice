//! T201: table commands go through the registry and a saved copy reopens.

use std::fs;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::import;
use joyword_model::Block;
use joyword_shell::command::{
    ArgValue, EDIT_INSERT, FILE_OPEN, FILE_SAVE_COPY, InvokeError, TABLE_COLUMN_DELETE,
    TABLE_COLUMN_INSERT, TABLE_INSERT, TABLE_ROW_DELETE, TABLE_ROW_INSERT, TABLE_SELECT,
};
use joyword_shell::host::Office;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn package(body: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#
    );
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let items = [
        ("[Content_Types].xml", types),
        ("_rels/.rels", rels),
        ("word/document.xml", document.as_str()),
    ];
    let owned: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&owned).unwrap()
}

fn open(body: &str) -> (tempfile::TempDir, Office) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.docx");
    fs::write(&source, package(body)).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
        .unwrap();
    (dir, office)
}

fn rows(office: &Office) -> usize {
    match office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .get(0)
    {
        Some(Block::Table(table)) => table.rows.len(),
        _ => 0,
    }
}

#[test]
fn table_commands_go_through_the_command_registry() {
    let (dir, mut office) = open(r#"<w:p><w:r><w:t>正文</w:t></w:r></w:p>"#);
    office.invoke(TABLE_INSERT, &[]).unwrap();
    assert_eq!(rows(&office), 2);
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("甲".into()))])
        .unwrap();
    office.invoke(TABLE_SELECT, &[]).unwrap();
    assert_eq!(office.active().unwrap().selected_text().unwrap(), "甲");
    office.invoke(TABLE_ROW_INSERT, &[]).unwrap();
    assert_eq!(rows(&office), 3);
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(rows(&office), 2);
    office.invoke(TABLE_COLUMN_INSERT, &[]).unwrap();
    office.invoke(TABLE_COLUMN_DELETE, &[]).unwrap();
    office.invoke(TABLE_ROW_DELETE, &[]).unwrap();
    assert_eq!(rows(&office), 1);
    let error = office.invoke(TABLE_ROW_DELETE, &[]).unwrap_err();
    assert!(matches!(error, InvokeError::Failed(_)), "{error:?}");

    let copy = dir.path().join("copy.docx");
    office
        .invoke(
            FILE_SAVE_COPY,
            &[("destination", ArgValue::Path(copy.clone()))],
        )
        .unwrap();
    let saved = import(&fs::read(copy).unwrap(), &Budget::default()).unwrap();
    let current = office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document();
    assert_eq!(current.semantic_difference(&saved.document), None);
    assert!(matches!(
        saved.document.body().blocks.get(0),
        Some(Block::Table(table)) if table.rows.len() == 1 && table.grid.len() == 2
    ));
}

#[test]
fn column_commands_refuse_a_merged_table() {
    let (_dir, mut office) = open(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="1500"/><w:gridCol w:w="1500"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>合</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:tcPr><w:gridSpan w:val="2"/><w:vMerge/></w:tcPr><w:p/></w:tc></w:tr></w:tbl>"#,
    );
    let error = office.invoke(TABLE_COLUMN_INSERT, &[]).unwrap_err();
    assert!(matches!(error, InvokeError::Failed(_)), "{error:?}");
    assert_eq!(rows(&office), 2);
}
