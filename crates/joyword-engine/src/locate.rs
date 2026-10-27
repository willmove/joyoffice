//! Finding blocks by identifier and borrowing their containers mutably with
//! path copying. Lookups are linear in story size, bounded by MAX_NESTING;
//! an index arrives when incremental layout (LAYOUT-010) needs it.

use std::sync::Arc;

use joyword_model::{Block, BlockSeq, Document, MAX_NESTING, NodeId, Paragraph, SdtLock, StoryId};

/// How a container nests inside the block at a given index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Cell { row: usize, cell: usize },
    Sdt,
}

/// Path to a block: the containers from the story root, then its index in
/// the innermost container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Location {
    pub story: StoryId,
    pub containers: Vec<(usize, Step)>,
    pub index: usize,
    /// An enclosing content control forbids editing its content.
    pub content_locked: bool,
}

impl Location {
    pub fn same_container(&self, other: &Location) -> bool {
        self.story == other.story && self.containers == other.containers
    }
}

pub(crate) fn locate(document: &Document, story: StoryId, node: NodeId) -> Option<Location> {
    fn search(
        blocks: &BlockSeq,
        node: NodeId,
        path: &mut Vec<(usize, Step)>,
        locked: bool,
        depth: usize,
    ) -> Option<(usize, bool)> {
        for (index, block) in blocks.iter().enumerate() {
            if block.id() == node {
                return Some((index, locked));
            }
            if depth >= MAX_NESTING {
                continue;
            }
            match block {
                Block::Table(table) => {
                    for (row_index, row) in table.rows.iter().enumerate() {
                        for (cell_index, cell) in row.cells.iter().enumerate() {
                            path.push((
                                index,
                                Step::Cell {
                                    row: row_index,
                                    cell: cell_index,
                                },
                            ));
                            if let Some(found) = search(&cell.blocks, node, path, locked, depth + 1)
                            {
                                return Some(found);
                            }
                            path.pop();
                        }
                    }
                }
                Block::Sdt(sdt) => {
                    path.push((index, Step::Sdt));
                    let locked = locked || !sdt.props.lock.content_editable();
                    if let Some(found) = search(&sdt.blocks, node, path, locked, depth + 1) {
                        return Some(found);
                    }
                    path.pop();
                }
                Block::Paragraph(_) | Block::Opaque(_) => {}
            }
        }
        None
    }

    let mut containers = Vec::new();
    let (index, content_locked) = search(
        &document.story(story)?.blocks,
        node,
        &mut containers,
        false,
        0,
    )?;
    Some(Location {
        story,
        containers,
        index,
        content_locked,
    })
}

pub(crate) fn container<'a>(
    document: &'a Document,
    story: StoryId,
    containers: &[(usize, Step)],
) -> Option<&'a BlockSeq> {
    let mut seq = &document.story(story)?.blocks;
    for &(index, step) in containers {
        seq = match (seq.get(index)?, step) {
            (Block::Table(table), Step::Cell { row, cell }) => {
                &table.rows.get(row)?.cells.get(cell)?.blocks
            }
            (Block::Sdt(sdt), Step::Sdt) => &sdt.blocks,
            _ => return None,
        };
    }
    Some(seq)
}

/// Mutable container, copying only the path to it.
pub(crate) fn container_mut<'a>(
    document: &'a mut Document,
    story: StoryId,
    containers: &[(usize, Step)],
) -> Option<&'a mut BlockSeq> {
    let mut seq = &mut document.story_mut(story)?.blocks;
    for &(index, step) in containers {
        seq = match ({ seq }.get_mut(index)?, step) {
            (Block::Table(table), Step::Cell { row, cell }) => {
                &mut Arc::make_mut(table)
                    .rows
                    .get_mut(row)?
                    .cells
                    .get_mut(cell)?
                    .blocks
            }
            (Block::Sdt(sdt), Step::Sdt) => &mut Arc::make_mut(sdt).blocks,
            _ => return None,
        };
    }
    Some(seq)
}

pub(crate) fn block<'a>(document: &'a Document, location: &Location) -> Option<&'a Block> {
    container(document, location.story, &location.containers)?.get(location.index)
}

pub(crate) fn paragraph<'a>(document: &'a Document, location: &Location) -> Option<&'a Paragraph> {
    block(document, location)?.as_paragraph()
}

pub(crate) fn paragraph_mut<'a>(
    document: &'a mut Document,
    location: &Location,
) -> Option<&'a mut Paragraph> {
    match container_mut(document, location.story, &location.containers)?.get_mut(location.index)? {
        Block::Paragraph(paragraph) => Some(Arc::make_mut(paragraph)),
        _ => None,
    }
}

