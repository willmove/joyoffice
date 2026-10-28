//! T301: pending table property revisions (`w:tblPrChange` / `w:trPrChange`
//! / `w:tcPrChange`) carry the author-colour margin bar beside every visual
//! row band they cover — table records mark every row, row records their own
//! band, cell records their row's band. The bar exists only in markup and
//! simple views, repeated header bands keep it across pages, resolving the
//! record removes it, and both layout entries paint identical pages.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutInvalidation, LayoutLimits, LayoutReuse,
    LayoutSnapshot, LoadedFont, PageGeometry, PageScene, ParagraphLayoutCache, SceneRect,
    SingleFontProfile, layout_document_viewed, layout_incremental,
};
use joyword_model::{
    Block, BlockSeq, Cell, CellBorders, CellChangeProps, Document, IdAllocator, Measure, PageSetup,
    Paragraph, PreviousTableProps, RevisionView, Row, RowChangeProps, Table, TableChangeProps,
    TablePropsRevision, VerticalMerge,
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

fn page() -> PageSetup {
    let mut page = PageSetup::a4_zh_cn();
    page.left = Measure::Twips(1440);
    page.right = Measure::Twips(1440);
    page.top = Measure::Twips(1440);
    page.bottom = Measure::Twips(1440);
    page.gutter = Measure::Twips(0);
    page
}

fn revision(id: u32, author: &str, previous: PreviousTableProps) -> Arc<TablePropsRevision> {
    Arc::new(TablePropsRevision {
        id,
        author: Arc::from(author),
        date: Some(Arc::from("2020-01-02T03:04:05Z")),
        previous,
    })
}

fn paragraph(ids: &mut IdAllocator, text: &str) -> Paragraph {
    let mut paragraph = Paragraph::new(ids.node());
    paragraph.push_text(text, None, Arc::default()).unwrap();
    paragraph
}

fn cell(ids: &mut IdAllocator, blocks: Vec<Block>) -> Cell {
    Cell {
        id: ids.node(),
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: Some(Measure::Twips(4000)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(blocks),
        preserved: None,
        props_revision: None,
    }
}

fn row(ids: &mut IdAllocator, cells: Vec<Cell>) -> Row {
    Row {
        id: ids.node(),
        grid_before: 0,
        grid_after: 0,
        cells,
        header: false,
        cant_split: false,
        height: None,
        preserved: None,
        props_revision: None,
    }
}

fn table(ids: &mut IdAllocator, rows: Vec<Row>) -> Table {
    Table {
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
    }
}

fn two_cell_row(ids: &mut IdAllocator, left: &str, right: &str) -> Row {
    let left_paragraph = paragraph(ids, left);
    let left_cell = cell(ids, vec![left_paragraph.into()]);
    let right_paragraph = paragraph(ids, right);
    let right_cell = cell(ids, vec![right_paragraph.into()]);
    row(ids, vec![left_cell, right_cell])
}

fn lay(document: &Document, view: RevisionView) -> LayoutSnapshot {
    layout_document_viewed(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
        view,
    )
    .unwrap()
}

/// The margin bars of one page: the only rects entirely left of the content
/// edge. Table borders and text decorations sit at or right of the edge.
fn bars<'a>(page: &'a PageScene, geometry: &PageGeometry) -> Vec<&'a SceneRect> {
    page.rects
        .iter()
        .filter(|rect| rect.x.raw() + rect.width.raw() <= geometry.left.raw())
        .collect()
}

fn bar_colours(snapshot: &LayoutSnapshot) -> Vec<(u8, u8, u8)> {
    let mut colours = Vec::new();
    for (page, geometry) in snapshot.pages().iter().zip(snapshot.page_geometry()) {
        for rect in bars(page, geometry) {
            colours.push((rect.red, rect.green, rect.blue));
        }
    }
    colours
}

#[test]
fn markup_and_simple_paint_one_bar_per_covered_row_band() {
    let mut document = Document::new(page());
    // A table-level record covers every visual row: two row bands, two bars.
    let table_record = {
        let ids = document.ids();
        let first = two_cell_row(ids, "甲一", "乙一");
        let second = two_cell_row(ids, "甲二", "乙二");
        let mut record = table(ids, vec![first, second]);
        record.props_revision = Some(revision(
            61,
            "表作者",
            PreviousTableProps::Table(TableChangeProps::default()),
        ));
        record
    };
    // A row-level record covers only its own band.
    let row_record = {
        let ids = document.ids();
        let first = two_cell_row(ids, "丙一", "丁一");
        let second = two_cell_row(ids, "丙二", "丁二");
        let mut record = table(ids, vec![first, second]);
        record.rows[1].props_revision = Some(revision(
            62,
            "行作者",
            PreviousTableProps::Row(RowChangeProps::default()),
        ));
        record
    };
    // A cell-level record covers the band of its row.
    let cell_record = {
        let ids = document.ids();
        let first = two_cell_row(ids, "戊一", "己一");
        let second = two_cell_row(ids, "戊二", "己二");
        let mut record = table(ids, vec![first, second]);
        record.rows[0].cells[1].props_revision = Some(revision(
            63,
            "格作者",
            PreviousTableProps::Cell(CellChangeProps::default()),
        ));
        record
    };
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![
        table_record.into(),
        row_record.into(),
        cell_record.into(),
    ]);

    for view in [RevisionView::FinalWithMarkup, RevisionView::Simple] {
        let snapshot = lay(&document, view);
        let colours = bar_colours(&snapshot);
        assert_eq!(colours.len(), 4, "{view:?}: {colours:?}");
        // Each record paints one colour for all the bands it covers.
        assert_eq!(colours[0], colours[1], "table record covers both rows");
        // The row record and the cell record each paint exactly one bar of
        // their own, already accounted for by the total count above.
    }

    // Final and original projections paint no bars at all.
    for view in [RevisionView::Final, RevisionView::Original] {
        let snapshot = lay(&document, view);
        assert!(bar_colours(&snapshot).is_empty(), "{view:?}");
    }
}

