//! Table structure edits (T201). Row and column edits still refuse a merged
//! grid. Merge and split are their own transactions: each one either leaves
//! a grid [`joyword_model::table_structure`] accepts or changes nothing.

use std::sync::Arc;

use joyword_model::{
    Bias, Block, BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, Color, Document, Measure,
    NodeId, Paragraph, Position, Row, ScalarOffset, StoryId, StoryKind, Table, TableWidth,
    VerticalMerge,
};

use crate::command::{CommandError, TableMerge, TablePropertyChange, TableStructure};
use crate::locate::{self, Step};
use crate::map::{MapStep, PositionMap, Selection};
use crate::session::ChangeSet;

/// A cell in a top-level body table. `row` and `cell` are indexes in that table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellAddress {
    pub story: StoryId,
    pub table: NodeId,
    pub row: usize,
    pub cell: usize,
}

const MAX_AXIS: u32 = 32;
const COLUMN_FALLBACK_TWIPS: i64 = 1440;

/// The table cell that contains `at`, when that cell's paragraphs sit directly
/// in a top-level body table.
pub fn cell_address(document: &Document, at: &Position) -> Result<CellAddress, CommandError> {
    let site = site(document, at)?;
    Ok(CellAddress {
        story: at.story,
        table: site.table,
        row: site.row,
        cell: site.cell,
    })
}

/// Caret at the start of `address`.
pub fn cell_caret(document: &Document, address: CellAddress) -> Result<Selection, CommandError> {
    let paragraph = cell_paragraphs(document, address)?
        .into_iter()
        .next()
        .ok_or(CommandError::NotATable(address.table))?;
    Ok(Selection::caret(Position {
        story: address.story,
        paragraph,
        offset: ScalarOffset(0),
        bias: Bias::After,
    }))
}

/// Every paragraph of `address`, from the start of the first to the end of the last.
pub fn cell_selection(
    document: &Document,
    address: CellAddress,
) -> Result<Selection, CommandError> {
    let paragraphs = cell_paragraphs(document, address)?;
    let first = *paragraphs
        .first()
        .ok_or(CommandError::NotATable(address.table))?;
    let last = *paragraphs
        .last()
        .ok_or(CommandError::NotATable(address.table))?;
    let len = paragraph_len(document, address, last)?;
    Ok(Selection {
        anchor: Position {
            story: address.story,
            paragraph: first,
            offset: ScalarOffset(0),
            bias: Bias::Before,
        },
        head: Position {
            story: address.story,
            paragraph: last,
            offset: ScalarOffset(len),
            bias: Bias::After,
        },
    })
}

