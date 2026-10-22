//! T204 model-to-PDF slice. Bookmark metadata is added after layout because
//! the baseline layout preflight does not yet admit bookmark annotations.
//! Read-back follows the actual PDF xref, page tree and /GoTo /D /XYZ objects.

use std::fmt::Debug;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont, PageScene,
    SingleFontProfile, layout_document, to_points,
};
use joyword_model::{
    Anchor, AnnotationKind, Bias, BlockSeq, Document, Hyperlink, Measure, PageSetup, Paragraph,
    Position, ScalarOffset, StoryKind,
};
use joyword_pdf::{
    MAX_PDF_BOOKMARKS, MAX_PDF_LINKS, MAX_PDF_NAME_BYTES, NavigationError, OutlineEntry,
    PdfBookmark, PdfDestination, PdfError, PdfExport, PdfGoToLink, PdfLink, PdfNavigation,
    inspect_navigation, inspect_pdf, links_from_document, navigation_from_document, render_export,
    render_export_with_navigation,
};

type TestResult<T = ()> = Result<T, String>;
fn error(value: impl Debug) -> String {
    format!("{value:?}")
}
fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

fn font() -> TestResult<LoadedFont> {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .map_err(error)
}

fn model(rows: &[(&str, Option<Hyperlink>)]) -> TestResult<Document> {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(100).raw());
    page.height = Measure::Emu(pt(48).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.header = Measure::Emu(pt(2).raw());
    page.footer = page.header;
    page.gutter = Measure::Emu(0);
    let mut document = Document::new(page);
    let mut blocks = Vec::new();
    for (text, link) in rows {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph
            .push_text(text, None, Arc::default())
            .map_err(error)?;
        if let Some(link) = link {
            paragraph
                .set_links(
                    ScalarOffset(0)..ScalarOffset(paragraph.len()),
                    Some(Arc::new(link.clone())),
                )
                .map_err(error)?;
        }
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).ok_or("body")?.blocks = BlockSeq::from_vec(blocks);
    Ok(document)
}

fn layout(document: &Document) -> TestResult<LayoutSnapshot> {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()?),
        SingleFontProfile {
            size: pt(12),
            leading: pt(18),
            baseline: pt(14),
            features: FeatureSet {
                standard_ligatures: true,
                ..FeatureSet::default()
            },
        },
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .map_err(error)
}

fn position(document: &Document, paragraph: usize, offset: usize) -> TestResult<Position> {
    Ok(Position {
        story: document.body_id(),
        paragraph: document
            .body()
            .blocks
            .get(paragraph)
            .ok_or("paragraph")?
            .id(),
        offset: ScalarOffset(offset),
        bias: Bias::After,
    })
}
fn bookmark(document: &mut Document, name: &str, start: Position, end: Position) {
    document.insert_annotation(
        AnnotationKind::Bookmark {
            name: Arc::from(name),
        },
        Anchor::Range { start, end },
    );
}
fn anchor(name: &str) -> TestResult<Hyperlink> {
    Hyperlink::for_edit(None, Some(name)).map_err(error)
}
fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.002, "{actual} != {expected}");
}

