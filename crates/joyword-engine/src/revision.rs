//! Accept, reject and tracked typing for text, move and format revisions
//! (T300/T301).
//!
//! Accept and reject do not create revisions. A move is one change: accepting
//! or rejecting either side's revision id resolves both ends. Accept keeps
//! the destination and removes the source; reject keeps the source and
//! removes the destination. Insertions and deletions that sit inside a side
//! are left for a single accept or reject, and batch accept or reject
//! resolves them too, so those commands match the final and original
//! projections. A move run that crosses a marker, or the other move kind
//! inside a side, is [`CommandError::RevisionConflict`]. An incomplete or
//! crossed pair is refused. Every refusal leaves the document unchanged.
//!
//! Format revisions (`w:rPrChange` §17.13.5.30, `w:pPrChange` §17.13.5.29)
//! resolve by the same commands and ids: accepting drops the record and
//! keeps the current formatting; rejecting restores the recorded previous
//! formatting. An id that carries both a text and a format revision is
//! settled as one change.
//!
//! Table property revisions (`w:tblPrChange`, `w:trPrChange`, `w:tcPrChange`)
//! resolve the same way on the table, row or cell they were recorded on;
//! batch accept and reject sweep nested tables too. A table inside a locked
//! content control refuses the command and leaves the document unchanged.

use std::collections::BTreeMap;
use std::sync::Arc;

use joyword_model::{
    Block, Document, MoveMarker, MoveSide, NodeId, Paragraph, Position, RevisionKind, ScalarOffset,
    StoryId, Table, TextRevision,
};

use crate::command::CommandError;
use crate::locate::{self, paragraphs_in};
use crate::map::{MapStep, PositionMap, Selection};
use crate::session::{ChangeSet, Origin};

pub(crate) struct EditContext {
    pub origin: Origin,
    pub author: Arc<str>,
    pub date: Option<Arc<str>>,
    /// Record insert/delete as tracked revisions even when `w:trackRevisions`
    /// is off. AI accepts set this so the author is not the session user.
    pub force_tracked: bool,
}

#[derive(Clone, Copy)]
enum Resolution {
    Accept,
    Reject,
}

/// Text, format or table property revision at the caret, or overlapping the
/// selection.
///
/// A caret on the boundary between two runs prefers the run that ends there,
/// so the change just typed is the current one. When no text revision is
/// touched, an unresolved format revision (`w:rPrChange` / `w:pPrChange`)
/// covering the caret is returned instead; a paragraph-level change applies
/// wherever the caret sits in that paragraph. Last, a pending table property
/// revision on the enclosing cell, row or table is returned, innermost
/// first. `None` means the selection does not touch a revision.
pub fn current_revision(document: &Document, selection: &Selection) -> Option<u32> {
    if let Some(id) = revision_at(document, &selection.head) {
        return Some(id);
    }
    if selection.anchor.story == selection.head.story
        && selection.anchor.paragraph == selection.head.paragraph
        && selection.anchor.offset != selection.head.offset
    {
        let lo = selection.anchor.offset.0.min(selection.head.offset.0);
        let hi = selection.anchor.offset.0.max(selection.head.offset.0);
        if let Some(id) = revision_overlapping(
            document,
            selection.head.story,
            selection.head.paragraph,
            lo,
            hi,
        ) {
            return Some(id);
        }
    }
    if let Some(id) = revision_at(document, &selection.anchor) {
        return Some(id);
    }
    format_revision_at(document, &selection.head)
        .or_else(|| format_revision_at(document, &selection.anchor))
        .or_else(|| table_revision_at(document, &selection.head))
        .or_else(|| table_revision_at(document, &selection.anchor))
}

/// The pending table property revision around a position: the innermost
/// cell change, then its row, then its table, then outward. A table-level
/// change applies wherever the caret sits in the table.
fn table_revision_at(document: &Document, position: &Position) -> Option<u32> {
    let location = locate::locate(document, position.story, position.paragraph)?;
    let mut seq = &document.story(location.story)?.blocks;
    let mut levels: Vec<(&Table, usize, usize)> = Vec::new();
    for &(index, step) in &location.containers {
        match (seq.get(index)?, step) {
            (Block::Table(table), locate::Step::Cell { row, cell }) => {
                levels.push((table, row, cell));
                seq = &table.rows.get(row)?.cells.get(cell)?.blocks;
            }
            (Block::Sdt(sdt), locate::Step::Sdt) => seq = &sdt.blocks,
            _ => return None,
        }
    }
    for (table, row, cell) in levels.iter().rev() {
        if let Some(revision) = &table
            .rows
            .get(*row)
            .and_then(|row| row.cells.get(*cell))
            .and_then(|cell| cell.props_revision.as_ref())
        {
            return Some(revision.id);
        }
        if let Some(revision) = &table
            .rows
            .get(*row)
            .and_then(|row| row.props_revision.as_ref())
        {
            return Some(revision.id);
        }
        if let Some(revision) = &table.props_revision {
            return Some(revision.id);
        }
    }
    None
}

/// The format revision (`w:rPrChange` / `w:pPrChange`) at a position: a
/// paragraph-level change applies wherever the caret sits in the paragraph,
/// otherwise the run-level change covering the offset.
fn format_revision_at(document: &Document, position: &Position) -> Option<u32> {
    let paragraph = document.find_paragraph(position.story, position.paragraph)?;
    if let Some(revision) = &paragraph.props_revision {
        return Some(revision.id);
    }
    let offset = position.offset.0;
    let mut at = 0usize;
    for run in paragraph.runs() {
        let end = at + run.len;
        if at <= offset
            && offset <= end
            && let Some(revision) = &run.format_revision
        {
            return Some(revision.id);
        }
        at = end;
    }
    None
}

fn revision_at(document: &Document, position: &Position) -> Option<u32> {
    let paragraph = document.find_paragraph(position.story, position.paragraph)?;
    let offset = position.offset.0;
    let mut at = 0usize;
    let mut ending = None;
    for run in paragraph.runs() {
        let end = at + run.len;
        if end == offset {
            ending = run.revision.as_ref().map(|revision| revision.id);
        }
        if at < offset && offset < end {
            return run.revision.as_ref().map(|revision| revision.id);
        }
        if at == offset && offset < end {
            let starting = run.revision.as_ref().map(|revision| revision.id);
            return ending.or(starting);
        }
        at = end;
    }
    if ending.is_none() && offset == paragraph.len() {
        // The caret sits on the paragraph mark; surface its pending deletion.
        if let Some(revision) = &paragraph.mark_revision {
            return Some(revision.id);
        }
    }
    ending
}

