//! T202: the PDF writer consumes the same page scene as the screen, including
//! the primary header.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document, to_points,
};
use joyword_model::{BlockSeq, Document, HeaderFooterKind, Measure, Paragraph, StoryKind};
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
