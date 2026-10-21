//! Model invariants with bounded diagnostics (T100 error path).
//!
//! Validation never recurses deeper than [`MAX_NESTING`] containers and never
//! reports more than [`MAX_DIAGNOSTICS`] items, so hostile input cannot turn a
//! check into unbounded work or output. Errors break model invariants; warnings
//! have a defined fallback (e.g. a missing style resolves to the default).

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::annotation::{Anchor, AnnotationKind, Position};
use crate::block::{Block, BlockSeq, InlineKind, Paragraph, Table, VerticalMerge};
use crate::document::{Document, StoryKind};
use crate::id::{AnnotationId, NodeId, ResourceId, StoryId};
use crate::style::{StyleId, StyleKind, StyleProblem};

/// Deepest container nesting (tables, cells, content controls) examined.
pub const MAX_NESTING: usize = 64;
/// Most diagnostics reported by one validation.
pub const MAX_DIAGNOSTICS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Issue {
    DuplicateId {
        id: u64,
    },
    UnissuedId {
        id: u64,
    },
    MissingBody,
    EmptyStory {
        story: StoryId,
    },
    EmptyCell {
        cell: NodeId,
    },
    NestingTooDeep {
        node: NodeId,
    },
    RunLengthMismatch {
        paragraph: NodeId,
        runs: usize,
        text: usize,
    },
    EmptyRun {
        paragraph: NodeId,
    },
    InlineCountMismatch {
        paragraph: NodeId,
        markers: usize,
        inlines: usize,
    },
    TableGridMismatch {
        row: NodeId,
        grid: usize,
        used: usize,
    },
    ZeroGridSpan {
        cell: NodeId,
    },
    OrphanVerticalMerge {
        cell: NodeId,
    },
    MissingResource {
        node: NodeId,
        resource: ResourceId,
    },
    MissingStyle {
        style: StyleId,
    },
    StyleKindMismatch {
        style: StyleId,
        expected: StyleKind,
    },
    StyleChain {
        problem: StyleProblem,
    },
    MissingStory {
        story: StoryId,
    },
    StoryKindMismatch {
        story: StoryId,
        expected: StoryKind,
    },
    NoSections,
    SectionBoundary {
        section: NodeId,
    },
    InvalidPosition {
        annotation: AnnotationId,
    },
    ReversedRange {
        annotation: AnnotationId,
    },
    SeparatorOutsideField {
        annotation: AnnotationId,
    },
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Issue::DuplicateId { id } => write!(f, "identifier {id} used twice"),
            Issue::UnissuedId { id } => write!(f, "identifier {id} was never allocated"),
            Issue::MissingBody => write!(f, "body story missing or not of kind body"),
            Issue::EmptyStory { story } => write!(f, "story {story} has no blocks"),
            Issue::EmptyCell { cell } => write!(f, "table cell {cell} has no blocks"),
            Issue::NestingTooDeep { node } => {
                write!(f, "{node} nests deeper than {MAX_NESTING} containers")
            }
            Issue::RunLengthMismatch {
                paragraph,
                runs,
                text,
            } => {
                write!(
                    f,
                    "{paragraph}: runs cover {runs} positions, text has {text}"
                )
            }
            Issue::EmptyRun { paragraph } => write!(f, "{paragraph}: empty run"),
            Issue::InlineCountMismatch {
                paragraph,
                markers,
                inlines,
            } => {
                write!(
                    f,
                    "{paragraph}: {markers} object positions, {inlines} inline objects"
                )
            }
            Issue::TableGridMismatch { row, grid, used } => {
                write!(f, "row {row} uses {used} of {grid} grid columns")
            }
            Issue::ZeroGridSpan { cell } => write!(f, "cell {cell} spans zero columns"),
            Issue::OrphanVerticalMerge { cell } => {
                write!(
                    f,
                    "cell {cell} continues a vertical merge with no cell above"
                )
            }
            Issue::MissingResource { node, resource } => {
                write!(f, "{node} refers to missing resource {resource}")
            }
            Issue::MissingStyle { style } => write!(f, "style {style:?} is not defined"),
            Issue::StyleKindMismatch { style, expected } => {
                write!(
                    f,
                    "style {style:?} used where a {expected} style is required"
                )
            }
            Issue::StyleChain { problem } => write!(f, "style inheritance: {problem:?}"),
            Issue::MissingStory { story } => write!(f, "story {story} is referenced but missing"),
            Issue::StoryKindMismatch { story, expected } => {
                write!(f, "story {story} is not a {expected} story")
            }
            Issue::NoSections => write!(f, "document has no section"),
            Issue::SectionBoundary { section } => {
                write!(f, "section {section} has an invalid boundary block")
            }
            Issue::InvalidPosition { annotation } => {
                write!(f, "annotation {annotation} points outside its story text")
            }
            Issue::ReversedRange { annotation } => {
                write!(f, "annotation {annotation} ends before it starts")
            }
            Issue::SeparatorOutsideField { annotation } => {
                write!(f, "field {annotation} has a separator outside its range")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub issue: Issue,
}

#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    truncated: bool,
}

