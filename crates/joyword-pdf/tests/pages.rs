//! T202: the PDF writer consumes the same page scene as the screen, including
//! the primary header.

use std::io::Read;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document, to_points,
};
use joyword_model::{
    BlockSeq, BreakKind, Document, HeaderFooterKind, InlineKind, InlineObject, Measure,
    NewspaperColumn, Paragraph, StoryKind, UnequalColumns,
};

use joyword_pdf::render;

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

#[test]
fn a_header_on_the_page_scene_is_the_pdf_page() {
    let mut page = joyword_model::PageSetup::a4_zh_cn();
    page.width = Measure::Twips(12_240);
    page.height = Measure::Twips(15_840);
    page.left = Measure::Twips(720);
    page.right = Measure::Twips(720);
    page.top = Measure::Twips(720);
    page.bottom = Measure::Twips(720);
    page.header = Measure::Twips(360);
    page.footer = Measure::Twips(360);
    page.gutter = Measure::Twips(0);
    let mut document = Document::new(page);
    let mut body = Paragraph::new(document.ids().node());
    body.push_text("V", None, Arc::default()).unwrap();
    let body_id = document.body_id();
    document.story_mut(body_id).unwrap().blocks = BlockSeq::from_vec(vec![body.into()]);
    let mut header = Paragraph::new(document.ids().node());
    header.push_text("A", None, Arc::default()).unwrap();
    let story = document.insert_story(StoryKind::Header, vec![header.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::Default, story));

    let profile = SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    };
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let scene = &snapshot.pages()[0];
    assert_eq!(scene.width, Measure::Twips(12_240).absolute().unwrap());
    assert_eq!(scene.height, Measure::Twips(15_840).absolute().unwrap());
    assert!(scene.runs.iter().any(|run| run.text.contains('A')));
    assert!(scene.runs.iter().any(|run| run.text.contains('V')));

    let pdf = render(snapshot.pages(), &[font()]).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let width = to_points(scene.width).round() as i32;
    let height = to_points(scene.height).round() as i32;
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains(&width.to_string()) && text.contains(&height.to_string()),
        "pdf page size {width}x{height} missing"
    );
}

#[test]
fn first_and_later_headers_share_one_page_scene_with_pdf() {
    let mut page = joyword_model::PageSetup::a4_zh_cn();
    page.width = Measure::Twips(12_240);
    page.height = Measure::Twips(1_800);
    page.left = Measure::Twips(720);
    page.right = Measure::Twips(720);
    page.top = Measure::Twips(720);
    page.bottom = Measure::Twips(720);
    page.header = Measure::Twips(360);
    page.footer = Measure::Twips(360);
    page.gutter = Measure::Twips(0);
    let mut document = Document::new(page);
    let mut first = Paragraph::new(document.ids().node());
    first.push_text("A", None, Arc::default()).unwrap();
    let mut second = Paragraph::new(document.ids().node());
    second.push_text("B", None, Arc::default()).unwrap();
    let body_id = document.body_id();
    document.story_mut(body_id).unwrap().blocks =
        BlockSeq::from_vec(vec![first.into(), second.into()]);
    document.sections_mut()[0].title_page = Some(true);
    let mut first_header = Paragraph::new(document.ids().node());
    first_header.push_text("H1", None, Arc::default()).unwrap();
    let first_story = document.insert_story(StoryKind::Header, vec![first_header.into()]);
    let mut later_header = Paragraph::new(document.ids().node());
    later_header.push_text("H0", None, Arc::default()).unwrap();
    let later_story = document.insert_story(StoryKind::Header, vec![later_header.into()]);
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::First, first_story));
    document.sections_mut()[0]
        .headers
        .push((HeaderFooterKind::Default, later_story));

    let profile = SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    };
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    assert_eq!(snapshot.pages().len(), 2);
    let first_page = &snapshot.pages()[0];
    let later_page = &snapshot.pages()[1];
    assert!(first_page.runs.iter().any(|run| run.text.contains("H1")));
    assert!(first_page.runs.iter().all(|run| !run.text.contains("H0")));
    assert!(later_page.runs.iter().any(|run| run.text.contains("H0")));
    assert!(later_page.runs.iter().all(|run| !run.text.contains("H1")));

    let pdf = render(snapshot.pages(), &[font()]).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    let width = to_points(first_page.width).round() as i32;
    let height = to_points(first_page.height).round() as i32;
    let text = String::from_utf8_lossy(&pdf);
    assert_eq!(
        text.matches(&format!("{width} {height}")).count(),
        2,
        "both page scenes must be PDF pages"
    );
}

