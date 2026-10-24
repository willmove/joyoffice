//! DOCX bookmark import (ECMA-376 Part 1 §17.13.6.1 / §17.13.6.2).
//!
//! Body ranges become model bookmarks the PDF named-destination path can
//! resolve. Names are kept exactly. Markers that PDF would refuse, or that
//! are not body carets, are diagnosed and not inserted. Marker bytes stay in
//! the package and lock the paragraph; this slice does not write them back.

use std::sync::Arc;

use joyoffice_opc::{Budget, Method, Package, write_archive};
use joyword_docx::{
    DiagnosticKind, ExportError, ExportMode, ImportResult, Severity, import, plan_export,
};
use joyword_model::{
    Anchor, Annotation, AnnotationKind, Bias, Block, Document, Paragraph, ScalarOffset,
};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

fn archive(parts: &[(&str, &str)]) -> Vec<u8> {
    let stored: Vec<(&str, &[u8], Method)> = parts
        .iter()
        .map(|(name, data)| (*name, data.as_bytes(), Method::Deflated))
        .collect();
    write_archive(&stored).unwrap()
}

fn types(overrides: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{overrides}</Types>"#
    )
}

fn package_rels() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#
}

fn document(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
    )
}

fn main_types() -> String {
    types(
        r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#,
    )
}

fn package(body: &str) -> Vec<u8> {
    archive(&[
        ("[Content_Types].xml", &main_types()),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", &document(body)),
    ])
}

fn open(bytes: &[u8]) -> ImportResult {
    let result = import(bytes, &Budget::default()).unwrap();
    let validation = result.document.validate();
    assert!(
        !validation.has_errors(),
        "{:?}",
        validation.iter().collect::<Vec<_>>()
    );
    result
}

fn paragraphs(result: &ImportResult) -> Vec<&Paragraph> {
    result
        .document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect()
}

fn texts(result: &ImportResult) -> Vec<String> {
    paragraphs(result)
        .iter()
        .map(|paragraph| paragraph.plain_text(|_| "\u{FFFC}".into()))
        .collect()
}

fn bookmark<'a>(document: &'a Document, name: &str) -> &'a Annotation {
    document
        .annotations()
        .find(|annotation| {
            matches!(&annotation.kind, AnnotationKind::Bookmark { name: found } if found.as_ref() == name)
        })
        .unwrap_or_else(|| panic!("missing bookmark {name}"))
}

fn names(document: &Document) -> Vec<String> {
    document
        .annotations()
        .filter_map(|annotation| match &annotation.kind {
            AnnotationKind::Bookmark { name } => Some(name.to_string()),
            _ => None,
        })
        .collect()
}

fn has(result: &ImportResult, kind: &DiagnosticKind) -> bool {
    result.diagnostics.iter().any(|item| &item.kind == kind)
}

fn part_text(bytes: &[u8], name: &str) -> String {
    let package = Package::read(bytes, &Budget::default()).unwrap();
    String::from_utf8(package.part(name).unwrap().data.to_vec()).unwrap()
}

fn body_paragraph(document: &mut Document, index: usize) -> &mut Paragraph {
    let body = document.body_id();
    let block = document
        .story_mut(body)
        .unwrap()
        .blocks
        .get_mut(index)
        .unwrap();
    match block {
        Block::Paragraph(paragraph) => Arc::make_mut(paragraph),
        _ => panic!("paragraph"),
    }
}