#[test]
fn the_outermost_record_decides_the_band_colour() {
    // The same authors across documents pin the priority without knowing the
    // palette: a table record plus a cell record paint the table colour, a
    // row record plus a cell record paint the row colour.
    let mut table_only = Document::new(page());
    {
        let ids = table_only.ids();
        let first = two_cell_row(ids, "一", "二");
        let mut table_record = table(ids, vec![first]);
        table_record.props_revision = Some(revision(
            71,
            "外层",
            PreviousTableProps::Table(TableChangeProps::default()),
        ));
        let body = table_only.body_id();
        table_only.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![table_record.into()]);
    }
    let mut row_only = Document::new(page());
    {
        let ids = row_only.ids();
        let first = two_cell_row(ids, "三", "四");
        let mut row_record = table(ids, vec![first]);
        row_record.rows[0].props_revision = Some(revision(
            72,
            "中层",
            PreviousTableProps::Row(RowChangeProps::default()),
        ));
        let body = row_only.body_id();
        row_only.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![row_record.into()]);
    }
    let table_colour = bar_colours(&lay(&table_only, RevisionView::FinalWithMarkup))[0];
    let row_colour = bar_colours(&lay(&row_only, RevisionView::FinalWithMarkup))[0];

    let mut both = Document::new(page());
    {
        let ids = both.ids();
        let first = two_cell_row(ids, "五", "六");
        let mut table_and_cell = table(ids, vec![first]);
        table_and_cell.props_revision = Some(revision(
            71,
            "外层",
            PreviousTableProps::Table(TableChangeProps::default()),
        ));
        table_and_cell.rows[0].cells[0].props_revision = Some(revision(
            73,
            "内层",
            PreviousTableProps::Cell(CellChangeProps::default()),
        ));
        let second = two_cell_row(ids, "七", "八");
        let mut row_and_cell = table(ids, vec![second]);
        row_and_cell.rows[0].props_revision = Some(revision(
            72,
            "中层",
            PreviousTableProps::Row(RowChangeProps::default()),
        ));
        row_and_cell.rows[0].cells[0].props_revision = Some(revision(
            73,
            "内层",
            PreviousTableProps::Cell(CellChangeProps::default()),
        ));
        let body = both.body_id();
        both.story_mut(body).unwrap().blocks =
            BlockSeq::from_vec(vec![table_and_cell.into(), row_and_cell.into()]);
    }
    let colours = bar_colours(&lay(&both, RevisionView::FinalWithMarkup));
    assert_eq!(colours.len(), 2);
    assert_eq!(colours[0], table_colour, "the table record wins its band");
    assert_eq!(colours[1], row_colour, "the row record wins its band");
}