pub(crate) fn insert_table(
    document: &mut Document,
    at: &Position,
    rows: u32,
    columns: u32,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    if !(1..=MAX_AXIS).contains(&rows) || !(1..=MAX_AXIS).contains(&columns) {
        return Err(CommandError::TableLimit);
    }
    let index = body_index(document, at)?;
    let widths = column_widths(document, columns);
    let mut table_rows = Vec::with_capacity(rows as usize);
    let mut paragraphs = Vec::new();
    for _ in 0..rows {
        let mut cells = Vec::with_capacity(columns as usize);
        for width in &widths {
            let paragraph = empty_paragraph(document, &mut paragraphs);
            cells.push(Cell {
                id: document.ids().node(),
                grid_span: 1,
                vertical_merge: VerticalMerge::None,
                width: Some(*width),
                borders: insertion_borders(),
                shading: None,
                margin: None,
                vertical_align: None,
                blocks: BlockSeq::from_vec(vec![paragraph.into()]),
                preserved: None,
            });
        }
        table_rows.push(Row {
            id: document.ids().node(),
            grid_before: 0,
            grid_after: 0,
            cells,
            header: false,
            cant_split: false,
            height: None,
            preserved: None,
        });
    }
    let table_id = document.ids().node();
    let table = Table {
        id: table_id,
        grid: widths,
        rows: table_rows,
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
    document
        .story_mut(at.story)
        .ok_or(CommandError::UnknownStory(at.story))?
        .blocks
        .insert(index, table.into());
    note_table(changes, at.story, table_id, &paragraphs, &[]);
    Ok(())
}

pub(crate) fn insert_row(
    document: &mut Document,
    at: &Position,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let site = site(document, at)?;
    let table_id = site.table;
    let story = at.story;
    let row_index = site.row;
    let template = {
        let table = table_mut(document, story, site.index, table_id)?;
        if table.rows.len() as u32 >= MAX_AXIS {
            return Err(CommandError::TableLimit);
        }
        if has_vertical_merge(table) {
            return Err(CommandError::MergedStructure(table.id));
        }
        let template = &table.rows[row_index];
        (
            template.grid_before,
            template.grid_after,
            template
                .cells
                .iter()
                .map(|cell| (cell.grid_span, cell.width))
                .collect::<Vec<_>>(),
        )
    };
    let mut paragraphs = Vec::new();
    let mut cells = Vec::with_capacity(template.2.len());
    for (span, width) in template.2 {
        let paragraph = empty_paragraph(document, &mut paragraphs);
        cells.push(Cell {
            id: document.ids().node(),
            grid_span: span,
            vertical_merge: VerticalMerge::None,
            width,
            borders: insertion_borders(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![paragraph.into()]),
            preserved: None,
        });
    }
    let row = Row {
        id: document.ids().node(),
        grid_before: template.0,
        grid_after: template.1,
        cells,
        header: false,
        cant_split: false,
        height: None,
        preserved: None,
    };
    table_mut(document, story, site.index, table_id)?
        .rows
        .insert(row_index + 1, row);
    note_table(changes, story, table_id, &paragraphs, &[]);
    Ok(())
}

pub(crate) fn delete_row(
    document: &mut Document,
    at: &Position,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let site = site(document, at)?;
    let table_id = site.table;
    let story = at.story;
    let row_index = site.row;
    let (removed, fallback) = {
        let table = table_mut(document, story, site.index, table_id)?;
        if table.rows.len() <= 1 {
            return Err(CommandError::LastRow(table_id));
        }
        if has_vertical_merge(table) {
            return Err(CommandError::MergedStructure(table_id));
        }
        let removed = paragraph_ids_in_row(&table.rows[row_index]);
        let fallback_row = if row_index + 1 < table.rows.len() {
            row_index + 1
        } else {
            row_index - 1
        };
        let (paragraph, offset) = if fallback_row > row_index {
            (first_paragraph(&table.rows[fallback_row])?, 0)
        } else {
            let paragraph = last_paragraph(&table.rows[fallback_row])?;
            let len = paragraph_len_in_row(&table.rows[fallback_row], paragraph).unwrap_or(0);
            (paragraph, len)
        };
        table.rows.remove(row_index);
        (removed, (story, paragraph, offset))
    };
    for paragraph in &removed {
        map.push(MapStep::Remove {
            paragraph: *paragraph,
            fallback: Some(fallback),
        });
    }
    note_table(changes, story, table_id, &[], &removed);
    Ok(())
}

pub(crate) fn insert_column(
    document: &mut Document,
    at: &Position,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let site = site(document, at)?;
    let table_id = site.table;
    let story = at.story;
    let at_column = site.cell + 1;
    let (width, row_count) = {
        let table = table_mut(document, story, site.index, table_id)?;
        plain_columns(table)?;
        if table.grid.len() as u32 >= MAX_AXIS {
            return Err(CommandError::TableLimit);
        }
        (table.grid[site.cell], table.rows.len())
    };
    let mut paragraphs = Vec::new();
    let mut cells = Vec::with_capacity(row_count);
    for _ in 0..row_count {
        let paragraph = empty_paragraph(document, &mut paragraphs);
        cells.push(Cell {
            id: document.ids().node(),
            grid_span: 1,
            vertical_merge: VerticalMerge::None,
            width: Some(width),
            borders: insertion_borders(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![paragraph.into()]),
            preserved: None,
        });
    }
    let table = table_mut(document, story, site.index, table_id)?;
    table.grid.insert(at_column, width);
    for (row, cell) in table.rows.iter_mut().zip(cells) {
        row.cells.insert(at_column, cell);
    }
    note_table(changes, story, table_id, &paragraphs, &[]);
    Ok(())
}

pub(crate) fn delete_column(
    document: &mut Document,
    at: &Position,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let site = site(document, at)?;
    let table_id = site.table;
    let story = at.story;
    let column = site.cell;
    let (removed, fallback) = {
        let table = table_mut(document, story, site.index, table_id)?;
        plain_columns(table)?;
        if table.grid.len() <= 1 {
            return Err(CommandError::LastColumn(table.id));
        }
        let mut removed = Vec::new();
        for row in &table.rows {
            removed.extend(paragraph_ids_of_cell(&row.cells[column]));
        }
        let neighbour = if column + 1 < table.grid.len() {
            column + 1
        } else {
            column - 1
        };
        let paragraph = first_paragraph_of_cell(&table.rows[site.row].cells[neighbour])?;
        let offset = if neighbour > column {
            0
        } else {
            paragraph_len_in_cell(&table.rows[site.row].cells[neighbour], paragraph).unwrap_or(0)
        };
        table.grid.remove(column);
        for row in &mut table.rows {
            row.cells.remove(column);
        }
        (removed, (story, paragraph, offset))
    };
    for paragraph in &removed {
        map.push(MapStep::Remove {
            paragraph: *paragraph,
            fallback: Some(fallback),
        });
    }
    note_table(changes, story, table_id, &[], &removed);
    Ok(())
}

pub(crate) fn merge_cells(
    document: &mut Document,
    at: &Position,
    direction: TableMerge,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    match direction {
        TableMerge::Right => merge_right(document, at, changes),
        TableMerge::Down => merge_down(document, at, changes),
    }
}

pub(crate) fn split_cell(
    document: &mut Document,
    at: &Position,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let site = site(document, at)?;
    let table_id = site.table;
    let story = at.story;
    let row_index = site.row;
    let cell_index = site.cell;
    let kind = {
        let table = table_at(document, story, site.index, table_id)?;
        let cell = cell_ref(table, row_index, cell_index, table_id)?;
        if held(table, row_index, cell_index) {
            return Err(refuse(table_id, TableStructure::Preserved));
        }
        if unsupported_cell(cell) {
            return Err(refuse(table_id, TableStructure::UnsupportedCell));
        }
        if cell_locked(document, story, cell)? {
            return Err(CommandError::Locked(at.paragraph));
        }
        cell.vertical_merge
    };
    if kind == VerticalMerge::Continue {
        return Err(refuse(table_id, TableStructure::ContinueCell));
    }
    if kind == VerticalMerge::Restart {
        return split_vertical(
            document, story, site.index, table_id, row_index, cell_index, changes,
        );
    }
    split_horizontal(
        document, story, site.index, table_id, row_index, cell_index, changes,
    )
}

/// Writes table width, alignment, table cell margins and direct cell borders.
/// One call changes the table or leaves it untouched. `w:tblGrid` is not
/// rewritten. Per-cell `w:tcMar` is not cleared.
pub(crate) fn set_properties(
    document: &mut Document,
    at: &Position,
    change: &TablePropertyChange,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    if change.is_empty() {
        return Err(CommandError::TableProperties("table properties are empty"));
    }
    check_property_change(change)?;
    let site = site(document, at)?;
    let story = at.story;
    let table_id = site.table;
    let paragraphs = {
        let table = table_mut(document, story, site.index, table_id)?;
        if table.preserved.is_some() {
            return Err(refuse(table_id, TableStructure::Preserved));
        }
        for row in &table.rows {
            if row.preserved.is_some() {
                return Err(refuse(table_id, TableStructure::Preserved));
            }
            for cell in &row.cells {
                if cell.preserved.is_some() {
                    return Err(refuse(table_id, TableStructure::Preserved));
                }
            }
        }
        if let Some(width) = change.width {
            table.width = Some(width);
        }
        if let Some(alignment) = change.alignment {
            table.alignment = Some(alignment);
        }
        if let Some(margin) = change.cell_margin {
            table.cell_margin = Some(margin);
        }
        if let Some(borders) = change.borders {
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    cell.borders = borders;
                }
            }
        }
        table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().flat_map(paragraph_ids_of_cell))
            .collect::<Vec<_>>()
    };
    note_table(changes, story, table_id, &[], &[]);
    changes.paragraphs.extend(paragraphs);
    Ok(())
}

