//! T301: the pending table property revision's margin bar reaches the PDF
//! through the same shared PageScene the screen consumes. The bar is painted
//! by layout into `PageScene::rects` left of the content edge, so the PDF
//! writer needs no bar-specific code; this test pins that end-to-end path.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document,
};
use joyword_model::{
    BlockSeq, Cell, CellBorders, Document, Measure, PageSetup, Paragraph, PreviousTableProps, Row,
    RowChangeProps, Table, TablePropsRevision, VerticalMerge,
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

fn page() -> PageSetup {
    let mut page = PageSetup::a4_zh_cn();
    page.left = Measure::Twips(1440);
    page.right = Measure::Twips(1440);
    page.gutter = Measure::Twips(0);
    page
}

/// One two-row table whose second row carries a pending `w:trPrChange`.
fn document(with_record: bool) -> Document {
    let mut document = Document::new(page());
    let ids = document.ids();
    let mut rows = Vec::new();
    for text in ["第一行", "第二行"] {
        let left = {
            let mut paragraph = Paragraph::new(ids.node());
            paragraph.push_text(text, None, Arc::default()).unwrap();
            paragraph
        };
        let left_cell = Cell {
            id: ids.node(),
            grid_span: 1,
            vertical_merge: VerticalMerge::None,
            width: Some(Measure::Twips(4000)),
            borders: CellBorders::default(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![left.into()]),
            preserved: None,
            props_revision: None,
        };
        let right = {
            let mut paragraph = Paragraph::new(ids.node());
            paragraph.push_text("右", None, Arc::default()).unwrap();
            paragraph
        };
        let right_cell = Cell {
            id: ids.node(),
            grid_span: 1,
            vertical_merge: VerticalMerge::None,
            width: Some(Measure::Twips(4000)),
            borders: CellBorders::default(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![right.into()]),
            preserved: None,
            props_revision: None,
        };
        rows.push(Row {
            id: ids.node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![left_cell, right_cell],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
            props_revision: None,
        });
    }
    if with_record {
        rows[1].props_revision = Some(Arc::new(TablePropsRevision {
            id: 31,
            author: Arc::from("作者甲"),
            date: None,
            previous: PreviousTableProps::Row(RowChangeProps::default()),
        }));
    }
    let table = Table {
        id: ids.node(),
        grid: vec![Measure::Twips(4000); 2],
        rows,
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        props_revision: None,
        source: None,
    };
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![table.into()]);
    document
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

#[test]
fn the_table_revision_margin_bar_is_in_the_pdf_page_scene() {
    // The default view is final-with-markup: the pending row record paints
    // exactly one bar, left of the content edge, in the second row's band.
    let pending = layout(&document(true));
    let resolved = layout(&document(false));
    let geometry = pending.page_geometry()[0];
    let bars: Vec<_> = pending.pages()[0]
        .rects
        .iter()
        .filter(|rect| rect.x.raw() + rect.width.raw() <= geometry.left.raw())
        .collect();
    assert_eq!(bars.len(), 1, "rects: {:?}", pending.pages()[0].rects);
    assert!(
        resolved.pages()[0]
            .rects
            .iter()
            .all(|rect| rect.x.raw() + rect.width.raw() > geometry.left.raw()),
        "the accepted state paints no bar"
    );
    // The scene renders, and the bar is what makes the bytes differ from the
    // accepted state of the same table.
    let painted = render(pending.pages(), &[font()]).unwrap();
    assert!(painted.starts_with(b"%PDF"));
    let plain = render(resolved.pages(), &[font()]).unwrap();
    assert_ne!(painted, plain, "the margin bar must change the PDF bytes");
}
