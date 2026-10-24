//! An imported DOCX bookmark is a PDF named destination and an anchor link
//! to that exact name is `/S /GoTo`. Heading-style outline bookmarks are a
//! separate path and are not exercised here.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, write_archive};
use joyword_docx::{MAX_IMPORTED_BOOKMARK_NAME_BYTES, MAX_IMPORTED_BOOKMARKS, import};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document, to_points,
};
use joyword_model::{Anchor, AnnotationKind, Document};
use joyword_pdf::{
    MAX_PDF_BOOKMARKS, MAX_PDF_NAME_BYTES, PdfExport, inspect_navigation, navigation_from_document,
    render_export_with_navigation,
};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn package(body: &str) -> Vec<u8> {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    );
    let names = ["[Content_Types].xml", "_rels/.rels", "word/document.xml"];
    let owned = [types.as_bytes(), rels.as_bytes(), document.as_bytes()];
    let items: Vec<(&str, &[u8], Method)> = names
        .iter()
        .zip(owned)
        .map(|(name, data)| (*name, data, Method::Stored))
        .collect();
    write_archive(&items).unwrap()
}

fn test_font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .expect("committed test face")
}

fn near(left: f32, right: f32) {
    assert!((left - right).abs() < 0.002, "{left} vs {right}");
}

#[test]
fn an_imported_bookmark_is_a_pdf_goto_named_destination() {
    assert_eq!(MAX_IMPORTED_BOOKMARKS, MAX_PDF_BOOKMARKS);
    assert_eq!(MAX_IMPORTED_BOOKMARK_NAME_BYTES, MAX_PDF_NAME_BYTES);
    let body = concat!(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="Target"/><w:r><w:t>Here</w:t></w:r>"#,
        r#"<w:bookmarkEnd w:id="1"/></w:p>"#,
        r#"<w:p><w:bookmarkStart w:id="2" w:name="Spare"/><w:bookmarkEnd w:id="2"/>"#,
        r#"<w:r><w:t>Tail</w:t></w:r></w:p>"#,
        r#"<w:p><w:hyperlink w:anchor="Target"><w:r><w:t>Go</w:t></w:r></w:hyperlink></w:p>"#,
        r#"<w:p><w:hyperlink w:anchor="target"><w:r><w:t>No</w:t></w:r></w:hyperlink></w:p>"#,
    );
    let imported = import(&package(body), &Budget::default()).unwrap();
    let document: &Document = &imported.document;
    assert!(!document.validate().has_errors());
    let font = test_font();
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font),
        SingleFontProfile {
            size: Emu::new(12 * Emu::PER_POINT),
            leading: Emu::new(18 * Emu::PER_POINT),
            baseline: Emu::new(14 * Emu::PER_POINT),
            features: FeatureSet {
                kerning: false,
                standard_ligatures: false,
                contextual_alternates: false,
            },
        },
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .expect("layout");
    assert_eq!(snapshot.pages().len(), 1);

    let navigation = navigation_from_document(document, &snapshot).expect("navigation");
    let target = navigation
        .bookmarks
        .iter()
        .find(|bookmark| bookmark.name == "Target")
        .expect("Target");
    assert!(
        navigation
            .bookmarks
            .iter()
            .any(|bookmark| bookmark.name == "Spare")
    );
    assert!(
        !navigation
            .bookmarks
            .iter()
            .any(|bookmark| bookmark.name == "target")
    );
    assert_eq!(navigation.unresolved_links, 1);
    assert_eq!(navigation.links.len(), 1);
    assert_eq!(navigation.links[0].destination, target.destination);

    let anchor = document
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Bookmark { name } if name.as_ref() == "Target")
        })
        .expect("model bookmark");
    let Anchor::Range { start, .. } = anchor.anchor else {
        panic!("range");
    };
    let caret = snapshot.caret(start).expect("caret");
    near(target.destination.x, to_points(caret.x) as f32);
    near(target.destination.y, to_points(caret.top) as f32);
    assert_eq!(target.destination.page, caret.page);

    let pdf = render_export_with_navigation(
        snapshot.pages(),
        &[font],
        &PdfExport::default(),
        &navigation,
    )
    .expect("pdf");
    let read = inspect_navigation(&pdf).expect("readback");
    assert_eq!(read.links.len(), 1);
    assert_eq!(read.bookmarks["Target"], read.links[0].destination);
    assert!(read.bookmarks.contains_key("Spare"));
    assert!(!read.bookmarks.contains_key("target"));
    let page_height = to_points(snapshot.pages()[caret.page].height) as f32;
    near(read.bookmarks["Target"].x, to_points(caret.x) as f32);
    near(
        read.bookmarks["Target"].y,
        page_height - to_points(caret.top) as f32,
    );
    assert_eq!(read.bookmarks["Target"].zoom, Some(0.0));
    assert_eq!(read.links[0].source_page, 0);
}