#[test]
fn a_body_bookmark_keeps_its_name_and_range() {
    let body = concat!(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="书签" w:displacedByCustomXml="next"/>"#,
        r#"<w:r><w:t>甲</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>乙</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
        r#"<w:p><w:r><w:bookmarkStart w:id="2" w:name="inrun"/><w:t>文</w:t>"#,
        r#"<w:bookmarkEnd w:id="2"/></w:r></w:p>"#,
        r#"<w:p><w:bookmarkStart w:id="3" w:name="point"/><w:bookmarkEnd w:id="3"/>"#,
        r#"<w:r><w:t>Tail</w:t></w:r></w:p>"#,
        r#"<w:p><w:hyperlink w:anchor="书签"><w:bookmarkStart w:id="4" w:name="Inside"/>"#,
        r#"<w:r><w:t>Go</w:t></w:r><w:bookmarkEnd w:id="4"/></w:hyperlink></w:p>"#,
    );
    let bytes = package(body);
    let result = open(&bytes);
    assert_eq!(texts(&result), ["甲", "乙", "文", "Tail", "Go"]);
    assert!(
        result
            .backing
            .fragments()
            .filter(|(_, fragment)| fragment.element.contains("bookmark"))
            .count()
            >= 8
    );
    assert!(has(
        &result,
        &DiagnosticKind::BookmarksPreserved { count: 8 }
    ));
    for paragraph in paragraphs(&result) {
        assert!(!result.editability.can_edit(paragraph.id()));
    }

    let body_id = result.document.body().id;
    let paras = paragraphs(&result);
    let cross = bookmark(&result.document, "书签");
    let Anchor::Range { start, end } = cross.anchor else {
        panic!("range");
    };
    assert_eq!(start.story, body_id);
    assert_eq!(start.paragraph, paras[0].id());
    assert_eq!(start.offset, ScalarOffset(0));
    assert_eq!(start.bias, Bias::After);
    assert_eq!(end.paragraph, paras[1].id());
    assert_eq!(end.offset, ScalarOffset(1));
    assert_eq!(end.bias, Bias::Before);
    assert_eq!(
        cross.source.as_ref().map(|source| source.part.as_ref()),
        Some("/word/document.xml")
    );

    let inside_run = bookmark(&result.document, "inrun");
    let Anchor::Range { start, end } = inside_run.anchor else {
        panic!("range");
    };
    assert_eq!(start.paragraph, paras[2].id());
    assert_eq!((start.offset.0, end.offset.0), (0, 1));
    assert!(!texts(&result)[2].contains('\u{FFFC}'));

    let point = bookmark(&result.document, "point");
    let Anchor::Range { start, end } = point.anchor else {
        panic!("range");
    };
    assert_eq!(start.paragraph, paras[3].id());
    assert_eq!((start.offset.0, end.offset.0), (0, 0));

    let link = paras[4].link_at(0).expect("anchor link");
    assert_eq!(link.anchor.as_deref(), Some("书签"));
    assert!(link.external.is_none());
    let inside = bookmark(&result.document, "Inside");
    let Anchor::Range { start, end } = inside.anchor else {
        panic!("range");
    };
    assert_eq!(start.paragraph, paras[4].id());
    assert_eq!((start.offset.0, end.offset.0), (0, 2));

    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(plan.mode(), ExportMode::OriginalPackage);
    assert_eq!(plan.bytes(), bytes.as_slice());
    assert!(part_text(plan.bytes(), "/word/document.xml").contains("w:bookmarkStart"));
}

