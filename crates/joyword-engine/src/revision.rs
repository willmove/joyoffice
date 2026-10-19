//! Accept, reject and tracked typing for text and move revisions (T300).
//!
//! Accept and reject do not create revisions. A move is resolved by its
//! `w:name` on both sides together. An incomplete or crossed pair is refused
//! with the document unchanged.

use std::collections::BTreeMap;
use std::sync::Arc;

use joyword_model::{
    Document, MoveMarker, MoveSide, NodeId, Paragraph, Position, RevisionKind, StoryId,
    TextRevision,
};

use crate::command::CommandError;
use crate::locate::{self, paragraphs_in};
use crate::map::{MapStep, PositionMap, Selection};
use crate::session::{ChangeSet, Origin};

pub(crate) struct EditContext {
    pub origin: Origin,
    pub author: Arc<str>,
    pub date: Option<Arc<str>>,
}

#[derive(Clone, Copy)]
enum Resolution {
    Accept,
    Reject,
}

/// Text revision at the caret, or overlapping the selection.
///
/// A caret on the boundary between two runs prefers the run that ends there,
/// so the change just typed is the current one. `None` means the selection
/// does not touch a text revision.
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
    revision_at(document, &selection.anchor)
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
    if start.paragraph != end.paragraph || start.story != end.story {
        return Err(CommandError::RevisionBoundary);
    }
    if start.offset > end.offset {
        return Err(CommandError::ReversedRange);
    }
    if start.offset == end.offset {
        return Ok(());
    }
    let location = locate::locate(document, start.story, start.paragraph)
        .ok_or(CommandError::UnknownNode(start.paragraph))?;
    if location.content_locked {
        return Err(CommandError::Locked(start.paragraph));
    }
    let mut plan = {
        let Some(paragraph) = locate::paragraph(document, &location) else {
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
    let Some(paragraph) = locate::paragraph_mut(document, &location) else {
        return Err(CommandError::UnknownNode(start.paragraph));
    };
    apply_delete_plan(paragraph, &plan, map, changes, start)?;
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
    if hits.is_empty() {
        return Err(CommandError::UnknownRevision(id));
    }
    let kind = hits[0].kind;
    if hits.iter().any(|hit| hit.kind != kind) {
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
    drop_names(document, &names, changes);
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
    Ok(())
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
    for (story, paragraph) in paragraphs(document) {
        if let Some((start, end)) = overlap_slice(&order, paragraph, pair.from_start, pair.from_end)
        {
            expect_kind(
                document,
                story,
                paragraph,
                start,
                end,
                RevisionKind::MoveFrom,
            )?;
            edits.push(Edit {
                story,
                paragraph,
                start,
                end,
                kind: match resolution {
                    Resolution::Accept => EditKind::Delete,
                    Resolution::Reject => EditKind::Clear,
                },
            });
        }
        if let Some((start, end)) = overlap_slice(&order, paragraph, pair.to_start, pair.to_end) {
            expect_kind(document, story, paragraph, start, end, RevisionKind::MoveTo)?;
            edits.push(Edit {
                story,
                paragraph,
                start,
                end,
                kind: match resolution {
                    Resolution::Accept => EditKind::Clear,
                    Resolution::Reject => EditKind::Delete,
                },
            });
        }
    }
    Ok(edits)
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

fn expect_kind(
    document: &Document,
    story: StoryId,
    paragraph: NodeId,
    start: usize,
    end: usize,
    kind: RevisionKind,
) -> Result<(), CommandError> {
    let paragraph =
        paragraph_ref(document, story, paragraph).ok_or(CommandError::UnknownNode(paragraph))?;
    let mut offset = 0usize;
    for run in paragraph.runs() {
        let run_end = offset + run.len;
        if offset < end && start < run_end {
            match &run.revision {
                Some(revision) if revision.kind == kind => {}
                _ => return Err(CommandError::RevisionConflict),
            }
        }
        offset = run_end;
    }
    Ok(())
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