/// Every paragraph inside `block`, in document order (including itself).
pub(crate) fn paragraphs_in(block: &Block, out: &mut Vec<NodeId>) {
    fn walk(block: &Block, out: &mut Vec<NodeId>, depth: usize) {
        if depth > MAX_NESTING {
            return;
        }
        match block {
            Block::Paragraph(paragraph) => out.push(paragraph.id()),
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                    for inner in cell.blocks.iter() {
                        walk(inner, out, depth + 1);
                    }
                }
            }
            Block::Sdt(sdt) => {
                for inner in sdt.blocks.iter() {
                    walk(inner, out, depth + 1);
                }
            }
            Block::Opaque(_) => {}
        }
    }
    walk(block, out, 0);
}

/// Every table inside `block`, in document order (including itself and
/// nested tables).
pub(crate) fn tables_in(block: &Block, out: &mut Vec<NodeId>) {
    fn walk(block: &Block, out: &mut Vec<NodeId>, depth: usize) {
        if depth > MAX_NESTING {
            return;
        }
        match block {
            Block::Table(table) => {
                out.push(table.id);
                for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                    for inner in cell.blocks.iter() {
                        walk(inner, out, depth + 1);
                    }
                }
            }
            Block::Sdt(sdt) => {
                for inner in sdt.blocks.iter() {
                    walk(inner, out, depth + 1);
                }
            }
            Block::Paragraph(_) | Block::Opaque(_) => {}
        }
    }
    walk(block, out, 0);
}

/// A content control in `block` that may not be deleted or whose content may
/// not change, if any.
pub(crate) fn deletion_lock(block: &Block) -> Option<NodeId> {
    fn walk(block: &Block, depth: usize) -> Option<NodeId> {
        if depth > MAX_NESTING {
            return None;
        }
        match block {
            Block::Sdt(sdt) if sdt.props.lock != SdtLock::Unlocked => Some(sdt.id),
            Block::Sdt(sdt) => sdt.blocks.iter().find_map(|inner| walk(inner, depth + 1)),
            Block::Table(table) => table
                .rows
                .iter()
                .flat_map(|row| row.cells.iter())
                .flat_map(|cell| cell.blocks.iter())
                .find_map(|inner| walk(inner, depth + 1)),
            Block::Paragraph(_) | Block::Opaque(_) => None,
        }
    }
    walk(block, 0)
}

/// First paragraph inside `block` with offset 0, or last paragraph with its
/// length, used as a collapse boundary when a neighbouring block goes away.
pub(crate) fn edge_paragraph(block: &Block, first: bool) -> Option<(NodeId, usize)> {
    let mut paragraphs = Vec::new();
    paragraphs_in(block, &mut paragraphs);
    let id = if first {
        *paragraphs.first()?
    } else {
        *paragraphs.last()?
    };
    let len = if first {
        0
    } else {
        find_len(block, id).unwrap_or(0)
    };
    Some((id, len))
}

fn find_len(block: &Block, id: NodeId) -> Option<usize> {
    match block {
        Block::Paragraph(paragraph) => (paragraph.id() == id).then(|| paragraph.len()),
        Block::Table(table) => table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .flat_map(|cell| cell.blocks.iter())
            .find_map(|inner| find_len(inner, id)),
        Block::Sdt(sdt) => sdt.blocks.iter().find_map(|inner| find_len(inner, id)),
        Block::Opaque(_) => None,
    }
}

/// Applies `f` to every paragraph inside the block at `index`, copying only
/// the touched paths. Stops at the first error.
pub(crate) fn for_each_paragraph_mut<E>(
    seq: &mut BlockSeq,
    index: usize,
    f: &mut dyn FnMut(&mut Paragraph, bool) -> Result<(), E>,
) -> Result<(), E> {
    fn walk<E>(
        block: &mut Block,
        locked: bool,
        depth: usize,
        f: &mut dyn FnMut(&mut Paragraph, bool) -> Result<(), E>,
    ) -> Result<(), E> {
        if depth > MAX_NESTING {
            return Ok(());
        }
        match block {
            Block::Paragraph(paragraph) => f(Arc::make_mut(paragraph), locked),
            Block::Table(table) => {
                for row in &mut Arc::make_mut(table).rows {
                    for cell in &mut row.cells {
                        for inner in 0..cell.blocks.len() {
                            let block = cell.blocks.get_mut(inner).expect("index in range");
                            walk(block, locked, depth + 1, f)?;
                        }
                    }
                }
                Ok(())
            }
            Block::Sdt(sdt) => {
                let sdt = Arc::make_mut(sdt);
                let locked = locked || !sdt.props.lock.content_editable();
                for inner in 0..sdt.blocks.len() {
                    let block = sdt.blocks.get_mut(inner).expect("index in range");
                    walk(block, locked, depth + 1, f)?;
                }
                Ok(())
            }
            Block::Opaque(_) => Ok(()),
        }
    }
    match seq.get_mut(index) {
        Some(block) => walk(block, false, 0, f),
        None => Ok(()),
    }
}
