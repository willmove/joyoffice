//! Grapheme search and replacement plans (EDIT-006).
//!
//! Matching walks extended grapheme clusters, so a query never lands inside a
//! cluster. Inline objects are barriers: a match cannot contain one. Locked
//! content controls and paragraphs that hold preserved markup are counted and
//! skipped, never edited.
//!
//! Two documented options extend plain matching. `FindQuery::fold_width`
//! folds full-width forms to half-width before comparing: U+FF01..=U+FF5E
//! map down by 0xFEE0 and U+3000 (ideographic space) maps to U+0020, so
//! "ABC" matches "ＡＢＣ". That folding table is a documented assumption,
//! not a Word observation. `FindQuery::regex` treats the query as a pattern
//! for the locked `regex` crate, which matches in guaranteed linear time
//! (no backtracking, no ReDoS). In regex mode the haystack is the paragraph
//! text folded the same way; a match that would split a grapheme cluster,
//! cover an inline object, or be empty is skipped.
//!
//! A literal replacement is copied unchanged, so `$1` stays `$1`. A regex
//! replacement expands capture references: `$$` is one `$`, `$0` is the
//! whole match, and `$` plus a run of ASCII digits names that group. A
//! number with no such group, including `$9` on `(a+)(b+)`, is copied
//! unchanged. The inserted text is the original paragraph text, not the
//! width-folded haystack. This syntax is a documented assumption, not a
//! Word observation.

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
    /// Fold full-width ASCII variants (U+FF01..=U+FF5E) and the ideographic
    /// space (U+3000) to their half-width counterparts before comparing, so
    /// "ABC" finds "ＡＢＣ". Documented assumption, not a Word observation.
    pub fold_width: bool,
    /// Treat `text` as a `regex`-crate pattern. The engine builds the pattern
    /// with Unicode-aware, linear-time matching; invalid patterns are
    /// rejected. The replacement then expands `$n` and `$$` (see the module
    /// docs). A literal search does not.
    pub regex: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindHit {
    pub story: StoryId,
    pub paragraph: NodeId,
    /// Paragraph sequence in search order. Body is first.
    pub order: u32,
    pub start: ScalarOffset,
    pub end: ScalarOffset,
    /// `None` for a literal hit, so the replacement is copied unchanged.
    /// `Some` for a regex hit: index 0 is the whole match and index n is
    /// group n. A group that did not participate is an empty string.
    pub captures: Option<Vec<String>>,
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
    /// The regex pattern did not compile. Matching never runs a pattern that
    /// failed to build.
    InvalidPattern,
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
            FindError::InvalidPattern => f.write_str("query is not a valid pattern"),
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
    pattern: Option<regex::Regex>,
    case_sensitive: bool,
    whole_word: bool,
    fold_width: bool,
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
        .map(|cluster| fold(cluster, query.case_sensitive, query.fold_width))
        .collect();
    let pattern = if query.regex {
        let built = regex::RegexBuilder::new(&query.text)
            .case_insensitive(!query.case_sensitive)
            .unicode(true)
            .build();
        match built {
            Ok(pattern) => Some(pattern),
            Err(_) => return Err(FindError::InvalidPattern),
        }
    } else {
        None
    };
    let mut scan = Scan {
        hits: Vec::new(),
        skipped: 0,
        truncated: false,
        order: 0,
        needle: &needle,
        pattern,
        case_sensitive: query.case_sensitive,
        whole_word: query.whole_word,
        fold_width: query.fold_width,
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
///
/// Literal hits copy `replacement` unchanged. Regex hits expand `$`
/// references against [`FindHit::captures`] before the text is inserted.
/// The expanded text is still bounded by [`MAX_REPLACEMENT_SCALARS`] and
/// must be ordinary text; either failure refuses the whole plan.
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
        let text = match &hit.captures {
            Some(groups) => expand_replacement(replacement, groups)?,
            None => replacement.to_owned(),
        };
        commands.push(Command::DeleteRange { start: at, end: to });
        if !text.is_empty() {
            commands.push(Command::InsertText { at, text });
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
        if let Some(pattern) = &self.pattern {
            let found = match_ranges_regex(&units, pattern, self.whole_word, self.fold_width);
            if locked || blocked(paragraph) {
                self.skipped = self.skipped.saturating_add(found.len() as u32);
                return;
            }
            for hit in found {
                if !self.push(
                    story,
                    paragraph.id(),
                    order,
                    hit.start,
                    hit.end,
                    Some(hit.groups),
                ) {
                    return;
                }
            }
        } else {
            let found = match_ranges(
                &units,
                self.needle,
                self.case_sensitive,
                self.whole_word,
                self.fold_width,
            );
            if locked || blocked(paragraph) {
                self.skipped = self.skipped.saturating_add(found.len() as u32);
                return;
            }
            for (start, end) in found {
                if !self.push(story, paragraph.id(), order, start, end, None) {
                    return;
                }
            }
        }
    }

    fn push(
        &mut self,
        story: StoryId,
        paragraph: NodeId,
        order: u32,
        start: usize,
        end: usize,
        captures: Option<Vec<String>>,
    ) -> bool {
        if self.hits.len() >= MAX_HITS {
            self.truncated = true;
            return false;
        }
        self.hits.push(FindHit {
            story,
            paragraph,
            order,
            start: ScalarOffset(start),
            end: ScalarOffset(end),
            captures,
        });
        true
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
    fold_width: bool,
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
            if unit.barrier || &fold(&unit.text, case_sensitive, fold_width) != wanted {
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

struct PatternHit {
    start: usize,
    end: usize,
    groups: Vec<String>,
}

/// Run a compiled pattern over one paragraph's units. The haystack applies
/// the same width folding as the literal path, so under `fold_width` a
/// half-width pattern also finds full-width text. A match that splits a
/// grapheme cluster, covers an inline object, or is empty is skipped;
/// `whole_word` reuses the literal boundary rules on the unit indexes.
///
/// Capture text is sliced from the original scalars. Width folding is one
/// scalar to one scalar, so a folded byte offset maps back without taking
/// the half-width form as the replacement.
fn match_ranges_regex(
    units: &[Unit],
    pattern: &regex::Regex,
    whole_word: bool,
    fold_width: bool,
) -> Vec<PatternHit> {
    let mut haystack = String::new();
    let mut originals = Vec::new();
    let mut scalar_of_byte = Vec::new();
    let mut byte_ends = Vec::with_capacity(units.len());
    let mut scalar = 0usize;
    for unit in units {
        if unit.barrier {
            push_folded_char(
                &mut haystack,
                &mut scalar_of_byte,
                &mut originals,
                '\u{FFFC}',
                '\u{FFFC}',
                scalar,
            );
            scalar += 1;
        } else {
            for ch in unit.text.chars() {
                let folded = if fold_width { fold_width_char(ch) } else { ch };
                push_folded_char(
                    &mut haystack,
                    &mut scalar_of_byte,
                    &mut originals,
                    ch,
                    folded,
                    scalar,
                );
                scalar += 1;
            }
        }
        byte_ends.push(haystack.len());
    }
    scalar_of_byte.push(scalar);
    let byte_start_of =
        |index: usize| -> usize { if index == 0 { 0 } else { byte_ends[index - 1] } };
    let mut found = Vec::new();
    // `captures_iter` yields non-overlapping matches in ascending order, so
    // the unit cursor only moves forward and the scan stays linear overall.
    let mut hint = 0usize;
    for caps in pattern.captures_iter(&haystack) {
        let Some(whole) = caps.get(0) else {
            continue;
        };
        let (byte_start, byte_end) = (whole.start(), whole.end());
        if byte_start == byte_end {
            continue;
        }
        while hint < units.len() && byte_start_of(hint) < byte_start {
            hint += 1;
        }
        if hint == units.len() || byte_start_of(hint) != byte_start {
            continue;
        }
        let first = hint;
        let mut last = first;
        while last + 1 < units.len() && byte_ends[last] < byte_end {
            last += 1;
        }
        if byte_ends[last] != byte_end {
            continue;
        }
        if units[first..=last].iter().any(|unit| unit.barrier) {
            continue;
        }
        if whole_word && !(left_boundary(units, first) && right_boundary(units, last + 1)) {
            continue;
        }
        let Some(groups) = groups_of(&caps, pattern.captures_len(), &scalar_of_byte, &originals)
        else {
            continue;
        };
        let start = units[first].start;
        let end = units[last].end;
        if groups
            .first()
            .is_none_or(|whole| whole.chars().count() != end - start)
        {
            continue;
        }
        found.push(PatternHit { start, end, groups });
    }
    found
}

fn push_folded_char(
    haystack: &mut String,
    scalar_of_byte: &mut Vec<usize>,
    originals: &mut Vec<char>,
    original: char,
    folded: char,
    scalar: usize,
) {
    haystack.push(folded);
    scalar_of_byte.resize(haystack.len(), scalar);
    originals.push(original);
}

fn groups_of(
    caps: &regex::Captures<'_>,
    count: usize,
    scalar_of_byte: &[usize],
    originals: &[char],
) -> Option<Vec<String>> {
    let mut groups = Vec::with_capacity(count);
    for index in 0..count {
        let Some(group) = caps.get(index) else {
            groups.push(String::new());
            continue;
        };
        let start = *scalar_of_byte.get(group.start())?;
        let end = *scalar_of_byte.get(group.end())?;
        if start > end || end > originals.len() {
            return None;
        }
        groups.push(originals[start..end].iter().collect());
    }
    Some(groups)
}

/// Expand `$` references in a regex replacement.
///
/// `$$` becomes `$`. `$` followed by a maximal run of ASCII digits names one
/// group: `$0` is the whole match and `$2` is the second group. `$10` is
/// group 10, not group 1 followed by `0`. An index that the pattern does not
/// have, and a number that does not fit in `usize`, is copied unchanged, so
/// `$9` on a two-group pattern stays `$9`. A `$` that is not part of either
/// form stays `$`. Group text is inserted as itself and is not scanned again.
fn expand_replacement(template: &str, groups: &[String]) -> Result<String, FindError> {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        index += 1;
        if ch != '$' {
            out.push(ch);
            continue;
        }
        if index < chars.len() && chars[index] == '$' {
            index += 1;
            out.push('$');
            continue;
        }
        if index < chars.len() && chars[index].is_ascii_digit() {
            let start = index - 1;
            let mut number: usize = 0;
            let mut overflow = false;
            while index < chars.len() && chars[index].is_ascii_digit() {
                let digit = usize::from(chars[index] as u8 - b'0');
                index += 1;
                match number
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(digit))
                {
                    Some(value) => number = value,
                    None => overflow = true,
                }
            }
            if !overflow && number < groups.len() {
                out.push_str(&groups[number]);
            } else {
                out.extend(chars[start..index].iter().copied());
            }
            continue;
        }
        out.push('$');
    }
    if out.chars().count() > MAX_REPLACEMENT_SCALARS {
        return Err(FindError::TooLong);
    }
    if let Err(CommandError::InvalidText(ch)) = check_text(&out) {
        return Err(FindError::InvalidReplacement(ch));
    }
    Ok(out)
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

/// Width folding per the documented assumption in the module docs:
/// U+FF01..=U+FF5E map down by 0xFEE0 and U+3000 maps to U+0020. Everything
/// else, including CJK ideographs and halfwidth katakana, stays untouched.
fn fold_width_char(ch: char) -> char {
    match ch {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
        '\u{3000}' => ' ',
        _ => ch,
    }
}

fn fold(cluster: &str, case_sensitive: bool, fold_width: bool) -> String {
    if case_sensitive && !fold_width {
        return cluster.to_owned();
    }
    let mut folded = String::with_capacity(cluster.len());
    for ch in cluster.chars() {
        let ch = if fold_width { fold_width_char(ch) } else { ch };
        if case_sensitive {
            folded.push(ch);
        } else {
            folded.extend(ch.to_lowercase());
        }
    }
    folded
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
