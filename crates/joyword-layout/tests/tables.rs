//! T201: cell paragraphs share the page scene with body text. Rows may split,
//! a vertical merge is one visual span, and a table is still laid out in full
//! on the incremental path.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutError, LayoutInvalidation, LayoutLimits,
    LayoutNote, LayoutReuse, LoadedFont, ParagraphLayoutCache, SingleFontProfile, layout_document,
    layout_incremental,
};
use joyword_model::{
    Block, BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, CellMargins, Color, Document,
    EqualColumns, Measure, Paragraph, Row, RowHeight, RowHeightRule, RunProps, Shading, Style,
    StyleId, StyleKind, Table, TableAlign, TableCondition, TableConditional, TableFormat,
    TableLayoutKind, TableLook, ThemeColorSlot, ThemeScheme, VerticalAlign, VerticalMerge,
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
        margin: None,
        vertical_align: None,
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

fn squeeze(document: &mut Document, lines: i64) {
    let page = &mut document.sections_mut()[0].page;
    page.height = Measure::Emu(pt(12 + 18 * lines).raw());
    page.top = Measure::Emu(pt(6).raw());
    page.bottom = Measure::Emu(pt(6).raw());
    page.header = Measure::Emu(pt(2).raw());
    page.footer = Measure::Emu(pt(2).raw());
}

fn text_paragraph(document: &mut Document, text: &str) -> Paragraph {
    let mut paragraph = Paragraph::new(document.ids().node());
    if !text.is_empty() {
        paragraph.push_text(text, None, Arc::default()).unwrap();
    }
    paragraph
}

fn plain_cell(
    document: &mut Document,
    paragraphs: Vec<Paragraph>,
    span: u32,
    merge: VerticalMerge,
    shading: Option<Shading>,
) -> Cell {
    Cell {
        id: document.ids().node(),
        grid_span: span,
        vertical_merge: merge,
        width: Some(Measure::Twips(2400)),
        borders: CellBorders::default(),
        shading,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(paragraphs.into_iter().map(Block::from).collect()),
        preserved: None,
    }
}

fn install_table(
    document: &mut Document,
    rows: Vec<Row>,
    columns: usize,
    margin: Option<CellMargins>,
) {
    let table = Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(2400); columns],
        rows,
        style: None,
        width: None,
        layout: None,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: margin,
        preserved: None,
        source: None,
    };
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![table.into()]);
}

fn row(document: &mut Document, cells: Vec<Cell>, header: bool, cant_split: bool) -> Row {
    Row {
        id: document.ids().node(),
        grid_before: 0,
        grid_after: 0,
        cells,
        header,
        cant_split,
        height: None,
        preserved: None,
    }
}

fn lay(document: &Document) -> joyword_layout::LayoutSnapshot {
    layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap()
}

fn run_texts(page: &joyword_layout::PageScene) -> Vec<&str> {
    page.runs.iter().map(|run| run.text.as_str()).collect()
}

#[test]
fn a_vertical_merge_spans_rows_and_hides_continue_text() {
    let mut document = Document::new_blank();
    let restart = text_paragraph(&mut document, "A");
    let hidden = text_paragraph(&mut document, "hidden");
    let hidden_id = hidden.id();
    let top = plain_cell(
        &mut document,
        vec![restart],
        1,
        VerticalMerge::Restart,
        Some(Shading {
            fill: Color::Rgb(255, 0, 0),
        }),
    );
    let bottom = plain_cell(
        &mut document,
        vec![hidden],
        1,
        VerticalMerge::Continue,
        None,
    );
    let top_row = row(&mut document, vec![top], false, false);
    let bottom_row = row(&mut document, vec![bottom], false, false);
    install_table(&mut document, vec![top_row, bottom_row], 1, None);
    let laid = lay(&document);
    assert_eq!(laid.pages().len(), 1);
    assert_eq!(run_texts(&laid.pages()[0]), vec!["A"]);
    assert!(laid.lines().iter().all(|line| line.paragraph != hidden_id));
    let reds: Vec<_> = laid.pages()[0]
        .rects
        .iter()
        .filter(|rect| rect.red == 255 && rect.green == 0 && rect.blue == 0)
        .collect();
    assert!(reds.len() >= 2);
    let top = reds.iter().map(|rect| rect.y.raw()).min().unwrap();
    let bottom = reds
        .iter()
        .map(|rect| rect.y.raw() + rect.height.raw())
        .max()
        .unwrap();
    assert_eq!(bottom - top, pt(36).raw());
}

