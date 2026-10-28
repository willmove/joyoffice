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
    MAX_PDF_BOOKMARKS, MAX_PDF_NAME_BYTES, PdfExport, PdfNavigationReport, inspect_navigation,
    navigation_from_document, render_export_with_navigation,
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

mod qpdf {
    //! External cross-check of the exported bytes with the qpdf structure
    //! reader (`qpdf --json=2`), independent of joyword-pdf's own reader.
    //! When qpdf is not installed the checks skip, unless
    //! `JOYWORD_REQUIRE_QPDF` is set (the unified-pdf CI job installs qpdf
    //! and sets it), in which case they fail.

    use std::collections::BTreeMap;
    use std::io::ErrorKind;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::Value;

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// An `[page /XYZ x y zoom]` destination resolved through the qpdf
    /// object dump.
    #[derive(Clone, Debug, PartialEq)]
    pub struct ExternalDestination {
        pub page_object: String,
        pub x: f64,
        pub y: f64,
    }

    /// Navigation facts qpdf read from the exported bytes.
    #[derive(Debug, Default)]
    pub struct ExternalNavigation {
        /// Page object reference per page index, in page order.
        pub page_objects: Vec<String>,
        /// Name-tree keys as raw octets: ISO 32000-1 compares them byte by
        /// byte, so the external identity is the byte sequence, not a
        /// decoded string.
        pub destinations: BTreeMap<Vec<u8>, ExternalDestination>,
        /// `/S /GoTo` link annotations with their resolved `/D` arrays.
        pub goto_links: Vec<(usize, ExternalDestination)>,
    }

    impl ExternalNavigation {
        pub fn page_index(&self, page_object: &str) -> usize {
            self.page_objects
                .iter()
                .position(|object| object == page_object)
                .unwrap_or_else(|| panic!("qpdf page {page_object} outside the page tree"))
        }
    }

    fn require() -> bool {
        std::env::var_os("JOYWORD_REQUIRE_QPDF").is_some()
    }

    fn skip(message: &str) {
        assert!(!require(), "{message}");
        eprintln!("skipped: {message}");
    }

    /// Run qpdf over the bytes and extract the navigation facts. Returns
    /// None only when qpdf is unavailable and not required.
    pub fn read_navigation(pdf: &[u8]) -> Option<ExternalNavigation> {
        let document = json(pdf)?;
        Some(parse(&document))
    }

    fn json(pdf: &[u8]) -> Option<Value> {
        let path = std::env::temp_dir().join(format!(
            "joyword-qpdf-{}-{}.pdf",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        if let Err(error) = std::fs::write(&path, pdf) {
            skip(&format!("could not write a PDF for qpdf: {error}"));
            return None;
        }
        let run = Command::new("qpdf")
            .arg("--json=2")
            .arg(&path)
            .arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = std::fs::remove_file(&path);
        let output = match run {
            Ok(output) if output.status.success() => output,
            Ok(output) => {
                skip(&format!(
                    "qpdf failed ({:?}): {}",
                    output.status.code(),
                    String::from_utf8_lossy(&output.stderr)
                ));
                return None;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                skip("qpdf is not installed");
                return None;
            }
            Err(error) => {
                skip(&format!("could not run qpdf: {error}"));
                return None;
            }
        };
        match serde_json::from_slice(&output.stdout) {
            Ok(value) => Some(value),
            Err(error) => panic!("qpdf --json=2 stdout was not JSON: {error}"),
        }
    }

    /// qpdf JSON format v2 keeps the full object dump in the second element
    /// of the top-level "qpdf" array.
    fn objects(document: &Value) -> &Value {
        let objects = &document["qpdf"][1];
        assert!(objects.is_object(), "qpdf reported no object dump");
        objects
    }

    fn entry<'v>(objects: &'v Value, reference: &str) -> &'v Value {
        &objects[&format!("obj:{reference}")]["value"]
    }

    /// Follow an indirect `"12 0 R"` reference one level.
    fn resolve<'v>(objects: &'v Value, value: &'v Value) -> &'v Value {
        match value.as_str().and_then(parse_reference) {
            Some(reference) => entry(objects, reference),
            None => value,
        }
    }