/// Twenty-two inches, the same direct-length limit as paragraph indents.
const MAX_PROPERTY_TWIPS: i64 = 22 * 1_440;
/// 1000% in fiftieths of a percent. Larger percentages stay outside the dialog.
const MAX_PROPERTY_PCT: i64 = 50_000;

fn check_property_change(change: &TablePropertyChange) -> Result<(), CommandError> {
    if let Some(width) = change.width {
        match width {
            TableWidth::Auto => {}
            TableWidth::Dxa(value) if (1..=MAX_PROPERTY_TWIPS).contains(&value) => {}
            TableWidth::Pct(value) if (1..=MAX_PROPERTY_PCT).contains(&value) => {}
            TableWidth::Dxa(_) | TableWidth::Pct(_) => {
                return Err(CommandError::TableProperties("table width is out of range"));
            }
        }
    }
    if let Some(margin) = change.cell_margin {
        for value in [margin.top, margin.left, margin.bottom, margin.right] {
            if !(0..=MAX_PROPERTY_TWIPS).contains(&value) {
                return Err(CommandError::TableProperties(
                    "table cell margin is out of range",
                ));
            }
        }
    }
    if let Some(borders) = change.borders {
        for edge in [borders.top, borders.left, borders.bottom, borders.right]
            .into_iter()
            .flatten()
        {
            check_border(edge)?;
        }
    }
    Ok(())
}

