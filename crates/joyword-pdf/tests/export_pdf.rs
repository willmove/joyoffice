//! T204: document info, heading outlines, section page boxes, image XObjects,
//! subset fonts with ToUnicode, and bytes that do not contain a clock.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, PageScene, SceneImage,
    SingleFontProfile, layout_document, to_points,
};
use joyword_model::{
    BlockSeq, BuiltinStyle, Document, Measure, Orientation, PageSetup, ParaProps, Paragraph, Style,
    StyleId, StyleKind, sample_png_1x1,
};
use joyword_pdf::{
    PRODUCER, PdfExport, PdfReport, core_title, inspect_pdf, outline_from_headings, render,
    render_export,
};

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

fn font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn page_for(lines: i64) -> PageSetup {
    let mut page = PageSetup::a4_zh_cn();
    page.width = Measure::Emu(pt(100).raw());
    page.height = Measure::Emu(pt(12 + 18 * lines).raw());
    page.left = Measure::Emu(pt(6).raw());
    page.right = page.left;
    page.top = page.left;
    page.bottom = page.left;
    page.header = Measure::Emu(pt(2).raw());
    page.footer = Measure::Emu(pt(2).raw());
    page.gutter = Measure::Emu(0);
    page
}

fn text_scene(text: &str) -> (PageScene, LoadedFont) {
    let font = font();
    let shaped = joyword_layout::shape(&font, text, pt(12), FeatureSet::default());
    let run = joyword_layout::GlyphRun::place(&shaped, pt(12), pt(24));
    let scene = PageScene {
        width: pt(200),
        height: pt(200),
        runs: vec![run],
        rects: Vec::new(),
        images: Vec::new(),
    };
    (scene, font)
}

fn export_of(scene: &PageScene, font: &LoadedFont, export: &PdfExport) -> Vec<u8> {
    render_export(
        std::slice::from_ref(scene),
        std::slice::from_ref(font),
        export,
    )
    .unwrap()
}

#[test]
fn two_renders_match_and_carry_no_clock() {
    let (scene, font) = text_scene("A");
    let export = PdfExport {
        title: Some("Contract".into()),
        outline: Vec::new(),
        links: Vec::new(),
    };
    let first = export_of(&scene, &font, &export);
    let second = export_of(&scene, &font, &export);
    assert_eq!(first, second);
    let report = inspect_pdf(&first).unwrap();
    assert_eq!(report.producer.as_deref(), Some(PRODUCER));
    assert_eq!(report.title.as_deref(), Some("Contract"));
    assert!(!report.has_creation_date);
    assert!(!report.has_mod_date);
    let text = String::from_utf8_lossy(&first);
    assert!(!text.contains("CreationDate"));
    assert!(!text.contains("ModDate"));
    assert!(report.fonts > 0);
    assert_eq!(report.fonts, report.fonts_with_to_unicode);
    assert_eq!(report.pages.len(), 1);
}

#[test]
fn plain_render_still_names_the_producer_without_a_title() {
    let (scene, font) = text_scene("A");
    let pdf = render(std::slice::from_ref(&scene), std::slice::from_ref(&font)).unwrap();
    let report = inspect_pdf(&pdf).unwrap();
    assert_eq!(report.producer.as_deref(), Some(PRODUCER));
    assert_eq!(report.title, None);
    assert!(report.outlines.is_empty());
}

#[test]
fn heading_styles_become_bookmarks_and_a_bare_outline_level_does_not() {
    let mut document = Document::new(page_for(6));
    let heading = Style::new(
        "H1",
        StyleKind::Paragraph,
        Some(&BuiltinStyle::Heading(1).canonical_name()),
    );
    let mut nested = Style::new("Mine", StyleKind::Paragraph, Some("Mine"));
    nested.based_on = Some(StyleId::new("H1"));
    let body = Style::new("Body", StyleKind::Paragraph, Some("Normal"));
    document.styles_mut().insert(heading).unwrap();
    document.styles_mut().insert(nested).unwrap();
    document.styles_mut().insert(body).unwrap();
    let mut blocks = Vec::new();
    for (text, style, level) in [
        ("A", Some("H1"), None),
        ("B", Some("Mine"), None),
        ("H", Some("Body"), Some(0)),
        ("", Some("H1"), None),
    ] {
        let mut paragraph = Paragraph::new(document.ids().node());
        if !text.is_empty() {
            paragraph.push_text(text, None, Arc::default()).unwrap();
        }
        paragraph.style = style.map(StyleId::new);
        if let Some(level) = level {
            paragraph.props = Arc::new(ParaProps {
                outline_level: Some(level),
                ..ParaProps::default()
            });
        }
        blocks.push(paragraph.into());
    }
    let body_id = document.body_id();
    document.story_mut(body_id).unwrap().blocks = BlockSeq::from_vec(blocks);
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let outline = outline_from_headings(&document, &snapshot);
    assert_eq!(
        outline
            .iter()
            .map(|entry| entry.title.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "B"]
    );
    assert_eq!(outline[0].level, 0);
    assert_eq!(outline[1].level, 0);
    let pdf = render_export(
        snapshot.pages(),
        std::slice::from_ref(&font()),
        &PdfExport {
            title: None,
            outline,
            links: Vec::new(),
        },
    )
    .unwrap();
    let report = inspect_pdf(&pdf).unwrap();
    assert_eq!(report.outlines, vec!["A".to_string(), "B".to_string()]);
}

