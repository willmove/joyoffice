//! T202: `w:cols/@w:sep` separator rules reach the PDF through the same shared
//! PageScene that the screen and the CPU rasterizer consume. The separator is
//! painted by layout into `PageScene::rects`, so the PDF writer needs no
//! separator-specific code; this test pins that end-to-end path.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document,
};
use joyword_model::{
    BlockSeq, BreakKind, Document, EqualColumns, InlineKind, InlineObject, Measure, Paragraph,
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

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn layout(document: &Document) -> LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

/// A two-equal-column document whose single paragraph is split by a column
/// break, with the separator flag toggled by `separator`.
fn two_column_document(separator: bool) -> Document {
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
    let section = &mut document.sections_mut()[0];
    section.columns = Some(EqualColumns {
        count: 2,
        space: 360,
    });
    section.column_separator = separator;
    document
}

#[test]
fn the_column_separator_rule_is_in_the_pdf_page_scene() {
    let snapshot = layout(&two_column_document(true));
    assert_eq!(snapshot.pages().len(), 1);
    let scene = &snapshot.pages()[0];

    // Exactly one black hairline rule, one point wide, spanning the body box.
    let rules: Vec<_> = scene
        .rects
        .iter()
        .filter(|rect| rect.width == Emu::new(Emu::PER_POINT))
        .collect();
    assert_eq!(rules.len(), 1, "scene rects: {:?}", scene.rects);
    let rule = rules[0];
    assert_eq!((rule.red, rule.green, rule.blue), (0, 0, 0));

    // Both column runs straddle the rule: column A ends left of it, column B
    // starts right of it.
    let runs = &scene.runs;
    assert_eq!(runs.len(), 2);
    assert!(runs[0].origin_x.raw() < rule.x.raw());
    assert!(runs[1].origin_x.raw() > rule.x.raw() + rule.width.raw());

    // The scene renders, and the rule is what makes the bytes differ from the
    // same page scene with the separator switched off.
    let painted = render(snapshot.pages(), &[font()]).unwrap();
    assert!(painted.starts_with(b"%PDF"));
    let plain_snapshot = layout(&two_column_document(false));
    assert!(
        plain_snapshot.pages()[0]
            .rects
            .iter()
            .all(|rect| rect.width != Emu::new(Emu::PER_POINT))
    );
    let plain = render(plain_snapshot.pages(), &[font()]).unwrap();
    assert_ne!(painted, plain, "separator must change the PDF bytes");
}
