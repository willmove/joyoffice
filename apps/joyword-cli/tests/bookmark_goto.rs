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
use joyword_model::{Anchor, AnnotationKind, Document, StoryKind};
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

#[test]
fn header_footer_and_comment_bookmarks_are_goto_destinations() {
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#;
    let package_rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#;
    let document = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>"#,
            r#"<w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Body</w:t></w:r>"#,
            r#"<w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r>"#,
            r#"<w:r><w:br w:type="page"/></w:r></w:p>"#,
            r#"<w:p><w:hyperlink w:anchor="head"><w:r><w:t>H</w:t></w:r></w:hyperlink>"#,
            r#"<w:hyperlink w:anchor="foot"><w:r><w:t>F</w:t></w:r></w:hyperlink>"#,
            r#"<w:hyperlink w:anchor="note"><w:r><w:t>N</w:t></w:r></w:hyperlink>"#,
            r#"<w:hyperlink w:anchor="HEAD"><w:r><w:t>X</w:t></w:r></w:hyperlink></w:p>"#,
            r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/>"#,
            r#"<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>"#,
            r#"<w:headerReference w:type="default" r:id="rId2"/>"#,
            r#"<w:footerReference w:type="default" r:id="rId3"/></w:sectPr>"#,
            r#"</w:body></w:document>"#,
        ),
        W = W,
        R = R,
    );
    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}"><w:p><w:bookmarkStart w:id="1" w:name="head"/><w:r><w:t>Head</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p></w:hdr>"#
    );
    let footer = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:ftr xmlns:w="{W}"><w:p><w:bookmarkStart w:id="1" w:name="foot"/><w:r><w:t>Foot</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p></w:ftr>"#
    );
    let comments = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{W}"><w:comment w:id="0" w:author="Ada"><w:p><w:bookmarkStart w:id="1" w:name="note"/><w:r><w:t>Note</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p></w:comment></w:comments>"#
    );
    let names = [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/header1.xml",
        "word/footer1.xml",
        "word/comments.xml",
    ];
    let owned = [
        types.as_bytes(),
        package_rels.as_bytes(),
        document.as_bytes(),
        rels.as_bytes(),
        header.as_bytes(),
        footer.as_bytes(),
        comments.as_bytes(),
    ];
    let items: Vec<(&str, &[u8], Method)> = names
        .iter()
        .zip(owned)
        .map(|(name, data)| (*name, data, Method::Stored))
        .collect();
    let imported = import(&write_archive(&items).unwrap(), &Budget::default()).unwrap();
    let document = &imported.document;
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
    assert_eq!(snapshot.pages().len(), 2);
    let painted: String = snapshot.pages()[0]
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect();
    assert!(painted.contains("Head"));
    assert!(painted.contains("Foot"));
    assert!(painted.contains("Body"));
    assert!(!painted.contains("Note"));

    let navigation = navigation_from_document(document, &snapshot).expect("navigation");
    let dest = |name: &str| {
        navigation
            .bookmarks
            .iter()
            .find(|bookmark| bookmark.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .destination
    };
    let head = dest("head");
    let foot = dest("foot");
    let note = dest("note");
    assert_eq!(head.page, 0);
    assert_eq!(foot.page, 0);
    assert_eq!(note.page, 0);
    assert!(head.y < note.y && note.y < foot.y);
    assert_eq!(navigation.unresolved_links, 1);
    assert_eq!(navigation.links.len(), 3);
    for (link, name) in navigation.links.iter().zip(["head", "foot", "note"]) {
        assert_eq!(link.page, 1);
        assert_eq!(link.destination, dest(name));
    }

    let model = |name: &str| {
        document
            .annotations()
            .find(|annotation| {
                matches!(&annotation.kind, AnnotationKind::Bookmark { name: found } if found.as_ref() == name)
            })
            .unwrap_or_else(|| panic!("model {name}"))
    };
    let note_mark = model("note");
    let Anchor::Range { start, .. } = note_mark.anchor else {
        panic!("range");
    };
    assert_eq!(
        document.story(start.story).unwrap().kind,
        StoryKind::Comment
    );
    let comment_anchor = document
        .annotations()
        .find(|annotation| matches!(annotation.kind, AnnotationKind::Comment(_)))
        .expect("comment");
    let Anchor::Range {
        start: anchor_start,
        ..
    } = comment_anchor.anchor
    else {
        panic!("comment range");
    };
    let bookmark_caret = snapshot.caret(start).expect("comment bookmark caret");
    let anchor_caret = snapshot.caret(anchor_start).expect("anchor caret");
    assert_eq!(bookmark_caret.page, anchor_caret.page);
    assert_eq!(bookmark_caret.x, anchor_caret.x);
    assert_eq!(bookmark_caret.top, anchor_caret.top);
    near(note.x, to_points(bookmark_caret.x) as f32);
    near(note.y, to_points(bookmark_caret.top) as f32);

    let pdf = render_export_with_navigation(
        snapshot.pages(),
        &[font],
        &PdfExport::default(),
        &navigation,
    )
    .expect("pdf");
    let read = inspect_navigation(&pdf).expect("readback");
    let page_height = to_points(snapshot.pages()[0].height) as f32;
    for name in ["head", "foot", "note"] {
        let expected = dest(name);
        assert_eq!(read.bookmarks[name].page, expected.page);
        near(read.bookmarks[name].x, expected.x);
        near(read.bookmarks[name].y, page_height - expected.y);
        assert_eq!(read.bookmarks[name].zoom, Some(0.0));
    }
    assert_eq!(read.links.len(), 3);
    assert!(read.links.iter().all(|link| link.source_page == 1));
}