fn revision_overlapping(
    document: &Document,
    story: StoryId,
    paragraph: NodeId,
    start: usize,
    end: usize,
) -> Option<u32> {
    let paragraph = document.find_paragraph(story, paragraph)?;
    let mut at = 0usize;
    for run in paragraph.runs() {
        let run_end = at + run.len;
        if at < end
            && start < run_end
            && let Some(revision) = &run.revision
        {
            return Some(revision.id);
        }
        at = run_end;
    }
    None
}

pub(crate) fn set_tracking(
    document: &mut Document,
    enabled: bool,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let next = Some(enabled);
    if document.settings().track_revisions == next {
        return Ok(());
    }
    document.settings_mut().track_revisions = next;
    changes.settings_changed = true;
    Ok(())
}

pub(crate) fn insert_tracked(
    document: &mut Document,
    at: &Position,
    text: &str,
    ctx: &EditContext,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let location = locate::locate(document, at.story, at.paragraph)
        .ok_or(CommandError::UnknownNode(at.paragraph))?;
    let revision = {
        let Some(paragraph) = locate::paragraph(document, &location) else {
            return Err(CommandError::UnknownNode(at.paragraph));
        };
        adjacent_insert(paragraph, at.offset.0, &ctx.author)
    };
    let revision = match revision {
        Some(revision) => revision,
        None => TextRevision {
            id: document
                .allocate_revision_id()
                .ok_or(CommandError::RevisionConflict)?,
            kind: RevisionKind::Insert,
            author: Arc::clone(&ctx.author),
            date: ctx.date.clone(),
        },
    };
    let Some(paragraph) = locate::paragraph_mut(document, &location) else {
        return Err(CommandError::UnknownNode(at.paragraph));
    };
    paragraph.insert_marked(at.offset, text, revision)?;
    map.push(MapStep::Insert {
        paragraph: at.paragraph,
        at: at.offset.0,
        len: text.chars().count(),
    });
    changes.stories.insert(at.story);
    changes.paragraphs.insert(at.paragraph);
    Ok(())
}

pub(crate) fn delete_tracked(
    document: &mut Document,
    start: &Position,
    end: &Position,
    ctx: &EditContext,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    if start.story != end.story {
        return Err(CommandError::CrossContainer);
    }
    let first_loc = locate::locate(document, start.story, start.paragraph)
        .ok_or(CommandError::UnknownNode(start.paragraph))?;
    let last_loc = locate::locate(document, start.story, end.paragraph)
        .ok_or(CommandError::UnknownNode(end.paragraph))?;
    if !first_loc.same_container(&last_loc) {
        return Err(CommandError::CrossContainer);
    }
    if first_loc.index > last_loc.index
        || (first_loc.index == last_loc.index && start.offset > end.offset)
    {
        return Err(CommandError::ReversedRange);
    }
    if first_loc.content_locked || last_loc.content_locked {
        return Err(CommandError::Locked(start.paragraph));
    }
    if first_loc.index == last_loc.index {
        if start.offset == end.offset {
            return Ok(());
        }
        let mut plan = {
            let Some(paragraph) = locate::paragraph(document, &first_loc) else {
                return Err(CommandError::UnknownNode(start.paragraph));
            };
            plan_tracked_delete(paragraph, start.offset.0, end.offset.0, ctx)?
        };
        if plan
            .iter()
            .any(|step| matches!(step, DeleteStep::Mark(_, revision) if revision.id == 0))
        {
            let id = document
                .allocate_revision_id()
                .ok_or(CommandError::RevisionConflict)?;
            for step in &mut plan {
                if let DeleteStep::Mark(_, revision) = step
                    && revision.id == 0
                {
                    revision.id = id;
                    revision.date.clone_from(&ctx.date);
                }
            }
        }
        if plan.is_empty() {
            return Ok(());
        }
        let Some(paragraph) = locate::paragraph_mut(document, &first_loc) else {
            return Err(CommandError::UnknownNode(start.paragraph));
        };
        apply_delete_plan(paragraph, &plan, map, changes, start)?;
        return Ok(());
    }

    // Cross-paragraph tracked deletion: every covered run is marked deleted
    // and the paragraph marks from the first paragraph up to (excluding) the
    // last one are recorded as deleted revisions (`w:pPr/w:rPr/w:del`), so
    // the final view joins the surrounding text exactly like Word. The
    // paragraphs stay in place until the revision is accepted. One revision
    // id covers the whole command: an adjacent same-author deletion is
    // reused, matching the single-paragraph path.
    let mut plans = Vec::new();
    {
        let seq = locate::container(document, start.story, &first_loc.containers)
            .ok_or(CommandError::UnknownNode(start.paragraph))?;
        for index in first_loc.index..=last_loc.index {
            let Some(Block::Paragraph(paragraph)) = seq.get(index) else {
                return Err(CommandError::RevisionBoundary);
            };
            let len = paragraph.len();
            let from = if index == first_loc.index {
                start.offset.0
            } else {
                0
            };
            let to = if index == last_loc.index {
                end.offset.0
            } else {
                len
            };
            if from > to || to > len {
                return Err(CommandError::ReversedRange);
            }
            plans.push((
                index,
                paragraph.id(),
                plan_tracked_delete(paragraph, from, to, ctx)?,
            ));
        }
    }
    let mut mark = TextRevision {
        id: 0,
        kind: RevisionKind::Delete,
        author: Arc::clone(&ctx.author),
        date: ctx.date.clone(),
    };
    for (_, _, plan) in &plans {
        for step in plan {
            if let DeleteStep::Mark(_, revision) = step
                && revision.id != 0
            {
                mark = revision.clone();
            }
        }
    }
    for index in first_loc.index..last_loc.index {
        let Some(paragraph) = locate::container(document, start.story, &first_loc.containers)
            .and_then(|seq| seq.get(index))
            .and_then(Block::as_paragraph)
        else {
            return Err(CommandError::RevisionBoundary);
        };
        match &paragraph.mark_revision {
            Some(existing)
                if existing.kind == RevisionKind::Delete && existing.author == ctx.author =>
            {
                mark = existing.clone();
            }
            Some(_) => return Err(CommandError::RevisionConflict),
            None => {}
        }
    }
    if mark.id == 0 {
        mark.id = document
            .allocate_revision_id()
            .ok_or(CommandError::RevisionConflict)?;
    }
    for (_, _, plan) in &mut plans {
        for step in plan {
            if let DeleteStep::Mark(_, revision) = step {
                *revision = mark.clone();
            }
        }
    }
    let story = start.story;
    for (index, id, plan) in plans {
        let at = Position {
            story,
            paragraph: id,
            offset: ScalarOffset(0),
            bias: start.bias,
        };
        if !plan.is_empty() {
            let mut location = first_loc.clone();
            location.index = index;
            let Some(paragraph) = locate::paragraph_mut(document, &location) else {
                return Err(CommandError::UnknownNode(id));
            };
            apply_delete_plan(paragraph, &plan, map, changes, &at)?;
        }
        if index < last_loc.index {
            let mut location = first_loc.clone();
            location.index = index;
            let Some(paragraph) = locate::paragraph_mut(document, &location) else {
                return Err(CommandError::UnknownNode(id));
            };
            paragraph.mark_revision = Some(mark.clone());
            changes.stories.insert(story);
            changes.paragraphs.insert(id);
        }
    }
    Ok(())
}