#[test]
fn model_forward_backward_same_page_and_uri_actions_survive_pdf_readback() -> TestResult {
    let mut document = model(&[
        ("AV", Some(anchor("later")?)),
        ("AV", Some(anchor("first")?)),
        (
            "A",
            Some(Hyperlink::for_edit(Some("http://example.invalid/a"), None).map_err(error)?),
        ),
        (
            "V",
            Some(
                Hyperlink::for_edit(
                    Some("https://example.invalid/b#fragment"),
                    Some("not-local"),
                )
                .map_err(error)?,
            ),
        ),
        (
            "A",
            Some(Hyperlink::for_edit(Some("mailto:a@example.invalid"), None).map_err(error)?),
        ),
        ("AVA", Some(anchor("first")?)),
        (
            "A",
            Some(Hyperlink {
                relationship: None,
                external: Some(Arc::from("javascript:alert(1)")),
                anchor: Some(Arc::from("first")),
            }),
        ),
    ])?;
    let snapshot = layout(&document)?;
    let first = position(&document, 0, 1)?;
    let later = position(&document, 5, 2)?;
    bookmark(&mut document, "first", first, first);
    bookmark(&mut document, "later", later, later);
    bookmark(&mut document, "unused", first, first);
    let navigation = navigation_from_document(&document, &snapshot).map_err(error)?;
    assert_eq!(navigation.bookmarks.len(), 3);
    assert_eq!(navigation.links.len(), 3);
    let export = PdfExport {
        links: links_from_document(&document, &snapshot),
        ..PdfExport::default()
    };
    let font = font()?;
    let bytes = render_export_with_navigation(
        snapshot.pages(),
        std::slice::from_ref(&font),
        &export,
        &navigation,
    )
    .map_err(error)?;
    assert_eq!(
        bytes,
        render_export_with_navigation(snapshot.pages(), &[font], &export, &navigation)
            .map_err(error)?
    );
    let info = inspect_pdf(&bytes).map_err(error)?;
    assert_eq!(
        info.link_uris,
        [
            "http://example.invalid/a",
            "https://example.invalid/b#fragment",
            "mailto:a@example.invalid"
        ]
    );
    let read = inspect_navigation(&bytes).map_err(error)?;
    assert_eq!(read.links.len(), 3);
    assert_eq!(read.bookmarks.len(), 3);
    for (link, (source, target)) in read.links.iter().zip([(0, later), (0, first), (2, first)]) {
        let caret = snapshot.caret(target).map_err(error)?;
        assert_eq!(link.source_page, source);
        assert_eq!(link.destination.page, caret.page);
        assert!(link.destination.page_object > 0);
        near(link.destination.x, to_points(caret.x) as f32);
        near(
            link.destination.y,
            to_points(snapshot.pages()[caret.page].height) as f32 - to_points(caret.top) as f32,
        );
        // pdf-writer 0.15.0 serializes krilla's None as zero (retain zoom).
        assert_eq!(link.destination.zoom, Some(0.0));
    }
    let later_read = read.bookmarks.get("later").ok_or("named destination")?;
    assert_eq!(later_read, &read.links[0].destination);
    assert_eq!(
        read.bookmarks.get("first"),
        Some(&read.links[1].destination)
    );
    assert_eq!(
        read.bookmarks.get("unused"),
        Some(&read.links[1].destination)
    );
    Ok(())
}

#[test]
fn empty_document_and_empty_paragraph_bookmark_are_distinct_from_empty_names() -> TestResult {
    let mut document = model(&[("", None)])?;
    let snapshot = layout(&document)?;
    assert_eq!(
        navigation_from_document(&document, &snapshot),
        Ok(PdfNavigation::default())
    );
    let point = position(&document, 0, 0)?;
    bookmark(&mut document, "empty-paragraph", point, point);
    let nav = navigation_from_document(&document, &snapshot).map_err(error)?;
    assert_eq!(nav.bookmarks.len(), 1);
    let pdf = render_export_with_navigation(snapshot.pages(), &[], &PdfExport::default(), &nav)
        .map_err(error)?;
    assert_eq!(inspect_navigation(&pdf).map_err(error)?.bookmarks.len(), 1);
    for name in ["", "   ", "\t"] {
        let mut invalid = document.clone();
        bookmark(&mut invalid, name, point, point);
        assert_eq!(
            navigation_from_document(&invalid, &snapshot),
            Err(NavigationError::EmptyName)
        );
    }
    Ok(())
}