fn check_border(edge: BorderEdge) -> Result<(), CommandError> {
    let size_ok = match edge.kind {
        BorderKind::Nil => edge.size_eighths <= 255,
        BorderKind::Single
        | BorderKind::Double
        | BorderKind::Dotted
        | BorderKind::Dashed
        | BorderKind::Thick => (1..=255).contains(&edge.size_eighths),
    };
    let color_ok = matches!(
        edge.color,
        Color::Auto | Color::Rgb(_, _, _) | Color::Theme { .. }
    );
    if size_ok && color_ok {
        Ok(())
    } else {
        Err(CommandError::TableProperties(
            "table border is outside the typed subset",
        ))
    }
}

fn merge_right(
    document: &mut Document,
    at: &Position,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let site = site(document, at)?;
    let table_id = site.table;
    let story = at.story;
    let row_index = site.row;
    let cell_index = site.cell;
    {
        let table = table_at(document, story, site.index, table_id)?;
        let row = table
            .rows
            .get(row_index)
            .ok_or(CommandError::NotATable(table_id))?;
        if cell_index + 1 >= row.cells.len() {
            return Err(refuse(table_id, TableStructure::NoNeighbour));
        }
        if row.cells[cell_index].vertical_merge != VerticalMerge::None
            || row.cells[cell_index + 1].vertical_merge != VerticalMerge::None
        {
            return Err(refuse(table_id, TableStructure::VerticalMerge));
        }
        if held(table, row_index, cell_index) || held(table, row_index, cell_index + 1) {
            return Err(refuse(table_id, TableStructure::Preserved));
        }
        if unsupported_cell(&row.cells[cell_index]) || unsupported_cell(&row.cells[cell_index + 1])
        {
            return Err(refuse(table_id, TableStructure::UnsupportedCell));
        }
        if cell_locked(document, story, &row.cells[cell_index])?
            || cell_locked(document, story, &row.cells[cell_index + 1])?
        {
            return Err(CommandError::Locked(at.paragraph));
        }
    }
    let table = table_mut(document, story, site.index, table_id)?;
    let mut neighbour = table.rows[row_index].cells.remove(cell_index + 1);
    let moved = drain_blocks(&mut neighbour);
    let touched = block_ids(&moved);
    let anchor = &mut table.rows[row_index].cells[cell_index];
    anchor.grid_span = anchor
        .grid_span
        .checked_add(neighbour.grid_span)
        .ok_or(CommandError::TableLimit)?;
    if let (Some(Measure::Twips(left)), Some(Measure::Twips(right))) =
        (anchor.width, neighbour.width)
        && let Some(sum) = left.checked_add(right)
    {
        anchor.width = Some(Measure::Twips(sum));
    }
    anchor.borders.right = neighbour.borders.right;
    for block in moved {
        anchor.blocks.push(block);
    }
    note_table(changes, story, table_id, &[], &[]);
    changes.paragraphs.extend(touched);
    Ok(())
}

