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

#[test]
fn text_after_an_inline_image_stays_on_the_same_line() {
    let png = sample_png_1x1();
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("A", None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let mut session = Session::new(document);
    let paragraph = session.document().body().blocks.get(0).unwrap().id();
    let story = session.document().body_id();
    let at = Position {
        story,
        paragraph,
        offset: ScalarOffset(1),
        bias: Bias::After,
    };
    session
        .apply_with_mapped_selection(
            Transaction::new(
                session.version(),
                vec![Command::InsertImage {
                    at,
                    data: Arc::from(png.as_slice()),
                }],
            ),
            Selection::caret(at),
        )
        .unwrap();
    let after = Position {
        story,
        paragraph,
        offset: ScalarOffset(2),
        bias: Bias::After,
    };
    session
        .apply_with_mapped_selection(
            Transaction::new(
                session.version(),
                vec![Command::InsertText {
                    at: after,
                    text: "B".into(),
                }],
            ),
            Selection::caret(after),
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
    let before = page
        .runs
        .iter()
        .find(|run| run.text.contains('A'))
        .expect("A");
    let after = page
        .runs
        .iter()
        .find(|run| run.text.contains('B'))
        .expect("B");
    assert_eq!(before.baseline_y, after.baseline_y);
    let cluster = after
        .glyphs
        .iter()
        .position(|glyph| after.text[glyph.cluster..].starts_with('B'))
        .expect("B glyph");
    assert!(
        after.glyph_x(cluster).raw() >= image.x.raw() + image.width.raw(),
        "A {:?} B {:?} image {:?}",
        (before.origin_x, before.baseline_y, &before.text),
        (
            after.origin_x,
            after.glyph_x(cluster),
            after.baseline_y,
            &after.text
        ),
        (image.x, image.y, image.width, image.height)
    );
    assert!(after.baseline_y.raw() > image.y.raw());
    assert!(after.baseline_y.raw() < image.y.raw() + image.height.raw() + pt(18).raw());
}

#[test]
fn a_hyperlink_is_an_underline_in_the_page_scene() {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("go", None, Arc::default()).unwrap();
    let link = joyword_model::Hyperlink::for_edit(Some("https://example.invalid/a"), None).unwrap();
    paragraph
        .set_links(ScalarOffset(0)..ScalarOffset(2), Some(Arc::new(link)))
        .unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let profile = SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    };
    let laid = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let page = &laid.pages()[0];
    assert!(page.rects.iter().any(|rect| {
        rect.red == 0x05 && rect.green == 0x63 && rect.blue == 0xc1 && rect.width.raw() > 0
    }));
}

#[test]
fn a_hyperlink_in_a_table_cell_is_an_underline_in_the_page_scene() {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("go", None, Arc::default()).unwrap();
    let link = joyword_model::Hyperlink::for_edit(Some("https://example.invalid/a"), None).unwrap();
    paragraph
        .set_links(ScalarOffset(0)..ScalarOffset(2), Some(Arc::new(link)))
        .unwrap();
    let table = joyword_model::Table {
        id: document.ids().node(),
        grid: vec![joyword_model::Measure::Twips(2400)],
        rows: vec![joyword_model::Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![joyword_model::Cell {
                id: document.ids().node(),
                grid_span: 1,
                vertical_merge: joyword_model::VerticalMerge::None,
                width: Some(joyword_model::Measure::Twips(2400)),
                borders: joyword_model::CellBorders::default(),
                shading: None,
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![paragraph.into()]),
                preserved: None,
            }],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
        }],
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        source: None,
    };
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![table.into()]);
    let profile = SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    };
    let laid = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let page = &laid.pages()[0];
    assert!(page.rects.iter().any(|rect| {
        rect.red == 0x05 && rect.green == 0x63 && rect.blue == 0xc1 && rect.width.raw() > 0
    }));
}