pub(crate) fn accept_one(
    document: &mut Document,
    id: u32,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    resolve_one(document, id, Resolution::Accept, map, changes)
}

pub(crate) fn reject_one(
    document: &mut Document,
    id: u32,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    resolve_one(document, id, Resolution::Reject, map, changes)
}

pub(crate) fn accept_all(
    document: &mut Document,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    resolve_all(document, Resolution::Accept, map, changes)
}

pub(crate) fn reject_all(
    document: &mut Document,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    resolve_all(document, Resolution::Reject, map, changes)
}

pub(crate) fn remap_markers(document: &mut Document, map: &PositionMap, changes: &mut ChangeSet) {
    if map.is_identity() {
        return;
    }
    let mut next = Vec::new();
    let mut changed = false;
    for marker in document.move_markers() {
        match map.map(marker.position) {
            crate::map::Mapped::Lost => changed = true,
            crate::map::Mapped::Kept(position) | crate::map::Mapped::Collapsed(position) => {
                if position != marker.position {
                    changed = true;
                }
                let mut marker = marker.clone();
                marker.position = position;
                next.push(marker);
            }
        }
    }
    if changed {
        *document.move_markers_mut() = next;
        changes.settings_changed = true;
    }
}

fn adjacent_insert(paragraph: &Paragraph, at: usize, author: &str) -> Option<TextRevision> {
    let matches =
        |rev: &TextRevision| rev.kind == RevisionKind::Insert && rev.author.as_ref() == author;
    if at > 0
        && let Some(rev) = paragraph.revision_covering(at - 1)
        && matches(rev)
    {
        return Some(rev.clone());
    }
    if at < paragraph.len()
        && let Some(rev) = paragraph.revision_covering(at)
        && matches(rev)
    {
        return Some(rev.clone());
    }
    None
}

#[derive(Clone)]
enum DeleteStep {
    Remove(std::ops::Range<usize>),
    Mark(std::ops::Range<usize>, TextRevision),
}

fn plan_tracked_delete(
    paragraph: &Paragraph,
    start: usize,
    end: usize,
    ctx: &EditContext,
) -> Result<Vec<DeleteStep>, CommandError> {
    let mut steps = Vec::new();
    let mut reuse = None;
    let mut offset = 0usize;
    for run in paragraph.runs() {
        let run_end = offset + run.len;
        let from = offset.max(start);
        let to = run_end.min(end);
        if from < to {
            match &run.revision {
                Some(rev) if rev.kind == RevisionKind::Insert && rev.author == ctx.author => {
                    steps.push(DeleteStep::Remove(from..to));
                }
                Some(rev) if rev.kind == RevisionKind::MoveTo && rev.author == ctx.author => {
                    steps.push(DeleteStep::Remove(from..to));
                }
                Some(rev) if matches!(rev.kind, RevisionKind::Delete | RevisionKind::MoveFrom) => {
                    if rev.kind == RevisionKind::Delete && rev.author == ctx.author {
                        reuse = Some(rev.clone());
                    }
                }
                Some(_) => return Err(CommandError::RevisionConflict),
                None => steps.push(DeleteStep::Mark(
                    from..to,
                    TextRevision {
                        id: 0,
                        kind: RevisionKind::Delete,
                        author: Arc::clone(&ctx.author),
                        date: None,
                    },
                )),
            }
        }
        offset = run_end;
    }
    if start > 0
        && let Some(rev) = paragraph.revision_covering(start - 1)
        && rev.kind == RevisionKind::Delete
        && rev.author == ctx.author
    {
        reuse = Some(rev.clone());
    }
    if end < paragraph.len()
        && let Some(rev) = paragraph.revision_covering(end)
        && rev.kind == RevisionKind::Delete
        && rev.author == ctx.author
    {
        reuse = Some(rev.clone());
    }
    let mark = reuse.unwrap_or(TextRevision {
        id: 0,
        kind: RevisionKind::Delete,
        author: Arc::clone(&ctx.author),
        date: ctx.date.clone(),
    });
    for step in &mut steps {
        if let DeleteStep::Mark(_, revision) = step {
            *revision = mark.clone();
        }
    }
    Ok(steps)
}

fn apply_delete_plan(
    paragraph: &mut Paragraph,
    plan: &[DeleteStep],
    map: &mut PositionMap,
    changes: &mut ChangeSet,
    at: &Position,
) -> Result<(), CommandError> {
    let mut ordered: Vec<&DeleteStep> = plan.iter().collect();
    ordered.sort_by_key(|step| std::cmp::Reverse(step_start(step)));
    for step in ordered {
        match step {
            DeleteStep::Mark(range, revision) => {
                paragraph.set_revision(
                    joyword_model::ScalarOffset(range.start)
                        ..joyword_model::ScalarOffset(range.end),
                    Some(revision.clone()),
                )?;
            }
            DeleteStep::Remove(range) => {
                paragraph.remove(
                    joyword_model::ScalarOffset(range.start)
                        ..joyword_model::ScalarOffset(range.end),
                )?;
                map.push(MapStep::Delete {
                    paragraph: at.paragraph,
                    start: range.start,
                    end: range.end,
                });
            }
        }
    }
    changes.stories.insert(at.story);
    changes.paragraphs.insert(at.paragraph);
    Ok(())
}

