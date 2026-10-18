//! M1 edit–save–reopen through the command registry and the bound DOCX session.

use std::fs;
use std::path::PathBuf;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_engine::RunChange;
use joyword_model::{BreakKind, InlineKind, Justification, Measure, NumberingRef, Orientation};
use joyword_shell::command::{
    ArgValue, EDIT_ALIGN_CENTER, EDIT_BOLD, EDIT_COPY, EDIT_INSERT, EDIT_PASTE, EDIT_SELECT_ALL,
    FILE_OPEN, FILE_SAVE_COPY, InvokeError, LIST_APPLY, LIST_LEVEL, LIST_REMOVE, PAGE_BREAK,
    PAGE_MARGINS, PAGE_ORIENTATION, PAGE_SIZE, STYLE_APPLY, STYLE_LIST,
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

#[test]
fn style_list_and_apply_go_through_the_command_registry() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("styled.docx");
    fs::write(&source, fixture(2)).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
        .unwrap();
    office.invoke(STYLE_LIST, &[]).unwrap();
    let listed = office.listed_styles().join("\n");
    assert!(listed.contains("heading 1"), "{listed}");
    assert!(listed.contains("Normal"), "{listed}");
    office
        .invoke(STYLE_APPLY, &[("style", ArgValue::Text("a".into()))])
        .unwrap();
    let style = office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .style
        .clone();
    assert_eq!(style.unwrap().as_str(), "a");
    office.active_mut().unwrap().undo().unwrap();
    let restored = office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap()
        .style
        .clone();
    assert_eq!(restored.unwrap().as_str(), "1");
}

fn numbering_package() -> Vec<u8> {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#;
    let package_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;
    let numbering = r#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1.%2."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#;
    let document = r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>甲</w:t></w:r></w:p><w:p><w:r><w:t>乙</w:t></w:r></w:p></w:body></w:document>"#;
    let items = [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", package_rels.as_bytes()),
        ("word/document.xml", document.as_bytes()),
        ("word/_rels/document.xml.rels", rels.as_bytes()),
        ("word/numbering.xml", numbering.as_bytes()),
    ];
    let stored: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, *data, Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

fn paragraph_numbering(office: &Office, index: usize) -> Option<NumberingRef> {
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
        .props
        .numbering
}

#[test]
fn list_apply_level_and_remove_go_through_the_command_registry() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("listed.docx");
    fs::write(&source, numbering_package()).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
        .unwrap();
    let missing = office.invoke(LIST_APPLY, &[("num", ArgValue::Text("9".into()))]);
    assert!(missing.is_err(), "{missing:?}");
    office
        .invoke(LIST_APPLY, &[("num", ArgValue::Text("1".into()))])
        .unwrap();
    assert_eq!(
        paragraph_numbering(&office, 0),
        Some(NumberingRef {
            num_id: 1,
            level: 0
        })
    );
    office
        .invoke(LIST_LEVEL, &[("level", ArgValue::Text("1".into()))])
        .unwrap();
    assert_eq!(
        paragraph_numbering(&office, 0),
        Some(NumberingRef {
            num_id: 1,
            level: 1
        })
    );
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(
        paragraph_numbering(&office, 0),
        Some(NumberingRef {
            num_id: 1,
            level: 0
        })
    );
    office.invoke(LIST_REMOVE, &[]).unwrap();
    assert_eq!(
        paragraph_numbering(&office, 0),
        Some(NumberingRef {
            num_id: 0,
            level: 0
        })
    );
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(
        paragraph_numbering(&office, 0),
        Some(NumberingRef {
            num_id: 1,
            level: 0
        })
    );
}

fn section_page(office: &Office) -> joyword_model::PageSetup {
    office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .sections()[0]
        .page
        .clone()
}

#[test]
fn page_setup_and_break_go_through_the_command_registry() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("page.docx");
    fs::write(&source, fixture(1)).unwrap();
    let mut office = Office::new();
    office
        .invoke(FILE_OPEN, &[("path", ArgValue::Path(source))])
        .unwrap();
    let original = section_page(&office);
    office
        .invoke(
            PAGE_SIZE,
            &[
                ("width", ArgValue::Text("10000".into())),
                ("height", ArgValue::Text("14000".into())),
            ],
        )
        .unwrap();
    let sized = section_page(&office);
    assert_eq!(sized.width, Measure::Twips(10_000));
    assert_eq!(sized.height, Measure::Twips(14_000));
    assert_eq!(sized.orientation, original.orientation);
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(section_page(&office).width, original.width);
    assert_eq!(section_page(&office).height, original.height);

    let rejected = office.invoke(
        PAGE_MARGINS,
        &[
            ("top", ArgValue::Text("100".into())),
            ("right", ArgValue::Text("10000".into())),
            ("bottom", ArgValue::Text("100".into())),
            ("left", ArgValue::Text("10000".into())),
        ],
    );
    assert!(rejected.is_err(), "{rejected:?}");
    assert_eq!(section_page(&office).left, original.left);

    office.invoke(PAGE_BREAK, &[]).unwrap();
    let paragraph = office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(
        paragraph
            .inlines()
            .iter()
            .any(|inline| { matches!(inline.kind, InlineKind::Break(BreakKind::Page)) })
    );
    office.active_mut().unwrap().undo().unwrap();
    let restored = office
        .active()
        .unwrap()
        .backend()
        .session()
        .session()
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(
        restored
            .inlines()
            .iter()
            .all(|inline| { !matches!(inline.kind, InlineKind::Break(BreakKind::Page)) })
    );

    let before = section_page(&office);
    office
        .invoke(
            PAGE_ORIENTATION,
            &[("orientation", ArgValue::Text("landscape".into()))],
        )
        .unwrap();
    let landscape = section_page(&office);
    assert_eq!(landscape.orientation, Orientation::Landscape);
    assert_eq!(landscape.width, before.height);
    assert_eq!(landscape.height, before.width);
    office.active_mut().unwrap().undo().unwrap();
    assert_eq!(section_page(&office).orientation, before.orientation);
    assert_eq!(section_page(&office).width, before.width);
}