#[test]
fn bookmark_names_keep_case_and_reject_empty_duplicates_and_overlong_values() {
    let long = "n".repeat(2049);
    let body = format!(
        concat!(
            r#"<w:p>"#,
            r#"<w:bookmarkStart w:id="1" w:name="Go"/><w:bookmarkEnd w:id="1"/>"#,
            r#"<w:bookmarkStart w:id="2" w:name="go"/><w:bookmarkEnd w:id="2"/>"#,
            r#"<w:bookmarkStart w:id="3" w:name="Go"/><w:bookmarkEnd w:id="3"/>"#,
            r#"<w:bookmarkStart w:id="4" w:name=""/><w:bookmarkEnd w:id="4"/>"#,
            r#"<w:bookmarkStart w:id="5" w:name="   "/><w:bookmarkEnd w:id="5"/>"#,
            r#"<w:bookmarkStart w:id="6" w:name="&#9;"/><w:bookmarkEnd w:id="6"/>"#,
            r#"<w:bookmarkStart w:id="7" w:name="a&#9;b"/><w:bookmarkEnd w:id="7"/>"#,
            r#"<w:bookmarkStart w:id="8" w:name="{long}"/><w:bookmarkEnd w:id="8"/>"#,
            r#"<w:bookmarkStart w:id="9" w:name=" Target "/><w:bookmarkEnd w:id="9"/>"#,
            r#"<w:bookmarkStart w:id="10" w:name="short"/><w:bookmarkEnd w:id="10"/>"#,
            r#"<w:r><w:t>x</w:t></w:r></w:p>"#,
        ),
        long = long
    );
    let result = open(&package(&body));
    assert_eq!(names(&result.document), ["Go", "go", " Target ", "short"]);
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkNameDuplicate { id: 3 }
    ));
    for id in [4, 5, 6] {
        assert!(
            has(&result, &DiagnosticKind::BookmarkNameEmpty { id }),
            "empty name {id}"
        );
    }
    // `&#9;` is the tab character, not a space. Alone it trims to empty.
    // Between other characters it is a control character.
    assert!(has(&result, &DiagnosticKind::BookmarkNameInvalid { id: 7 }));
    assert!(has(&result, &DiagnosticKind::BookmarkBudget { id: 8 }));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|item| item.severity != Severity::Error)
    );
}

#[test]
fn detached_column_table_and_header_bookmarks_are_not_destinations() {
    let body = concat!(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="open"/><w:r><w:t>A</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>A</w:t></w:r><w:bookmarkEnd w:id="2"/>"#,
        r#"<w:r><w:t>B</w:t></w:r><w:bookmarkStart w:id="2" w:name="rev"/></w:p>"#,
        r#"<w:p><w:bookmarkStart w:id="3" w:name="col" w:colFirst="0" w:colLast="0"/>"#,
        r#"<w:r><w:t>C</w:t></w:r><w:bookmarkEnd w:id="3"/></w:p>"#,
        r#"<w:p><w:bookmarkStart w:name="noid"/><w:bookmarkEnd/>"#,
        r#"<w:bookmarkStart w:id="4" w:name="bad" w:foo="1"/><w:bookmarkEnd w:id="4"/></w:p>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkStart w:id="5" w:name="cell"/>"#,
        r#"<w:r><w:t>T</w:t></w:r><w:bookmarkEnd w:id="5"/></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:p><w:bookmarkStart w:id="6" w:name="kept"/><w:r><w:t>K</w:t></w:r>"#,
        r#"<w:bookmarkEnd w:id="6"/></w:p>"#,
    );
    let result = open(&package(body));
    assert_eq!(names(&result.document), ["kept"]);
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 1 }
    ));
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 2 }
    ));
    assert!(has(&result, &DiagnosticKind::BookmarkColumn { id: 3 }));
    assert!(
        result
            .diagnostics
            .iter()
            .filter(|item| item.kind == DiagnosticKind::BookmarkIdMissing)
            .count()
            >= 2
    );
    assert!(has(&result, &DiagnosticKind::BookmarkUnsupported { id: 4 }));
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorUnmapped { id: 5 }
    ));
    assert!(
        result
            .document
            .body()
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Opaque(_)))
    );

    let header = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}"><w:p><w:bookmarkStart w:id="2" w:name="head"/><w:r><w:t>Head</w:t></w:r><w:bookmarkEnd w:id="2"/></w:p></w:hdr>"#
    );
    let sect = r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:headerReference w:type="default" r:id="rId2"/></w:sectPr>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#;
    let header_bytes = archive(&[
        (
            "[Content_Types].xml",
            &types(
                r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#,
            ),
        ),
        ("_rels/.rels", package_rels()),
        (
            "word/document.xml",
            &document(&format!(
                r#"<w:p><w:bookmarkStart w:id="1" w:name="body"/><w:r><w:t>Body</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>{sect}"#
            )),
        ),
        ("word/_rels/document.xml.rels", rels),
        ("word/header1.xml", &header),
    ]);
    let header_result = open(&header_bytes);
    assert_eq!(names(&header_result.document), ["body"]);
    assert!(has(
        &header_result,
        &DiagnosticKind::BookmarkOutsideBody { id: 2 }
    ));

    let comments = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{W}"><w:comment w:id="0" w:author="Ada"><w:p><w:bookmarkStart w:id="9" w:name="note"/><w:r><w:t>Note</w:t></w:r><w:bookmarkEnd w:id="9"/></w:p></w:comment></w:comments>"#
    );
    let comment_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#
    );
    let comment_bytes = archive(&[
        (
            "[Content_Types].xml",
            &types(
                r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/>"#,
            ),
        ),
        ("_rels/.rels", package_rels()),
        (
            "word/document.xml",
            &document(r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>"#),
        ),
        ("word/_rels/document.xml.rels", &comment_rels),
        ("word/comments.xml", &comments),
    ]);
    let comment_result = open(&comment_bytes);
    assert!(names(&comment_result.document).is_empty());
    assert!(has(
        &comment_result,
        &DiagnosticKind::BookmarkOutsideBody { id: 9 }
    ));
}