#[test]
fn column_and_opaque_table_bookmarks_are_goto_destinations() {
    // A colFirst/colLast pair in mapped paragraphs imports as an ordinary
    // range bookmark. A pair inside an opaque table has no text range: it
    // becomes a point destination at the end of the nearest preceding
    // paragraph. Both names resolve through layout carets and the PDF
    // readback.
    let body = concat!(
        r#"<w:p><w:hyperlink w:anchor="col"><w:r><w:t>C</w:t></w:r></w:hyperlink>"#,
        r#"<w:hyperlink w:anchor="cell"><w:r><w:t>T</w:t></w:r></w:hyperlink></w:p>"#,
        r#"<w:p><w:bookmarkStart w:id="1" w:name="col" w:colFirst="0" w:colLast="0"/>"#,
        r#"<w:r><w:t>Col</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkStart w:id="2" w:name="cell"/>"#,
        r#"<w:r><w:t>Cell</w:t></w:r><w:bookmarkEnd w:id="2"/></w:p></w:tc></w:tr></w:tbl>"#,
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
    let dest = |name: &str| {
        navigation
            .bookmarks
            .iter()
            .find(|bookmark| bookmark.name == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .destination
    };
    let model = |name: &str| {
        document
            .annotations()
            .find(|annotation| {
                matches!(&annotation.kind, AnnotationKind::Bookmark { name: found } if found.as_ref() == name)
            })
            .unwrap_or_else(|| panic!("model {name}"))
    };
    // The column bookmark is a real range; the opaque-table bookmark is a
    // point at the end of the paragraph that precedes the table.
    let col = model("col");
    let Anchor::Range {
        start: col_start, ..
    } = col.anchor
    else {
        panic!("range");
    };
    let cell = model("cell");
    assert!(cell.source.is_none());
    let Anchor::Range {
        start: cell_start, ..
    } = cell.anchor
    else {
        panic!("range");
    };
    assert_eq!(col_start.paragraph, cell_start.paragraph);
    assert_eq!(cell_start.offset.0, 3);
    let col_caret = snapshot.caret(col_start).expect("col caret");
    let cell_caret = snapshot.caret(cell_start).expect("cell caret");
    near(dest("col").x, to_points(col_caret.x) as f32);
    near(dest("col").y, to_points(col_caret.top) as f32);
    near(dest("cell").x, to_points(cell_caret.x) as f32);
    near(dest("cell").y, to_points(cell_caret.top) as f32);
    assert_eq!(dest("col").page, col_caret.page);
    assert_eq!(dest("cell").page, cell_caret.page);
    assert_ne!(dest("col"), dest("cell"));
    assert_eq!(navigation.unresolved_links, 0);
    assert_eq!(navigation.links.len(), 2);
    assert!(navigation.links.iter().all(|link| link.page == 0));

    let pdf = render_export_with_navigation(
        snapshot.pages(),
        &[font],
        &PdfExport::default(),
        &navigation,
    )
    .expect("pdf");
    let read = inspect_navigation(&pdf).expect("readback");
    let page_height = to_points(snapshot.pages()[0].height) as f32;
    for name in ["col", "cell"] {
        let expected = dest(name);
        assert_eq!(read.bookmarks[name].page, expected.page);
        near(read.bookmarks[name].x, expected.x);
        near(read.bookmarks[name].y, page_height - expected.y);
        assert_eq!(read.bookmarks[name].zoom, Some(0.0));
    }
    assert_eq!(read.links.len(), 2);
    assert!(read.links.iter().all(|link| link.source_page == 0));
}