impl Diagnostics {
    fn push(&mut self, severity: Severity, issue: Issue) {
        if self.items.len() < MAX_DIAGNOSTICS {
            self.items.push(Diagnostic { severity, issue });
        } else {
            self.truncated = true;
        }
    }

    fn error(&mut self, issue: Issue) {
        self.push(Severity::Error, issue);
    }

    fn warning(&mut self, issue: Issue) {
        self.push(Severity::Warning, issue);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.items
            .iter()
            .filter(|item| item.severity == Severity::Error)
            .map(|item| &item.issue)
    }

    pub fn has_errors(&self) -> bool {
        self.errors().next().is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && !self.truncated
    }

    /// More problems existed than [`MAX_DIAGNOSTICS`].
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Where a paragraph sits: its story, document-order index and length.
struct ParagraphInfo {
    story: StoryId,
    order: usize,
    len: usize,
}

/// Grid, span, vertical-merge and empty-cell issues for one table.
/// Nested blocks and style kinds are checked by the document walk.
pub fn table_structure(table: &Table) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut above: HashMap<u32, u32> = HashMap::new();
    for row in &table.rows {
        let mut column = row.grid_before;
        let mut current = HashMap::new();
        for cell in &row.cells {
            if cell.grid_span == 0 {
                issues.push(Issue::ZeroGridSpan { cell: cell.id });
            }
            if cell.vertical_merge == VerticalMerge::Continue
                && above.get(&column) != Some(&cell.grid_span)
            {
                issues.push(Issue::OrphanVerticalMerge { cell: cell.id });
            }
            current.insert(column, cell.grid_span);
            column = column.saturating_add(cell.grid_span);
            if cell.blocks.is_empty() {
                issues.push(Issue::EmptyCell { cell: cell.id });
            }
        }
        let used = column.saturating_add(row.grid_after) as usize;
        if used != table.grid.len() {
            issues.push(Issue::TableGridMismatch {
                row: row.id,
                grid: table.grid.len(),
                used,
            });
        }
        above = current;
    }
    issues
}

struct Validator<'a> {
    document: &'a Document,
    out: Diagnostics,
    seen: HashSet<u64>,
    paragraphs: HashMap<NodeId, ParagraphInfo>,
    order: usize,
}