fn merge_down(
    document: &mut Document,
    at: &Position,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let site = site(document, at)?;
    let table_id = site.table;
    let story = at.story;
    let row_index = site.row;
    let cell_index = site.cell;
    let (column, span, end, below_index) = {
        let table = table_at(document, story, site.index, table_id)?;
        let cell = cell_ref(table, row_index, cell_index, table_id)?;
        if cell.vertical_merge == VerticalMerge::Continue {
            return Err(refuse(table_id, TableStructure::ContinueCell));
        }
        if held(table, row_index, cell_index) {
            return Err(refuse(table_id, TableStructure::Preserved));
        }
        if unsupported_cell(cell) {
            return Err(refuse(table_id, TableStructure::UnsupportedCell));
        }
        if cell_locked(document, story, cell)? {
            return Err(CommandError::Locked(at.paragraph));
        }
        let column = column_of(&table.rows[row_index], cell_index);
        let span = cell.grid_span;
        let end = span_end(table, row_index, column, span);
        let Some(below_row) = table.rows.get(end + 1) else {
            return Err(refuse(table_id, TableStructure::NoNeighbour));
        };
        let Some(below_index) = cell_covering(below_row, column, span) else {
            return Err(refuse(table_id, TableStructure::SpanMismatch));
        };
        if below_row.cells[below_index].vertical_merge != VerticalMerge::None {
            return Err(refuse(table_id, TableStructure::VerticalConflict));
        }
        if held(table, end + 1, below_index) {
            return Err(refuse(table_id, TableStructure::Preserved));
        }
        if unsupported_cell(&below_row.cells[below_index]) {
            return Err(refuse(table_id, TableStructure::UnsupportedCell));
        }
        if cell_locked(document, story, &below_row.cells[below_index])? {
            return Err(CommandError::Locked(at.paragraph));
        }
        (column, span, end, below_index)
    };
    let _ = (column, span);
    let (bottom, taken) = {
        let table = table_mut(document, story, site.index, table_id)?;
        let bottom = table.rows[end + 1].cells[below_index].borders.bottom;
        let taken = drain_blocks(&mut table.rows[end + 1].cells[below_index]);
        (bottom, taken)
    };
    let mut kept = Vec::new();
    let mut moved = Vec::new();
    for block in taken {
        if block_has_content(&block) {
            moved.push(block);
        } else {
            kept.push(block);
        }
    }
    let mut inserted = Vec::new();
    if kept.is_empty() {
        let paragraph = empty_paragraph(document, &mut inserted);
        kept.push(paragraph.into());
    }
    let touched = block_ids(&moved);
    {
        let table = table_mut(document, story, site.index, table_id)?;
        table.rows[end + 1].cells[below_index].blocks = BlockSeq::from_vec(kept);
        table.rows[end + 1].cells[below_index].vertical_merge = VerticalMerge::Continue;
        let anchor = &mut table.rows[row_index].cells[cell_index];
        anchor.vertical_merge = VerticalMerge::Restart;
        anchor.borders.bottom = bottom;
        for block in moved {
            anchor.blocks.push(block);
        }
    }
    note_table(changes, story, table_id, &inserted, &[]);
    changes.paragraphs.extend(touched);
    changes.paragraphs.extend(&inserted);
    Ok(())
}

fn split_vertical(
    document: &mut Document,
    story: StoryId,
    index: usize,
    table_id: NodeId,
    row_index: usize,
    cell_index: usize,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    {
        let table = table_at(document, story, index, table_id)?;
        let column = column_of(&table.rows[row_index], cell_index);
        let span = table.rows[row_index].cells[cell_index].grid_span;
        for row in table.rows.iter().skip(row_index + 1) {
            let Some(found) = cell_covering(row, column, span) else {
                break;
            };
            if row.cells[found].vertical_merge != VerticalMerge::Continue {
                break;
            }
            if row.preserved.is_some() || row.cells[found].preserved.is_some() {
                return Err(refuse(table_id, TableStructure::Preserved));
            }
        }
    }
    let table = table_mut(document, story, index, table_id)?;
    let column = column_of(&table.rows[row_index], cell_index);
    let span = table.rows[row_index].cells[cell_index].grid_span;
    table.rows[row_index].cells[cell_index].vertical_merge = VerticalMerge::None;
    for row in table.rows.iter_mut().skip(row_index + 1) {
        let Some(found) = cell_covering(row, column, span) else {
            break;
        };
        if row.cells[found].vertical_merge != VerticalMerge::Continue {
            break;
        }
        row.cells[found].vertical_merge = VerticalMerge::None;
    }
    note_table(changes, story, table_id, &[], &[]);
    Ok(())
}

