//! T202 column slice import/export: `w:cols/@w:sep` separator flag and
//! `w:type="nextColumn"` section breaks import typed and round-trip.

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{import, plan_export};
use joyword_model::SectionBreak;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const PACKAGE_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const PAGE: &str = r#"<w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1800" w:bottom="1440" w:left="1800" w:header="851" w:footer="992" w:gutter="0"/>"#;

fn archive(items: &[(&str, &str)]) -> Vec<u8> {
    let items: Vec<(&str, &[u8], Method)> = items
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&items).unwrap()
}

fn body(markup: &str) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{markup}</w:body></w:document>"#
    );
    archive(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", PACKAGE_RELS),
        ("word/document.xml", &document),
    ])
}

fn sectpr(inner: &str) -> String {
    format!(r#"<w:p><w:r><w:t>A</w:t></w:r></w:p><w:sectPr>{PAGE}{inner}</w:sectPr>"#)
}

fn exported_document_xml(result: &joyword_docx::ImportResult) -> String {
    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    let package = joyoffice_opc::Package::read(plan.bytes(), &Budget::default()).unwrap();
    std::str::from_utf8(package.part("/word/document.xml").unwrap().data.as_ref())
        .unwrap()
        .to_owned()
}

#[test]
fn equal_column_separator_imports_typed_and_round_trips() {
    let result = import(
        &body(&sectpr(r#"<w:cols w:num="2" w:sep="true"/>"#)),
        &Budget::default(),
    )
    .unwrap();
    assert!(!result.document.validate().has_errors());
    let section = &result.document.sections()[0];
    assert!(section.preserved.is_none());
    assert!(section.column_separator);
    let columns = section.columns.unwrap();
    assert_eq!(columns.count, 2);
    assert!(result.editability.is_unrestricted());
    // The separator attribute survives an unchanged export byte for byte.
    assert!(exported_document_xml(&result).contains(r#"w:sep="true""#));
}

#[test]
fn unequal_column_separator_imports_typed() {
    let markup =
        r#"<w:cols w:equalWidth="0" w:sep="1"><w:col w:w="2000"/><w:col w:w="4000"/></w:cols>"#;
    let result = import(&body(&sectpr(markup)), &Budget::default()).unwrap();
    let section = &result.document.sections()[0];
    assert!(section.preserved.is_none());
    assert!(section.column_separator);
    assert!(section.columns.is_none());
    let unequal = section.unequal_columns.as_ref().unwrap();
    assert_eq!(unequal.columns.len(), 2);
    assert_eq!(unequal.columns[0].width, 2000);
    assert_eq!(unequal.columns[1].width, 4000);
    assert!(exported_document_xml(&result).contains(r#"w:sep="1""#));
}

#[test]
fn separator_false_or_omitted_is_not_flagged() {
    for markup in [r#"<w:cols w:num="2"/>"#, r#"<w:cols w:num="2" w:sep="0"/>"#] {
        let result = import(&body(&sectpr(markup)), &Budget::default()).unwrap();
        let section = &result.document.sections()[0];
        assert!(section.preserved.is_none(), "{markup}");
        assert!(!section.column_separator, "{markup}");
        assert_eq!(section.columns.unwrap().count, 2, "{markup}");
    }
}

#[test]
fn invalid_separator_value_stays_preserved() {
    let markup = r#"<w:cols w:num="2" w:sep="maybe"/>"#;
    let result = import(&body(&sectpr(markup)), &Budget::default()).unwrap();
    let section = &result.document.sections()[0];
    assert!(section.preserved.is_some(), "{markup}");
    assert!(section.columns.is_none());
    assert!(!section.column_separator);
    // Preserved markup is copied through unchanged.
    assert!(exported_document_xml(&result).contains(r#"w:sep="maybe""#));
}

#[test]
fn next_column_section_break_imports_typed_and_round_trips() {
    let markup = r#"<w:type w:val="nextColumn"/><w:cols w:num="2"/>"#;
    let result = import(&body(&sectpr(markup)), &Budget::default()).unwrap();
    let section = &result.document.sections()[0];
    assert!(section.preserved.is_none());
    assert_eq!(section.break_kind, SectionBreak::NextColumn);
    assert_eq!(section.columns.unwrap().count, 2);
    assert!(result.editability.is_unrestricted());
    assert!(exported_document_xml(&result).contains("nextColumn"));
}
