//! Preferred table widths. These live outside `src/` so the panic gate does
//! not treat test assertions as production panic sites.

use joyword_model::{
    BlockSeq, Cell, CellBorders, IdAllocator, Measure, Row, Table, TableAlign, TableLayoutKind,
    TableWidth, VerticalMerge, table_metrics,
};

fn bare(grid: Vec<i64>, width: Option<TableWidth>, layout: Option<TableLayoutKind>) -> Table {
    let mut ids = IdAllocator::default();
    Table {
        id: ids.node(),
        grid: grid.into_iter().map(Measure::Twips).collect(),
        rows: vec![Row {
            id: ids.node(),
            grid_before: 0,
            grid_after: 0,
            cells: vec![],
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
            props_revision: None,
        }],
        style: None,
        width,
        layout,
        alignment: None,
        indent: None,
        look: None,
        cell_margin: None,
        preserved: None,
        props_revision: None,
        source: None,
    }
}

fn metrics(table: &Table, content: i64) -> joyword_model::TableMetrics {
    match table_metrics(table, content) {
        Some(metrics) => metrics,
        None => panic!("table metrics"),
    }
}

#[test]
fn omitted_layout_shrinks_only_when_wider_than_the_content() {
    let narrow = bare(vec![2400, 2400], None, None);
    let resolved = metrics(&narrow, 8306);
    assert_eq!(resolved.columns, vec![2400, 2400]);
    assert_eq!(resolved.origin, 0);

    let wide = bare(vec![5000, 5000], None, None);
    let resolved = metrics(&wide, 8306);
    assert_eq!(resolved.columns.iter().sum::<i64>(), 8306);

    let fixed = bare(vec![5000, 5000], None, Some(TableLayoutKind::Fixed));
    let resolved = metrics(&fixed, 8306);
    assert_eq!(resolved.columns, vec![5000, 5000]);
}

#[test]
fn percentage_table_width_uses_the_text_extent_and_fixed_does_not_shrink() {
    let table = bare(
        vec![1000, 1000],
        Some(TableWidth::Pct(2500)),
        Some(TableLayoutKind::Fixed),
    );
    let resolved = metrics(&table, 8000);
    assert_eq!(resolved.columns.iter().sum::<i64>(), 4000);
}

#[test]
fn a_span_one_percentage_sets_that_column_against_an_automatic_table() {
    let mut table = bare(vec![1000, 3000], Some(TableWidth::Auto), None);
    let mut ids = IdAllocator::default();
    let _ = ids.node();
    table.rows[0].cells.push(Cell {
        id: ids.node(),
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: Some(Measure::PercentFiftieths(2500)),
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(vec![]),
        preserved: None,
        props_revision: None,
    });
    table.rows[0].cells.push(Cell {
        id: ids.node(),
        grid_span: 1,
        vertical_merge: VerticalMerge::None,
        width: None,
        borders: CellBorders::default(),
        shading: None,
        margin: None,
        vertical_align: None,
        blocks: BlockSeq::from_vec(vec![]),
        preserved: None,
        props_revision: None,
    });
    let resolved = metrics(&table, 8000);
    assert_eq!(resolved.columns[0], 4000);
    assert_eq!(resolved.columns.iter().sum::<i64>(), 8000);
}

#[test]
fn centre_ignores_indent_and_a_negative_indent_is_refused() {
    let mut table = bare(vec![2000], None, Some(TableLayoutKind::Fixed));
    table.alignment = Some(TableAlign::Center);
    table.indent = Some(500);
    let resolved = metrics(&table, 8000);
    assert_eq!(resolved.origin, 3000);

    table.alignment = Some(TableAlign::Left);
    table.indent = Some(-1);
    assert!(table_metrics(&table, 8000).is_none());
}