#[test]
fn duplicate_names_never_select_a_first_target_and_case_is_exact() -> TestResult {
    let mut document = model(&[("AV", Some(anchor("Case")?))])?;
    let snapshot = layout(&document)?;
    let a = position(&document, 0, 0)?;
    let b = position(&document, 0, 1)?;
    bookmark(&mut document, "case", a, a);
    assert_eq!(
        navigation_from_document(&document, &snapshot),
        Err(NavigationError::MissingBookmark)
    );
    bookmark(&mut document, "Case", b, b);
    assert_eq!(
        navigation_from_document(&document, &snapshot)
            .map_err(error)?
            .bookmarks
            .len(),
        2
    );
    let mut invalid = document.clone();
    bookmark(&mut invalid, "bad\u{0000}name", a, a);
    assert_eq!(
        navigation_from_document(&invalid, &snapshot),
        Err(NavigationError::InvalidName)
    );
    for target in [a, b] {
        let mut invalid = document.clone();
        bookmark(&mut invalid, "Case", target, target);
        assert_eq!(
            navigation_from_document(&invalid, &snapshot),
            Err(NavigationError::DuplicateName)
        );
    }
    Ok(())
}

#[test]
fn detached_missing_reversed_out_of_range_and_unplaced_stories_are_refused() -> TestResult {
    let document = model(&[("AV", None), ("A", None)])?;
    let snapshot = layout(&document)?;
    let start = position(&document, 0, 0)?;
    let mut base = document.clone();
    let paragraph = Paragraph::new(base.ids().node());
    let unplaced_node = paragraph.id();
    let unplaced_story = base.insert_story(StoryKind::Comment, vec![paragraph.into()]);
    let absent = base.ids().node();
    let cases = [
        (Anchor::Detached, NavigationError::DetachedBookmark),
        (
            Anchor::Range {
                start: Position {
                    paragraph: absent,
                    ..start
                },
                end: start,
            },
            NavigationError::InvalidPosition,
        ),
        (
            Anchor::Range {
                start,
                end: Position {
                    offset: ScalarOffset(100),
                    ..start
                },
            },
            NavigationError::InvalidPosition,
        ),
        (
            Anchor::Range {
                start: position(&base, 1, 0)?,
                end: start,
            },
            NavigationError::InvalidPosition,
        ),
        (
            Anchor::Range {
                start,
                end: Position {
                    story: unplaced_story,
                    paragraph: unplaced_node,
                    ..start
                },
            },
            NavigationError::InvalidPosition,
        ),
        (
            Anchor::Range {
                start: Position {
                    story: unplaced_story,
                    paragraph: unplaced_node,
                    ..start
                },
                end: Position {
                    story: unplaced_story,
                    paragraph: unplaced_node,
                    ..start
                },
            },
            NavigationError::InvalidPosition,
        ),
    ];
    for (anchor, expected) in cases {
        let mut invalid = base.clone();
        invalid.insert_annotation(
            AnnotationKind::Bookmark {
                name: Arc::from("target"),
            },
            anchor,
        );
        assert_eq!(navigation_from_document(&invalid, &snapshot), Err(expected));
    }
    let mut stale = document;
    stale.advance_version();
    assert_eq!(
        navigation_from_document(&stale, &snapshot),
        Err(NavigationError::StaleLayout)
    );
    Ok(())
}

#[test]
fn wrap_boundary_bias_and_cluster_interiors_are_not_guessed() -> TestResult {
    let text = "AV ".repeat(25);
    let mut document = model(&[(&text, None)])?;
    let snapshot = layout(&document)?;
    assert!(snapshot.lines().len() > 2);
    let boundary = snapshot.lines()[1].source.scalars.end;
    let after = position(&document, 0, boundary)?;
    let before = Position {
        bias: Bias::Before,
        ..after
    };
    bookmark(&mut document, "before", before, before);
    bookmark(&mut document, "after", after, after);
    let nav = navigation_from_document(&document, &snapshot).map_err(error)?;
    let points: Vec<_> = nav.bookmarks.iter().map(|b| b.destination).collect();
    assert_ne!(points[0].page, points[1].page);
    let pdf =
        render_export_with_navigation(snapshot.pages(), &[font()?], &PdfExport::default(), &nav)
            .map_err(error)?;
    let read = inspect_navigation(&pdf).map_err(error)?;
    assert_ne!(read.bookmarks["before"].page, read.bookmarks["after"].page);

    let mut document = model(&[("fi", None)])?;
    let snapshot = layout(&document)?;
    let inside = position(&document, 0, 1)?;
    assert!(snapshot.caret(inside).is_err());
    bookmark(&mut document, "inside", inside, inside);
    assert_eq!(
        navigation_from_document(&document, &snapshot),
        Err(NavigationError::InvalidPosition)
    );
    Ok(())
}