fn split_horizontal(
    document: &mut Document,
    story: StoryId,
    index: usize,
    table_id: NodeId,
    row_index: usize,
    cell_index: usize,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let span = {
        let table = table_at(document, story, index, table_id)?;
        let span = cell_ref(table, row_index, cell_index, table_id)?.grid_span;
        if span <= 1 {
            return Err(refuse(table_id, TableStructure::NotMerged));
        }
        span
    };
    let mut inserted = Vec::new();
    let mut paragraphs = Vec::new();
    let mut cell_ids = Vec::new();
    for _ in 1..span {
        paragraphs.push(empty_paragraph(document, &mut inserted));
        cell_ids.push(document.ids().node());
    }
    let table = table_mut(document, story, index, table_id)?;
    let start = column_of(&table.rows[row_index], cell_index);
    let mut original = table.rows[row_index].cells.remove(cell_index);
    let top = original.borders.top;
    let bottom = original.borders.bottom;
    let right = original.borders.right;
    original.grid_span = 1;
    original.width = table.grid.get(start as usize).copied();
    original.borders.right = Some(insertion_edge());
    let mut replacements = vec![original];
    for (offset, (paragraph, id)) in paragraphs.into_iter().zip(cell_ids).enumerate() {
        let offset = offset as u32 + 1;
        let edge = insertion_edge();
        replacements.push(Cell {
            id,
            grid_span: 1,
            vertical_merge: VerticalMerge::None,
            width: table.grid.get((start + offset) as usize).copied(),
            borders: CellBorders {
                top,
                left: Some(edge),
                bottom,
                right: if offset + 1 == span {
                    right
                } else {
                    Some(edge)
                },
            },
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![paragraph.into()]),
            preserved: None,
        });
    }
    for (offset, cell) in replacements.into_iter().enumerate() {
        table.rows[row_index]
            .cells
            .insert(cell_index + offset, cell);
    }
    note_table(changes, story, table_id, &inserted, &[]);
    Ok(())
}

fn refuse(table: NodeId, reason: TableStructure) -> CommandError {
    CommandError::TableStructure(table, reason)
}

fn table_at(
    document: &Document,
    story: StoryId,
    index: usize,
    table: NodeId,
) -> Result<&Table, CommandError> {
    match document
        .story(story)
        .and_then(|story| story.blocks.get(index))
    {
        Some(Block::Table(table)) => Ok(table),
        _ => Err(CommandError::NotATable(table)),
    }
}

fn cell_ref(table: &Table, row: usize, cell: usize, id: NodeId) -> Result<&Cell, CommandError> {
    table
        .rows
        .get(row)
        .and_then(|row| row.cells.get(cell))
        .ok_or(CommandError::NotATable(id))
}

fn held(table: &Table, row: usize, cell: usize) -> bool {
    table.preserved.is_some()
        || table.rows[row].preserved.is_some()
        || table.rows[row].cells[cell].preserved.is_some()
}

fn unsupported_cell(cell: &Cell) -> bool {
    cell.blocks
        .iter()
        .any(|block| block.as_paragraph().is_none())
}

fn cell_locked(document: &Document, story: StoryId, cell: &Cell) -> Result<bool, CommandError> {
    for block in cell.blocks.iter() {
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        let location = locate::locate(document, story, paragraph.id())
            .ok_or(CommandError::UnknownNode(paragraph.id()))?;
        if location.content_locked {
            return Ok(true);
        }
    }
    Ok(false)
}

fn column_of(row: &Row, cell_index: usize) -> u32 {
    row.grid_before
        + row
            .cells
            .iter()
            .take(cell_index)
            .map(|cell| cell.grid_span)
            .sum::<u32>()
}

fn cell_covering(row: &Row, column: u32, span: u32) -> Option<usize> {
    let mut cursor = row.grid_before;
    for (index, cell) in row.cells.iter().enumerate() {
        if cursor == column && cell.grid_span == span {
            return Some(index);
        }
        if cursor > column {
            return None;
        }
        cursor = cursor.saturating_add(cell.grid_span);
    }
    None
}