fn step_start(step: &DeleteStep) -> usize {
    match step {
        DeleteStep::Remove(range) | DeleteStep::Mark(range, _) => range.start,
    }
}

fn resolve_one(
    document: &mut Document,
    id: u32,
    resolution: Resolution,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let hits = hits_for(document, id);
    let marks = mark_targets(document, id);
    if hits.is_empty() && marks.is_empty() {
        // No text or mark revision carries this id; it may be a format
        // revision (`w:rPrChange` / `w:pPrChange`) or a table property
        // revision, neither of which has a text position.
        if resolve_format_one(document, id, resolution, changes)? {
            return Ok(());
        }
        if resolve_table_one(document, id, resolution, changes)? {
            return Ok(());
        }
        return Err(CommandError::UnknownRevision(id));
    }
    let kind = hits.first().map_or(RevisionKind::Delete, |hit| hit.kind);
    // A mark revision is always a deletion; an id that also carries an
    // insertion or a move is not one change.
    if hits.iter().any(|hit| hit.kind != kind)
        || marks
            .iter()
            .any(|mark| mark.2 != RevisionKind::Delete || kind != RevisionKind::Delete)
    {
        return Err(CommandError::RevisionConflict);
    }
    let (edits, names) = if kind.is_move() {
        let name = move_name_for(document, &hits)?;
        let edits = move_edits(document, &name, resolution)?;
        (edits, vec![name])
    } else {
        (simple_edits(&hits, resolution), Vec::new())
    };
    apply_edits(document, &edits, map, changes)?;
    resolve_marks(
        document,
        &marks,
        MarkScope::One(id),
        resolution,
        map,
        changes,
    )?;
    drop_names(document, &names, changes);
    // A single id can carry both a text and a format revision; resolve both so
    // one accept or reject settles the whole change. The same holds for a
    // table property revision sharing the id.
    resolve_format_one(document, id, resolution, changes)?;
    resolve_table_one(document, id, resolution, changes)?;
    Ok(())
}

fn resolve_all(
    document: &mut Document,
    resolution: Resolution,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let pairs = classified_moves(document)?;
    let order = order_of(document);
    if runs(document).iter().any(|hit| {
        hit.kind.is_move()
            && !pairs.iter().any(|pair| {
                let (start, end) = if hit.kind == RevisionKind::MoveFrom {
                    (pair.from_start, pair.from_end)
                } else {
                    (pair.to_start, pair.to_end)
                };
                overlaps(&order, hit, start, end)
            })
    }) {
        return Err(CommandError::IncompleteMove(String::new()));
    }
    let mut edits = Vec::new();
    let mut names = Vec::new();
    for pair in &pairs {
        names.push(pair.name.clone());
        edits.extend(pair_edits(document, pair, resolution)?);
    }
    for hit in runs(document) {
        if hit.kind.is_move() {
            continue;
        }
        edits.extend(simple_edits(&[hit], resolution));
    }
    apply_edits(document, &edits, map, changes)?;
    drop_names(document, &names, changes);
    resolve_format_all(document, resolution, changes)?;
    resolve_table_all(document, resolution, changes)?;
    // Paragraph-mark deletions resolve last so merges run over paragraphs
    // whose accepted text deletions have already been applied.
    let marks = all_mark_targets(document);
    resolve_marks(document, &marks, MarkScope::All, resolution, map, changes)?;
    Ok(())
}

/// Which paragraph-mark revisions a resolution pass covers.
#[derive(Clone, Copy)]
enum MarkScope {
    One(u32),
    All,
}

/// Paragraphs whose pending mark revision carries `id`, with its kind.
fn mark_targets(document: &Document, id: u32) -> Vec<(StoryId, NodeId, RevisionKind)> {
    let mut out = Vec::new();
    for (story, paragraph) in paragraphs(document) {
        if let Some(paragraph) = paragraph_ref(document, story, paragraph)
            && let Some(revision) = &paragraph.mark_revision
            && revision.id == id
        {
            out.push((story, paragraph.id(), revision.kind));
        }
    }
    out
}

/// Every paragraph with a pending mark revision, in document order. Both
/// kinds resolve: an accepted deleted mark and a rejected inserted mark both
/// remove the break the mark stands for.
fn all_mark_targets(document: &Document) -> Vec<(StoryId, NodeId, RevisionKind)> {
    let mut out = Vec::new();
    for (story, paragraph) in paragraphs(document) {
        if let Some(paragraph) = paragraph_ref(document, story, paragraph)
            && let Some(revision) = &paragraph.mark_revision
        {
            out.push((story, paragraph.id(), revision.kind));
        }
    }
    out
}

/// Accept merges the paragraph with the next sibling — the boundary the
/// deleted mark stood for disappears and the merged paragraph inherits the
/// absorbed mark's own pending revision, so a chain of same-change marks
/// merges in one resolution. Reject clears the mark: the break stays. A mark
/// with no paragraph sibling after it cannot be merged and refuses the whole
/// command rather than guessing.
fn resolve_marks(
    document: &mut Document,
    targets: &[(StoryId, NodeId, RevisionKind)],
    scope: MarkScope,
    resolution: Resolution,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let accept = matches!(resolution, Resolution::Accept);
    for &(story, id, _) in targets {
        let covered = |revision: &TextRevision| match scope {
            MarkScope::One(wanted) => revision.id == wanted,
            MarkScope::All => true,
        };
        let Some(location) = locate::locate(document, story, id) else {
            continue;
        };
        if location.content_locked {
            return Err(CommandError::Locked(id));
        }
        let pending = locate::paragraph(document, &location)
            .and_then(|paragraph| paragraph.mark_revision.clone());
        let Some(revision) = pending else {
            continue;
        };
        if !covered(&revision) {
            continue;
        }
        // The action pairs the mark's kind with the resolution: an accepted
        // deleted mark and a rejected inserted mark both remove the break
        // the mark stands for; the other two combinations keep the break and
        // only drop the record.
        let removes_break = (revision.kind == RevisionKind::Delete) == accept;
        if !removes_break {
            let Some(paragraph) = locate::paragraph_mut(document, &location) else {
                continue;
            };
            paragraph.mark_revision = None;
            changes.stories.insert(story);
            changes.paragraphs.insert(id);
            continue;
        }
        match revision.kind {
            RevisionKind::Delete => {
                resolve_deleted_marks(document, story, id, &covered, map, changes)?;
            }
            RevisionKind::Insert => {
                resolve_inserted_marks(document, story, id, &covered, map, changes)?;
            }
            RevisionKind::MoveFrom | RevisionKind::MoveTo => {
                return Err(CommandError::RevisionConflict);
            }
        }
    }
    Ok(())
}