#[test]
fn wrapped_source_rectangles_keep_each_page_and_raw_pdf_rect_transform() -> TestResult {
    let text = "AV ".repeat(25);
    let mut document = model(&[(&text, Some(anchor("start")?))])?;
    let snapshot = layout(&document)?;
    let start = position(&document, 0, 0)?;
    bookmark(&mut document, "start", start, start);
    let nav = navigation_from_document(&document, &snapshot).map_err(error)?;
    assert!(nav.links.len() > 2);
    let pdf =
        render_export_with_navigation(snapshot.pages(), &[font()?], &PdfExport::default(), &nav)
            .map_err(error)?;
    let read = inspect_navigation(&pdf).map_err(error)?;
    assert_eq!(read.links.len(), nav.links.len());
    for (link, rect) in read.links.iter().zip(&nav.links) {
        assert_eq!(link.source_page, rect.page);
        let height = to_points(snapshot.pages()[rect.page].height) as f32;
        for (actual, expected) in link.rect.into_iter().zip([
            rect.x,
            height - rect.y - rect.height,
            rect.x + rect.width,
            height - rect.y,
        ]) {
            near(actual, expected);
        }
    }
    Ok(())
}

fn scene(height: i64) -> PageScene {
    PageScene {
        width: pt(200),
        height: pt(height),
        runs: Vec::new(),
        rects: Vec::new(),
        images: Vec::new(),
    }
}
fn goto() -> PdfGoToLink {
    PdfGoToLink {
        page: 0,
        x: 20.0,
        y: 30.0,
        width: 40.0,
        height: 10.0,
        destination: PdfDestination {
            page: 1,
            x: 65.0,
            y: 75.0,
        },
    }
}

#[test]
fn destination_uses_target_page_height_and_accepts_legal_edge_points() -> TestResult {
    let pages = [scene(200), scene(300)];
    for (x, y) in [(65.0, 75.0), (0.0, 0.0), (200.0, 300.0)] {
        let mut link = goto();
        link.destination.x = x;
        link.destination.y = y;
        let nav = PdfNavigation {
            bookmarks: Vec::new(),
            links: vec![link],
        };
        let pdf = render_export_with_navigation(&pages, &[], &PdfExport::default(), &nav)
            .map_err(error)?;
        let read = inspect_navigation(&pdf).map_err(error)?;
        let dest = &read.links[0].destination;
        assert_eq!(dest.page, 1);
        near(dest.x, x);
        near(dest.y, 300.0 - y);
        assert_eq!(read.links[0].rect, [20.0, 160.0, 60.0, 170.0]);
    }
    Ok(())
}