#[test]
fn a_splittable_row_breaks_across_pages_and_cant_split_moves_whole() {
    let mut splitting = Document::new_blank();
    squeeze(&mut splitting, 2);
    let intro = text_paragraph(&mut splitting, "P");
    let first = text_paragraph(&mut splitting, "A");
    let second = text_paragraph(&mut splitting, "B");
    let cell = plain_cell(
        &mut splitting,
        vec![first, second],
        1,
        VerticalMerge::None,
        Some(Shading {
            fill: Color::Rgb(0, 0, 1),
        }),
    );
    let table_row = row(&mut splitting, vec![cell], false, false);
    let table = Table {
        id: splitting.ids().node(),
        grid: vec![Measure::Twips(2400)],
        rows: vec![table_row],
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
    let body = splitting.body_id();
    splitting.story_mut(body).unwrap().blocks =
        BlockSeq::from_vec(vec![intro.into(), table.into()]);
    let laid = lay(&splitting);
    assert_eq!(laid.pages().len(), 2);
    assert_eq!(run_texts(&laid.pages()[0]), vec!["P", "A"]);
    assert_eq!(run_texts(&laid.pages()[1]), vec!["B"]);
    assert!(laid.notes().is_empty());

    let mut whole = Document::new_blank();
    squeeze(&mut whole, 2);
    let intro = text_paragraph(&mut whole, "P");
    let first = text_paragraph(&mut whole, "A");
    let second = text_paragraph(&mut whole, "B");
    let cell = plain_cell(
        &mut whole,
        vec![first, second],
        1,
        VerticalMerge::None,
        Some(Shading {
            fill: Color::Rgb(0, 0, 1),
        }),
    );
    let table_row = row(&mut whole, vec![cell], false, true);
    let table = Table {
        id: whole.ids().node(),
        grid: vec![Measure::Twips(2400)],
        rows: vec![table_row],
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
    let body = whole.body_id();
    whole.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![intro.into(), table.into()]);
    let laid = lay(&whole);
    assert_eq!(laid.pages().len(), 2, "{:?}", run_texts(&laid.pages()[0]));
    assert_eq!(run_texts(&laid.pages()[0]), vec!["P"]);
    assert_eq!(run_texts(&laid.pages()[1]), vec!["A", "B"]);
    assert!(laid.notes().is_empty());
}

#[test]
fn a_cant_split_row_taller_than_the_page_is_noted_and_kept() {
    let mut document = Document::new_blank();
    squeeze(&mut document, 1);
    let first = text_paragraph(&mut document, "A");
    let second = text_paragraph(&mut document, "B");
    let cell = plain_cell(
        &mut document,
        vec![first, second],
        1,
        VerticalMerge::None,
        None,
    );
    let table_row = row(&mut document, vec![cell], false, true);
    install_table(&mut document, vec![table_row], 1, None);
    let laid = lay(&document);
    assert_eq!(laid.pages().len(), 1);
    assert_eq!(run_texts(&laid.pages()[0]), vec!["A", "B"]);
    assert!(
        laid.notes()
            .iter()
            .any(|note| matches!(note, LayoutNote::RowTallerThanPage { .. }))
    );
}

#[test]
fn a_leading_header_repeats_and_a_later_header_does_not() {
    let mut document = Document::new_blank();
    squeeze(&mut document, 2);
    let header = text_paragraph(&mut document, "H");
    let first = text_paragraph(&mut document, "A");
    let second = text_paragraph(&mut document, "B");
    let header_cell = plain_cell(
        &mut document,
        vec![header],
        1,
        VerticalMerge::None,
        Some(Shading {
            fill: Color::Rgb(0, 128, 0),
        }),
    );
    let body_cell = plain_cell(
        &mut document,
        vec![first, second],
        1,
        VerticalMerge::None,
        Some(Shading {
            fill: Color::Rgb(0, 0, 1),
        }),
    );
    let header_row = row(&mut document, vec![header_cell], true, false);
    let body_row = row(&mut document, vec![body_cell], false, false);
    install_table(&mut document, vec![header_row, body_row], 1, None);
    let laid = lay(&document);
    assert_eq!(laid.pages().len(), 2);
    assert_eq!(run_texts(&laid.pages()[0]), vec!["H", "A"]);
    assert_eq!(run_texts(&laid.pages()[1]), vec!["H", "B"]);
    let repeated = &laid.pages()[1].runs;
    assert!(repeated[0].baseline_y.raw() < repeated[1].baseline_y.raw());

    let mut later = Document::new_blank();
    squeeze(&mut later, 2);
    let lead = text_paragraph(&mut later, "A");
    let marked = text_paragraph(&mut later, "H");
    let first = text_paragraph(&mut later, "B");
    let second = text_paragraph(&mut later, "C");
    let lead_cell = plain_cell(&mut later, vec![lead], 1, VerticalMerge::None, None);
    let marked_cell = plain_cell(&mut later, vec![marked], 1, VerticalMerge::None, None);
    let body_cell = plain_cell(
        &mut later,
        vec![first, second],
        1,
        VerticalMerge::None,
        Some(Shading {
            fill: Color::Rgb(0, 0, 1),
        }),
    );
    let lead_row = row(&mut later, vec![lead_cell], false, false);
    let marked_row = row(&mut later, vec![marked_cell], true, false);
    let body_row = row(&mut later, vec![body_cell], false, false);
    install_table(&mut later, vec![lead_row, marked_row, body_row], 1, None);
    let laid = lay(&later);
    assert!(laid.pages().len() >= 2);
    assert!(run_texts(&laid.pages()[1]).iter().all(|text| *text != "H"));
}

#[test]
fn a_header_taller_than_the_page_is_not_repeated() {
    let mut document = Document::new_blank();
    squeeze(&mut document, 1);
    let first = text_paragraph(&mut document, "H1");
    let second = text_paragraph(&mut document, "H2");
    let cell = plain_cell(
        &mut document,
        vec![first, second],
        1,
        VerticalMerge::None,
        None,
    );
    let table_id = {
        let row = row(&mut document, vec![cell], true, false);
        let id = document.ids().node();
        let table = Table {
            id,
            grid: vec![Measure::Twips(2400)],
            rows: vec![row],
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
        id
    };
    let laid = lay(&document);
    let texts: Vec<&str> = laid
        .pages()
        .iter()
        .flat_map(|page| page.runs.iter().map(|run| run.text.as_str()))
        .collect();
    assert_eq!(texts.iter().filter(|text| **text == "H1").count(), 1);
    assert_eq!(texts.iter().filter(|text| **text == "H2").count(), 1);
    assert!(laid.notes().iter().any(|note| matches!(
        note,
        LayoutNote::HeaderTallerThanPage { table } if *table == table_id
    )));
}

#[test]
fn an_explicit_zero_left_margin_shifts_the_glyph_left_of_the_default() {
    let mut with_default = Document::new_blank();
    let letter = text_paragraph(&mut with_default, "A");
    let cell = plain_cell(
        &mut with_default,
        vec![letter],
        1,
        VerticalMerge::None,
        None,
    );
    let table_row = row(&mut with_default, vec![cell], false, false);
    install_table(&mut with_default, vec![table_row], 1, None);
    let default_x = lay(&with_default).pages()[0].runs[0].origin_x.raw();

    let mut explicit = Document::new_blank();
    let letter = text_paragraph(&mut explicit, "A");
    let cell = plain_cell(&mut explicit, vec![letter], 1, VerticalMerge::None, None);
    let table_row = row(&mut explicit, vec![cell], false, false);
    install_table(
        &mut explicit,
        vec![table_row],
        1,
        Some(CellMargins {
            top: 0,
            left: 0,
            bottom: 0,
            right: 0,
        }),
    );
    let explicit_x = lay(&explicit).pages()[0].runs[0].origin_x.raw();
    assert_eq!(default_x - explicit_x, 108 * Emu::PER_TWIP);
    assert_eq!(explicit_x, 1800 * Emu::PER_TWIP);
}

#[test]
fn incremental_layout_of_a_split_table_matches_a_full_layout() {
    let mut document = Document::new_blank();
    squeeze(&mut document, 1);
    let first = text_paragraph(&mut document, "A");
    let second = text_paragraph(&mut document, "B");
    let cell = plain_cell(
        &mut document,
        vec![first, second],
        1,
        VerticalMerge::None,
        Some(Shading {
            fill: Color::Rgb(0, 0, 1),
        }),
    );
    let table_row = row(&mut document, vec![cell], true, false);
    install_table(&mut document, vec![table_row], 1, None);
    let full = lay(&document);
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
    assert_eq!(incremental.notes(), full.notes());
    assert!(full.pages().len() >= 2);
}

fn single_edge() -> BorderEdge {
    BorderEdge {
        kind: BorderKind::Single,
        size_eighths: 4,
        color: Color::Auto,
    }
}

#[test]
fn table_styles_paint_bands_and_direct_formatting_wins() {
    let mut document = Document::new_blank();
    let mut base = Style::new("base", StyleKind::Table, Some("Base"));
    base.table.bold = Some(false);
    base.table.borders.top = Some(single_edge());
    let mut child = Style::new("grid", StyleKind::Table, Some("Grid"));
    child.based_on = Some(StyleId::new("base"));
    child.table.borders.left = Some(BorderEdge {
        kind: BorderKind::Nil,
        size_eighths: 4,
        color: Color::Auto,
    });
    child.table_conditionals.push(TableConditional {
        condition: TableCondition::Band1Horizontal,
        format: TableFormat {
            shading: Some(Shading {
                fill: Color::Rgb(0, 0, 255),
            }),
            ..TableFormat::default()
        },
    });
    child.table_conditionals.push(TableConditional {
        condition: TableCondition::Band2Horizontal,
        format: TableFormat {
            shading: Some(Shading {
                fill: Color::Rgb(0, 255, 0),
            }),
            ..TableFormat::default()
        },
    });
    child.table_conditionals.push(TableConditional {
        condition: TableCondition::FirstRow,
        format: TableFormat {
            bold: Some(true),
            shading: Some(Shading {
                fill: Color::Rgb(255, 0, 0),
            }),
            ..TableFormat::default()
        },
    });
    child.table_conditionals.push(TableConditional {
        condition: TableCondition::LastRow,
        format: TableFormat {
            bold: Some(false),
            shading: Some(Shading {
                fill: Color::Rgb(255, 255, 0),
            }),
            ..TableFormat::default()
        },
    });
    document.styles_mut().insert(base).unwrap();
    document.styles_mut().insert(child).unwrap();

    let direct_off = RunProps {
        bold: Some(false),
        ..RunProps::default()
    };
    let direct_on = RunProps {
        bold: Some(true),
        ..RunProps::default()
    };
    let header = text_paragraph(&mut document, "A");
    let mut header_plain = Paragraph::new(document.ids().node());
    header_plain
        .push_text("E", None, Arc::new(direct_off))
        .unwrap();
    let mut band_direct = Paragraph::new(document.ids().node());
    band_direct
        .push_text("B", None, Arc::new(direct_on))
        .unwrap();
    let band_plain = text_paragraph(&mut document, "F");
    let band2_left = text_paragraph(&mut document, "C");
    let band2_right = text_paragraph(&mut document, "G");
    let last_left = text_paragraph(&mut document, "D");
    let last_right = text_paragraph(&mut document, "H");

    let header_cell = plain_cell(&mut document, vec![header], 1, VerticalMerge::None, None);
    let header_plain_cell = plain_cell(
        &mut document,
        vec![header_plain],
        1,
        VerticalMerge::None,
        None,
    );
    let shaded = plain_cell(
        &mut document,
        vec![band_direct],
        1,
        VerticalMerge::None,
        Some(Shading {
            fill: Color::Rgb(9, 9, 9),
        }),
    );
    let band_plain_cell = plain_cell(
        &mut document,
        vec![band_plain],
        1,
        VerticalMerge::None,
        None,
    );
    let mut bordered = plain_cell(
        &mut document,
        vec![band2_left],
        1,
        VerticalMerge::None,
        None,
    );
    bordered.borders.left = Some(single_edge());
    let band2_right_cell = plain_cell(
        &mut document,
        vec![band2_right],
        1,
        VerticalMerge::None,
        None,
    );
    let last_left_cell = plain_cell(&mut document, vec![last_left], 1, VerticalMerge::None, None);
    let last_right_cell = plain_cell(
        &mut document,
        vec![last_right],
        1,
        VerticalMerge::None,
        None,
    );
    let row0 = row(
        &mut document,
        vec![header_cell, header_plain_cell],
        false,
        false,
    );
    let row1 = row(&mut document, vec![shaded, band_plain_cell], false, false);
    let row2 = row(
        &mut document,
        vec![bordered, band2_right_cell],
        false,
        false,
    );
    let row3 = row(
        &mut document,
        vec![last_left_cell, last_right_cell],
        false,
        false,
    );
    install_table(&mut document, vec![row0, row1, row2, row3], 2, None);
    let body = document.body_id();
    document.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        let table = Arc::make_mut(table);
        table.style = Some(StyleId::new("grid"));
        table.look = Some(TableLook {
            first_row: true,
            last_row: true,
            first_column: false,
            last_column: false,
            no_h_band: false,
            no_v_band: true,
        });
    });

    let laid = lay(&document);
    let page = &laid.pages()[0];
    let run = |text: &str| page.runs.iter().find(|run| run.text == text).unwrap();
    assert!(run("A").bold);
    assert!(!run("E").bold);
    assert!(run("B").bold);
    assert!(!run("F").bold);
    assert!(!run("C").bold);
    assert!(!run("H").bold);
    let has = |red, green, blue| {
        page.rects
            .iter()
            .any(|rect| rect.red == red && rect.green == green && rect.blue == blue)
    };
    assert!(has(255, 0, 0));
    assert!(has(0, 0, 255));
    assert!(has(0, 255, 0));
    assert!(has(255, 255, 0));
    assert!(has(9, 9, 9));
    let thin = 4 * Emu::PER_POINT / 8;
    assert!(page.rects.iter().any(|rect| {
        rect.red == 0
            && rect.green == 0
            && rect.blue == 0
            && rect.width.raw() == thin
            && rect.height.raw() > thin
    }));

    let full = laid;
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
fn a_table_in_newspaper_columns_is_refused() {
    let mut document = document();
    document.sections_mut()[0].columns = Some(EqualColumns {
        count: 2,
        space: 720,
    });
    let error = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        LayoutError::Unsupported {
            feature: "table in newspaper columns",
            ..
        }
    ));
}