/// Accepting deleted marks: the head absorbs the following paragraph and
/// keeps its own identity and formatting, chaining while the absorbed mark is
/// also covered.
fn resolve_deleted_marks(
    document: &mut Document,
    story: StoryId,
    id: NodeId,
    covered: &dyn Fn(&TextRevision) -> bool,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    while let Some(location) = locate::locate(document, story, id) {
        let next_pair = locate::container(document, story, &location.containers).and_then(|seq| {
            let head = seq.get(location.index)?.as_paragraph()?;
            let tail = seq.get(location.index + 1)?.as_paragraph()?;
            Some((tail.id(), head.len()))
        });
        let Some((tail_id, head_len)) = next_pair else {
            // A deleted mark at the end of a container: the engine never
            // creates one, an import can. An empty head loses nothing by
            // keeping the container's final mark, so its record is dropped;
            // anything else would have to merge into nothing and refuses.
            let current = locate::locate(document, story, id)
                .and_then(|located| locate::paragraph(document, &located));
            let empty = current.is_some_and(Paragraph::is_empty);
            if empty {
                if let Some(located) = locate::locate(document, story, id)
                    && let Some(paragraph) = locate::paragraph_mut(document, &located)
                {
                    paragraph.mark_revision = None;
                    changes.stories.insert(story);
                    changes.paragraphs.insert(id);
                }
                break;
            }
            return Err(CommandError::RevisionConflict);
        };
        let seq = locate::container_mut(document, story, &location.containers)
            .ok_or(CommandError::UnknownNode(id))?;
        let Block::Paragraph(tail) = seq.remove(location.index + 1) else {
            return Err(CommandError::RevisionConflict);
        };
        let mut tail = Arc::unwrap_or_clone(tail);
        let inherited = tail.mark_revision.take();
        let Some(Block::Paragraph(head)) = seq.get_mut(location.index) else {
            return Err(CommandError::UnknownNode(id));
        };
        let head = Arc::make_mut(head);
        // The merged paragraph keeps the first half's formatting and
        // identity, and ends at the absorbed mark, whose own pending
        // revision survives on the merged paragraph.
        head.append(tail);
        head.mark_revision = inherited;
        map.push(MapStep::DeleteAcross {
            story,
            first: id,
            start: head_len,
            last: tail_id,
            end: 0,
            removed: Vec::new(),
        });
        changes.stories.insert(story);
        changes.paragraphs.insert(id);
        changes.removed.insert(tail_id);
        let next = locate::locate(document, story, id)
            .and_then(|location| locate::paragraph(document, &location))
            .and_then(|paragraph| paragraph.mark_revision.clone());
        if !next.as_ref().is_some_and(covered) {
            break;
        }
    }
    Ok(())
}

/// Rejecting inserted marks: the head's content joins the following paragraph
/// in front of it, which keeps its own identity and formatting — the state
/// before the break was inserted — chaining while the absorbed mark is also
/// covered.
fn resolve_inserted_marks(
    document: &mut Document,
    story: StoryId,
    mut id: NodeId,
    covered: &dyn Fn(&TextRevision) -> bool,
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    while let Some(location) = locate::locate(document, story, id) {
        let next_pair = locate::container(document, story, &location.containers).and_then(|seq| {
            let head = seq.get(location.index)?.as_paragraph()?;
            let tail = seq.get(location.index + 1)?.as_paragraph()?;
            Some((tail.id(), head.len(), tail.clone()))
        });
        let Some((_, head_len, _)) = next_pair else {
            // An inserted mark on the container's final paragraph: with the
            // text revisions already resolved the paragraph is empty, so
            // rejecting the mark removes the whole paragraph — the state
            // before it was inserted. Anything non-empty refuses.
            let located = locate::locate(document, story, id);
            let Some(location) = located else {
                break;
            };
            let empty = locate::paragraph(document, &location).is_some_and(Paragraph::is_empty);
            if !empty || location.content_locked {
                return Err(CommandError::RevisionConflict);
            }
            let fallback =
                locate::container(document, story, &location.containers).and_then(|seq| {
                    match location
                        .index
                        .checked_sub(1)
                        .and_then(|index| seq.get(index))
                    {
                        Some(Block::Paragraph(previous)) => {
                            Some((story, previous.id(), previous.len()))
                        }
                        _ => None,
                    }
                });
            let seq = locate::container_mut(document, story, &location.containers)
                .ok_or(CommandError::UnknownNode(id))?;
            seq.remove(location.index);
            map.push(MapStep::Remove {
                paragraph: id,
                fallback,
            });
            changes.stories.insert(story);
            changes.removed.insert(id);
            break;
        };
        let seq = locate::container_mut(document, story, &location.containers)
            .ok_or(CommandError::UnknownNode(id))?;
        let Block::Paragraph(head) = seq.remove(location.index) else {
            return Err(CommandError::RevisionConflict);
        };
        let head = Arc::unwrap_or_clone(head);
        // The content joins in front of the tail, which keeps its own
        // formatting, identity and pending mark; the head's mark is the
        // rejected break and disappears with it.
        let tail_id = {
            let Some(Block::Paragraph(tail)) = seq.get_mut(location.index) else {
                return Err(CommandError::RevisionConflict);
            };
            let tail_id = tail.id();
            Arc::make_mut(tail).prepend(head);
            tail_id
        };
        map.push(MapStep::MergeBefore {
            story,
            head: id,
            head_len,
            tail: tail_id,
        });
        changes.stories.insert(story);
        changes.paragraphs.insert(tail_id);
        changes.removed.insert(id);
        let next = locate::locate(document, story, tail_id)
            .and_then(|located| locate::paragraph(document, &located))
            .and_then(|paragraph| paragraph.mark_revision.clone());
        if !next.as_ref().is_some_and(covered) {
            break;
        }
        // The chain continues from the tail, which now carries the mark.
        id = tail_id;
    }
    Ok(())
}