#[test]
fn an_https_link_is_a_uri_annotation_and_a_script_is_not() {
    let mut document = Document::new(page_for(4));
    let mut allowed = Paragraph::new(document.ids().node());
    allowed.push_text("A", None, Arc::default()).unwrap();
    let https =
        joyword_model::Hyperlink::for_edit(Some("https://example.invalid/a"), None).unwrap();
    allowed
        .set_links(
            joyword_model::ScalarOffset(0)..joyword_model::ScalarOffset(1),
            Some(Arc::new(https)),
        )
        .unwrap();
    let mut refused = Paragraph::new(document.ids().node());
    refused.push_text("B", None, Arc::default()).unwrap();
    let script =
        joyword_model::Hyperlink::imported(None, Some(Arc::from("javascript:alert(1)")), None)
            .unwrap();
    refused
        .set_links(
            joyword_model::ScalarOffset(0)..joyword_model::ScalarOffset(1),
            Some(Arc::new(script)),
        )
        .unwrap();
    let mut bookmark = Paragraph::new(document.ids().node());
    bookmark.push_text("H", None, Arc::default()).unwrap();
    let anchor = joyword_model::Hyperlink::for_edit(None, Some("top")).unwrap();
    bookmark
        .set_links(
            joyword_model::ScalarOffset(0)..joyword_model::ScalarOffset(1),
            Some(Arc::new(anchor)),
        )
        .unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks =
        BlockSeq::from_vec(vec![allowed.into(), refused.into(), bookmark.into()]);
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let links = joyword_pdf::links_from_document(&document, &snapshot);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].uri, "https://example.invalid/a");
    assert!(links[0].width > 0.0 && links[0].height > 0.0);
    let export = PdfExport {
        title: None,
        outline: Vec::new(),
        links: links.clone(),
    };
    let font = font();
    let first = render_export(snapshot.pages(), std::slice::from_ref(&font), &export).unwrap();
    let second = render_export(snapshot.pages(), std::slice::from_ref(&font), &export).unwrap();
    assert_eq!(first, second);
    let report = inspect_pdf(&first).unwrap();
    assert_eq!(
        report.link_uris,
        vec!["https://example.invalid/a".to_string()]
    );
}

#[test]
fn each_section_keeps_its_page_box() {
    let mut document = Document::new(page_for(2));
    let mut blocks = Vec::new();
    for text in ["A", "B"] {
        let mut paragraph = Paragraph::new(document.ids().node());
        paragraph.push_text(text, None, Arc::default()).unwrap();
        blocks.push(paragraph.into());
    }
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(blocks);
    let boundary = document.body().blocks.get(0).unwrap().id();
    let mut next = document.sections()[0].clone();
    next.id = document.ids().node();
    next.last_block = None;
    next.page.width = Measure::Twips(10_000);
    next.page.height = Measure::Twips(8_000);
    next.page.orientation = Orientation::Landscape;
    next.page.left = Measure::Twips(720);
    next.page.right = Measure::Twips(720);
    next.page.top = Measure::Twips(720);
    next.page.bottom = Measure::Twips(720);
    document.sections_mut()[0].last_block = Some(boundary);
    document.sections_mut().push(next);
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    assert!(snapshot.pages().len() >= 2);
    let pdf = render(snapshot.pages(), std::slice::from_ref(&font())).unwrap();
    let report = inspect_pdf(&pdf).unwrap();
    assert_eq!(report.pages.len(), snapshot.pages().len());
    for (scene, (width, height)) in snapshot.pages().iter().zip(&report.pages) {
        assert!((f64::from(*width) - to_points(scene.width)).abs() < 0.2);
        assert!((f64::from(*height) - to_points(scene.height)).abs() < 0.2);
    }
    assert_ne!(report.pages[0], report.pages[1]);
    let stored_width = Measure::Twips(10_000).absolute().unwrap();
    let stored_height = Measure::Twips(8_000).absolute().unwrap();
    let later = snapshot
        .pages()
        .iter()
        .position(|page| page.width == stored_width)
        .expect("later section page");
    assert_eq!(snapshot.pages()[later].height, stored_height);
    assert!(
        (f64::from(report.pages[later].0) - to_points(stored_width)).abs() < 0.2
            && (f64::from(report.pages[later].1) - to_points(stored_height)).abs() < 0.2,
        "landscape orientation must not swap the stored page box"
    );
}