#[test]
fn a_cell_margin_overrides_the_table_margin() {
    let margins = CellMargins {
        top: 0,
        left: 200,
        bottom: 0,
        right: 0,
    };
    let mut table_only = Document::new_blank();
    let letter = text_paragraph(&mut table_only, "A");
    let cell = plain_cell(&mut table_only, vec![letter], 1, VerticalMerge::None, None);
    let table_row = row(&mut table_only, vec![cell], false, false);
    install_table(&mut table_only, vec![table_row], 1, Some(margins));
    let table_x = lay(&table_only).pages()[0].runs[0].origin_x.raw();

    let mut overridden = Document::new_blank();
    let letter = text_paragraph(&mut overridden, "A");
    let mut cell = plain_cell(&mut overridden, vec![letter], 1, VerticalMerge::None, None);
    cell.margin = Some(CellMargins {
        top: 0,
        left: 0,
        bottom: 0,
        right: 0,
    });
    let table_row = row(&mut overridden, vec![cell], false, false);
    install_table(&mut overridden, vec![table_row], 1, Some(margins));
    let cell_x = lay(&overridden).pages()[0].runs[0].origin_x.raw();
    assert_eq!(table_x - cell_x, 200 * Emu::PER_TWIP);
}

#[test]
fn double_borders_paint_two_bands() {
    let mut document = Document::new_blank();
    let letter = text_paragraph(&mut document, "A");
    let mut cell = plain_cell(&mut document, vec![letter], 1, VerticalMerge::None, None);
    let edge = BorderEdge {
        kind: BorderKind::Double,
        size_eighths: 12,
        color: Color::Rgb(0, 0, 255),
    };
    cell.borders.top = Some(edge);
    let table_row = row(&mut document, vec![cell], false, false);
    install_table(&mut document, vec![table_row], 1, None);
    let laid = lay(&document);
    let rects = &laid.pages()[0].rects;
    let bands = rects
        .iter()
        .filter(|rect| rect.blue == 255 && rect.red == 0)
        .count();
    assert!(bands >= 2, "{rects:?}");
}