/// Resolves every format revision (`w:rPrChange` / `w:pPrChange`) whose id
/// matches. Returns whether any was found. Locked paragraphs are refused.
fn resolve_format_one(
    document: &mut Document,
    id: u32,
    resolution: Resolution,
    changes: &mut ChangeSet,
) -> Result<bool, CommandError> {
    let accept = matches!(resolution, Resolution::Accept);
    let targets: Vec<(StoryId, NodeId)> = paragraphs(document)
        .into_iter()
        .filter(|(story, paragraph)| {
            paragraph_ref(document, *story, *paragraph)
                .is_some_and(|paragraph| has_format_revision_id(paragraph, id))
        })
        .collect();
    let mut found = false;
    for (story, paragraph) in targets {
        let location = locate::locate(document, story, paragraph)
            .ok_or(CommandError::UnknownNode(paragraph))?;
        if location.content_locked {
            return Err(CommandError::Locked(paragraph));
        }
        let Some(paragraph_mut) = locate::paragraph_mut(document, &location) else {
            return Err(CommandError::UnknownNode(paragraph));
        };
        if paragraph_mut.resolve_format_revision(id, accept) {
            found = true;
            changes.stories.insert(story);
            changes.paragraphs.insert(paragraph);
        }
    }
    Ok(found)
}

/// Resolves every format revision in the document (batch accept or reject).
fn resolve_format_all(
    document: &mut Document,
    resolution: Resolution,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let accept = matches!(resolution, Resolution::Accept);
    let targets: Vec<(StoryId, NodeId)> = paragraphs(document)
        .into_iter()
        .filter(|(story, paragraph)| {
            paragraph_ref(document, *story, *paragraph).is_some_and(has_any_format_revision)
        })
        .collect();
    for (story, paragraph) in targets {
        let location = locate::locate(document, story, paragraph)
            .ok_or(CommandError::UnknownNode(paragraph))?;
        if location.content_locked {
            return Err(CommandError::Locked(paragraph));
        }
        let Some(paragraph_mut) = locate::paragraph_mut(document, &location) else {
            return Err(CommandError::UnknownNode(paragraph));
        };
        if paragraph_mut.resolve_all_format(accept) {
            changes.stories.insert(story);
            changes.paragraphs.insert(paragraph);
        }
    }
    Ok(())
}

fn has_format_revision_id(paragraph: &Paragraph, id: u32) -> bool {
    paragraph
        .runs()
        .iter()
        .any(|run| run.format_revision.as_ref().is_some_and(|rev| rev.id == id))
        || paragraph
            .props_revision
            .as_ref()
            .is_some_and(|rev| rev.id == id)
}

fn has_any_format_revision(paragraph: &Paragraph) -> bool {
    paragraph.has_run_format_revision() || paragraph.props_revision.is_some()
}

/// Resolves every table property revision (`w:tblPrChange` / `w:trPrChange`
/// / `w:tcPrChange`) whose id matches. Returns whether any was found. A
/// table inside a locked content control is refused.
fn resolve_table_one(
    document: &mut Document,
    id: u32,
    resolution: Resolution,
    changes: &mut ChangeSet,
) -> Result<bool, CommandError> {
    resolve_tables(document, Some(id), resolution, changes)
}

/// Resolves every table property revision in the document (batch accept or
/// reject).
fn resolve_table_all(
    document: &mut Document,
    resolution: Resolution,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    resolve_tables(document, None, resolution, changes).map(|_| ())
}

fn resolve_tables(
    document: &mut Document,
    id: Option<u32>,
    resolution: Resolution,
    changes: &mut ChangeSet,
) -> Result<bool, CommandError> {
    let accept = matches!(resolution, Resolution::Accept);
    let targets: Vec<(StoryId, NodeId)> = tables(document)
        .into_iter()
        .filter(|(story, table)| {
            table_ref(document, *story, *table).is_some_and(|table| table.has_props_revision(id))
        })
        .collect();
    let mut found = false;
    for (story, table) in targets {
        let location =
            locate::locate(document, story, table).ok_or(CommandError::UnknownNode(table))?;
        if location.content_locked {
            return Err(CommandError::Locked(table));
        }
        let Some(Block::Table(table_mut)) =
            locate::container_mut(document, story, &location.containers)
                .and_then(|seq| seq.get_mut(location.index))
        else {
            return Err(CommandError::UnknownNode(table));
        };
        if Arc::make_mut(table_mut).resolve_props_revisions(id, accept) {
            found = true;
            changes.stories.insert(story);
            changes.tables.insert(table);
        }
    }
    Ok(found)
}

/// Every table, in document order, including nested ones.
fn tables(document: &Document) -> Vec<(StoryId, NodeId)> {
    let mut out = Vec::new();
    for story in document.stories() {
        let mut ids = Vec::new();
        for block in story.blocks.iter() {
            locate::tables_in(block, &mut ids);
        }
        for id in ids {
            out.push((story.id, id));
        }
    }
    out
}

fn table_ref(document: &Document, story: StoryId, table: NodeId) -> Option<&Table> {
    let location = locate::locate(document, story, table)?;
    match locate::block(document, &location)? {
        Block::Table(table) => Some(table),
        _ => None,
    }
}

struct Hit {
    story: StoryId,
    paragraph: NodeId,
    start: usize,
    end: usize,
    kind: RevisionKind,
}

#[derive(Clone, Copy)]
struct Place {
    story: usize,
    seq: usize,
    offset: usize,
}

impl PartialEq for Place {
    fn eq(&self, other: &Self) -> bool {
        self.story == other.story && self.seq == other.seq && self.offset == other.offset
    }
}
impl Eq for Place {}
impl PartialOrd for Place {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Place {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.story, self.seq, self.offset).cmp(&(other.story, other.seq, other.offset))
    }
}

struct MovePair {
    name: String,
    from_start: Place,
    from_end: Place,
    to_start: Place,
    to_end: Place,
}

#[derive(Clone, Copy)]
enum EditKind {
    Clear,
    Delete,
}

struct Edit {
    story: StoryId,
    paragraph: NodeId,
    start: usize,
    end: usize,
    kind: EditKind,
}

fn runs(document: &Document) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (story, paragraph) in paragraphs(document) {
        let Some(paragraph) = paragraph_ref(document, story, paragraph) else {
            continue;
        };
        let mut offset = 0usize;
        for run in paragraph.runs() {
            let end = offset + run.len;
            if let Some(revision) = &run.revision {
                hits.push(Hit {
                    story,
                    paragraph: paragraph.id(),
                    start: offset,
                    end,
                    kind: revision.kind,
                });
            }
            offset = end;
        }
    }
    hits
}