#[test]
fn an_image_is_an_xobject() {
    let page = PageScene {
        width: pt(72),
        height: pt(72),
        runs: Vec::new(),
        rects: Vec::new(),
        images: vec![SceneImage {
            media_type: Arc::from("image/png"),
            bytes: Arc::from(sample_png_1x1().as_slice()),
            x: pt(1),
            y: pt(1),
            width: pt(36),
            height: pt(36),
        }],
    };
    let pdf = render(std::slice::from_ref(&page), &[]).unwrap();
    let report = inspect_pdf(&pdf).unwrap();
    assert!(report.image_xobjects >= 1);
}

#[test]
fn core_title_reads_dublin_core_and_rejects_active_markup() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties">
  <dc:title>Contract &amp; A</dc:title>
</cp:coreProperties>"#;
    assert_eq!(core_title(xml).as_deref(), Some("Contract & A"));
    assert_eq!(
        core_title(b"<!DOCTYPE dc:title [<!ENTITY e 'x'>]><dc:title>&e;</dc:title>"),
        None
    );
    assert_eq!(
        core_title(br#"<dc:title xmlns:dc="http://purl.org/dc/elements/1.1/"><b>A</b></dc:title>"#),
        None
    );
    assert_eq!(core_title(b""), None);
}

#[test]
fn a_classic_cross_reference_file_reports_structure_without_the_writer() {
    let pdf = classic_contract();
    let report = inspect_pdf(&pdf).unwrap();
    let expected = PdfReport {
        pages: vec![(612.0, 792.0)],
        producer: Some("JoyWord".into()),
        title: Some("Contract".into()),
        outlines: vec!["Chapter".into()],
        fonts: 1,
        fonts_with_to_unicode: 1,
        image_xobjects: 1,
        link_uris: Vec::new(),
        has_creation_date: false,
        has_mod_date: false,
    };
    assert_eq!(report, expected);
}

#[test]
fn pdftotext_reads_the_scene_text() {
    let (scene, font) = text_scene("A");
    let pdf = render(std::slice::from_ref(&scene), std::slice::from_ref(&font)).unwrap();
    let Some(text) = pdftotext(&pdf) else {
        return;
    };
    assert!(text.contains('A'), "{text:?}");
}

fn pdftotext(pdf: &[u8]) -> Option<String> {
    let require = std::env::var_os("JOYWORD_REQUIRE_TEST_FONT").is_some();
    let mut child = match Command::new("pdftotext")
        .args(["-enc", "UTF-8", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            assert!(!require, "pdftotext is required");
            eprintln!("skipped: pdftotext is not installed");
            return None;
        }
    };
    let mut stdin = child.stdin.take().expect("pdftotext stdin");
    let wrote = stdin.write_all(pdf);
    drop(stdin);
    let output = child.wait_with_output().expect("pdftotext wait");
    if wrote.is_err() || !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !require,
            "pdftotext failed ({:?}): {stderr}",
            output.status.code()
        );
        eprintln!("skipped: pdftotext could not read the PDF");
        return None;
    }
    match String::from_utf8(output.stdout) {
        Ok(text) => Some(text),
        Err(_) => {
            assert!(!require, "pdftotext stdout was not UTF-8");
            None
        }
    }
}

fn classic_contract() -> Vec<u8> {
    let objects = [
        "",
        "<< /Type /Catalog /Pages 2 0 R /Outlines 5 0 R >>",
        "<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> /XObject << /Im1 7 0 R >> >> >>",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 6 0 R >>",
        "<< /Type /Outlines /First 8 0 R /Last 8 0 R /Count 1 >>",
        "<< /Length 0 >>\nstream\nendstream",
        "<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Length 1 >>\nstream\nX\nendstream",
        "<< /Title (Chapter) /Parent 5 0 R /Dest [3 0 R /XYZ 0 792 0] >>",
        "<< /Title (Contract) /Producer (JoyWord) >>",
    ];
    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets = [0usize; 10];
    for number in 1..10 {
        offsets[number] = out.len();
        out.extend_from_slice(format!("{number} 0 obj\n{}\nendobj\n", objects[number]).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(b"xref\n0 10\n");
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size 10 /Root 1 0 R /Info 9 0 R >>\nstartxref\n{xref}\n%%EOF\n")
            .as_bytes(),
    );
    out
}