fn span_end(table: &Table, row: usize, column: u32, span: u32) -> usize {
    let Some(cell_index) = cell_covering(&table.rows[row], column, span) else {
        return row;
    };
    if table.rows[row].cells[cell_index].vertical_merge != VerticalMerge::Restart {
        return row;
    }
    let mut end = row;
    for next in (row + 1)..table.rows.len() {
        let Some(found) = cell_covering(&table.rows[next], column, span) else {
            break;
        };
        if table.rows[next].cells[found].vertical_merge != VerticalMerge::Continue {
            break;
        }
        end = next;
    }
    end
}

fn drain_blocks(cell: &mut Cell) -> Vec<Block> {
    let mut blocks = Vec::new();
    while !cell.blocks.is_empty() {
        blocks.push(cell.blocks.remove(0));
    }
    blocks
}

fn block_ids(blocks: &[Block]) -> Vec<NodeId> {
    blocks.iter().map(|block| block.id()).collect()
}

fn block_has_content(block: &Block) -> bool {
    match block.as_paragraph() {
        Some(paragraph) => !paragraph.is_empty() || !paragraph.inlines().is_empty(),
        None => true,
    }
}

fn insertion_edge() -> BorderEdge {
    BorderEdge {
        kind: BorderKind::Single,
        size_eighths: 4,
        color: Color::Auto,
    }
}

struct Site {
    index: usize,
    table: NodeId,
    row: usize,
    cell: usize,
}

fn site(document: &Document, at: &Position) -> Result<Site, CommandError> {
    let location = locate::locate(document, at.story, at.paragraph)
        .ok_or(CommandError::UnknownNode(at.paragraph))?;
    if location.content_locked {
        return Err(CommandError::Locked(at.paragraph));
    }
    let &[(index, Step::Cell { row, cell })] = location.containers.as_slice() else {
        return Err(CommandError::NotATable(at.paragraph));
    };
    let block = document
        .story(at.story)
        .ok_or(CommandError::UnknownStory(at.story))?
        .blocks
        .get(index)
        .ok_or(CommandError::UnknownNode(at.paragraph))?;
    let Block::Table(table) = block else {
        return Err(CommandError::NotATable(at.paragraph));
    };
    Ok(Site {
        index,
        table: table.id,
        row,
        cell,
    })
}

fn body_index(document: &Document, at: &Position) -> Result<usize, CommandError> {
    let location = locate::locate(document, at.story, at.paragraph)
        .ok_or(CommandError::UnknownNode(at.paragraph))?;
    if location.content_locked {
        return Err(CommandError::Locked(at.paragraph));
    }
    let story = document
        .story(at.story)
        .ok_or(CommandError::UnknownStory(at.story))?;
    if story.kind != StoryKind::Body {
        return Err(CommandError::NotTopLevel);
    }
    Ok(location
        .containers
        .first()
        .map(|(index, _)| *index)
        .unwrap_or(location.index))
}

fn table_mut(
    document: &mut Document,
    story: StoryId,
    index: usize,
    table: NodeId,
) -> Result<&mut Table, CommandError> {
    match document
        .story_mut(story)
        .and_then(|story| story.blocks.get_mut(index))
    {
        Some(Block::Table(table)) => Ok(Arc::make_mut(table)),
        _ => Err(CommandError::NotATable(table)),
    }
}

fn has_vertical_merge(table: &Table) -> bool {
    table.rows.iter().any(|row| {
        row.cells
            .iter()
            .any(|cell| cell.vertical_merge != VerticalMerge::None)
    })
}

fn plain_columns(table: &Table) -> Result<(), CommandError> {
    let merged = table.rows.iter().any(|row| {
        row.cells
            .iter()
            .any(|cell| cell.grid_span != 1 || cell.vertical_merge != VerticalMerge::None)
    });
    if merged {
        return Err(CommandError::MergedStructure(table.id));
    }
    let gapped = table
        .rows
        .iter()
        .any(|row| row.grid_before != 0 || row.grid_after != 0);
    if gapped {
        return Err(CommandError::GridGap(table.id));
    }
    Ok(())
}

fn column_widths(document: &Document, columns: u32) -> Vec<Measure> {
    let columns = columns as i64;
    let content = document.sections().first().and_then(|section| {
        match (section.page.width, section.page.left, section.page.right) {
            (Measure::Twips(width), Measure::Twips(left), Measure::Twips(right)) => {
                let content = width.saturating_sub(left).saturating_sub(right);
                (content > 0).then_some(content)
            }
            _ => None,
        }
    });
    let Some(content) = content else {
        return vec![Measure::Twips(COLUMN_FALLBACK_TWIPS); columns as usize];
    };
    let base = content / columns;
    if base <= 0 {
        return vec![Measure::Twips(COLUMN_FALLBACK_TWIPS); columns as usize];
    }
    let extra = content % columns;
    (0..columns)
        .map(|index| Measure::Twips(base + i64::from(index < extra)))
        .collect()
}