fn hits_for(document: &Document, id: u32) -> Vec<Hit> {
    let mut hits = Vec::new();
    for (story, paragraph) in paragraphs(document) {
        let Some(paragraph) = paragraph_ref(document, story, paragraph) else {
            continue;
        };
        let mut offset = 0usize;
        for run in paragraph.runs() {
            let end = offset + run.len;
            if let Some(revision) = run.revision.as_ref().filter(|revision| revision.id == id) {
                hits.push(Hit {
                    story,
                    paragraph: paragraph.id(),
                    start: offset,
                    end,
                    kind: revision.kind,
                });
            }
            offset = end;
        }
    }
    hits
}

fn paragraphs(document: &Document) -> Vec<(StoryId, NodeId)> {
    let mut out = Vec::new();
    for story in document.stories() {
        let mut ids = Vec::new();
        for block in story.blocks.iter() {
            paragraphs_in(block, &mut ids);
        }
        for id in ids {
            out.push((story.id, id));
        }
    }
    out
}

fn paragraph_ref(document: &Document, story: StoryId, paragraph: NodeId) -> Option<&Paragraph> {
    let location = locate::locate(document, story, paragraph)?;
    locate::paragraph(document, &location)
}

fn order_of(document: &Document) -> BTreeMap<NodeId, (usize, usize, StoryId, usize)> {
    let mut order = BTreeMap::new();
    let mut seq = 0usize;
    for (story_index, story) in document.stories().enumerate() {
        let mut ids = Vec::new();
        for block in story.blocks.iter() {
            paragraphs_in(block, &mut ids);
        }
        for id in ids {
            let len = paragraph_ref(document, story.id, id)
                .map(|p| p.len())
                .unwrap_or(0);
            order.insert(id, (story_index, seq, story.id, len));
            seq += 1;
        }
    }
    order
}

fn classified_moves(document: &Document) -> Result<Vec<MovePair>, CommandError> {
    let order = order_of(document);
    let mut by_name: BTreeMap<String, Vec<MoveMarker>> = BTreeMap::new();
    for marker in document.move_markers() {
        by_name
            .entry(marker.name.to_string())
            .or_default()
            .push(marker.clone());
    }
    let mut pairs = Vec::new();
    for (name, markers) in by_name {
        pairs.push(classify(&name, &markers, &order)?);
    }
    Ok(pairs)
}

fn classify(
    name: &str,
    markers: &[MoveMarker],
    order: &BTreeMap<NodeId, (usize, usize, StoryId, usize)>,
) -> Result<MovePair, CommandError> {
    let mut slots: [Option<&MoveMarker>; 4] = [None, None, None, None];
    for marker in markers {
        let index = match (marker.side, marker.at_end) {
            (MoveSide::From, false) => 0,
            (MoveSide::From, true) => 1,
            (MoveSide::To, false) => 2,
            (MoveSide::To, true) => 3,
        };
        if slots[index].is_some() {
            return Err(CommandError::IncompleteMove(name.to_owned()));
        }
        slots[index] = Some(marker);
    }
    let Some(
        [
            Some(from_start),
            Some(from_end),
            Some(to_start),
            Some(to_end),
        ],
    ) = Some(slots)
    else {
        return Err(CommandError::IncompleteMove(name.to_owned()));
    };
    let from_start = marker_place(from_start, order)
        .ok_or_else(|| CommandError::IncompleteMove(name.to_owned()))?;
    let from_end = marker_place(from_end, order)
        .ok_or_else(|| CommandError::IncompleteMove(name.to_owned()))?;
    let to_start = marker_place(to_start, order)
        .ok_or_else(|| CommandError::IncompleteMove(name.to_owned()))?;
    let to_end =
        marker_place(to_end, order).ok_or_else(|| CommandError::IncompleteMove(name.to_owned()))?;
    if from_start > from_end || to_start > to_end || (from_start < to_end && to_start < from_end) {
        return Err(CommandError::CrossedMove(name.to_owned()));
    }
    Ok(MovePair {
        name: name.to_owned(),
        from_start,
        from_end,
        to_start,
        to_end,
    })
}

fn marker_place(
    marker: &MoveMarker,
    order: &BTreeMap<NodeId, (usize, usize, StoryId, usize)>,
) -> Option<Place> {
    let (story, seq, _, _) = order.get(&marker.position.paragraph).copied()?;
    Some(Place {
        story,
        seq,
        offset: marker.position.offset.0,
    })
}

fn move_name_for(document: &Document, hits: &[Hit]) -> Result<String, CommandError> {
    let order = order_of(document);
    let groups = marker_groups(document);
    let mut found = None;
    for hit in hits {
        let mut matched = None;
        let mut problem = None;
        for (name, markers) in &groups {
            match classify(name, markers, &order) {
                Ok(pair) => {
                    let (start, end) = match hit.kind {
                        RevisionKind::MoveFrom => (pair.from_start, pair.from_end),
                        RevisionKind::MoveTo => (pair.to_start, pair.to_end),
                        _ => return Err(CommandError::RevisionConflict),
                    };
                    if overlaps(&order, hit, start, end) {
                        matched = Some(name.clone());
                    }
                }
                Err(error)
                    if markers
                        .iter()
                        .any(|marker| marker.position.paragraph == hit.paragraph) =>
                {
                    problem = Some(error);
                }
                Err(_) => {}
            }
        }
        let Some(name) = matched else {
            return Err(problem.unwrap_or_else(|| CommandError::IncompleteMove(String::new())));
        };
        if let Some(previous) = &found
            && previous != &name
        {
            return Err(CommandError::RevisionConflict);
        }
        found = Some(name);
    }
    found.ok_or_else(|| CommandError::IncompleteMove(String::new()))
}

fn overlaps(
    order: &BTreeMap<NodeId, (usize, usize, StoryId, usize)>,
    hit: &Hit,
    start: Place,
    end: Place,
) -> bool {
    let Some((story, seq, _, _)) = order.get(&hit.paragraph).copied() else {
        return false;
    };
    let hit_start = Place {
        story,
        seq,
        offset: hit.start,
    };
    let hit_end = Place {
        story,
        seq,
        offset: hit.end,
    };
    hit_start < end && start < hit_end
}

fn move_edits(
    document: &Document,
    name: &str,
    resolution: Resolution,
) -> Result<Vec<Edit>, CommandError> {
    let order = order_of(document);
    let markers = marker_groups(document)
        .remove(name)
        .ok_or_else(|| CommandError::IncompleteMove(name.to_owned()))?;
    let pair = classify(name, &markers, &order)?;
    pair_edits(document, &pair, resolution)
}

