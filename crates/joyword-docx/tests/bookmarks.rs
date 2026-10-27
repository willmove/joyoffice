//! DOCX bookmark import (ECMA-376 Part 1 §17.13.6.1 / §17.13.6.2).
//!
//! Body, header, footer, and placeable comment ranges become model bookmarks
//! the PDF named-destination path can resolve. Names are kept exactly.
//! Markers that PDF would refuse, or that have no page caret (a detached
//! comment), are diagnosed and not inserted. A `colFirst` / `colLast` pair
//! in mapped paragraphs is inserted as a range bookmark, but its host
//! paragraph stays locked. A well-formed pair inside an opaque table becomes
//! a point destination beside the block and keeps an informational
//! `BookmarkAnchorUnmapped` note. Accepted marker bytes stay in the package.
//! An unchanged export copies them. An edit that keeps a bookmark in the
//! part it was imported from writes that marker back. Body markers stay in
//! `document.xml`. Header, footer, and comment markers stay in their own
//! parts. A marker that was not accepted still locks its paragraph.

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
    // Accepted body bookmarks do not lock the paragraph. Export splices the
    // marker bytes back, so editing the text does not have to drop them.
    for paragraph in paragraphs(&result) {
        assert!(result.editability.can_edit(paragraph.id()));
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
fn column_and_opaque_table_bookmarks_are_destinations() {
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
    assert_eq!(names(&result.document), ["col", "kept", "cell"]);
    // The detached and reversed pairs stay refused, as do the missing id
    // and the unsupported attribute.
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 1 }
    ));
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 2 }
    ));
    assert!(
        result
            .diagnostics
            .iter()
            .filter(|item| item.kind == DiagnosticKind::BookmarkIdMissing)
            .count()
            >= 2
    );
    assert!(has(&result, &DiagnosticKind::BookmarkUnsupported { id: 4 }));
    assert!(
        result
            .document
            .body()
            .blocks
            .iter()
            .any(|block| matches!(block, Block::Opaque(_)))
    );

    let paras = paragraphs(&result);
    // The column pair imports as an ordinary range bookmark with an exact,
    // case-sensitive name.
    let col = bookmark(&result.document, "col");
    let Anchor::Range { start, end } = col.anchor else {
        panic!("range");
    };
    assert_eq!(start.paragraph, paras[2].id());
    assert_eq!((start.offset.0, end.offset.0), (0, 1));
    assert_eq!(start.bias, Bias::After);
    assert_eq!(end.bias, Bias::Before);
    // Its host paragraph stays locked: colFirst/colLast are not written
    // back, so an edited export is Restricted rather than dropping them.
    assert!(!result.editability.can_edit(paras[2].id()));
    assert!(!result.editability.is_unrestricted());
    let unchanged = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(unchanged.mode(), ExportMode::OriginalPackage);
    let mut edited = result.document.clone();
    body_paragraph(&mut edited, 0)
        .insert_text(ScalarOffset(0), "X")
        .unwrap();
    assert!(matches!(
        plan_export(&result.backing, &edited.snapshot(), &Budget::default()),
        Err(ExportError::Restricted)
    ));

    // The opaque-table pair is a point destination at the end of the
    // paragraph before the table. It has no source: write-back must not
    // move the markers out of the preserved markup. The pair keeps its
    // informational diagnostic.
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorUnmapped { id: 5 }
    ));
    let cell = bookmark(&result.document, "cell");
    assert!(cell.source.is_none());
    let Anchor::Range { start, end } = cell.anchor else {
        panic!("range");
    };
    assert_eq!(start.paragraph, paras[3].id());
    assert_eq!(start.offset, end.offset);
    assert_eq!(start.offset.0, 0);
    assert_eq!(start.bias, Bias::After);
    assert_eq!(end.bias, Bias::Before);
}

#[test]
fn an_opaque_table_at_the_body_start_anchors_the_first_paragraph() {
    let body = concat!(
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkStart w:id="1" w:name="lead"/>"#,
        r#"<w:r><w:t>T</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:p><w:r><w:t>After</w:t></w:r></w:p>"#,
    );
    let result = open(&package(body));
    assert_eq!(names(&result.document), ["lead"]);
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorUnmapped { id: 1 }
    ));
    let mark = bookmark(&result.document, "lead");
    let paras = paragraphs(&result);
    let Anchor::Range { start, end } = mark.anchor else {
        panic!("range");
    };
    // No paragraph precedes the table: the point destination is the start
    // of the story's first paragraph.
    assert_eq!(start.paragraph, paras[0].id());
    assert_eq!(start.offset, end.offset);
    assert_eq!(start.offset.0, 0);
}