#[test]
fn an_unresolved_theme_border_keeps_no_ink() {
    let mut document = Document::new_blank();
    let letter = text_paragraph(&mut document, "A");
    let mut cell = plain_cell(&mut document, vec![letter], 1, VerticalMerge::None, None);
    cell.borders.top = Some(BorderEdge {
        kind: BorderKind::Single,
        size_eighths: 8,
        color: Color::theme(ThemeColorSlot::Accent1),
    });
    let table_row = row(&mut document, vec![cell], false, false);
    install_table(&mut document, vec![table_row], 1, None);
    assert!(lay(&document).pages()[0].rects.is_empty());

    document.styles_mut().set_theme(ThemeScheme {
        colors: [(ThemeColorSlot::Accent1, (10, 20, 30))].into(),
        ..ThemeScheme::default()
    });
    let laid = lay(&document);
    let rects = &laid.pages()[0].rects;
    assert!(
        rects
            .iter()
            .any(|rect| rect.red == 10 && rect.green == 20 && rect.blue == 30),
        "{rects:?}"
    );
}

#[test]
fn fixed_layout_does_not_shrink_and_centre_uses_the_leftover() {
    let mut wide = Document::new_blank();
    let left = text_paragraph(&mut wide, "A");
    let right = text_paragraph(&mut wide, "B");
    let cells = vec![
        plain_cell(&mut wide, vec![left], 1, VerticalMerge::None, None),
        plain_cell(&mut wide, vec![right], 1, VerticalMerge::None, None),
    ];
    let table_row = row(&mut wide, cells, false, false);
    install_table(
        &mut wide,
        vec![table_row],
        2,
        Some(CellMargins {
            top: 0,
            left: 0,
            bottom: 0,
            right: 0,
        }),
    );
    let body = wide.body_id();
    wide.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        let table = Arc::make_mut(table);
        table.grid = vec![Measure::Twips(5_000), Measure::Twips(5_000)];
        table.layout = Some(TableLayoutKind::Fixed);
    });
    let fixed = lay(&wide);
    let fixed_shift =
        fixed.pages()[0].runs[1].origin_x.raw() - fixed.pages()[0].runs[0].origin_x.raw();
    assert_eq!(fixed_shift, 5_000 * Emu::PER_TWIP);

    wide.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        Arc::make_mut(table).layout = None;
    });
    let shrunk = lay(&wide);
    let shrunk_shift =
        shrunk.pages()[0].runs[1].origin_x.raw() - shrunk.pages()[0].runs[0].origin_x.raw();
    assert!(shrunk_shift < fixed_shift);
    assert_eq!(shrunk_shift, 4_153 * Emu::PER_TWIP);

    wide.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(table) = block else {
            panic!("table");
        };
        let table = Arc::make_mut(table);
        table.grid = vec![Measure::Twips(2_400), Measure::Twips(2_400)];
        table.alignment = Some(TableAlign::Center);
        table.layout = Some(TableLayoutKind::Fixed);
    });
    let centred = lay(&wide).pages()[0].runs[0].origin_x.raw();
    let mut left_table = Document::new_blank();
    let letter = text_paragraph(&mut left_table, "A");
    let cell = plain_cell(&mut left_table, vec![letter], 1, VerticalMerge::None, None);
    let table_row = row(&mut left_table, vec![cell], false, false);
    install_table(
        &mut left_table,
        vec![table_row],
        1,
        Some(CellMargins {
            top: 0,
            left: 0,
            bottom: 0,
            right: 0,
        }),
    );
    let left_x = lay(&left_table).pages()[0].runs[0].origin_x.raw();
    assert_eq!(centred - left_x, 1_753 * Emu::PER_TWIP);
}