    fn parse_reference(value: &str) -> Option<&str> {
        let mut parts = value.split(' ');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(number), Some(generation), Some("R"), None) => {
                number.parse::<u32>().ok()?;
                generation.parse::<u32>().ok()?;
                Some(value)
            }
            _ => None,
        }
    }

    /// qpdf v2 tags text strings as `u:<utf8>` and other byte strings as
    /// `b:<hex>`. Name-tree key identity is the raw octet sequence.
    fn string_bytes(value: &Value) -> Vec<u8> {
        let text = value.as_str().expect("qpdf string");
        if let Some(unicode) = text.strip_prefix("u:") {
            unicode.as_bytes().to_vec()
        } else if let Some(hex) = text.strip_prefix("b:") {
            assert!(hex.len() % 2 == 0, "qpdf hex string {text:?}");
            (0..hex.len())
                .step_by(2)
                .map(|index| {
                    u8::from_str_radix(&hex[index..index + 2], 16).expect("qpdf hex string")
                })
                .collect()
        } else {
            text.as_bytes().to_vec()
        }
    }

    fn number(value: &Value) -> f64 {
        value.as_f64().expect("qpdf number")
    }

    fn xyz(objects: &Value, value: &Value) -> ExternalDestination {
        let array = resolve(objects, value)
            .as_array()
            .expect("qpdf destination array");
        assert!(
            array.len() == 5 && array[1].as_str() == Some("/XYZ"),
            "qpdf destination is not [page /XYZ x y zoom]: {array:?}"
        );
        ExternalDestination {
            page_object: array[0].as_str().expect("qpdf destination page").to_owned(),
            x: number(&array[2]),
            y: number(&array[3]),
        }
    }

    fn collect_destinations(
        objects: &Value,
        value: &Value,
        out: &mut BTreeMap<Vec<u8>, ExternalDestination>,
        depth: usize,
    ) {
        assert!(depth < 8, "qpdf destination tree too deep");
        let dict = resolve(objects, value);
        if let Some(kids) = dict.get("/Kids").and_then(Value::as_array) {
            for kid in kids {
                collect_destinations(objects, kid, out, depth + 1);
            }
        }
        if let Some(names) = dict.get("/Names").and_then(Value::as_array) {
            assert!(names.len() % 2 == 0, "qpdf destination name pairs");
            for pair in names.as_chunks::<2>().0 {
                let name = string_bytes(&pair[0]);
                let destination = xyz(objects, &pair[1]);
                assert!(
                    out.insert(name.clone(), destination).is_none(),
                    "qpdf duplicate destination name {name:?}"
                );
            }
        }
    }

    fn parse(document: &Value) -> ExternalNavigation {
        let objects = objects(document);
        let pages = document["pages"].as_array().expect("qpdf pages");
        let mut navigation = ExternalNavigation {
            page_objects: pages
                .iter()
                .map(|page| {
                    page["object"]
                        .as_str()
                        .expect("qpdf page object")
                        .to_owned()
                })
                .collect(),
            ..ExternalNavigation::default()
        };
        let root = objects["trailer"]["value"]["/Root"]
            .as_str()
            .expect("qpdf trailer /Root");
        let catalog = entry(objects, root);
        if let Some(dests) = catalog
            .get("/Names")
            .map(|names| resolve(objects, names))
            .and_then(|names| names.get("/Dests"))
        {
            collect_destinations(objects, dests, &mut navigation.destinations, 0);
        }
        let page_objects = navigation.page_objects.clone();
        for (source_page, page_object) in page_objects.iter().enumerate() {
            let annots = entry(objects, page_object)
                .get("/Annots")
                .map(|value| resolve(objects, value))
                .and_then(Value::as_array);
            for annot in annots.into_iter().flatten() {
                let annot = resolve(objects, annot);
                if annot.get("/Subtype").and_then(Value::as_str) != Some("/Link") {
                    continue;
                }
                let action = resolve(
                    objects,
                    annot.get("/A").expect("qpdf link annotation action"),
                );
                if action.get("/S").and_then(Value::as_str) == Some("/GoTo") {
                    let destination = xyz(objects, &action["/D"]);
                    navigation.goto_links.push((source_page, destination));
                }
            }
        }
        navigation
    }
}

/// Run the same exported bytes through the external qpdf structure reader and
/// check it sees the same named destinations and `/S /GoTo` resolutions as
/// joyword-pdf's internal readback. Skips when qpdf is not installed.
fn assert_external_goto_destinations(pdf: &[u8], read: &PdfNavigationReport) {
    let Some(external) = qpdf::read_navigation(pdf) else {
        return;
    };
    assert_eq!(
        external.destinations.len(),
        read.bookmarks.len(),
        "qpdf named destination count"
    );
    for (name, expected) in &read.bookmarks {
        let found = external
            .destinations
            .get(name.as_bytes())
            .unwrap_or_else(|| panic!("qpdf named destination {name:?}"));
        assert_eq!(external.page_index(&found.page_object), expected.page);
        near64(found.x, expected.x as f64);
        near64(found.y, expected.y as f64);
    }
    assert_eq!(
        external.goto_links.len(),
        read.links.len(),
        "qpdf GoTo link count"
    );
    for link in &read.links {
        let (_, found) = external
            .goto_links
            .iter()
            .find(|(page, found)| {
                *page == link.source_page
                    && external.page_index(&found.page_object) == link.destination.page
                    && (found.x - link.destination.x as f64).abs() < 0.002
                    && (found.y - link.destination.y as f64).abs() < 0.002
            })
            .unwrap_or_else(|| panic!("qpdf GoTo for {link:?}"));
        assert!(
            external.destinations.values().any(|named| named == found),
            "qpdf GoTo {found:?} resolves to no named destination"
        );
    }
}

fn near64(left: f64, right: f64) {
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
    assert_external_goto_destinations(&pdf, &read);
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
    assert_external_goto_destinations(&pdf, &read);
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
    assert_external_goto_destinations(&pdf, &read);
}