#[test]
fn bad_opaque_table_bookmark_pairs_stay_refused() {
    let body = concat!(
        r#"<w:p><w:bookmarkStart w:id="9" w:name="half"/><w:r><w:t>A</w:t></w:r></w:p>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkEnd w:id="9"/></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkEnd w:id="1"/>"#,
        r#"<w:r><w:t>T</w:t></w:r><w:bookmarkStart w:id="1" w:name="rev"/></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkStart w:id="2" w:name="attr" w:foo="1"/>"#,
        r#"<w:r><w:t>T</w:t></w:r><w:bookmarkEnd w:id="2"/></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkStart w:id="3" w:name="col" w:colFirst="0"/>"#,
        r#"<w:r><w:t>T</w:t></w:r><w:bookmarkEnd w:id="3"/></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:bookmarkStart w:id="4" w:name="lone"/>"#,
        r#"<w:r><w:t>T</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:p><w:r><w:t>End</w:t></w:r></w:p>"#,
    );
    let result = open(&package(body));
    // Half of the id 9 pair sits in a mapped paragraph: the whole pair is
    // refused and the paragraph stays locked.
    assert!(names(&result.document).is_empty());
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 9 }
    ));
    assert!(!result.editability.can_edit(paragraphs(&result)[0].id()));
    // Reversed, unsupported, and column pairs inside preserved markup are
    // refused with their own diagnostics; a lone start is unmapped.
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorUnmapped { id: 1 }
    ));
    assert!(has(&result, &DiagnosticKind::BookmarkUnsupported { id: 2 }));
    assert!(has(&result, &DiagnosticKind::BookmarkColumn { id: 3 }));
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorUnmapped { id: 4 }
    ));
}

fn hdr(inner: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:hdr xmlns:w="{W}">{inner}</w:hdr>"#)
}

fn ftr(inner: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:ftr xmlns:w="{W}">{inner}</w:ftr>"#)
}

fn comments_xml(inner: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:comments xmlns:w="{W}"><w:comment w:id="0" w:author="Ada"><w:p>{inner}</w:p></w:comment></w:comments>"#
    )
}

const SECT: &str = concat!(
    r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/>"#,
    r#"<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>"#,
    r#"<w:headerReference w:type="default" r:id="rId2"/>"#,
    r#"<w:footerReference w:type="default" r:id="rId3"/>"#,
    r#"</w:sectPr>"#,
);

fn story_package(body: &str, header: &str, footer: &str, comments: Option<&str>) -> Vec<u8> {
    let mut overrides = String::from(
        r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/><Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>"#,
    );
    let mut rels = String::from(
        r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>"#,
    );
    if comments.is_some() {
        overrides.push_str(
            r#"<Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/>"#,
        );
        rels.push_str(
            r#"<Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/>"#,
        );
    }
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL}">{rels}</Relationships>"#
    );
    let mut parts = vec![
        ("[Content_Types].xml".to_string(), types(&overrides)),
        ("_rels/.rels".to_string(), package_rels().to_string()),
        ("word/document.xml".to_string(), document(body)),
        ("word/_rels/document.xml.rels".to_string(), rels),
        ("word/header1.xml".to_string(), header.to_string()),
        ("word/footer1.xml".to_string(), footer.to_string()),
    ];
    if let Some(comments) = comments {
        parts.push(("word/comments.xml".to_string(), comments.to_string()));
    }
    let borrowed: Vec<(&str, &str)> = parts
        .iter()
        .map(|(name, data)| (name.as_str(), data.as_str()))
        .collect();
    archive(&borrowed)
}

fn story_paragraph(document: &Document, kind: joyword_model::StoryKind) -> joyword_model::NodeId {
    document
        .stories()
        .find(|story| story.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?}"))
        .blocks
        .iter()
        .find_map(Block::as_paragraph)
        .unwrap_or_else(|| panic!("missing paragraph in {kind:?}"))
        .id()
}

#[test]
fn header_footer_and_comment_bookmarks_import_with_body_rules() {
    // The same w:id in different parts is a different bookmark. ECMA-376
    // pairs ids inside one story part, not across the package.
    let body = format!(
        concat!(
            r#"<w:p><w:bookmarkStart w:id="1" w:name="body"/><w:r><w:t>Body</w:t></w:r>"#,
            r#"<w:bookmarkEnd w:id="1"/></w:p>"#,
            r#"<w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Said</w:t></w:r>"#,
            r#"<w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p>"#,
            "{sect}",
        ),
        sect = SECT,
    );
    let bytes = story_package(
        &body,
        &hdr(
            r#"<w:p><w:bookmarkStart w:id="1" w:name="head"/><w:r><w:t>Head</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
        ),
        &ftr(
            r#"<w:p><w:bookmarkStart w:id="1" w:name="foot"/><w:r><w:t>Foot</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
        ),
        Some(&comments_xml(
            r#"<w:bookmarkStart w:id="1" w:name="note"/><w:r><w:t>Note</w:t></w:r><w:bookmarkEnd w:id="1"/>"#,
        )),
    );
    let result = open(&bytes);
    assert_eq!(names(&result.document), ["body", "head", "foot", "note"]);
    assert!(has(
        &result,
        &DiagnosticKind::BookmarksPreserved { count: 8 }
    ));
    for (name, kind, end) in [
        ("body", joyword_model::StoryKind::Body, 4),
        ("head", joyword_model::StoryKind::Header, 4),
        ("foot", joyword_model::StoryKind::Footer, 4),
        ("note", joyword_model::StoryKind::Comment, 4),
    ] {
        let mark = bookmark(&result.document, name);
        let Anchor::Range { start, end: stop } = &mark.anchor else {
            panic!("range");
        };
        assert_eq!(result.document.story(start.story).unwrap().kind, kind);
        assert_eq!(start.story, stop.story);
        assert_eq!(start.offset, ScalarOffset(0));
        assert_eq!(stop.offset, ScalarOffset(end));
        assert_eq!(start.bias, Bias::After);
        assert_eq!(stop.bias, Bias::Before);
    }
    assert_eq!(
        bookmark(&result.document, "head")
            .source
            .as_ref()
            .map(|source| source.part.as_ref()),
        Some("/word/header1.xml")
    );
    assert_eq!(
        bookmark(&result.document, "foot")
            .source
            .as_ref()
            .map(|source| source.part.as_ref()),
        Some("/word/footer1.xml")
    );
    assert_eq!(
        bookmark(&result.document, "note")
            .source
            .as_ref()
            .map(|source| source.part.as_ref()),
        Some("/word/comments.xml")
    );
    assert!(result.editability.can_edit(paragraphs(&result)[0].id()));
    for kind in [
        joyword_model::StoryKind::Header,
        joyword_model::StoryKind::Footer,
        joyword_model::StoryKind::Comment,
    ] {
        assert!(
            result
                .editability
                .can_edit(story_paragraph(&result.document, kind)),
            "{kind:?}"
        );
    }
    assert!(result.editability.is_unrestricted());
    let plan = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(plan.mode(), ExportMode::OriginalPackage);
    assert_eq!(plan.bytes(), bytes.as_slice());
    assert!(
        result
            .diagnostics
            .iter()
            .all(|item| item.severity != Severity::Error)
    );
}

