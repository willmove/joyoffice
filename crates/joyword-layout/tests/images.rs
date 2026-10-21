//! Inline images share the page scene with text (LAYOUT-006).

use std::sync::Arc;

use joyword_engine::{Command, Selection, Session, Transaction};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutNote, LoadedFont, PageScene,
    SingleFontProfile, layout_document,
};
use joyword_model::{
    AnchorWrap, Bias, BlockSeq, Document, HeaderFooterKind, ImageAnchor, InlineKind, InlineObject,
    Measure, PageSetup, Paragraph, Position, ScalarOffset, SquareWrap, StoryKind, TopBottomWrap,
    WrapSide, display_extent, sample_png_1x1,
};
use joyword_render::{Projection, RasterCancellation, RasterLimits, rasterize};

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

fn small_page() -> PageSetup {
    let mut page = PageSetup::a4_zh_cn();
    let pt = |value: i64| Measure::Emu(value * Emu::PER_POINT);
    page.width = pt(200);
    page.height = pt(200);
    page.left = pt(24);
    page.right = pt(24);
    page.top = pt(40);
    page.bottom = pt(40);
    page.header = pt(16);
    page.footer = pt(16);
    page.gutter = pt(0);
    page
}

fn profile() -> SingleFontProfile {
    SingleFontProfile {
        size: pt(12),
        leading: pt(18),
        baseline: pt(14),
        features: FeatureSet::default(),
    }
}

fn layout(document: &Document) -> joyword_layout::LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

fn push_picture(
    document: &mut Document,
    paragraph: &mut Paragraph,
    side: i64,
    anchor: Option<ImageAnchor>,
) {
    let resource = document.ids().resource();
    document
        .resources_mut()
        .insert(resource, "image/png", Arc::from(sample_png_1x1()), None);
    paragraph.push_inline(
        InlineObject {
            id: document.ids().node(),
            kind: InlineKind::Image {
                resource,
                width: Measure::Emu(side),
                height: Measure::Emu(side),
                anchor,
            },
            source: None,
        },
        None,
        Arc::default(),
    );
}

