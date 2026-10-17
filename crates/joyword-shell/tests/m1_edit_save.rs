//! M1 edit–save–reopen through the command registry and the bound DOCX session.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_engine::RunChange;
use joyword_model::Justification;
use joyword_shell::command::{
    ArgValue, EDIT_ALIGN_CENTER, EDIT_BOLD, EDIT_COPY, EDIT_INSERT, EDIT_PASTE, EDIT_SELECT_ALL,
    FILE_OPEN, FILE_SAVE_COPY, InvokeError,
};
use joyword_shell::host::Office;

fn fixture(id: u8) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
    )
    .unwrap()
}

fn part_bytes(package: &[u8], name_part: &str) -> Vec<u8> {
    let package = Package::read(package, &Budget::default()).unwrap();
    package
        .parts()
        .find(|part| part.name.as_str().contains(name_part))
        .unwrap_or_else(|| panic!("missing {name_part}"))
        .data
        .to_vec()
}

fn with_unknown_part(docx: &[u8]) -> Vec<u8> {
    let package = Package::read(docx, &Budget::default()).unwrap();
    let custom = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><contract xmlns=\"urn:joyoffice:test:custom\"><party>保留的自定义部件</party></contract>";
    let mut owned: Vec<(String, Vec<u8>, Method)> = Vec::new();
    owned.push((
        "[Content_Types].xml".into(),
        package.content_types_xml().to_vec(),
        Method::Stored,
    ));
    for part in package.parts() {
        owned.push((
            part.name.as_str().trim_start_matches('/').to_owned(),
            part.data.to_vec(),
            part.method,
        ));
    }
    owned.push((
        "customXml/item1.xml".into(),
        custom.as_bytes().to_vec(),
        Method::Stored,
    ));
    let refs: Vec<(&str, &[u8], Method)> = owned
        .iter()
        .map(|(name, data, method)| (name.as_str(), data.as_slice(), *method))
        .collect();
    write_archive(&refs).unwrap()
}

#[test]
fn edit_format_clipboard_save_and_reopen_keeps_text_and_unknown_parts() {
    let bytes = with_unknown_part(&fixture(1));
    let custom = part_bytes(&bytes, "customXml");
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("unknown.docx");
    fs::write(&source, &bytes).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source.clone()))])
        .unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("增补".into()))])
        .unwrap();
    office.invoke(EDIT_SELECT_ALL, &[]).unwrap();
    office.invoke(EDIT_BOLD, &[]).unwrap();
    office.invoke(EDIT_ALIGN_CENTER, &[]).unwrap();
    office.invoke(EDIT_COPY, &[]).unwrap();
    let end = office.active().unwrap().view().index().len_utf16();
    office
        .active_mut()
        .unwrap()
        .select(end..end, false)
        .unwrap();
    office.invoke(EDIT_PASTE, &[]).unwrap();

    let copy = dir.path().join("unknown-copy.docx");
    office
        .invoke(
            FILE_SAVE_COPY,
            &[("destination", ArgValue::Path(copy.clone()))],
        )
        .unwrap();
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(part_bytes(&fs::read(&copy).unwrap(), "customXml"), custom);
    let saved = fs::read(&copy).unwrap();
    let document_xml = part_bytes(&saved, "document.xml");
    let document = String::from_utf8_lossy(&document_xml);
    assert!(document.contains("增补"), "{document}");
    assert!(document.contains("w:b"), "{document}");
    assert!(
        document.contains("center") || document.contains("w:jc"),
        "{document}"
    );

    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(copy))])
        .unwrap();
    let reopened = office.active().unwrap();
    let text = reopened.selected_text().unwrap_or_default();
    let _ = text;
    let paragraph = reopened
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .iter()
        .find_map(|block| block.as_paragraph())
        .unwrap();
    let plain = paragraph.plain_text(|_| String::new());
    assert!(plain.contains("增补"), "{plain}");
    assert_eq!(paragraph.props.justification, Some(Justification::Center));
    assert!(
        paragraph
            .runs()
            .iter()
            .any(|run| run.props.bold == Some(true))
    );
    if let Ok(dir) = std::env::var("JOYWORD_M1_KEEP_DIR") {
        fs::write(PathBuf::from(dir).join("m1-saved.docx"), &saved).unwrap();
    }
    let _ = RunChange::Bold(Some(true));
}

#[test]
fn protected_documents_refuse_edits_and_a_failed_save_keeps_bytes() {
    let bytes = fixture(5);
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("protected.docx");
    fs::write(&source, &bytes).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source.clone()))])
        .unwrap();
    let error = office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("改".into()))])
        .unwrap_err();
    assert!(matches!(error, InvokeError::Disabled(_)));
    assert_eq!(fs::read(&source).unwrap(), bytes);

    let editable = fixture(1);
    let open = dir.path().join("plain.docx");
    fs::write(&open, &editable).unwrap();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(open.clone()))])
        .unwrap();
    office
        .invoke(EDIT_INSERT, &[("text", ArgValue::Text("甲".into()))])
        .unwrap();
    let blocked = dir.path().join("plain-copy.docx");
    fs::write(&blocked, b"already").unwrap();
    let error = office
        .invoke(
            FILE_SAVE_COPY,
            &[("destination", ArgValue::Path(blocked.clone()))],
        )
        .unwrap_err();
    assert!(matches!(error, InvokeError::Failed(_)));
    assert_eq!(fs::read(&open).unwrap(), editable);
    assert_eq!(fs::read(&blocked).unwrap(), b"already");
}