#[test]
fn all_public_navigation_numbers_are_finite_in_range_and_bounded() {
    let pages = [scene(200), scene(300)];
    let reject = |link| {
        assert_eq!(
            render_export_with_navigation(
                &pages,
                &[],
                &PdfExport::default(),
                &PdfNavigation {
                    bookmarks: Vec::new(),
                    links: vec![link]
                }
            ),
            Err(PdfError::Navigation(NavigationError::InvalidGeometry))
        )
    };
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, f32::MAX] {
        let link = goto();
        reject(PdfGoToLink {
            x: value,
            ..link.clone()
        });
        reject(PdfGoToLink {
            y: value,
            ..link.clone()
        });
        reject(PdfGoToLink {
            width: value,
            ..link.clone()
        });
        reject(PdfGoToLink {
            height: value,
            ..link.clone()
        });
        reject(PdfGoToLink {
            destination: PdfDestination {
                x: value,
                ..link.destination
            },
            ..link.clone()
        });
        reject(PdfGoToLink {
            destination: PdfDestination {
                y: value,
                ..link.destination
            },
            ..link
        });
    }
    reject(PdfGoToLink {
        page: usize::MAX,
        ..goto()
    });
    reject(PdfGoToLink {
        destination: PdfDestination {
            page: 2,
            ..goto().destination
        },
        ..goto()
    });
    reject(PdfGoToLink {
        width: 0.0,
        ..goto()
    });
    reject(PdfGoToLink {
        height: 0.0,
        ..goto()
    });
    reject(PdfGoToLink {
        x: 199.0,
        width: 2.0,
        ..goto()
    });
    reject(PdfGoToLink {
        y: 199.0,
        height: 2.0,
        ..goto()
    });
    assert_eq!(
        render_export(&[scene(14_401)], &[], &PdfExport::default()),
        Err(PdfError::InvalidPage)
    );
    let export = PdfExport {
        outline: vec![OutlineEntry {
            title: "Heading".into(),
            level: 0,
            page: 0,
            top: f32::NAN,
        }],
        ..PdfExport::default()
    };
    assert_eq!(
        render_export(&pages, &[], &export),
        Err(PdfError::Navigation(NavigationError::InvalidGeometry))
    );
}

#[test]
fn existing_uri_api_keeps_allowlist_and_rejects_invalid_geometry() -> TestResult {
    let pages = [scene(200)];
    let mut link = PdfLink {
        page: 0,
        x: 2.0,
        y: 3.0,
        width: 5.0,
        height: 6.0,
        uri: "https://example.invalid/x".into(),
    };
    for uri in [
        "http://example.invalid",
        "https://example.invalid/a#b",
        "mailto:a@example.invalid",
    ] {
        link.uri = uri.into();
        let pdf = render_export(
            &pages,
            &[],
            &PdfExport {
                links: vec![link.clone()],
                ..PdfExport::default()
            },
        )
        .map_err(error)?;
        assert_eq!(inspect_pdf(&pdf).map_err(error)?.link_uris, [uri]);
    }
    for uri in [
        "javascript:alert(1)",
        "file:///tmp/a",
        "data:x",
        "target-bookmark",
        "https://user@example.invalid",
    ] {
        link.uri = uri.into();
        assert_eq!(
            render_export(
                &pages,
                &[],
                &PdfExport {
                    links: vec![link.clone()],
                    ..PdfExport::default()
                }
            ),
            Err(PdfError::Navigation(NavigationError::InvalidUri))
        );
    }
    link.uri = "https://example.invalid".into();
    link.width = f32::INFINITY;
    assert_eq!(
        render_export(
            &pages,
            &[],
            &PdfExport {
                links: vec![link],
                ..PdfExport::default()
            }
        ),
        Err(PdfError::Navigation(NavigationError::InvalidGeometry))
    );
    Ok(())
}