#[test]
fn unequal_columns_share_one_page_scene_with_pdf() {
    let mut page = joyword_model::PageSetup::a4_zh_cn();
    page.width = Measure::Twips(12_240);
    page.height = Measure::Twips(15_840);
    page.left = Measure::Twips(720);
    page.right = Measure::Twips(720);
    page.top = Measure::Twips(720);
    page.bottom = Measure::Twips(720);
    page.header = Measure::Twips(360);
    page.footer = Measure::Twips(360);
    page.gutter = Measure::Twips(0);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("A", None, Arc::default()).unwrap();
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Break(BreakKind::Column),
            source: None,
        },
        None,
        Arc::default(),
    );
    paragraph.push_text("B", None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    document.sections_mut()[0].unequal_columns = Some(UnequalColumns {
        columns: vec![
            NewspaperColumn {
                width: 2880,
                space: 360,
            },
            NewspaperColumn {
                width: 5760,
                space: 0,
            },
        ],
    });
    let profile = SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    };
    let snapshot = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    assert_eq!(snapshot.pages().len(), 1);
    let runs = &snapshot.pages()[0].runs;
    assert_eq!(runs.len(), 2);
    assert!(runs[1].origin_x.raw() > runs[0].origin_x.raw());
    let pdf = render(snapshot.pages(), &[font()]).unwrap();
    let matrices = text_matrices(&pdf);
    for run in runs {
        let x = to_points(run.origin_x);
        let y = to_points(run.baseline_y);
        assert!(
            matrices.iter().any(|(mx, my)| near(*mx, x) && near(*my, y)),
            "no PDF text matrix within 0.01 pt of ({x}, {y}); saw {matrices:?}"
        );
    }
}

fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 0.01
}

fn inflate_streams(pdf: &[u8]) -> Vec<Vec<u8>> {
    let mut streams = Vec::new();
    let mut rest = pdf;
    while let Some(index) = rest.windows(6).position(|window| window == b"stream") {
        let after = &rest[index + 6..];
        let data = if after.starts_with(b"\r\n") {
            &after[2..]
        } else if let Some(stripped) = after.strip_prefix(b"\n") {
            stripped
        } else {
            rest = &rest[index + 6..];
            continue;
        };
        let mut decoder = flate2::read::ZlibDecoder::new(data);
        let mut inflated = Vec::new();
        if decoder.read_to_end(&mut inflated).is_ok() && !inflated.is_empty() {
            streams.push(inflated);
        }
        rest = if data.is_empty() { &[] } else { &data[1..] };
    }
    streams
}

fn text_matrices(pdf: &[u8]) -> Vec<(f64, f64)> {
    let mut matrices = Vec::new();
    for stream in inflate_streams(pdf) {
        let text = String::from_utf8_lossy(&stream);
        let tokens: Vec<&str> = text.split_whitespace().collect();
        for (index, token) in tokens.iter().enumerate() {
            let operator = *token == "Tm"
                || token.starts_with("Tm[")
                || token.starts_with("Tm(")
                || token.starts_with("Tm/");
            if !operator || index < 6 {
                continue;
            }
            let numbers: Vec<f64> = tokens[index - 6..index]
                .iter()
                .filter_map(|token| token.parse().ok())
                .collect();
            if numbers.len() == 6 {
                matrices.push((numbers[4], numbers[5]));
            }
        }
    }
    matrices
}
