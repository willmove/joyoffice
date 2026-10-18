//! Inline images share the page scene with text (LAYOUT-006).

use std::sync::Arc;

use joyword_engine::{Command, Selection, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LoadedFont, SingleFontProfile,
    layout_document,
};
use joyword_model::{
    Bias, BlockSeq, Document, InlineKind, Paragraph, Position, ScalarOffset, display_extent,
    sample_png_1x1,
};

fn font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

#[test]
fn an_inline_image_is_placed_in_the_page_scene() {
    let png = sample_png_1x1();
    let (width, height) = display_extent(1, 1);
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("A", None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let mut session = Session::new(document);
    let paragraph = session.document().body().blocks.get(0).unwrap().id();
    let story = session.document().body_id();
    let version = session.version();
    let at = Position {
        story,
        paragraph,
        offset: ScalarOffset(1),
        bias: Bias::After,
    };
    session
        .apply_with_mapped_selection(
            Transaction::new(
                version,
                vec![Command::InsertImage {
                    at,
                    data: Arc::from(png.as_slice()),
                }],
            ),
            Selection::caret(at),
        )
        .unwrap();
    let profile = SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    };
    let laid = layout_document(
        &session.document().snapshot(),
        FaceSet::uniform(&font()),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let page = &laid.pages()[0];
    assert_eq!(page.images.len(), 1);
    let image = &page.images[0];
    assert_eq!(image.width, width.absolute().unwrap());
    assert_eq!(image.height, height.absolute().unwrap());
    assert_eq!(image.media_type.as_ref(), "image/png");
    assert_eq!(image.bytes.as_ref(), png.as_slice());
    assert!(image.x.raw() > 0);
    assert!(image.y.raw() >= 0);
    let paragraph = session
        .document()
        .body()
        .blocks
        .get(0)
        .unwrap()
        .as_paragraph()
        .unwrap();
    assert!(matches!(
        paragraph.inlines()[0].kind,
        InlineKind::Image { .. }
    ));
}