#[test]
fn story_bookmarks_refuse_bad_names_duplicates_and_detached_comments() {
    let long = "n".repeat(2049);
    let header = hdr(&format!(
        concat!(
            r#"<w:p>"#,
            r#"<w:bookmarkStart w:id="2" w:name=""/><w:bookmarkEnd w:id="2"/>"#,
            r#"<w:bookmarkStart w:id="3" w:name="a&#9;b"/><w:bookmarkEnd w:id="3"/>"#,
            r#"<w:bookmarkStart w:id="4" w:name="{long}"/><w:bookmarkEnd w:id="4"/>"#,
            r#"<w:bookmarkStart w:id="5" w:name="keep"/><w:bookmarkEnd w:id="5"/>"#,
            r#"<w:bookmarkStart w:id="6" w:name="col" w:colFirst="0" w:colLast="0"/>"#,
            r#"<w:bookmarkEnd w:id="6"/>"#,
            r#"<w:bookmarkStart w:id="7" w:name="split"/>"#,
            r#"<w:bookmarkStart w:id="8" w:name="ok"/><w:r><w:t>Ok</w:t></w:r>"#,
            r#"<w:bookmarkEnd w:id="8"/></w:p>"#,
        ),
        long = long
    ));
    let body = format!(
        concat!(
            r#"<w:p><w:bookmarkStart w:id="1" w:name="keep"/><w:r><w:t>Body</w:t></w:r>"#,
            r#"<w:bookmarkEnd w:id="1"/><w:bookmarkEnd w:id="7"/></w:p>"#,
            "{sect}",
        ),
        sect = SECT,
    );
    let result = open(&story_package(
        &body,
        &header,
        &ftr(r#"<w:p><w:r><w:t>Foot</w:t></w:r></w:p>"#),
        None,
    ));
    // The column pair is accepted like any other range bookmark; the header
    // paragraph stays locked because colFirst/colLast are not written back.
    assert_eq!(names(&result.document), ["keep", "col", "ok"]);
    let col = bookmark(&result.document, "col");
    let Anchor::Range { start, end } = col.anchor else {
        panic!("range");
    };
    assert_eq!(
        result.document.story(start.story).unwrap().kind,
        joyword_model::StoryKind::Header
    );
    assert_eq!(start.offset, end.offset);
    assert_eq!(start.bias, Bias::After);
    assert_eq!(end.bias, Bias::Before);
    assert!(has(&result, &DiagnosticKind::BookmarkNameEmpty { id: 2 }));
    assert!(has(&result, &DiagnosticKind::BookmarkNameInvalid { id: 3 }));
    assert!(has(&result, &DiagnosticKind::BookmarkBudget { id: 4 }));
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkNameDuplicate { id: 5 }
    ));
    assert!(!has(&result, &DiagnosticKind::BookmarkColumn { id: 6 }));
    assert!(has(
        &result,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 7 }
    ));
    assert!(!has(
        &result,
        &DiagnosticKind::BookmarkOutsideBody { id: 8 }
    ));
    assert!(!result.editability.can_edit(story_paragraph(
        &result.document,
        joyword_model::StoryKind::Header
    )));
    assert!(
        result
            .diagnostics
            .iter()
            .all(|item| item.severity != Severity::Error)
    );

    let detached = story_package(
        &format!(r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{SECT}"#),
        &hdr(r#"<w:p><w:r><w:t>Head</w:t></w:r></w:p>"#),
        &ftr(r#"<w:p><w:r><w:t>Foot</w:t></w:r></w:p>"#),
        Some(&comments_xml(
            r#"<w:bookmarkStart w:id="9" w:name="note"/><w:r><w:t>Note</w:t></w:r><w:bookmarkEnd w:id="9"/>"#,
        )),
    );
    let detached = open(&detached);
    assert!(names(&detached.document).is_empty());
    assert!(has(
        &detached,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 9 }
    ));
    assert!(!has(
        &detached,
        &DiagnosticKind::BookmarkOutsideBody { id: 9 }
    ));
    assert!(has(
        &detached,
        &DiagnosticKind::CommentAnchorMissing { id: 0 }
    ));
    assert!(!detached.editability.can_edit(story_paragraph(
        &detached.document,
        joyword_model::StoryKind::Comment
    )));
}

#[test]
fn a_header_bookmark_past_the_shared_budget_is_refused() {
    let mut body = String::from("<w:p>");
    for id in 0..512 {
        body.push_str(&format!(
            r#"<w:bookmarkStart w:id="{id}" w:name="b{id}"/><w:bookmarkEnd w:id="{id}"/>"#
        ));
    }
    body.push_str("<w:r><w:t>x</w:t></w:r></w:p>");
    body.push_str(SECT);
    let bytes = story_package(
        &body,
        &hdr(
            r#"<w:p><w:bookmarkStart w:id="512" w:name="extra"/><w:r><w:t>H</w:t></w:r><w:bookmarkEnd w:id="512"/></w:p>"#,
        ),
        &ftr(r#"<w:p><w:r><w:t>F</w:t></w:r></w:p>"#),
        None,
    );
    let result = open(&bytes);
    let imported = names(&result.document);
    assert_eq!(imported.len(), 512);
    assert!(!imported.iter().any(|name| name == "extra"));
    assert!(has(&result, &DiagnosticKind::BookmarkBudget { id: 512 }));
    assert_eq!(
        result
            .diagnostics
            .iter()
            .filter(|item| matches!(item.kind, DiagnosticKind::BookmarkBudget { .. }))
            .count(),
        1
    );
    assert!(!result.editability.can_edit(story_paragraph(
        &result.document,
        joyword_model::StoryKind::Header
    )));
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

fn ranges(document: &Document, name: &str) -> (usize, usize, usize, usize) {
    let paras: Vec<_> = document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect();
    let mark = bookmark(document, name);
    let Anchor::Range { start, end } = &mark.anchor else {
        panic!("range");
    };
    let index = |id| {
        paras
            .iter()
            .position(|paragraph| paragraph.id() == id)
            .unwrap()
    };
    (
        index(start.paragraph),
        start.offset.0,
        index(end.paragraph),
        end.offset.0,
    )
}

fn round_trip(bytes: &[u8], document: &Document) -> ImportResult {
    let plan = plan_export(
        &open(bytes).backing,
        &document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let xml = part_text(plan.bytes(), "/word/document.xml");
    assert!(xml.contains("w:bookmarkStart"), "{xml}");
    assert!(xml.contains("w:bookmarkEnd"), "{xml}");
    let again = open(plan.bytes());
    assert_eq!(names(&again.document), names(document));
    for name in names(document) {
        assert_eq!(
            ranges(&again.document, &name),
            ranges(document, &name),
            "{name}"
        );
    }
    again
}

#[test]
fn an_edited_export_writes_body_bookmarks_back() {
    let body = concat!(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="mark" w:displacedByCustomXml="next"/>"#,
        r#"<w:r><w:t>甲</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
        r#"<w:p><w:r><w:bookmarkStart w:id="2" w:name="inrun"/><w:t>文</w:t>"#,
        r#"<w:bookmarkEnd w:id="2"/></w:r></w:p>"#,
        r#"<w:p><w:hyperlink w:anchor="mark"><w:bookmarkStart w:id="3" w:name="Inside"/>"#,
        r#"<w:r><w:t>Go</w:t></w:r><w:bookmarkEnd w:id="3"/></w:hyperlink></w:p>"#,
        r#"<w:p><w:r><w:t>乙</w:t></w:r></w:p>"#,
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

    // Editing a paragraph that does not host a bookmark keeps the marker
    // bytes. Reimport recovers the same names and ranges.
    let mut current = result.document.clone();
    body_paragraph(&mut current, 3)
        .insert_text(ScalarOffset(0), "X")
        .unwrap();
    let again = round_trip(&bytes, &current);
    assert_eq!(texts(&again), ["甲", "文", "Go", "X乙"]);
    assert!(
        part_text(
            plan_export(&result.backing, &current.snapshot(), &Budget::default())
                .unwrap()
                .bytes(),
            "/word/document.xml"
        )
        .contains(r#"w:displacedByCustomXml="next""#)
    );

    // Editing the host rewrites the markers at the model offsets. The anchor
    // here follows the inserted scalar, the same way the engine maps
    // Bias::After / Bias::Before.
    let mut shifted = result.document.clone();
    body_paragraph(&mut shifted, 0)
        .insert_text(ScalarOffset(0), "Q")
        .unwrap();
    let id = bookmark(&shifted, "mark").id;
    let Anchor::Range { start, end } = &mut shifted.annotation_mut(id).unwrap().anchor else {
        panic!("range");
    };
    start.offset = ScalarOffset(1);
    end.offset = ScalarOffset(2);
    let again = round_trip(&bytes, &shifted);
    assert_eq!(texts(&again), ["Q甲", "文", "Go", "乙"]);
    assert_eq!(ranges(&again.document, "mark"), (0, 1, 0, 2));
    assert_eq!(ranges(&again.document, "inrun"), (1, 0, 1, 1));
    assert_eq!(ranges(&again.document, "Inside"), (2, 0, 2, 2));
    let xml = part_text(
        plan_export(&result.backing, &shifted.snapshot(), &Budget::default())
            .unwrap()
            .bytes(),
        "/word/document.xml",
    );
    assert!(xml.contains(r#"w:name="mark""#), "{xml}");
    assert!(xml.contains(r#"w:displacedByCustomXml="next""#), "{xml}");
    assert!(xml.contains(r#"w:id="1""#), "{xml}");
}

#[test]
fn bookmark_write_back_refuses_unrepresentable_shapes() {
    let body = concat!(
        r#"<w:p><w:bookmarkStart w:id="1" w:name="mark"/><w:r><w:t>甲</w:t></w:r>"#,
        r#"<w:bookmarkEnd w:id="1"/></w:p><w:p><w:r><w:t>乙</w:t></w:r></w:p>"#,
    );
    let bytes = package(body);
    let result = open(&bytes);
    let id = bookmark(&result.document, "mark").id;

    let mut renamed = result.document.clone();
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

    let mut reversed = result.document.clone();
    let Anchor::Range { start, end } = &mut reversed.annotation_mut(id).unwrap().anchor else {
        panic!("range");
    };
    start.offset = ScalarOffset(1);
    end.offset = ScalarOffset(0);
    assert!(matches!(
        plan_export(&result.backing, &reversed.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    let mut removed = result.document.clone();
    // There is no public remove. Replacing the kind is the refusal path for
    // a bookmark that is no longer a bookmark.
    removed.annotation_mut(id).unwrap().kind = AnnotationKind::Field { separator: None };
    assert!(matches!(
        plan_export(&result.backing, &removed.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "non-text document content changed",
        })
    ));

    let mut fresh = result.document.clone();
    let story = fresh.body_id();
    let paragraph = paragraphs_of(&fresh)[0];
    fresh.insert_annotation(
        AnnotationKind::Bookmark {
            name: Arc::from("new"),
        },
        Anchor::Range {
            start: joyword_model::Position {
                story,
                paragraph,
                offset: ScalarOffset(0),
                bias: Bias::After,
            },
            end: joyword_model::Position {
                story,
                paragraph,
                offset: ScalarOffset(0),
                bias: Bias::Before,
            },
        },
    );
    assert!(matches!(
        plan_export(&result.backing, &fresh.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "non-text document content changed",
        })
    ));

    // A column bookmark is a range annotation, but its host paragraph stays
    // locked because colFirst/colLast are not written back. An edit is
    // Restricted, so the marker cannot be dropped; an unchanged export
    // still copies the original package.
    let column = package(concat!(
        r#"<w:p><w:bookmarkStart w:id="3" w:name="col" w:colFirst="0" w:colLast="0"/>"#,
        r#"<w:r><w:t>C</w:t></w:r><w:bookmarkEnd w:id="3"/></w:p>"#,
        r#"<w:p><w:r><w:t>乙</w:t></w:r></w:p>"#,
    ));
    let column_bytes = column.clone();
    let column_result = open(&column);
    assert_eq!(names(&column_result.document), ["col"]);
    let col = bookmark(&column_result.document, "col");
    let Anchor::Range { start, end } = col.anchor else {
        panic!("range");
    };
    assert_eq!((start.offset.0, end.offset.0), (0, 1));
    assert!(!column_result.editability.is_unrestricted());
    let unchanged = plan_export(
        &column_result.backing,
        &column_result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(unchanged.mode(), ExportMode::OriginalPackage);
    assert_eq!(unchanged.bytes(), column_bytes.as_slice());
    let mut edited = column_result.document.clone();
    body_paragraph(&mut edited, 1)
        .insert_text(ScalarOffset(0), "X")
        .unwrap();
    assert!(matches!(
        plan_export(
            &column_result.backing,
            &edited.snapshot(),
            &Budget::default()
        ),
        Err(ExportError::Restricted)
    ));
}

fn part_bytes(package: &[u8], name: &str) -> Vec<u8> {
    Package::read(package, &Budget::default())
        .unwrap()
        .part(name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .data
        .as_ref()
        .to_vec()
}

fn story_id(document: &Document, kind: joyword_model::StoryKind) -> joyword_model::StoryId {
    document
        .stories()
        .find(|story| story.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?}"))
        .id
}

fn edit_story(document: &mut Document, kind: joyword_model::StoryKind, at: usize, text: &str) {
    let id = story_id(document, kind);
    document.story_mut(id).unwrap().blocks.update(0, |block| {
        let Block::Paragraph(paragraph) = block else {
            panic!("paragraph");
        };
        Arc::make_mut(paragraph)
            .insert_text(ScalarOffset(at), text)
            .unwrap();
    });
}

fn set_offsets(document: &mut Document, name: &str, start_at: usize, end_at: usize) {
    let id = bookmark(document, name).id;
    let Anchor::Range { start, end } = &mut document.annotation_mut(id).unwrap().anchor else {
        panic!("range");
    };
    start.offset = ScalarOffset(start_at);
    end.offset = ScalarOffset(end_at);
}

fn story_ranges(document: &Document, name: &str) -> (joyword_model::StoryKind, usize, usize) {
    let mark = bookmark(document, name);
    let Anchor::Range { start, end } = &mark.anchor else {
        panic!("range");
    };
    let story = document.story(start.story).unwrap();
    let paragraphs: Vec<_> = story
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect();
    let index = |id| {
        paragraphs
            .iter()
            .position(|paragraph| paragraph.id() == id)
            .unwrap()
    };
    assert_eq!(index(start.paragraph), index(end.paragraph));
    (story.kind, start.offset.0, end.offset.0)
}

fn story_fixture() -> (Vec<u8>, ImportResult) {
    let body = format!(
        concat!(
            r#"<w:p><w:bookmarkStart w:id="1" w:name="body" w:displacedByCustomXml="next"/>"#,
            r#"<w:r><w:t>Body</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
            r#"<w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Said</w:t></w:r>"#,
            r#"<w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p>"#,
            "{sect}",
        ),
        sect = SECT,
    );
    let bytes = story_package(
        &body,
        &hdr(concat!(
            r#"<w:p><w:bookmarkStart w:id="1" w:name="head" w:displacedByCustomXml="next"/>"#,
            r#"<w:r><w:t>Head</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p>"#,
        )),
        &ftr(concat!(
            r#"<w:p><w:r><w:bookmarkStart w:id="1" w:name="foot"/><w:t>Foot</w:t>"#,
            r#"<w:bookmarkEnd w:id="1"/></w:r></w:p>"#,
        )),
        Some(&comments_xml(concat!(
            r#"<w:bookmarkStart w:id="1" w:name="note"/>"#,
            r#"<w:r><w:t>Note</w:t></w:r><w:bookmarkEnd w:id="1"/>"#,
        ))),
    );
    let result = open(&bytes);
    (bytes, result)
}

#[test]
fn an_edited_export_writes_story_bookmarks_back() {
    let (bytes, result) = story_fixture();
    assert_eq!(names(&result.document), ["body", "head", "foot", "note"]);
    assert!(result.editability.is_unrestricted());
    let unchanged = plan_export(
        &result.backing,
        &result.document.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    assert_eq!(unchanged.mode(), ExportMode::OriginalPackage);
    assert_eq!(unchanged.bytes(), bytes.as_slice());

    // Editing the body keeps header, footer, and comment marker bytes in
    // their own parts. The shared w:id is not copied into document.xml.
    let mut body_edit = result.document.clone();
    body_paragraph(&mut body_edit, 0)
        .insert_text(ScalarOffset(4), "X")
        .unwrap();
    let plan = plan_export(&result.backing, &body_edit.snapshot(), &Budget::default()).unwrap();
    assert_eq!(plan.mode(), ExportMode::TextPatch);
    let document_xml = part_text(plan.bytes(), "/word/document.xml");
    assert!(document_xml.contains(r#"w:name="body""#), "{document_xml}");
    assert!(
        document_xml.contains(r#"w:displacedByCustomXml="next""#),
        "{document_xml}"
    );
    assert!(!document_xml.contains(r#"w:name="head""#), "{document_xml}");
    assert!(!document_xml.contains(r#"w:name="foot""#), "{document_xml}");
    assert!(!document_xml.contains(r#"w:name="note""#), "{document_xml}");
    assert_eq!(
        part_bytes(plan.bytes(), "/word/header1.xml"),
        part_bytes(&bytes, "/word/header1.xml")
    );
    assert_eq!(
        part_bytes(plan.bytes(), "/word/footer1.xml"),
        part_bytes(&bytes, "/word/footer1.xml")
    );
    assert_eq!(
        part_bytes(plan.bytes(), "/word/comments.xml"),
        part_bytes(&bytes, "/word/comments.xml")
    );
    let again = open(plan.bytes());
    assert_eq!(names(&again.document), ["body", "head", "foot", "note"]);
    assert_eq!(
        story_ranges(&again.document, "body"),
        story_ranges(&body_edit, "body")
    );
    assert_eq!(
        story_ranges(&again.document, "head"),
        (joyword_model::StoryKind::Header, 0, 4)
    );
    assert_eq!(
        story_ranges(&again.document, "note"),
        (joyword_model::StoryKind::Comment, 0, 4)
    );

    // Editing the header host rewrites that part's markers at the model
    // offsets and leaves the footer bytes alone. The same w:id stays in the
    // header part.
    let mut header_edit = result.document.clone();
    edit_story(&mut header_edit, joyword_model::StoryKind::Header, 0, "Q");
    set_offsets(&mut header_edit, "head", 1, 5);
    let plan = plan_export(&result.backing, &header_edit.snapshot(), &Budget::default()).unwrap();
    let header_xml = part_text(plan.bytes(), "/word/header1.xml");
    assert!(header_xml.contains(r#"w:name="head""#), "{header_xml}");
    assert!(header_xml.contains(r#"w:id="1""#), "{header_xml}");
    assert!(
        header_xml.contains(r#"w:displacedByCustomXml="next""#),
        "{header_xml}"
    );
    assert!(!header_xml.contains(r#"w:name="body""#), "{header_xml}");
    assert_eq!(
        part_bytes(plan.bytes(), "/word/footer1.xml"),
        part_bytes(&bytes, "/word/footer1.xml")
    );
    let again = open(plan.bytes());
    assert_eq!(
        story_ranges(&again.document, "head"),
        (joyword_model::StoryKind::Header, 1, 5)
    );
    assert_eq!(
        texts_of(&again, joyword_model::StoryKind::Header),
        ["QHead"]
    );
    assert_eq!(
        story_ranges(&again.document, "foot"),
        (joyword_model::StoryKind::Footer, 0, 4)
    );

    // An anchor move with no text change still rewrites the footer part.
    let mut moved = result.document.clone();
    set_offsets(&mut moved, "foot", 1, 3);
    let plan = plan_export(&result.backing, &moved.snapshot(), &Budget::default()).unwrap();
    assert_ne!(
        part_bytes(plan.bytes(), "/word/footer1.xml"),
        part_bytes(&bytes, "/word/footer1.xml")
    );
    assert_eq!(
        part_bytes(plan.bytes(), "/word/header1.xml"),
        part_bytes(&bytes, "/word/header1.xml")
    );
    let again = open(plan.bytes());
    assert_eq!(
        story_ranges(&again.document, "foot"),
        (joyword_model::StoryKind::Footer, 1, 3)
    );
    assert!(part_text(plan.bytes(), "/word/document.xml").contains(r#"w:name="body""#));
    assert!(!part_text(plan.bytes(), "/word/document.xml").contains(r#"w:name="foot""#));

    // Editing the comment host writes the marker back into comments.xml.
    let mut comment_edit = result.document.clone();
    edit_story(&mut comment_edit, joyword_model::StoryKind::Comment, 0, "Q");
    set_offsets(&mut comment_edit, "note", 1, 5);
    let plan = plan_export(
        &result.backing,
        &comment_edit.snapshot(),
        &Budget::default(),
    )
    .unwrap();
    let comments = part_text(plan.bytes(), "/word/comments.xml");
    assert!(comments.contains(r#"w:name="note""#), "{comments}");
    assert!(comments.contains(r#"w:id="1""#), "{comments}");
    assert!(!part_text(plan.bytes(), "/word/document.xml").contains(r#"w:name="note""#));
    let again = open(plan.bytes());
    assert_eq!(
        story_ranges(&again.document, "note"),
        (joyword_model::StoryKind::Comment, 1, 5)
    );
    assert_eq!(
        texts_of(&again, joyword_model::StoryKind::Comment),
        ["QNote"]
    );
    assert_eq!(
        part_bytes(plan.bytes(), "/word/header1.xml"),
        part_bytes(&bytes, "/word/header1.xml")
    );
}

fn texts_of(result: &ImportResult, kind: joyword_model::StoryKind) -> Vec<String> {
    let id = story_id(&result.document, kind);
    result
        .document
        .story(id)
        .unwrap()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.plain_text(|_| "\u{FFFC}".into()))
        .collect()
}

#[test]
fn story_bookmark_write_back_refuses_cross_part_and_bad_shapes() {
    let (bytes, result) = story_fixture();
    let head = bookmark(&result.document, "head").id;
    let body_mark = bookmark(&result.document, "body").id;
    let note = bookmark(&result.document, "note").id;

    let mut renamed = result.document.clone();
    match &mut renamed.annotation_mut(head).unwrap().kind {
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
    detached.annotation_mut(head).unwrap().anchor = Anchor::Detached;
    assert!(matches!(
        plan_export(&result.backing, &detached.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    let mut reversed = result.document.clone();
    set_offsets(&mut reversed, "head", 3, 1);
    assert!(matches!(
        plan_export(&result.backing, &reversed.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    let mut bias = result.document.clone();
    let Anchor::Range { start, .. } = &mut bias.annotation_mut(head).unwrap().anchor else {
        panic!("range");
    };
    start.bias = Bias::Before;
    assert!(matches!(
        plan_export(&result.backing, &bias.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    // A header marker must not be invented into document.xml, and a body
    // marker must not be invented into the header part.
    let header_story = story_id(&result.document, joyword_model::StoryKind::Header);
    let header_paragraph = story_paragraph(&result.document, joyword_model::StoryKind::Header);
    let body_paragraph_id = paragraphs(&result)[0].id();
    let body_story = result.document.body_id();

    let mut header_to_body = result.document.clone();
    header_to_body.annotation_mut(head).unwrap().anchor = Anchor::Range {
        start: joyword_model::Position {
            story: body_story,
            paragraph: body_paragraph_id,
            offset: ScalarOffset(0),
            bias: Bias::After,
        },
        end: joyword_model::Position {
            story: body_story,
            paragraph: body_paragraph_id,
            offset: ScalarOffset(0),
            bias: Bias::Before,
        },
    };
    assert!(matches!(
        plan_export(
            &result.backing,
            &header_to_body.snapshot(),
            &Budget::default()
        ),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    let mut body_to_header = result.document.clone();
    body_to_header.annotation_mut(body_mark).unwrap().anchor = Anchor::Range {
        start: joyword_model::Position {
            story: header_story,
            paragraph: header_paragraph,
            offset: ScalarOffset(0),
            bias: Bias::After,
        },
        end: joyword_model::Position {
            story: header_story,
            paragraph: header_paragraph,
            offset: ScalarOffset(0),
            bias: Bias::Before,
        },
    };
    assert!(matches!(
        plan_export(
            &result.backing,
            &body_to_header.snapshot(),
            &Budget::default()
        ),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    let mut note_to_body = result.document.clone();
    note_to_body.annotation_mut(note).unwrap().anchor = Anchor::Range {
        start: joyword_model::Position {
            story: body_story,
            paragraph: body_paragraph_id,
            offset: ScalarOffset(0),
            bias: Bias::After,
        },
        end: joyword_model::Position {
            story: body_story,
            paragraph: body_paragraph_id,
            offset: ScalarOffset(4),
            bias: Bias::Before,
        },
    };
    assert!(matches!(
        plan_export(
            &result.backing,
            &note_to_body.snapshot(),
            &Budget::default()
        ),
        Err(ExportError::Unsupported {
            node: None,
            reason: "a bookmark changed",
        })
    ));

    let mut fresh = result.document.clone();
    fresh.insert_annotation(
        AnnotationKind::Bookmark {
            name: Arc::from("new"),
        },
        Anchor::Range {
            start: joyword_model::Position {
                story: header_story,
                paragraph: header_paragraph,
                offset: ScalarOffset(0),
                bias: Bias::After,
            },
            end: joyword_model::Position {
                story: header_story,
                paragraph: header_paragraph,
                offset: ScalarOffset(0),
                bias: Bias::Before,
            },
        },
    );
    assert!(matches!(
        plan_export(&result.backing, &fresh.snapshot(), &Budget::default()),
        Err(ExportError::Unsupported {
            node: None,
            reason: "non-text document content changed",
        })
    ));

    // A column bookmark is a destination, but its host paragraph stays
    // locked because colFirst/colLast are not written back, so an edit is
    // Restricted and cannot drop the marker. A detached comment bookmark is
    // different: it is not inserted, and the comment paragraph stays locked.
    let _ = bytes;
    let column = story_package(
        &format!(r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{SECT}"#),
        &hdr(concat!(
            r#"<w:p><w:bookmarkStart w:id="3" w:name="col" w:colFirst="0" w:colLast="0"/>"#,
            r#"<w:r><w:t>Head</w:t></w:r><w:bookmarkEnd w:id="3"/></w:p>"#,
        )),
        &ftr(r#"<w:p><w:r><w:t>Foot</w:t></w:r></w:p>"#),
        None,
    );
    let column_result = open(&column);
    assert_eq!(names(&column_result.document), ["col"]);
    assert!(!column_result.editability.is_unrestricted());
    let mut edited = column_result.document.clone();
    body_paragraph(&mut edited, 0)
        .insert_text(ScalarOffset(0), "X")
        .unwrap();
    assert!(matches!(
        plan_export(
            &column_result.backing,
            &edited.snapshot(),
            &Budget::default()
        ),
        Err(ExportError::Restricted)
    ));

    let detached = story_package(
        &format!(r#"<w:p><w:r><w:t>Body</w:t></w:r></w:p>{SECT}"#),
        &hdr(r#"<w:p><w:r><w:t>Head</w:t></w:r></w:p>"#),
        &ftr(r#"<w:p><w:r><w:t>Foot</w:t></w:r></w:p>"#),
        Some(&comments_xml(
            r#"<w:bookmarkStart w:id="9" w:name="note"/><w:r><w:t>Note</w:t></w:r><w:bookmarkEnd w:id="9"/>"#,
        )),
    );
    let detached = open(&detached);
    assert!(names(&detached.document).is_empty());
    assert!(has(
        &detached,
        &DiagnosticKind::BookmarkAnchorInvalid { id: 9 }
    ));
    assert!(!detached.editability.is_unrestricted());
    let mut edited = detached.document.clone();
    body_paragraph(&mut edited, 0)
        .insert_text(ScalarOffset(0), "X")
        .unwrap();
    assert!(matches!(
        plan_export(&detached.backing, &edited.snapshot(), &Budget::default()),
        Err(ExportError::Restricted)
    ));
}

fn paragraphs_of(document: &Document) -> Vec<joyword_model::NodeId> {
    document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .map(|paragraph| paragraph.id())
        .collect()
}