#[test]
fn nested_tables_paint_their_own_row_bands_and_resolving_removes_the_bar() {
    // The standalone colours of the same authors pin which record painted
    // which band without assuming the palette separates the two authors.
    let mut outer_only = Document::new(page());
    {
        let ids = outer_only.ids();
        let first = two_cell_row(ids, "独一", "独二");
        let mut outer = table(ids, vec![first]);
        outer.props_revision = Some(revision(
            80,
            "外表作者",
            PreviousTableProps::Table(TableChangeProps::default()),
        ));
        let body = outer_only.body_id();
        outer_only.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![outer.into()]);
    }
    let mut inner_only = Document::new(page());
    {
        let ids = inner_only.ids();
        let first = two_cell_row(ids, "内一", "内二");
        let mut inner = table(ids, vec![first]);
        inner.rows[0].props_revision = Some(revision(
            81,
            "内表作者",
            PreviousTableProps::Row(RowChangeProps::default()),
        ));
        let body = inner_only.body_id();
        inner_only.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![inner.into()]);
    }
    let outer_colour = bar_colours(&lay(&outer_only, RevisionView::FinalWithMarkup))[0];
    let inner_colour = bar_colours(&lay(&inner_only, RevisionView::FinalWithMarkup))[0];

    let build = |nested_record: bool| {
        let mut document = Document::new(page());
        let ids = document.ids();
        let inner_row = two_cell_row(ids, "内一", "内二");
        let mut inner = table(ids, vec![inner_row]);
        if nested_record {
            inner.rows[0].props_revision = Some(revision(
                81,
                "内表作者",
                PreviousTableProps::Row(RowChangeProps::default()),
            ));
        }
        let outer_text = paragraph(ids, "外格");
        let host_cell = cell(ids, vec![outer_text.into(), inner.into()]);
        let side_text = paragraph(ids, "旁格");
        let side_cell = cell(ids, vec![side_text.into()]);
        let outer_row = row(ids, vec![host_cell, side_cell]);
        let mut outer = table(ids, vec![outer_row]);
        outer.props_revision = Some(revision(
            80,
            "外表作者",
            PreviousTableProps::Table(TableChangeProps::default()),
        ));
        let body = document.body_id();
        document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![outer.into()]);
        document
    };
    let pending = build(true);
    let mut colours = bar_colours(&lay(&pending, RevisionView::FinalWithMarkup));
    colours.sort();
    let mut expected = vec![outer_colour, inner_colour];
    expected.sort();
    // The outer table's band and the inner table's own band each carry a
    // bar; the inner record does not inherit the outer colour.
    assert_eq!(colours, expected, "{colours:?}");

    // With the inner record resolved away (its accepted state), only the
    // outer table's band keeps a bar — nested rows inherit nothing.
    let resolved = build(false);
    let colours = bar_colours(&lay(&resolved, RevisionView::FinalWithMarkup));
    assert_eq!(colours, vec![outer_colour], "{colours:?}");

    // A table without any pending record paints nothing.
    let mut plain = Document::new(page());
    {
        let ids = plain.ids();
        let first = two_cell_row(ids, "净一", "净二");
        let outer = table(ids, vec![first]);
        let body = plain.body_id();
        plain.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![outer.into()]);
    }
    assert!(bar_colours(&lay(&plain, RevisionView::FinalWithMarkup)).is_empty());
}

#[test]
fn a_repeated_header_row_keeps_its_bar_on_later_pages() {
    let mut document = Document::new(page());
    let ids = document.ids();
    let mut header = two_cell_row(ids, "表头甲", "表头乙");
    header.header = true;
    header.props_revision = Some(revision(
        91,
        "表头作者",
        PreviousTableProps::Row(RowChangeProps::default()),
    ));
    let mut rows = vec![header];
    for index in 0..40 {
        let mut left = paragraph(ids, &format!("数据{index}左列第一段"));
        left.push_text("第二段保持两行高度。", None, Arc::default())
            .unwrap();
        let right = paragraph(ids, &format!("数据{index}右列"));
        let left_cell = cell(ids, vec![left.into()]);
        let right_cell = cell(ids, vec![right.into()]);
        rows.push(row(ids, vec![left_cell, right_cell]));
    }
    let table = table(ids, rows);
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![table.into()]);

    let snapshot = lay(&document, RevisionView::FinalWithMarkup);
    assert!(
        snapshot.pages().len() >= 2,
        "{} pages",
        snapshot.pages().len()
    );
    let header_pages = snapshot
        .pages()
        .iter()
        .zip(snapshot.page_geometry())
        .filter(|(page, _)| page.runs.iter().any(|run| run.text.contains("表头甲")))
        .count();
    assert!(header_pages >= 2, "header on {header_pages} pages");
    // Every repeated header band carries the bar, and only the header band
    // does: one bar per page that shows the header.
    for (index, (page, geometry)) in snapshot
        .pages()
        .iter()
        .zip(snapshot.page_geometry())
        .enumerate()
    {
        let page_bars = bars(page, geometry);
        let header_here = page.runs.iter().any(|run| run.text.contains("表头甲"));
        assert_eq!(
            page_bars.len(),
            usize::from(header_here),
            "page {index}: {:?}",
            page_bars
        );
    }
}

#[test]
fn the_incremental_entry_matches_the_full_layout_with_bars() {
    let mut document = Document::new(page());
    let ids = document.ids();
    let first = two_cell_row(ids, "左一", "右一");
    let second = two_cell_row(ids, "左二", "右二");
    let mut with_record = table(ids, vec![first, second]);
    with_record.props_revision = Some(revision(
        95,
        "增量作者",
        PreviousTableProps::Table(TableChangeProps::default()),
    ));
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![with_record.into()]);

    let full = lay(&document, RevisionView::FinalWithMarkup);
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
    assert_eq!(full.pages().len(), incremental.pages().len());
    for (index, (full_page, warm_page)) in full.pages().iter().zip(incremental.pages()).enumerate()
    {
        assert_eq!(full_page.runs, warm_page.runs, "page {index} runs");
        assert_eq!(full_page.rects, warm_page.rects, "page {index} rects");
    }
    assert_eq!(bar_colours(&incremental).len(), 2);
}
