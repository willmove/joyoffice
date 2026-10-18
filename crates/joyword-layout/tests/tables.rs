//! T201: cell paragraphs share the page scene with body text. A table is
//! laid out in full on the incremental path.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutInvalidation, LayoutLimits, LayoutReuse,
    LoadedFont, ParagraphLayoutCache, SingleFontProfile, layout_document, layout_incremental,
};
use joyword_model::{
    Block, BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, Color, Document, Measure,
    Paragraph, Row, Shading, Table, VerticalMerge,
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

fn edge() -> BorderEdge {
    BorderEdge {
        kind: BorderKind::Single,
        size_eighths: 4,
        color: Color::Auto,
    }
}

fn document() -> Document {
    let mut document = Document::new_blank();
    let mut left = Paragraph::new(document.ids().node());
    left.push_text("A", None, Arc::default()).unwrap();
    let mut right = Paragraph::new(document.ids().node());
    right.push_text("B", None, Arc::default()).unwrap();
    let borders = CellBorders {
        top: Some(edge()),
        left: Some(edge()),
        bottom: Some(edge()),
        right: Some(edge()),
    };
    let left_id = document.ids().node();
    let right_id = document.ids().node();
    let row_id = document.ids().node();
    let table_id = document.ids().node();
    let cell = |id, paragraph: Paragraph| Cell {
        id,
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: Some(Measure::Twips(2400)),
        borders,
        shading: Some(Shading {
            fill: Color::Rgb(255, 0, 0),
        }),
        blocks: BlockSeq::from_vec(vec![paragraph.into()]),
        preserved: None,
    };
    let table = Table {
        id: table_id,
        grid: vec![Measure::Twips(2400), Measure::Twips(2400)],
        rows: vec![Row {
            id: row_id,
            grid_before: 0,
            grid_after: 0,
            cells: vec![cell(left_id, left), cell(right_id, right)],
            header: false,
            preserved: None,
        }],
        style: None,
        width: None,
        preserved: None,
        source: None,
    };
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![table.into()]);
    document
}

#[test]
fn two_cells_place_glyphs_at_different_x_and_paint_borders() {
    let document = document();
    let laid = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let page = &laid.pages()[0];
    assert_eq!(page.runs.len(), 2);
    assert_eq!(page.runs[0].text, "A");
    assert_eq!(page.runs[1].text, "B");
    let shift = 2400 * Emu::PER_TWIP;
    assert_eq!(
        page.runs[1].origin_x.raw() - page.runs[0].origin_x.raw(),
        shift
    );
    assert!(page.runs[0].glyph_x(0).raw() < page.runs[1].glyph_x(0).raw());
    assert!(
        page.rects
            .iter()
            .any(|rect| rect.red == 255 && rect.green == 0)
    );
    assert!(
        page.rects
            .iter()
            .any(|rect| rect.red == 0 && rect.green == 0 && rect.blue == 0)
    );
}

#[test]
fn incremental_layout_of_a_table_matches_a_full_layout() {
    let document = document();
    let full = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let mut cache = ParagraphLayoutCache::default();
    let incremental = layout_incremental(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        LayoutReuse {
            cache: &mut cache,
            previous: None,
            invalidation: &LayoutInvalidation::default(),
        },
    )
    .unwrap();
    assert_eq!(incremental.pages(), full.pages());
}

#[test]
fn a_paragraph_only_document_still_places_text_at_the_left_margin() {
    let mut document = Document::new_blank();
    let mut paragraph = Paragraph::new(document.ids().node());
    paragraph.push_text("A", None, Arc::default()).unwrap();
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![paragraph.into()]);
    let laid = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let page = &laid.pages()[0];
    assert!(page.rects.is_empty());
    assert_eq!(page.runs.len(), 1);
    assert_eq!(page.runs[0].origin_x.raw(), 1800 * Emu::PER_TWIP);
    assert!(matches!(
        document.body().blocks.get(0),
        Some(Block::Paragraph(_))
    ));
}