impl Validator<'_> {
    fn id(&mut self, raw: u64) {
        if !self.seen.insert(raw) {
            self.out.error(Issue::DuplicateId { id: raw });
        }
        if !self.document.id_allocator().issued(raw) {
            self.out.error(Issue::UnissuedId { id: raw });
        }
    }

    fn style(&mut self, style: &StyleId, expected: StyleKind) {
        match self.document.styles().get(style) {
            None => self.out.warning(Issue::MissingStyle {
                style: style.clone(),
            }),
            Some(found) if found.kind != expected => self.out.warning(Issue::StyleKindMismatch {
                style: style.clone(),
                expected,
            }),
            Some(_) => {}
        }
    }

    fn blocks(&mut self, story: StoryId, blocks: &BlockSeq, depth: usize) {
        for block in blocks.iter() {
            self.id(block.id().get());
            if depth >= MAX_NESTING {
                self.out.error(Issue::NestingTooDeep { node: block.id() });
                continue;
            }
            match block {
                Block::Paragraph(paragraph) => self.paragraph(story, paragraph),
                Block::Table(table) => self.table(story, table, depth),
                Block::Sdt(sdt) => self.blocks(story, &sdt.blocks, depth + 1),
                Block::Opaque(_) => {}
            }
        }
    }

    fn paragraph(&mut self, story: StoryId, paragraph: &Paragraph) {
        let id = paragraph.id();
        let len = paragraph.len();
        self.paragraphs.insert(
            id,
            ParagraphInfo {
                story,
                order: self.order,
                len,
            },
        );
        self.order += 1;
        for issue in paragraph.invariant_issues() {
            self.out.error(issue);
        }
        if let Some(style) = &paragraph.style {
            self.style(style, StyleKind::Paragraph);
        }
        for run in paragraph.runs() {
            if let Some(style) = &run.style {
                self.style(style, StyleKind::Character);
            }
        }
        for inline in paragraph.inlines() {
            self.id(inline.id.get());
            if let InlineKind::Image { resource, .. } = inline.kind
                && self.document.resources().get(resource).is_none()
            {
                self.out.error(Issue::MissingResource {
                    node: inline.id,
                    resource,
                });
            }
        }
    }

    fn table(&mut self, story: StoryId, table: &Table, depth: usize) {
        if let Some(style) = &table.style {
            self.style(style, StyleKind::Table);
        }
        for row in &table.rows {
            self.id(row.id.get());
            for cell in &row.cells {
                self.id(cell.id.get());
                self.blocks(story, &cell.blocks, depth + 1);
            }
        }
        for issue in table_structure(table) {
            self.out.error(issue);
        }
    }

    fn story_ref(&mut self, story: StoryId, expected: StoryKind) {
        match self.document.story(story) {
            None => self.out.error(Issue::MissingStory { story }),
            Some(found) if found.kind != expected => {
                self.out.error(Issue::StoryKindMismatch { story, expected });
            }
            Some(_) => {}
        }
    }

    fn sections(&mut self) {
        let document = self.document;
        let sections = document.sections();
        if sections.is_empty() {
            self.out.error(Issue::NoSections);
        }
        let top_level: HashMap<NodeId, usize> = document
            .body()
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| (block.id(), index))
            .collect();
        let mut previous = None;
        for (index, section) in sections.iter().enumerate() {
            self.id(section.id.get());
            let last = index + 1 == sections.len();
            let valid = match (section.last_block, last) {
                (None, true) => true,
                (Some(block), false) => match top_level.get(&block) {
                    Some(&position) => {
                        let ordered = previous.is_none_or(|before| position > before);
                        previous = Some(position);
                        ordered
                    }
                    None => false,
                },
                _ => false,
            };
            if !valid {
                self.out.error(Issue::SectionBoundary {
                    section: section.id,
                });
            }
            for &(_, story) in &section.headers {
                self.story_ref(story, StoryKind::Header);
            }
            for &(_, story) in &section.footers {
                self.story_ref(story, StoryKind::Footer);
            }
        }
    }

    /// Document-order key of a valid position.
    fn position(&self, position: &Position) -> Option<(usize, usize)> {
        let info = self.paragraphs.get(&position.paragraph)?;
        (info.story == position.story && position.offset.0 <= info.len)
            .then_some((info.order, position.offset.0))
    }

    fn annotations(&mut self) {
        let document = self.document;
        for annotation in document.annotations() {
            self.id(annotation.id.get());
            if let AnnotationKind::Comment(comment) = &annotation.kind {
                self.story_ref(comment.body, StoryKind::Comment);
            }
            let Anchor::Range { start, end } = &annotation.anchor else {
                continue;
            };
            let (Some(from), Some(to)) = (self.position(start), self.position(end)) else {
                self.out.error(Issue::InvalidPosition {
                    annotation: annotation.id,
                });
                continue;
            };
            if start.story != end.story {
                self.out.error(Issue::InvalidPosition {
                    annotation: annotation.id,
                });
            } else if from > to {
                self.out.error(Issue::ReversedRange {
                    annotation: annotation.id,
                });
            }
            if let AnnotationKind::Field {
                separator: Some(separator),
            } = &annotation.kind
            {
                let inside = separator.story == start.story
                    && self
                        .position(separator)
                        .is_some_and(|at| from <= at && at <= to);
                if !inside {
                    self.out.error(Issue::SeparatorOutsideField {
                        annotation: annotation.id,
                    });
                }
            }
        }
    }

    fn styles(&mut self) {
        let mut reported = HashSet::new();
        let mut in_cycle: HashSet<StyleId> = HashSet::new();
        for style in self.document.styles().iter() {
            let chain = self.document.styles().chain(&style.id);
            let Some(problem) = chain.problem else {
                continue;
            };
            if let StyleProblem::Cycle(entry) = &problem {
                // Report each cycle once, however many styles lead into it.
                let members: Vec<StyleId> = chain
                    .styles
                    .iter()
                    .map(|member| member.id.clone())
                    .skip_while(|id| id != entry)
                    .collect();
                if members.iter().any(|id| in_cycle.contains(id)) {
                    continue;
                }
                in_cycle.extend(members);
            }
            if reported.insert(problem.clone()) {
                let severe = matches!(problem, StyleProblem::Cycle(_) | StyleProblem::TooDeep);
                let issue = Issue::StyleChain { problem };
                if severe {
                    self.out.error(issue);
                } else {
                    self.out.warning(issue);
                }
            }
        }
    }
}