#[test]
fn a_page_anchor_is_a_positioned_rectangle_shared_with_the_cpu() {
    let side = 20 * Emu::PER_POINT;
    let origin_x = 72 * Emu::PER_POINT;
    let origin_y = 80 * Emu::PER_POINT;
    let mut document = Document::new(small_page());
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("A", None, Arc::default()).unwrap();
    push_picture(
        &mut document,
        &mut paragraph,
        side,
        Some(ImageAnchor {
            x: Measure::Emu(origin_x),
            y: Measure::Emu(origin_y),
            wrap: AnchorWrap::None,
        }),
    );
    paragraph.push_text("B", None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let laid = layout(&document);
    let page = &laid.pages()[0];
    assert_eq!(page.images.len(), 1);
    let image = &page.images[0];
    assert_eq!(image.x, Emu::new(origin_x));
    assert_eq!(image.y, Emu::new(origin_y));
    assert_eq!(image.width, Emu::new(side));
    assert_eq!(image.height, Emu::new(side));
    let letter = |ch: char| {
        page.runs
            .iter()
            .find(|run| run.text.contains(ch))
            .unwrap_or_else(|| panic!("{ch}"))
    };
    let a = letter('A');
    let b = letter('B');
    assert_eq!(a.baseline_y, b.baseline_y);
    assert!(a.origin_x.raw() < origin_x);
    assert!(b.origin_x.raw() < origin_x);
    assert_red_at(page, image.x, image.y, image.width);
    assert_white_at(page, Emu::new(Emu::PER_POINT), Emu::new(Emu::PER_POINT));
}

#[test]
fn a_wrap_square_keeps_text_outside_the_rectangle_and_wrap_none_does_not() {
    let image_x = 24 * Emu::PER_POINT;
    let image_y = 40 * Emu::PER_POINT;
    let side = 60 * Emu::PER_POINT;
    let dist_r = 6 * Emu::PER_POINT;
    let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".repeat(2);
    let square = layout(&square_page(
        &text,
        AnchorWrap::Square(SquareWrap {
            top: 0,
            bottom: 0,
            left: 0,
            right: dist_r,
            side: WrapSide::Larger,
        }),
        side,
        image_x,
        image_y,
    ));
    let geometry = square.page_geometry()[0];
    let page = &square.pages()[0];
    assert_eq!(page.images.len(), 1);
    let image = &page.images[0];
    assert_eq!(image.x, Emu::new(image_x));
    assert_eq!(image.y, Emu::new(image_y));
    assert_eq!(image.width, Emu::new(side));
    assert_eq!(image.height, Emu::new(side));
    let excl_left = image.x.raw();
    let excl_right = image.x.raw() + image.width.raw() + dist_r;
    let excl_top = image.y.raw();
    let excl_bottom = image.y.raw() + image.height.raw();
    let mut beside = false;
    let mut below = false;
    for placed in square.lines() {
        if placed.page != 0 {
            continue;
        }
        let run = &page.runs[placed.run];
        if run.glyphs.is_empty() {
            continue;
        }
        let band_top = placed.top.raw();
        let band_bottom = band_top + placed.height.raw();
        let overlaps = band_top < excl_bottom && band_bottom > excl_top;
        if overlaps {
            beside = true;
            for glyph in &run.glyphs {
                let left = run.origin_x.raw() + glyph.x.raw();
                let right = left + glyph.advance.raw();
                assert!(
                    right <= excl_left || left >= excl_right,
                    "glyph {left}..{right} entered {excl_left}..{excl_right}"
                );
            }
        } else if band_top >= excl_bottom {
            below = true;
            assert_eq!(run.origin_x, geometry.left);
        }
    }
    assert!(beside, "no line sat beside the square");
    assert!(below, "no line resumed under the square");
    assert_red_at(page, image.x, image.y, image.width);

    let plain = layout(&square_page(
        &text,
        AnchorWrap::None,
        side,
        image_x,
        image_y,
    ));
    let plain_page = &plain.pages()[0];
    let plain_image = &plain_page.images[0];
    assert_eq!(plain_image.x, image.x);
    assert_eq!(plain_image.y, image.y);
    assert_eq!(plain_image.width, image.width);
    assert_eq!(plain_image.height, image.height);
    let mut through = false;
    for placed in plain.lines() {
        let run = &plain_page.runs[placed.run];
        let band_top = placed.top.raw();
        let band_bottom = band_top + placed.height.raw();
        if band_top >= plain_image.y.raw() + plain_image.height.raw()
            || band_bottom <= plain_image.y.raw()
        {
            continue;
        }
        for glyph in &run.glyphs {
            let left = run.origin_x.raw() + glyph.x.raw();
            let right = left + glyph.advance.raw();
            if left < plain_image.x.raw() + plain_image.width.raw() && right > plain_image.x.raw() {
                through = true;
            }
        }
    }
    assert!(through, "wrapNone text should still cross the rectangle");
}

#[test]
fn a_wrap_top_and_bottom_pushes_overlapping_lines_below_and_square_stays_beside() {
    let image_x = 24 * Emu::PER_POINT;
    let image_y = 100 * Emu::PER_POINT;
    let side = 48 * Emu::PER_POINT;
    let dist_t = 6 * Emu::PER_POINT;
    let dist_b = 6 * Emu::PER_POINT;
    let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".repeat(6);
    let laid = layout(&square_page(
        &text,
        AnchorWrap::TopAndBottom(TopBottomWrap {
            top: dist_t,
            bottom: dist_b,
        }),
        side,
        image_x,
        image_y,
    ));
    let geometry = laid.page_geometry()[0];
    let page = &laid.pages()[0];
    assert_eq!(page.images.len(), 1);
    let image = &page.images[0];
    assert_eq!(image.x, Emu::new(image_x));
    assert_eq!(image.y, Emu::new(image_y));
    assert_eq!(image.width, Emu::new(side));
    assert_eq!(image.height, Emu::new(side));
    let band_top = image.y.raw() - dist_t;
    let band_bottom = image.y.raw() + image.height.raw() + dist_b;
    let mut above = false;
    let mut below = false;
    for placed in laid.lines() {
        if placed.page != 0 {
            continue;
        }
        let run = &page.runs[placed.run];
        if run.glyphs.is_empty() {
            continue;
        }
        let line_top = placed.top.raw();
        let line_bottom = line_top + placed.height.raw();
        assert!(
            line_bottom <= band_top || line_top >= band_bottom,
            "line {line_top}..{line_bottom} met the band {band_top}..{band_bottom}"
        );
        assert_eq!(run.origin_x, geometry.left);
        if line_bottom <= band_top {
            above = true;
        }
        if line_top >= band_bottom {
            below = true;
        }
    }
    assert!(above, "no line stayed above the band");
    assert!(below, "no line resumed under the band");
    assert_red_at(page, image.x, image.y, image.width);

    let square = layout(&square_page(
        &text,
        AnchorWrap::Square(SquareWrap {
            top: dist_t,
            bottom: dist_b,
            left: 0,
            right: 0,
            side: WrapSide::Larger,
        }),
        side,
        image_x,
        image_y,
    ));
    let square_page = &square.pages()[0];
    let square_image = &square_page.images[0];
    assert_eq!(square_image.x, image.x);
    assert_eq!(square_image.y, image.y);
    let mut beside = false;
    for placed in square.lines() {
        if placed.page != 0 {
            continue;
        }
        let run = &square_page.runs[placed.run];
        let line_top = placed.top.raw();
        let line_bottom = line_top + placed.height.raw();
        if line_top >= square_image.y.raw() + square_image.height.raw()
            || line_bottom <= square_image.y.raw()
        {
            continue;
        }
        for glyph in &run.glyphs {
            let left = run.origin_x.raw() + glyph.x.raw();
            let right = left + glyph.advance.raw();
            if right <= square_image.x.raw()
                || left >= square_image.x.raw() + square_image.width.raw()
            {
                beside = true;
            }
        }
    }
    assert!(
        beside,
        "wrapSquare should still keep glyphs beside the rectangle"
    );
}

fn square_page(text: &str, wrap: AnchorWrap, side: i64, x: i64, y: i64) -> Document {
    let mut page = small_page();
    page.height = Measure::Emu(360 * Emu::PER_POINT);
    let mut document = Document::new(page);
    let mut paragraph = Paragraph::new(document.ids().node());
    push_picture(
        &mut document,
        &mut paragraph,
        side,
        Some(ImageAnchor {
            x: Measure::Emu(x),
            y: Measure::Emu(y),
            wrap,
        }),
    );
    paragraph.push_text(text, None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    document
}

#[test]
fn a_header_and_footer_image_paint_on_their_bands() {
    let side = 12 * Emu::PER_POINT;
    let mut document = Document::new(small_page());
    let mut body = Paragraph::new(document.ids().node());
    body.push_text("Body", None, Arc::default()).unwrap();
    let body_id = document.body_id();
    document.story_mut(body_id).unwrap().blocks = BlockSeq::from_vec(vec![body.into()]);
    for header in [true, false] {
        let mut paragraph = Paragraph::new(document.ids().node());
        push_picture(&mut document, &mut paragraph, side, None);
        let id = document.insert_story(
            if header {
                StoryKind::Header
            } else {
                StoryKind::Footer
            },
            vec![paragraph.into()],
        );
        let refs = if header {
            &mut document.sections_mut()[0].headers
        } else {
            &mut document.sections_mut()[0].footers
        };
        refs.push((HeaderFooterKind::Default, id));
    }
    let laid = layout(&document);
    assert!(
        !laid
            .notes()
            .iter()
            .any(|note| matches!(note, LayoutNote::StoryContentSkipped { .. }))
    );
    let geometry = laid.page_geometry()[0];
    let page = &laid.pages()[0];
    assert_eq!(page.images.len(), 2);
    let header = page
        .images
        .iter()
        .find(|image| image.y.raw() < geometry.top.raw())
        .expect("header image");
    let footer = page
        .images
        .iter()
        .find(|image| image.y.raw() >= geometry.height.raw() - geometry.bottom.raw())
        .expect("footer image");
    assert_eq!(header.x, geometry.left);
    assert_eq!(footer.x, geometry.left);
    assert!(header.y.raw() >= geometry.header.raw());
    assert!(header.y.raw() + header.height.raw() <= geometry.top.raw());
    assert!(footer.y.raw() + footer.height.raw() <= geometry.height.raw());
    assert_eq!(header.width, Emu::new(side));
    assert_eq!(footer.height, Emu::new(side));
    assert_red_at(page, header.x, header.y, header.width);
    assert_red_at(page, footer.x, footer.y, footer.width);
}

fn assert_red_at(page: &PageScene, x: Emu, y: Emu, side: Emu) {
    let raster = rasterize(
        page,
        &[font()],
        Projection::new(72.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let px = |length: Emu| (length.raw() as f64 / Emu::PER_INCH as f64 * 72.0).round() as usize;
    let column = px(x) + px(side) / 2;
    let row = px(y) + px(side) / 2;
    let width = raster.width() as usize;
    let pixel = &raster.rgba()[(row * width + column) * 4..][..4];
    assert!(
        pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40 && pixel[3] == 255,
        "pixel at {column},{row} is {pixel:?}"
    );
}

fn assert_white_at(page: &PageScene, x: Emu, y: Emu) {
    let raster = rasterize(
        page,
        &[font()],
        Projection::new(72.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let px = |length: Emu| (length.raw() as f64 / Emu::PER_INCH as f64 * 72.0).round() as usize;
    let width = raster.width() as usize;
    let pixel = &raster.rgba()[(px(y) * width + px(x)) * 4..][..4];
    assert_eq!(pixel, &[255, 255, 255, 255]);
}
