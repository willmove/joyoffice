//! Table structure edits (T201). Merge and split stay preserved state: a
//! command that would change a vertical merge, a horizontal span or a grid
//! gap is refused, so the stored grid stays one the model already accepts.

use std::sync::Arc;

use joyword_model::{
    Bias, Block, BlockSeq, BorderEdge, BorderKind, Cell, CellBorders, Color, Document, Measure,
    NodeId, Paragraph, Position, Row, ScalarOffset, StoryId, StoryKind, Table, VerticalMerge,
};

use crate::command::CommandError;
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
    let last = *paragraphs.last().expect("non-empty");
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
        preserved: None,
        source: None,
    };
    document
        .story_mut(at.story)
        .expect("located story")
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