impl Paragraph {
    /// Structural invariants of one paragraph: runs cover the text exactly,
    /// no run is empty, and every object position has an inline object.
    pub fn invariant_issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let id = self.id();
        let len = self.len();
        let covered: usize = self.runs().iter().map(|run| run.len).sum();
        if covered != len {
            issues.push(Issue::RunLengthMismatch {
                paragraph: id,
                runs: covered,
                text: len,
            });
        }
        if self.runs().iter().any(|run| run.len == 0) {
            issues.push(Issue::EmptyRun { paragraph: id });
        }
        let markers = self.text().summary().markers;
        if markers != self.inlines().len() {
            issues.push(Issue::InlineCountMismatch {
                paragraph: id,
                markers,
                inlines: self.inlines().len(),
            });
        }
        issues
    }
}

impl Document {
    /// Checks every model invariant; see the module documentation.
    pub fn validate(&self) -> Diagnostics {
        let mut validator = Validator {
            document: self,
            out: Diagnostics::default(),
            seen: HashSet::new(),
            paragraphs: HashMap::new(),
            order: 0,
        };
        match self.story(self.body_id()) {
            Some(body) if body.kind == StoryKind::Body => {}
            _ => validator.out.error(Issue::MissingBody),
        }
        for story in self.stories() {
            validator.id(story.id.get());
            if story.blocks.is_empty() {
                validator.out.error(Issue::EmptyStory { story: story.id });
            }
            validator.blocks(story.id, &story.blocks, 0);
        }
        validator.sections();
        validator.annotations();
        validator.styles();
        for resource in self.resources().iter() {
            validator.id(resource.id().get());
        }
        validator.out
    }
}
