//! Grapheme search and replacement plans (EDIT-006).
//!
//! Matching walks extended grapheme clusters, so a query never lands inside a
//! cluster. Inline objects are barriers: a match cannot contain one. Locked
//! content controls and paragraphs that hold preserved markup are counted and
//! skipped, never edited. There is no regular-expression mode: the workspace
//! has no linear regex engine, and an unbounded one is out of this slice.

use std::collections::BTreeMap;

use joyword_model::{
    Bias, Block, BlockSeq, Document, InlineKind, MAX_NESTING, NodeId, Paragraph, Position,
    ScalarOffset, StoryId,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::command::{Command, CommandError, check_text};

/// Longest query, in Unicode scalars.
pub const MAX_QUERY_SCALARS: usize = 1024;
/// Longest replacement, in Unicode scalars.
pub const MAX_REPLACEMENT_SCALARS: usize = 4096;
/// Hits returned from one search. Further hits set [`FindReport::truncated`].
pub const MAX_HITS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindQuery {
    pub text: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindHit {
    pub story: StoryId,
    pub paragraph: NodeId,
    /// Paragraph sequence in search order. Body is first.
    pub order: u32,
    pub start: ScalarOffset,
    pub end: ScalarOffset,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindReport {
    pub hits: Vec<FindHit>,
    pub truncated: bool,
    pub skipped: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplacePlan {
    pub commands: Vec<Command>,
    pub replaced: usize,
    pub skipped: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindError {
    Empty,
    TooLong,
    InvalidReplacement(char),
}

impl std::fmt::Display for FindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindError::Empty => f.write_str("empty query"),
            FindError::TooLong => f.write_str("query or replacement is too long"),
            FindError::InvalidReplacement(ch) => {
                write!(
                    f,
                    "U+{ch:04X} is not ordinary replacement text",
                    ch = *ch as u32
                )
            }
        }
    }
}

struct Unit {
    text: String,
    start: usize,
    end: usize,
    barrier: bool,
}

struct Scan<'a> {
    hits: Vec<FindHit>,
    skipped: u32,
    truncated: bool,
    order: u32,
    needle: &'a [String],
    case_sensitive: bool,
    whole_word: bool,
}

/// Search every story. The body is visited first; other stories follow the
/// document's story map. Opaque blocks are not text and are not searched.
pub fn search(document: &Document, query: &FindQuery) -> Result<FindReport, FindError> {
    let scalars = query.text.chars().count();
    if scalars == 0 {
        return Err(FindError::Empty);
    }
    if scalars > MAX_QUERY_SCALARS {
        return Err(FindError::TooLong);
    }
    let needle: Vec<String> = query
        .text
        .graphemes(true)
        .map(|cluster| fold(cluster, query.case_sensitive))
        .collect();
    let mut scan = Scan {
        hits: Vec::new(),
        skipped: 0,
        truncated: false,
        order: 0,
        needle: &needle,
        case_sensitive: query.case_sensitive,
        whole_word: query.whole_word,
    };
    if let Some(body) = document.story(document.body_id()) {
        visit_blocks(&body.blocks, body.id, false, 0, &mut scan);
    }
    for story in document.stories() {
        if story.id == document.body_id() || scan.truncated {
            continue;
        }
        visit_blocks(&story.blocks, story.id, false, 0, &mut scan);
    }
    Ok(FindReport {
        hits: scan.hits,
        truncated: scan.truncated,
        skipped: scan.skipped,
    })
}

/// Paragraph sequence used by [`FindHit::order`], including paragraphs that
/// produced no hit. `None` when the paragraph is not in the searched tree.
pub fn paragraph_order(document: &Document, story: StoryId, paragraph: NodeId) -> Option<u32> {
    let mut order = 0u32;
    let mut found = None;
    if let Some(body) = document.story(document.body_id()) {
        visit_order(
            &body.blocks,
            body.id,
            0,
            story,
            paragraph,
            &mut order,
            &mut found,
        );
    }
    if found.is_none() {
        for item in document.stories() {
            if item.id == document.body_id() {
                continue;
            }
            visit_order(
                &item.blocks,
                item.id,
                0,
                story,
                paragraph,
                &mut order,
                &mut found,
            );
        }
    }
    found
}

/// One `DeleteRange` and, when the replacement is non-empty, one `InsertText`
/// per accepted hit. Later hits are emitted first so earlier offsets stay
/// valid inside one transaction. Overlaps, locks, preserved markup and ranges
/// that contain an inline object are skipped.
pub fn replace_commands(
    document: &Document,
    hits: &[FindHit],
    replacement: &str,
) -> Result<ReplacePlan, FindError> {
    if replacement.chars().count() > MAX_REPLACEMENT_SCALARS {
        return Err(FindError::TooLong);
    }
    if let Err(CommandError::InvalidText(ch)) = check_text(replacement) {
        return Err(FindError::InvalidReplacement(ch));
    }
    let mut ordered: Vec<&FindHit> = hits.iter().collect();
    ordered.sort_by(|left, right| {
        right
            .order
            .cmp(&left.order)
            .then(right.start.cmp(&left.start))
    });
    let mut commands = Vec::new();
    let mut replaced = 0usize;
    let mut skipped = 0usize;
    let mut next_free: BTreeMap<NodeId, usize> = BTreeMap::new();
    for hit in ordered {
        let Some((locked, paragraph)) = find_paragraph(document, hit.story, hit.paragraph) else {
            skipped += 1;
            continue;
        };
        let start = hit.start.0;
        let end = hit.end.0;
        let overlaps = next_free
            .get(&hit.paragraph)
            .is_some_and(|limit| end > *limit);
        if locked || blocked(paragraph) || overlaps || !clean_range(paragraph, start, end) {
            skipped += 1;
            continue;
        }
        next_free.insert(hit.paragraph, start);
        let at = Position {
            story: hit.story,
            paragraph: hit.paragraph,
            offset: hit.start,
            bias: Bias::After,
        };
        let to = Position {
            story: hit.story,
            paragraph: hit.paragraph,
            offset: hit.end,
            bias: Bias::Before,
        };
        commands.push(Command::DeleteRange { start: at, end: to });
        if !replacement.is_empty() {
            commands.push(Command::InsertText {
                at,
                text: replacement.to_owned(),
            });
        }
        replaced += 1;
    }
    Ok(ReplacePlan {
        commands,
        replaced,
        skipped,
    })
}

fn visit_blocks(
    blocks: &BlockSeq,
    story: StoryId,
    locked: bool,
    depth: usize,
    scan: &mut Scan<'_>,
) {
    if depth > MAX_NESTING || scan.truncated {
        return;
    }
    for block in blocks.iter() {
        if scan.truncated {
            return;
        }
        match block {
            Block::Paragraph(paragraph) => scan.consider(story, paragraph, locked),
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        visit_blocks(&cell.blocks, story, locked, depth + 1, scan);
                    }
                }
            }
            Block::Sdt(sdt) => {
                let locked = locked || !sdt.props.lock.content_editable();
                visit_blocks(&sdt.blocks, story, locked, depth + 1, scan);
            }
            Block::Opaque(_) => {}
        }
    }
}