#[test]
fn at_least_centers_the_line_exact_omits_it_and_auto_ignores_val() {
    let zero = CellMargins {
        top: 0,
        left: 0,
        bottom: 0,
        right: 0,
    };
    let mut document = Document::new_blank();
    let letter = text_paragraph(&mut document, "A");
    let mut cell = plain_cell(&mut document, vec![letter], 1, VerticalMerge::None, None);
    cell.margin = Some(zero);
    cell.borders.bottom = Some(BorderEdge {
        kind: BorderKind::Single,
        size_eighths: 8,
        color: Color::Auto,
    });
    let table_row = row(&mut document, vec![cell], false, false);
    install_table(&mut document, vec![table_row], 1, Some(zero));
    let body = document.body_id();
    let top_base = lay(&document).pages()[0].runs[0].baseline_y.raw();

    let set = |document: &mut Document, height: Option<RowHeight>, align: Option<VerticalAlign>| {
        document.story_mut(body).unwrap().blocks.update(0, |block| {
            let Block::Table(table) = block else {
                panic!("table");
            };
            let table = Arc::make_mut(table);
            table.rows[0].height = height;
            table.rows[0].cells[0].vertical_align = align;
        });
    };
    set(
        &mut document,
        Some(RowHeight {
            twips: 1_440,
            rule: RowHeightRule::AtLeast,
        }),
        Some(VerticalAlign::Center),
    );
    let centered = lay(&document).pages()[0].runs[0].baseline_y.raw();
    let line = 18 * Emu::PER_POINT;
    let border = Emu::PER_POINT;
    let row_height = 1_440 * Emu::PER_TWIP;
    let slack = row_height - border - line;
    assert!(slack > 0);
    assert_eq!(centered - top_base, slack / 2);

    set(
        &mut document,
        Some(RowHeight {
            twips: 1_440,
            rule: RowHeightRule::AtLeast,
        }),
        Some(VerticalAlign::Bottom),
    );
    let bottom = lay(&document).pages()[0].runs[0].baseline_y.raw();
    assert_eq!(bottom - top_base, slack);

    set(
        &mut document,
        Some(RowHeight {
            twips: 9_000,
            rule: RowHeightRule::Auto,
        }),
        Some(VerticalAlign::Top),
    );
    assert_eq!(lay(&document).pages()[0].runs[0].baseline_y.raw(), top_base);

    set(
        &mut document,
        Some(RowHeight {
            twips: 200,
            rule: RowHeightRule::Exact,
        }),
        Some(VerticalAlign::Center),
    );
    let exact = lay(&document);
    assert!(exact.pages()[0].runs.is_empty());
    let border_y = exact.geometry().top.raw() + 200 * Emu::PER_TWIP - border;
    assert!(
        exact.pages()[0]
            .rects
            .iter()
            .any(|rect| rect.y.raw() == border_y && rect.height.raw() == border)
    );
}