#[test]
fn bookmark_name_count_and_combined_rectangle_budgets_refuse_without_truncation() -> TestResult {
    let mut document = model(&[("AV", None)])?;
    let snapshot = layout(&document)?;
    let point = position(&document, 0, 0)?;
    for n in 0..MAX_PDF_BOOKMARKS {
        bookmark(&mut document, &format!("b{n:04}"), point, point);
    }
    assert_eq!(
        navigation_from_document(&document, &snapshot)
            .map_err(error)?
            .bookmarks
            .len(),
        MAX_PDF_BOOKMARKS
    );
    bookmark(&mut document, "too-many", point, point);
    assert_eq!(
        navigation_from_document(&document, &snapshot),
        Err(NavigationError::Budget)
    );
    let mut document = model(&[("AV", None)])?;
    bookmark(
        &mut document,
        &"x".repeat(MAX_PDF_NAME_BYTES + 1),
        point,
        point,
    );
    assert_eq!(
        navigation_from_document(&document, &snapshot),
        Err(NavigationError::Budget)
    );
    let pages = [scene(200), scene(300)];
    let nav = PdfNavigation {
        bookmarks: vec![],
        links: vec![goto(); MAX_PDF_LINKS + 1],
    };
    assert_eq!(
        render_export_with_navigation(&pages, &[], &PdfExport::default(), &nav),
        Err(PdfError::Navigation(NavigationError::Budget))
    );
    let nav = PdfNavigation {
        bookmarks: vec![],
        links: vec![goto(); MAX_PDF_LINKS],
    };
    let export = PdfExport {
        links: vec![PdfLink {
            page: 0,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            uri: "http://example.invalid".into(),
        }],
        ..PdfExport::default()
    };
    assert_eq!(
        render_export_with_navigation(&pages, &[], &export, &nav),
        Err(PdfError::Navigation(NavigationError::Budget))
    );
    let mark = PdfBookmark {
        name: "same".into(),
        destination: goto().destination,
    };
    let nav = PdfNavigation {
        bookmarks: vec![mark.clone(), mark],
        links: vec![],
    };
    assert_eq!(
        render_export_with_navigation(&pages, &[], &PdfExport::default(), &nav),
        Err(PdfError::Navigation(NavigationError::DuplicateName))
    );
    Ok(())
}

#[test]
fn wrapped_links_exhaust_the_rectangle_budget_before_the_span_budget() -> TestResult {
    let text = "AV ".repeat(12);
    let rows = vec![(text.as_str(), Some(anchor("start")?)); MAX_PDF_LINKS / 2];
    let mut document = model(&rows)?;
    let snapshot = layout(&document)?;
    assert!(snapshot.lines().len() > MAX_PDF_LINKS);
    let start = position(&document, 0, 0)?;
    bookmark(&mut document, "start", start, start);
    assert_eq!(
        navigation_from_document(&document, &snapshot),
        Err(NavigationError::Budget)
    );
    Ok(())
}

// Independently authored classic PDF fixture: page object numbers deliberately
// differ from page indices. Invalid /D refs still point to real objects.
fn classic(destination: &str) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Count 2 /Kids [3 0 R 5 0 R] >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots [4 0 R] >>".to_owned(),
        "<< /Subtype /Link /Rect [20 160 60 170] /A << /S /GoTo /D 6 0 R >> >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] >>".to_owned(),
        destination.to_owned(),
    ];
    let mut bytes = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (n, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", n + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    bytes
}

#[test]
fn independent_parser_checks_page_identity_xyz_kind_and_coordinate_range() -> TestResult {
    let report = inspect_navigation(&classic("[5 0 R /XYZ 65 225 null]")).map_err(error)?;
    assert_eq!(report.links[0].source_page, 0);
    assert_eq!(report.links[0].destination.page, 1);
    assert_eq!(report.links[0].destination.page_object, 5);
    assert_eq!(report.links[0].destination.y, 225.0);
    assert!(inspect_navigation(&classic("[4 0 R /XYZ 65 225 null]")).is_err());
    assert!(inspect_navigation(&classic("[99 0 R /XYZ 65 225 null]")).is_err());
    assert!(inspect_navigation(&classic("[5 0 R /Fit 65 225 null]")).is_err());
    assert!(inspect_navigation(&classic("[5 0 R /XYZ 65 301 null]")).is_err());
    assert!(inspect_navigation(&classic("[5 0 R /XYZ -1 225 null]")).is_err());
    Ok(())
}