fn visit_order(
    blocks: &BlockSeq,
    story: StoryId,
    depth: usize,
    want_story: StoryId,
    want: NodeId,
    order: &mut u32,
    found: &mut Option<u32>,
) {
    if depth > MAX_NESTING || found.is_some() {
        return;
    }
    for block in blocks.iter() {
        if found.is_some() {
            return;
        }
        match block {
            Block::Paragraph(paragraph) => {
                if story == want_story && paragraph.id() == want {
                    *found = Some(*order);
                }
                *order = order.saturating_add(1);
            }
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        visit_order(
                            &cell.blocks,
                            story,
                            depth + 1,
                            want_story,
                            want,
                            order,
                            found,
                        );
                    }
                }
            }
            Block::Sdt(sdt) => {
                visit_order(
                    &sdt.blocks,
                    story,
                    depth + 1,
                    want_story,
                    want,
                    order,
                    found,
                );
            }
            Block::Opaque(_) => {}
        }
    }
}

impl Scan<'_> {
    fn consider(&mut self, story: StoryId, paragraph: &Paragraph, locked: bool) {
        let order = self.order;
        self.order = self.order.saturating_add(1);
        let units = units_of(paragraph);
        let ranges = match_ranges(&units, self.needle, self.case_sensitive, self.whole_word);
        if locked || blocked(paragraph) {
            self.skipped = self.skipped.saturating_add(ranges.len() as u32);
            return;
        }
        for (start, end) in ranges {
            if self.hits.len() >= MAX_HITS {
                self.truncated = true;
                return;
            }
            self.hits.push(FindHit {
                story,
                paragraph: paragraph.id(),
                order,
                start: ScalarOffset(start),
                end: ScalarOffset(end),
            });
        }
    }
}

fn blocked(paragraph: &Paragraph) -> bool {
    paragraph.props.preserved.is_some()
        || paragraph.mark_props.preserved.is_some()
        || paragraph
            .runs()
            .iter()
            .any(|run| run.props.preserved.is_some())
        || paragraph
            .inlines()
            .iter()
            .any(|inline| matches!(inline.kind, InlineKind::Preserved(_)))
}