#[test]
fn the_pdf_bookmark_budget_keeps_the_first_names() {
    let mut body = String::from("<w:p>");
    for id in 0..=512 {
        body.push_str(&format!(
            r#"<w:bookmarkStart w:id="{id}" w:name="b{id}"/><w:bookmarkEnd w:id="{id}"/>"#
        ));
    }
    body.push_str("<w:r><w:t>x</w:t></w:r></w:p>");
    let result = open(&package(&body));
    let imported = names(&result.document);
    assert_eq!(imported.len(), 512);
    assert_eq!(imported.first().map(String::as_str), Some("b0"));
    assert_eq!(imported.last().map(String::as_str), Some("b511"));
    assert!(!imported.iter().any(|name| name == "b512"));
    assert!(has(&result, &DiagnosticKind::BookmarkBudget { id: 512 }));
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|item| matches!(item.kind, DiagnosticKind::BookmarkBudget { .. }))
            .count(),
        1
    );
}

#[test]
fn an_edited_export_refuses_to_drop_bookmark_markup() {
    let body = concat!(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="mark"/><w:r><w:t>甲</w:t></w:r>"#,
        r#"<w:bookmarkEnd w:id="1"/></w:p><w:p><w:r><w:t>乙</w:t></w:r></w:p>"#,
    );
    let bytes = package(body);
    let result = open(&bytes);
    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(plan.mode(), ExportMode::OriginalPackage);
    assert_eq!(plan.bytes(), bytes.as_slice());
    assert!(part_text(plan.bytes(), "/word/document.xml").contains("w:bookmarkStart"));

    let mut current = result.document.clone();
    body_paragraph(&mut current, 1)
        .insert_text(ScalarOffset(0), "X")
        .unwrap();
    assert!(matches!(
        plan_export(&result.backing, &current.snapshot(), &Budget::default()),
        Err(ExportError::Restricted)
    ));

    let mut renamed = result.document.clone();
    let id = bookmark(&renamed, "mark").id;
    match &mut renamed.annotation_mut(id).unwrap().kind {
        AnnotationKind::Bookmark { name } => *name = Arc::from("other"),
        _ => panic!("bookmark"),
    }
    assert!(matches!(
        plan_export(&result.backing, &renamed.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    let mut detached = result.document.clone();
    detached.annotation_mut(id).unwrap().anchor = Anchor::Detached;
    assert!(matches!(
        plan_export(&result.backing, &detached.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));
}