fn nested_document() -> Document {
    let mut document = Document::new_blank();
    let mut before = Paragraph::new(document.ids().node());
    before.push_text("外", None, Arc::default()).unwrap();
    let mut inside = Paragraph::new(document.ids().node());
    inside.push_text("内", None, Arc::default()).unwrap();
    let mut after = Paragraph::new(document.ids().node());
    after.push_text("后", None, Arc::default()).unwrap();
    let borders = CellBorders {
        top: Some(edge()),
        left: Some(edge()),
        bottom: Some(edge()),
        right: Some(edge()),
    };
    let inner = Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(2000)],
        rows: vec![Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![Cell {
                id: document.ids().node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: Some(Measure::Twips(2000)),
                borders,
                shading: Some(Shading {
                    fill: Color::Rgb(0, 0, 255),
                }),
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![inside.into()]),
                preserved: None,
            }],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
        }],
        style: None,
        width: Some(joyword_model::TableWidth::Dxa(2000)),
        layout: Some(TableLayoutKind::Fixed),
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        source: None,
    };
    let outer = Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(4000)],
        rows: vec![Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![Cell {
                id: document.ids().node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: Some(Measure::Twips(4000)),
                borders,
                shading: Some(Shading {
                    fill: Color::Rgb(255, 0, 0),
                }),
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![before.into(), inner.into(), after.into()]),
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
    document.story_mut(body).unwrap().blocks = BlockSeq::from_vec(vec![outer.into()]);
    document
}

#[test]
fn a_nested_table_places_glyphs_inside_the_parent_cell() {
    let document = nested_document();
    let laid = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap();
    let page = &laid.pages()[0];
    let outer = page.runs.iter().find(|run| run.text == "外").unwrap();
    let inner = page.runs.iter().find(|run| run.text == "内").unwrap();
    let after = page.runs.iter().find(|run| run.text == "后").unwrap();
    let pad = 108 * Emu::PER_TWIP;
    assert_eq!(inner.origin_x.raw() - outer.origin_x.raw(), pad);
    assert_eq!(after.origin_x.raw(), outer.origin_x.raw());
    assert!(outer.baseline_y.raw() < inner.baseline_y.raw());
    assert!(inner.baseline_y.raw() < after.baseline_y.raw());
    let cell_left = outer.origin_x.raw() - pad;
    let cell_right = cell_left + 4000 * Emu::PER_TWIP;
    assert!(inner.glyph_x(0).raw() > cell_left);
    assert!(inner.glyph_x(0).raw() < cell_right);
    assert!(page.rects.iter().any(|rect| {
        rect.blue == 255
            && rect.red == 0
            && rect.x.raw() >= cell_left
            && rect.x.raw() + rect.width.raw() <= cell_right
    }));
}

#[test]
fn incremental_layout_of_a_nested_table_matches_a_full_layout() {
    let document = nested_document();
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
fn a_table_nested_two_levels_deep_is_refused() {
    let mut document = nested_document();
    let mut marked = Paragraph::new(document.ids().node());
    marked.push_text("深", None, Arc::default()).unwrap();
    let deeper = Table {
        id: document.ids().node(),
        grid: vec![Measure::Twips(800)],
        rows: vec![Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![Cell {
                id: document.ids().node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: Some(Measure::Twips(800)),
                borders: CellBorders::default(),
                shading: None,
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![marked.into()]),
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
    document.story_mut(body).unwrap().blocks.update(0, |block| {
        let Block::Table(outer) = block else {
            panic!("outer");
        };
        let Block::Table(inner) = Arc::make_mut(outer).rows[0].cells[0]
            .blocks
            .get_mut(1)
            .unwrap()
        else {
            panic!("inner");
        };
        Arc::make_mut(inner).rows[0].cells[0]
            .blocks
            .push(deeper.into());
    });
    let error = layout_document(
        &document.snapshot(),
        FaceSet::uniform(&font()),
        profile(),
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        LayoutError::Unsupported {
            feature: "nested table deeper than one level",
            ..
        }
    ));
}