fn insertion_borders() -> CellBorders {
    let edge = BorderEdge {
        kind: BorderKind::Single,
        size_eighths: 4,
        color: Color::Auto,
    };
    CellBorders {
        top: Some(edge),
        left: Some(edge),
        bottom: Some(edge),
        right: Some(edge),
    }
}

fn empty_paragraph(document: &mut Document, ids: &mut Vec<NodeId>) -> Paragraph {
    let id = document.ids().node();
    ids.push(id);
    Paragraph::new(id)
}

fn note_table(
    changes: &mut ChangeSet,
    story: StoryId,
    table: NodeId,
    inserted: &[NodeId],
    removed: &[NodeId],
) {
    changes.stories.insert(story);
    changes.tables.insert(table);
    changes.inserted.extend(inserted);
    changes.paragraphs.extend(inserted);
    changes.removed.extend(removed);
}

fn paragraph_ids_in_row(row: &Row) -> Vec<NodeId> {
    row.cells.iter().flat_map(paragraph_ids_of_cell).collect()
}

fn paragraph_ids_of_cell(cell: &Cell) -> Vec<NodeId> {
    cell.blocks.iter().map(|block| block.id()).collect()
}

fn first_paragraph(row: &Row) -> Result<NodeId, CommandError> {
    row.cells
        .first()
        .and_then(|cell| cell.blocks.iter().next())
        .map(|block| block.id())
        .ok_or(CommandError::TableLimit)
}

fn last_paragraph(row: &Row) -> Result<NodeId, CommandError> {
    row.cells
        .last()
        .and_then(|cell| cell.blocks.iter().last())
        .map(|block| block.id())
        .ok_or(CommandError::TableLimit)
}

fn first_paragraph_of_cell(cell: &Cell) -> Result<NodeId, CommandError> {
    cell.blocks
        .iter()
        .next()
        .map(|block| block.id())
        .ok_or(CommandError::TableLimit)
}

fn paragraph_len_in_row(row: &Row, id: NodeId) -> Option<usize> {
    row.cells
        .iter()
        .find_map(|cell| paragraph_len_in_cell(cell, id))
}

fn paragraph_len_in_cell(cell: &Cell, id: NodeId) -> Option<usize> {
    cell.blocks.iter().find_map(|block| {
        block
            .as_paragraph()
            .filter(|paragraph| paragraph.id() == id)
            .map(|paragraph| paragraph.len())
    })
}

fn cell_paragraphs(document: &Document, address: CellAddress) -> Result<Vec<NodeId>, CommandError> {
    let table = find_table(document, address)?;
    let cell = table
        .rows
        .get(address.row)
        .and_then(|row| row.cells.get(address.cell))
        .ok_or(CommandError::NotATable(address.table))?;
    let mut ids = Vec::new();
    for block in cell.blocks.iter() {
        let Some(paragraph) = block.as_paragraph() else {
            return Err(CommandError::NotATable(address.table));
        };
        ids.push(paragraph.id());
    }
    if ids.is_empty() {
        return Err(CommandError::NotATable(address.table));
    }
    Ok(ids)
}

fn paragraph_len(
    document: &Document,
    address: CellAddress,
    id: NodeId,
) -> Result<usize, CommandError> {
    let table = find_table(document, address)?;
    paragraph_len_in_row(
        table
            .rows
            .get(address.row)
            .ok_or(CommandError::NotATable(address.table))?,
        id,
    )
    .ok_or(CommandError::UnknownNode(id))
}

fn find_table(document: &Document, address: CellAddress) -> Result<&Table, CommandError> {
    let block = document
        .story(address.story)
        .ok_or(CommandError::UnknownStory(address.story))?
        .blocks
        .iter()
        .find(|block| block.id() == address.table)
        .ok_or(CommandError::UnknownNode(address.table))?;
    match block {
        Block::Table(table) => Ok(table),
        _ => Err(CommandError::NotATable(address.table)),
    }
}