fn pair_edits(
    document: &Document,
    pair: &MovePair,
    resolution: Resolution,
) -> Result<Vec<Edit>, CommandError> {
    let order = order_of(document);
    let mut edits = Vec::new();
    let (from_edit, to_edit) = match resolution {
        Resolution::Accept => (EditKind::Delete, EditKind::Clear),
        Resolution::Reject => (EditKind::Clear, EditKind::Delete),
    };
    side_edits(
        document,
        &order,
        pair.from_start,
        pair.from_end,
        RevisionKind::MoveFrom,
        from_edit,
        &mut edits,
    )?;
    side_edits(
        document,
        &order,
        pair.to_start,
        pair.to_end,
        RevisionKind::MoveTo,
        to_edit,
        &mut edits,
    )?;
    Ok(edits)
}

/// Edits every run of `kind` that lies wholly inside one side of a pair.
///
/// Plain text, insertions and deletions inside the side are not edits here.
/// A single accept or reject therefore leaves them, and accept-all or
/// reject-all still sees them as their own revisions. A move run that
/// crosses the marker, or the opposite move kind inside the side, is refused.
fn side_edits(
    document: &Document,
    order: &BTreeMap<NodeId, (usize, usize, StoryId, usize)>,
    side_start: Place,
    side_end: Place,
    kind: RevisionKind,
    edit_kind: EditKind,
    edits: &mut Vec<Edit>,
) -> Result<(), CommandError> {
    for (story, paragraph) in paragraphs(document) {
        let Some((start, end)) = overlap_slice(order, paragraph, side_start, side_end) else {
            continue;
        };
        let paragraph_ref = paragraph_ref(document, story, paragraph)
            .ok_or(CommandError::UnknownNode(paragraph))?;
        let mut offset = 0usize;
        for run in paragraph_ref.runs() {
            let run_end = offset + run.len;
            if offset < end && start < run_end {
                match &run.revision {
                    Some(revision) if revision.kind == kind => {
                        if offset < start || run_end > end {
                            return Err(CommandError::RevisionConflict);
                        }
                        edits.push(Edit {
                            story,
                            paragraph,
                            start: offset,
                            end: run_end,
                            kind: edit_kind,
                        });
                    }
                    Some(revision) if revision.kind.is_move() => {
                        return Err(CommandError::RevisionConflict);
                    }
                    _ => {}
                }
            }
            offset = run_end;
        }
    }
    Ok(())
}

fn overlap_slice(
    order: &BTreeMap<NodeId, (usize, usize, StoryId, usize)>,
    paragraph: NodeId,
    start: Place,
    end: Place,
) -> Option<(usize, usize)> {
    let (story, seq, _, len) = order.get(&paragraph).copied()?;
    let para_start = Place {
        story,
        seq,
        offset: 0,
    };
    let para_end = Place {
        story,
        seq,
        offset: len,
    };
    let lo = para_start.max(start);
    let hi = para_end.min(end);
    if lo.story == story && lo.seq == seq && hi.story == story && hi.seq == seq && lo < hi {
        Some((lo.offset, hi.offset))
    } else {
        None
    }
}

fn marker_groups(document: &Document) -> BTreeMap<String, Vec<MoveMarker>> {
    let mut by_name: BTreeMap<String, Vec<MoveMarker>> = BTreeMap::new();
    for marker in document.move_markers() {
        by_name
            .entry(marker.name.to_string())
            .or_default()
            .push(marker.clone());
    }
    by_name
}

fn simple_edits(hits: &[Hit], resolution: Resolution) -> Vec<Edit> {
    hits.iter()
        .filter(|hit| hit.start < hit.end)
        .map(|hit| Edit {
            story: hit.story,
            paragraph: hit.paragraph,
            start: hit.start,
            end: hit.end,
            kind: match (hit.kind, resolution) {
                (RevisionKind::Insert | RevisionKind::MoveTo, Resolution::Accept) => {
                    EditKind::Clear
                }
                (RevisionKind::Insert | RevisionKind::MoveTo, Resolution::Reject) => {
                    EditKind::Delete
                }
                (RevisionKind::Delete | RevisionKind::MoveFrom, Resolution::Accept) => {
                    EditKind::Delete
                }
                (RevisionKind::Delete | RevisionKind::MoveFrom, Resolution::Reject) => {
                    EditKind::Clear
                }
            },
        })
        .collect()
}

fn apply_edits(
    document: &mut Document,
    edits: &[Edit],
    map: &mut PositionMap,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let mut grouped: BTreeMap<(StoryId, NodeId), Vec<&Edit>> = BTreeMap::new();
    for edit in edits {
        grouped
            .entry((edit.story, edit.paragraph))
            .or_default()
            .push(edit);
    }
    for ((story, paragraph), mut group) in grouped {
        let location = locate::locate(document, story, paragraph)
            .ok_or(CommandError::UnknownNode(paragraph))?;
        if location.content_locked {
            return Err(CommandError::Locked(paragraph));
        }
        group.sort_by_key(|edit| std::cmp::Reverse(edit.start));
        let Some(paragraph_mut) = locate::paragraph_mut(document, &location) else {
            return Err(CommandError::UnknownNode(paragraph));
        };
        for edit in group {
            match edit.kind {
                EditKind::Clear => paragraph_mut.set_revision(
                    joyword_model::ScalarOffset(edit.start)..joyword_model::ScalarOffset(edit.end),
                    None,
                )?,
                EditKind::Delete => {
                    paragraph_mut.remove(
                        joyword_model::ScalarOffset(edit.start)
                            ..joyword_model::ScalarOffset(edit.end),
                    )?;
                    map.push(MapStep::Delete {
                        paragraph,
                        start: edit.start,
                        end: edit.end,
                    });
                }
            }
        }
        changes.stories.insert(story);
        changes.paragraphs.insert(paragraph);
    }
    Ok(())
}

fn drop_names(document: &mut Document, names: &[String], changes: &mut ChangeSet) {
    if names.is_empty() {
        return;
    }
    let before = document.move_markers().len();
    document
        .move_markers_mut()
        .retain(|marker| !names.iter().any(|name| marker.name.as_ref() == name));
    if document.move_markers().len() != before {
        changes.settings_changed = true;
    }
}