fn units_of(paragraph: &Paragraph) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut offset = 0usize;
    for segment in paragraph.segments() {
        match segment {
            joyword_model::Segment::Text(text) => {
                for cluster in text.graphemes(true) {
                    let len = cluster.chars().count();
                    units.push(Unit {
                        text: cluster.to_owned(),
                        start: offset,
                        end: offset + len,
                        barrier: false,
                    });
                    offset += len;
                }
            }
            joyword_model::Segment::Inline(_) => {
                units.push(Unit {
                    text: String::new(),
                    start: offset,
                    end: offset + 1,
                    barrier: true,
                });
                offset += 1;
            }
        }
    }
    units
}

fn match_ranges(
    units: &[Unit],
    needle: &[String],
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    let width = needle.len();
    if width == 0 {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut index = 0usize;
    while index < units.len() {
        if units[index].barrier {
            index += 1;
            continue;
        }
        let mut matched = true;
        for (offset, wanted) in needle.iter().enumerate() {
            let Some(unit) = units.get(index + offset) else {
                matched = false;
                break;
            };
            if unit.barrier || &fold(&unit.text, case_sensitive) != wanted {
                matched = false;
                break;
            }
        }
        if matched
            && (!whole_word
                || (left_boundary(units, index) && right_boundary(units, index + width)))
        {
            let start = units[index].start;
            let end = units[index + width - 1].end;
            found.push((start, end));
            index += width;
        } else {
            index += 1;
        }
    }
    found
}

fn left_boundary(units: &[Unit], index: usize) -> bool {
    index == 0 || word_edge(&units[index - 1], &units[index])
}

fn right_boundary(units: &[Unit], end: usize) -> bool {
    end >= units.len() || word_edge(&units[end - 1], &units[end])
}

fn word_edge(left: &Unit, right: &Unit) -> bool {
    left.barrier || right.barrier || !(joins(&left.text) && joins(&right.text))
}

/// A cluster continues a word when its first scalar is ASCII alphanumeric, or
/// alphabetic and outside the CJK ranges below. Han, Hiragana, Katakana and
/// Hangul do not join their neighbours, so a whole-word query of "中" matches
/// inside "中国", while "cat" does not match inside "category".
fn joins(cluster: &str) -> bool {
    let Some(ch) = cluster.chars().next() else {
        return false;
    };
    if ch.is_ascii_alphanumeric() {
        return true;
    }
    ch.is_alphabetic() && !is_cjk(ch)
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{1100}'..='\u{11FF}'
            | '\u{2E80}'..='\u{2FDF}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{3100}'..='\u{312F}'
            | '\u{3130}'..='\u{318F}'
            | '\u{31A0}'..='\u{31BF}'
            | '\u{31C0}'..='\u{31EF}'
            | '\u{31F0}'..='\u{31FF}'
            | '\u{3200}'..='\u{9FFF}'
            | '\u{A960}'..='\u{A97F}'
            | '\u{AC00}'..='\u{D7FF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF66}'..='\u{FF9D}'
    ) || ('\u{20000}'..='\u{3FFFF}').contains(&ch)
}

fn fold(cluster: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        cluster.to_owned()
    } else {
        cluster.chars().flat_map(char::to_lowercase).collect()
    }
}

fn clean_range(paragraph: &Paragraph, start: usize, end: usize) -> bool {
    if start >= end || end > paragraph.len() {
        return false;
    }
    !units_of(paragraph)
        .iter()
        .any(|unit| unit.barrier && unit.start < end && start < unit.end)
}

fn find_paragraph(
    document: &Document,
    story: StoryId,
    paragraph: NodeId,
) -> Option<(bool, &Paragraph)> {
    let blocks = &document.story(story)?.blocks;
    find_in(blocks, paragraph, false, 0)
}

fn find_in(
    blocks: &BlockSeq,
    want: NodeId,
    locked: bool,
    depth: usize,
) -> Option<(bool, &Paragraph)> {
    if depth > MAX_NESTING {
        return None;
    }
    for block in blocks.iter() {
        match block {
            Block::Paragraph(paragraph) if paragraph.id() == want => {
                return Some((locked, paragraph));
            }
            Block::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        if let Some(found) = find_in(&cell.blocks, want, locked, depth + 1) {
                            return Some(found);
                        }
                    }
                }
            }
            Block::Sdt(sdt) => {
                let locked = locked || !sdt.props.lock.content_editable();
                if let Some(found) = find_in(&sdt.blocks, want, locked, depth + 1) {
                    return Some(found);
                }
            }
            Block::Paragraph(_) | Block::Opaque(_) => {}
        }
    }
    None
}
