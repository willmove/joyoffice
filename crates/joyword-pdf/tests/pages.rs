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
