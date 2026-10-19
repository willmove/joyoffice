//! Block structure: paragraphs, tables, content controls and preserved blocks
//! (ARCHITECTURE §2, §2.2).
//!
//! Blocks are reference counted so a [`BlockSeq`] edit copies only the edited
//! block and its tree path. Inline objects, breaks and preserved inline content
//! occupy one atomic position each, represented in text by [`OBJECT_MARKER`]
//! and in order by the paragraph's inline list; consumers read typed
//! [`Segment`]s and never see the marker.

use std::sync::Arc;

use crate::hyperlink::Hyperlink;
use crate::id::{NodeId, PreservedId, ResourceId, SourceRef};
use crate::props::{Color, Measure, ParaProps, RunProps};
use crate::style::StyleId;
use crate::text::{OBJECT_MARKER, ParagraphText, ScalarOffset, TextError};
use crate::tree::{Item, SeqTree};

/// Ordered blocks of a story, table cell or content control.
pub type BlockSeq = SeqTree<Block>;

/// A contiguous span of characters sharing formatting. Runs cover the whole
/// paragraph and are never empty.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Length in Unicode scalars, inline object markers included.
    pub len: usize,
    pub style: Option<StyleId>,
    pub props: Arc<RunProps>,
    /// `w:hyperlink` wrapping this run. Absent when the run is ordinary text.
    pub link: Option<Arc<Hyperlink>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BreakKind {
    /// Soft line break (`w:br` without a type).
    Line,
    Page,
    Column,
}

/// A page-number field this slice can project. ECMA-376 Part 1 §17.16.1
/// names `PAGE`, `NUMPAGES` and `SECTIONPAGES`. Other instructions stay
/// uninterpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageFieldKind {
    /// Current page number after pagination has settled.
    Page,
    /// Page count of the laid-out document.
    NumPages,
    /// Page count of the section that owns the field's page.
    SectionPages,
}

/// `PAGE`, `NUMPAGES` or `SECTIONPAGES`, plus the result stored in the file.
/// Layout draws the computed number. The cached text is used only when the
/// number cannot be formatted or the live value does not settle.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PageField {
    pub kind: PageFieldKind,
    pub cached: Option<std::sync::Arc<str>>,
}

impl PageField {
    pub fn new(kind: PageFieldKind) -> Self {
        Self { kind, cached: None }
    }

    pub fn with_cache(kind: PageFieldKind, cached: impl Into<std::sync::Arc<str>>) -> Self {
        Self {
            kind,
            cached: Some(cached.into()),
        }
    }
}

/// Source content the model does not interpret, held by the format layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preserved {
    /// Qualified element name, e.g. `m:oMath` or `w:object`.
    pub element: Arc<str>,
    pub handle: PreservedId,
}

#[derive(Clone, Debug, PartialEq)]
pub enum InlineKind {
    Break(BreakKind),
    /// `PAGE`, `NUMPAGES` or `SECTIONPAGES` in the body or a header or footer.
    PageField(PageField),
    Image {
        resource: ResourceId,
        width: Measure,
        height: Measure,
    },
    Preserved(Preserved),
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineObject {
    pub id: NodeId,
    pub kind: InlineKind,
    pub source: Option<Arc<SourceRef>>,
}

/// Typed view of paragraph content for export, clipboard and Agent tools.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment<'a> {
    Text(String),
    Inline(&'a InlineObject),
}

/// Content removed from a paragraph, kept for undo.
#[derive(Clone, Debug, PartialEq)]
pub struct Removed {
    pub text: String,
    pub inlines: Vec<InlineObject>,
    pub runs: Vec<Run>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Paragraph {
    id: NodeId,
    text: ParagraphText,
    runs: Vec<Run>,
    inlines: Vec<InlineObject>,
    pub style: Option<StyleId>,
    pub props: Arc<ParaProps>,
    /// Properties of the paragraph mark (`w:pPr/w:rPr`); also the formatting
    /// of text typed into an empty paragraph.
    pub mark_props: Arc<RunProps>,
    pub source: Option<Arc<SourceRef>>,
}

impl Paragraph {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            text: ParagraphText::new(),
            runs: Vec::new(),
            inlines: Vec::new(),
            style: None,
            props: Arc::default(),
            mark_props: Arc::default(),
            source: None,
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn text(&self) -> &ParagraphText {
        &self.text
    }

    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    pub fn inlines(&self) -> &[InlineObject] {
        &self.inlines
    }

    /// Length in positions: scalars plus one per inline object.
    pub fn len(&self) -> usize {
        self.text.len_scalars()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Appends text as a new run (importer and builder entry point).
    pub fn push_text(
        &mut self,
        text: &str,
        style: Option<StyleId>,
        props: Arc<RunProps>,
    ) -> Result<(), TextError> {
        let end = ScalarOffset(self.len());
        self.text.insert_str(end, text)?;
        let len = text.chars().count();
        if len > 0 {
            self.runs.push(Run {
                len,
                style,
                props,
                link: None,
            });
        }
        Ok(())
    }

    /// Appends an inline object as its own run.
    pub fn push_inline(
        &mut self,
        object: InlineObject,
        style: Option<StyleId>,
        props: Arc<RunProps>,
    ) {
        let end = ScalarOffset(self.len());
        self.text
            .insert_raw(end, OBJECT_MARKER.encode_utf8(&mut [0; 4]))
            .expect("end offset is valid");
        self.inlines.push(object);
        self.runs.push(Run {
            len: 1,
            style,
            props,
            link: None,
        });
    }

    /// Run containing the character at `offset`, with the run's start.
    fn run_at(&self, offset: usize) -> Option<(usize, usize)> {
        let mut start = 0;
        for (index, run) in self.runs.iter().enumerate() {
            if offset < start + run.len {
                return Some((index, start));
            }
            start += run.len;
        }
        None
    }

    /// Splits runs so that `offset` falls on a run boundary; returns the index
    /// of the first run at or after `offset`.
    fn split_run_at(&mut self, offset: usize) -> usize {
        match self.run_at(offset) {
            None => self.runs.len(),
            Some((index, start)) if start == offset => index,
            Some((index, start)) => {
                let tail = self.runs[index].len - (offset - start);
                self.runs[index].len = offset - start;
                let mut right = self.runs[index].clone();
                right.len = tail;
                self.runs.insert(index + 1, right);
                index + 1
            }
        }
    }

    /// Formatting that content inserted at `at` receives: the run of the
    /// preceding character, the first run at the paragraph start, or the
    /// paragraph mark formatting in an empty paragraph.
    pub fn format_at(&self, at: ScalarOffset) -> (Option<StyleId>, Arc<RunProps>) {
        let run = match at.0 {
            0 => self.runs.first(),
            offset => self.run_at(offset - 1).map(|(index, _)| &self.runs[index]),
        };
        match run {
            Some(run) => (run.style.clone(), run.props.clone()),
            None => (None, self.mark_props.clone()),
        }
    }

    /// Joins `runs[index - 1]` and `runs[index]` when their format is equal.
    fn join_runs_at(&mut self, index: usize) {
        if index == 0 || index >= self.runs.len() {
            return;
        }
        let (left, right) = (&self.runs[index - 1], &self.runs[index]);
        if left.style == right.style && left.props == right.props && left.link == right.link {
            let len = right.len;
            self.runs[index - 1].len += len;
            self.runs.remove(index);
        }
    }

    /// Applies `change` to every run overlapping `range`, splitting runs at the
    /// range ends and joining equal neighbours within it. Returns whether any
    /// run's properties changed.
    pub fn format_runs(
        &mut self,
        range: std::ops::Range<ScalarOffset>,
        mut change: impl FnMut(&mut RunProps),
    ) -> Result<bool, TextError> {
        if range.start > range.end {
            return Err(TextError::InvalidRange {
                start: range.start.0,
                end: range.end.0,
            });
        }
        if range.end.0 > self.len() {
            return Err(TextError::OutOfBounds {
                offset: range.end.0,
                len: self.len(),
            });
        }
        if range.start == range.end {
            return Ok(false);
        }
        let first = self.split_run_at(range.start.0);
        let last = self.split_run_at(range.end.0);
        let mut changed = false;
        for run in &mut self.runs[first..last] {
            let mut props = RunProps::clone(&run.props);
            change(&mut props);
            if props != *run.props {
                run.props = Arc::new(props);
                changed = true;
            }
        }
        for boundary in (first..=last).rev() {
            self.join_runs_at(boundary);
        }
        Ok(changed)
    }

    /// Sets or clears the hyperlink on every run overlapping `range`.
    /// An empty range changes nothing.
    pub fn set_links(
        &mut self,
        range: std::ops::Range<ScalarOffset>,
        link: Option<Arc<Hyperlink>>,
    ) -> Result<bool, TextError> {
        if range.start > range.end {
            return Err(TextError::InvalidRange {
                start: range.start.0,
                end: range.end.0,
            });
        }
        if range.end.0 > self.len() {
            return Err(TextError::OutOfBounds {
                offset: range.end.0,
                len: self.len(),
            });
        }
        if range.start == range.end {
            return Ok(false);
        }
        let first = self.split_run_at(range.start.0);
        let last = self.split_run_at(range.end.0);
        let mut changed = false;
        for run in &mut self.runs[first..last] {
            if run.link != link {
                run.link.clone_from(&link);
                changed = true;
            }
        }
        for boundary in (first..=last).rev() {
            self.join_runs_at(boundary);
        }
        Ok(changed)
    }

    /// Hyperlink covering the scalar at `offset`, if that position is in a run.
    pub fn link_at(&self, offset: usize) -> Option<&Hyperlink> {
        self.run_at(offset)
            .and_then(|(index, _)| self.runs.get(index))
            .and_then(|run| run.link.as_deref())
    }

    /// Puts `link` on the run most recently appended. Import and rebuild use
    /// this after `push_text` or `push_inline`.
    pub fn set_last_run_link(&mut self, link: Option<Arc<Hyperlink>>) {
        if let Some(run) = self.runs.last_mut() {
            run.link = link;
        }
    }

    /// Puts the same link on every run from `from` to the end.
    pub fn stamp_runs_from(&mut self, from: usize, link: Arc<Hyperlink>) {
        for run in self.runs.iter_mut().skip(from) {
            run.link = Some(Arc::clone(&link));
        }
    }

    /// Sets the character style of every run overlapping `range`. Direct
    /// formatting on those runs is left as stored.
    pub fn restyle_runs(
        &mut self,
        range: std::ops::Range<ScalarOffset>,
        style: Option<StyleId>,
    ) -> Result<bool, TextError> {
        if range.start > range.end {
            return Err(TextError::InvalidRange {
                start: range.start.0,
                end: range.end.0,
            });
        }
        if range.end.0 > self.len() {
            return Err(TextError::OutOfBounds {
                offset: range.end.0,
                len: self.len(),
            });
        }
        if range.start == range.end {
            return Ok(false);
        }
        let first = self.split_run_at(range.start.0);
        let last = self.split_run_at(range.end.0);
        let mut changed = false;
        for run in &mut self.runs[first..last] {
            if run.style != style {
                run.style.clone_from(&style);
                changed = true;
            }
        }
        for boundary in (first..=last).rev() {
            self.join_runs_at(boundary);
        }
        Ok(changed)
    }

    /// Inserts ordinary text. It joins the run of the preceding character, or
    /// the first run at the paragraph start, or the paragraph mark formatting
    /// in an empty paragraph.
    pub fn insert_text(&mut self, at: ScalarOffset, text: &str) -> Result<(), TextError> {
        self.text.insert_str(at, text)?;
        let len = text.chars().count();
        if len == 0 {
            return Ok(());
        }
        if self.runs.is_empty() {
            self.runs.push(Run {
                len,
                style: None,
                props: self.mark_props.clone(),
                link: None,
            });
        } else {
            let index = match at.0 {
                0 => 0,
                offset => self.run_at(offset - 1).expect("offset within runs").0,
            };
            self.runs[index].len += len;
        }
        Ok(())
    }

    /// Inserts an inline object at `at` as its own run with the given format.
    pub fn insert_inline(
        &mut self,
        at: ScalarOffset,
        object: InlineObject,
        style: Option<StyleId>,
        props: Arc<RunProps>,
    ) -> Result<(), TextError> {
        let index = self.text.markers_before(at)?;
        self.text
            .insert_raw(at, OBJECT_MARKER.encode_utf8(&mut [0; 4]))?;
        self.inlines.insert(index, object);
        let run = self.split_run_at(at.0);
        self.runs.insert(
            run,
            Run {
                len: 1,
                style,
                props,
                link: None,
            },
        );
        Ok(())
    }

    /// Removes a range of positions, returning text, inline objects and the
    /// removed runs so the caller can build an exact inverse.
    pub fn remove(&mut self, range: std::ops::Range<ScalarOffset>) -> Result<Removed, TextError> {
        let first_inline = self.text.markers_before(range.start)?;
        let text = self.text.remove_raw(range.clone())?;
        let inline_count = text.chars().filter(|&ch| ch == OBJECT_MARKER).count();
        let inlines = self
            .inlines
            .drain(first_inline..first_inline + inline_count)
            .collect();
        let start = self.split_run_at(range.start.0);
        let end = self.split_run_at(range.end.0);
        let runs = self.runs.drain(start..end).collect();
        Ok(Removed {
            text: text.replace(OBJECT_MARKER, ""),
            inlines,
            runs,
        })
    }

    /// Splits the paragraph at `at`. The tail keeps paragraph formatting under
    /// a new identity and no source reference, since it is new content.
    pub fn split_off(&mut self, at: ScalarOffset, new_id: NodeId) -> Result<Paragraph, TextError> {
        let first_inline = self.text.markers_before(at)?;
        let text = self.text.split_off(at)?;
        let run = self.split_run_at(at.0);
        Ok(Paragraph {
            id: new_id,
            text,
            runs: self.runs.split_off(run),
            inlines: self.inlines.split_off(first_inline),
            style: self.style.clone(),
            props: self.props.clone(),
            mark_props: self.mark_props.clone(),
            source: None,
        })
    }

    /// Appends the content of `next` (paragraph merge). Formatting of `self`
    /// is kept; `next`'s identity ends.
    pub fn append(&mut self, next: Paragraph) {
        self.text.append(&next.text);
        self.runs.extend(next.runs);
        self.inlines.extend(next.inlines);
    }

    /// Text and inline objects in order, without object markers.
    pub fn segments(&self) -> Vec<Segment<'_>> {
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut inlines = self.inlines.iter();
        for ch in self.text.chars() {
            if ch == OBJECT_MARKER {
                if !current.is_empty() {
                    segments.push(Segment::Text(std::mem::take(&mut current)));
                }
                if let Some(inline) = inlines.next() {
                    segments.push(Segment::Inline(inline));
                }
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            segments.push(Segment::Text(current));
        }
        segments
    }

    /// Plain text with each inline object rendered by `inline`; markers never
    /// appear in the result.
    pub fn plain_text(&self, inline: impl Fn(&InlineObject) -> String) -> String {
        self.segments()
            .into_iter()
            .map(|segment| match segment {
                Segment::Text(text) => text,
                Segment::Inline(object) => inline(object),
            })
            .collect()
    }
}

/// Vertical merge state of a table cell (`w:vMerge`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VerticalMerge {
    #[default]
    None,
    Restart,
    Continue,
}

/// `w:tblLook` flags (ECMA-376 §17.4.56). First and last column and vertical
/// banding are stored so export can write them back; layout applies the row
/// flags only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableLook {
    pub first_row: bool,
    pub last_row: bool,
    pub first_column: bool,
    pub last_column: bool,
    pub no_h_band: bool,
    pub no_v_band: bool,
}

impl TableLook {
    /// Used when the `w:tblLook` element itself is absent.
    ///
    /// This is an application default, not a measurement of Word: the first
    /// and last rows are on, horizontal banding is on, vertical banding is
    /// off, and both column looks are off.
    pub fn application_default() -> Self {
        Self {
            first_row: true,
            last_row: true,
            first_column: false,
            last_column: false,
            no_h_band: false,
            no_v_band: true,
        }
    }
}

/// `w:tblW`. Absent markup stays `None`, distinct from an explicit `auto`.
/// `Pct` is fiftieths of a percent (ECMA-376 §17.18.90): 5000 is 100%.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableWidth {
    Auto,
    Dxa(i64),
    Pct(i64),
}

/// `w:tblLayout` (ECMA-376 §17.4.52, `ST_TblLayoutType` §17.18.87).
/// `None` means the element was omitted, which §17.4.52 treats as `auto`:
/// placement shrinks the preferred widths when they are wider than the
/// content box. Explicit [`TableLayoutKind::Fixed`] does not shrink. This is
/// not a content-measured autofit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableLayoutKind {
    Fixed,
    Autofit,
}

/// `w:jc` inside `w:tblPr` (ECMA-376 §17.4.28). `start` and `end` are not
/// stored; a document that uses them stays opaque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableAlign {
    Left,
    Center,
    Right,
}

/// `w:vAlign` (ECMA-376 §17.4.83). `both` is outside this subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

/// `w:hRule` on `w:trHeight` (ECMA-376 §17.4.80, `ST_HeightRule` §17.18.37).
/// Omitted `hRule` is `auto`, and `auto` ignores `w:val`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowHeightRule {
    Auto,
    AtLeast,
    Exact,
}

/// Explicit `w:trHeight`. `twips` is `w:val`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowHeight {
    pub twips: i64,
    pub rule: RowHeightRule,
}

/// Four edges of `w:tblCellMar`, in twips (ECMA-376 §17.4.43). When the
/// element is present, an omitted edge is 0. [`Table::cell_margin`] `None`
/// means the element itself was absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellMargins {
    pub top: i64,
    pub left: i64,
    pub bottom: i64,
    pub right: i64,
}

/// Border line recorded from `w:tcBorders`.
///
/// The subset is ECMA-376 §17.18.2 `nil`, `single`, `double`, `dotted`,
/// `dashed` and `thick`. Other `ST_Border` values stay outside the typed table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderKind {
    Nil,
    Single,
    Double,
    Dotted,
    Dashed,
    Thick,
}

/// One edge of `w:tcBorders`. `size_eighths` is `w:sz` (ECMA-376 §17.3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorderEdge {
    pub kind: BorderKind,
    pub size_eighths: u32,
    pub color: Color,
}

/// Cell borders. `None` means the edge element was absent, not `nil`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellBorders {
    pub top: Option<BorderEdge>,
    pub left: Option<BorderEdge>,
    pub bottom: Option<BorderEdge>,
    pub right: Option<BorderEdge>,
}

/// Solid cell shading from `w:shd` when `w:val` is `clear` and the fill is
/// `auto` or an sRGB value. Theme fills are not stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shading {
    pub fill: Color,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub id: NodeId,
    /// Grid columns spanned (`w:gridSpan`), at least 1.
    pub grid_span: u32,
    pub vertical_merge: VerticalMerge,
    pub width: Option<Measure>,
    pub borders: CellBorders,
    pub shading: Option<Shading>,
    /// `w:tcMar`. `None` means the element was absent, so the table's
    /// `w:tblCellMar` (or the application default) applies. When present, an
    /// omitted edge is 0 and does not inherit that edge from the table
    /// (ECMA-376 §17.4.68). `w:start` and `w:end` are stored as left and right
    /// because `bidiVisual` is outside this subset.
    pub margin: Option<CellMargins>,
    /// `w:vAlign`. `None` is omitted and lays out as top.
    pub vertical_align: Option<VerticalAlign>,
    /// Cell content; Word requires at least one block. This slice allows
    /// paragraphs and one level of nested tables.
    pub blocks: BlockSeq,
    pub preserved: Option<PreservedId>,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: NodeId,
    /// Grid columns skipped before the first cell (`w:gridBefore`).
    pub grid_before: u32,
    /// Grid columns left empty after the last cell (`w:gridAfter`).
    pub grid_after: u32,
    pub cells: Vec<Cell>,
    pub header: bool,
    /// `w:cantSplit`. Absent markup is false: the row may break across pages.
    pub cant_split: bool,
    /// `w:trHeight`. `None` means the element was absent.
    pub height: Option<RowHeight>,
    pub preserved: Option<PreservedId>,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub id: NodeId,
    /// Column widths of `w:tblGrid`, independent of any screen geometry.
    pub grid: Vec<Measure>,
    pub rows: Vec<Row>,
    pub style: Option<StyleId>,
    pub width: Option<TableWidth>,
    /// `w:tblLayout`. `None` is an omitted element.
    pub layout: Option<TableLayoutKind>,
    /// `w:jc` in table properties. `None` is omitted and lays out on the left.
    pub alignment: Option<TableAlign>,
    /// `w:tblInd` in twips. `None` is an omitted element, not an explicit zero.
    pub indent: Option<i64>,
    /// `w:tblLook`. `None` means the element was omitted.
    pub look: Option<TableLook>,
    /// `w:tblCellMar`. `None` is an omitted element, not four explicit zeros.
    pub cell_margin: Option<CellMargins>,
    pub preserved: Option<PreservedId>,
    pub source: Option<Arc<SourceRef>>,
}

impl Table {
    /// Structural equality, including cell paragraphs. Export uses it to
    /// leave an untouched table's original bytes alone.
    pub fn same_content(&self, other: &Self) -> bool {
        self.grid == other.grid
            && self.style == other.style
            && self.width == other.width
            && self.layout == other.layout
            && self.alignment == other.alignment
            && self.indent == other.indent
            && self.look == other.look
            && self.cell_margin == other.cell_margin
            && self.preserved == other.preserved
            && self.source == other.source
            && self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(&other.rows)
                .all(|(left, right)| same_row(left, right))
    }
}

fn same_row(left: &Row, right: &Row) -> bool {
    left.id == right.id
        && left.grid_before == right.grid_before
        && left.grid_after == right.grid_after
        && left.header == right.header
        && left.cant_split == right.cant_split
        && left.height == right.height
        && left.preserved == right.preserved
        && left.cells.len() == right.cells.len()
        && left
            .cells
            .iter()
            .zip(&right.cells)
            .all(|(left, right)| same_cell(left, right))
}

fn same_cell(left: &Cell, right: &Cell) -> bool {
    left.id == right.id
        && left.grid_span == right.grid_span
        && left.vertical_merge == right.vertical_merge
        && left.width == right.width
        && left.borders == right.borders
        && left.shading == right.shading
        && left.margin == right.margin
        && left.vertical_align == right.vertical_align
        && left.preserved == right.preserved
        && left.blocks.len() == right.blocks.len()
        && left
            .blocks
            .iter()
            .zip(right.blocks.iter())
            .all(|(left, right)| same_block(left, right))
}

fn same_block(left: &Block, right: &Block) -> bool {
    match (left, right) {
        (Block::Paragraph(left), Block::Paragraph(right)) => left.as_ref() == right.as_ref(),
        (Block::Table(left), Block::Table(right)) => left.same_content(right),
        (Block::Opaque(left), Block::Opaque(right)) => {
            left.id == right.id && left.preserved == right.preserved && left.source == right.source
        }
        (Block::Sdt(left), Block::Sdt(right)) => {
            left.id == right.id
                && left.props == right.props
                && left.source == right.source
                && left.blocks.len() == right.blocks.len()
                && left
                    .blocks
                    .iter()
                    .zip(right.blocks.iter())
                    .all(|(left, right)| same_block(left, right))
        }
        _ => false,
    }
}

/// Content control locking (`w:lock`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SdtLock {
    #[default]
    Unlocked,
    /// The control cannot be deleted.
    SdtLocked,
    /// The content cannot be edited.
    ContentLocked,
    SdtContentLocked,
}

impl SdtLock {
    pub fn content_editable(self) -> bool {
        matches!(self, SdtLock::Unlocked | SdtLock::SdtLocked)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SdtProps {
    pub tag: Option<Arc<str>>,
    pub alias: Option<Arc<str>>,
    pub lock: SdtLock,
    pub preserved: Option<PreservedId>,
}

/// Block-level content control: a transparent container (ARCHITECTURE §2).
#[derive(Clone, Debug)]
pub struct Sdt {
    pub id: NodeId,
    pub props: SdtProps,
    pub blocks: BlockSeq,
    pub source: Option<Arc<SourceRef>>,
}

/// A block the model keeps but does not interpret, e.g. an unknown element.
#[derive(Clone, Debug)]
pub struct OpaqueBlock {
    pub id: NodeId,
    pub preserved: Preserved,
    pub source: Option<Arc<SourceRef>>,
}

#[derive(Clone, Debug)]
pub enum Block {
    Paragraph(Arc<Paragraph>),
    Table(Arc<Table>),
    Sdt(Arc<Sdt>),
    Opaque(Arc<OpaqueBlock>),
}

impl Block {
    pub fn id(&self) -> NodeId {
        match self {
            Block::Paragraph(paragraph) => paragraph.id,
            Block::Table(table) => table.id,
            Block::Sdt(sdt) => sdt.id,
            Block::Opaque(opaque) => opaque.id,
        }
    }

    pub fn as_paragraph(&self) -> Option<&Paragraph> {
        match self {
            Block::Paragraph(paragraph) => Some(paragraph),
            _ => None,
        }
    }
}

impl From<Paragraph> for Block {
    fn from(paragraph: Paragraph) -> Self {
        Block::Paragraph(Arc::new(paragraph))
    }
}

impl From<Table> for Block {
    fn from(table: Table) -> Self {
        Block::Table(Arc::new(table))
    }
}

impl From<Sdt> for Block {
    fn from(sdt: Sdt) -> Self {
        Block::Sdt(Arc::new(sdt))
    }
}

impl From<OpaqueBlock> for Block {
    fn from(opaque: OpaqueBlock) -> Self {
        Block::Opaque(Arc::new(opaque))
    }
}

impl Item for Block {
    type Summary = ();

    fn summary(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::IdAllocator;

    fn bold() -> Arc<RunProps> {
        Arc::new(RunProps {
            bold: Some(true),
            ..RunProps::default()
        })
    }

    fn sample(ids: &mut IdAllocator) -> Paragraph {
        let mut paragraph = Paragraph::new(ids.node());
        paragraph.push_text("甲方", None, Arc::default()).unwrap();
        paragraph.push_text("签字", None, bold()).unwrap();
        paragraph.push_inline(
            InlineObject {
                id: ids.node(),
                kind: InlineKind::Break(BreakKind::Line),
                source: None,
            },
            None,
            Arc::default(),
        );
        paragraph.push_text("日期", None, Arc::default()).unwrap();
        paragraph
    }

    fn run_lengths(paragraph: &Paragraph) -> Vec<usize> {
        paragraph.runs().iter().map(|run| run.len).collect()
    }

    fn plain(paragraph: &Paragraph) -> String {
        paragraph.plain_text(|_| "⏎".into())
    }

    #[test]
    fn segments_hide_object_markers() {
        let mut ids = IdAllocator::default();
        let paragraph = sample(&mut ids);
        assert_eq!(paragraph.len(), 7);
        assert_eq!(plain(&paragraph), "甲方签字⏎日期");
        assert!(!plain(&paragraph).contains(OBJECT_MARKER));
        assert!(matches!(paragraph.segments()[1], Segment::Inline(_)));
    }

    #[test]
    fn typing_joins_preceding_run_and_empty_paragraph_uses_mark_format() {
        let mut ids = IdAllocator::default();
        let mut paragraph = sample(&mut ids);
        paragraph.insert_text(ScalarOffset(4), "人").unwrap();
        assert_eq!(run_lengths(&paragraph), vec![2, 3, 1, 2]);
        paragraph.insert_text(ScalarOffset(0), "（").unwrap();
        assert_eq!(run_lengths(&paragraph), vec![3, 3, 1, 2]);

        let mut empty = Paragraph::new(ids.node());
        empty.mark_props = bold();
        empty.insert_text(ScalarOffset(0), "标题").unwrap();
        assert_eq!(empty.runs()[0].props, bold());
    }

    #[test]
    fn remove_returns_exact_content_and_inlines() {
        let mut ids = IdAllocator::default();
        let mut paragraph = sample(&mut ids);
        let removed = paragraph.remove(ScalarOffset(3)..ScalarOffset(6)).unwrap();
        assert_eq!(removed.text, "字日");
        assert_eq!(removed.inlines.len(), 1);
        assert_eq!(removed.runs.iter().map(|r| r.len).sum::<usize>(), 3);
        assert_eq!(plain(&paragraph), "甲方签期");
        assert_eq!(run_lengths(&paragraph), vec![2, 1, 1]);
        assert!(paragraph.inlines().is_empty());
        assert!(paragraph.remove(ScalarOffset(3)..ScalarOffset(9)).is_err());
        assert_eq!(plain(&paragraph), "甲方签期");
    }

    #[test]
    fn insert_inline_splits_run_and_keeps_order() {
        let mut ids = IdAllocator::default();
        let mut paragraph = sample(&mut ids);
        let image = InlineObject {
            id: ids.node(),
            kind: InlineKind::Break(BreakKind::Page),
            source: None,
        };
        paragraph
            .insert_inline(ScalarOffset(1), image.clone(), None, Arc::default())
            .unwrap();
        assert_eq!(run_lengths(&paragraph), vec![1, 1, 1, 2, 1, 2]);
        assert_eq!(paragraph.inlines()[0], image);
        assert_eq!(
            paragraph.plain_text(|o| format!("[{}]", o.id)),
            format!("甲[{}]方签字[{}]日期", image.id, paragraph.inlines()[1].id)
        );
    }

    #[test]
    fn format_runs_splits_and_rejoins_boundaries() {
        let mut ids = IdAllocator::default();
        let mut paragraph = sample(&mut ids);
        // Bold "方签": splits run 0 and joins the existing bold run.
        let changed = paragraph
            .format_runs(ScalarOffset(1)..ScalarOffset(3), |p| p.bold = Some(true))
            .unwrap();
        assert!(changed);
        assert_eq!(run_lengths(&paragraph), vec![1, 3, 1, 2]);
        // Applying the same format again is a no-op and leaves runs unchanged.
        let again = paragraph
            .format_runs(ScalarOffset(1)..ScalarOffset(4), |p| p.bold = Some(true))
            .unwrap();
        assert!(!again);
        assert_eq!(run_lengths(&paragraph), vec![1, 3, 1, 2]);
        assert!(
            paragraph
                .format_runs(ScalarOffset(2)..ScalarOffset(1), |_| {})
                .is_err()
        );
        assert!(
            paragraph
                .format_runs(ScalarOffset(0)..ScalarOffset(8), |_| {})
                .is_err()
        );
        assert_eq!(paragraph.format_at(ScalarOffset(2)).1, bold());
        assert_eq!(paragraph.format_at(ScalarOffset(0)).1, Arc::default());
    }

    #[test]
    fn split_and_merge_round_trip() {
        let mut ids = IdAllocator::default();
        let original = sample(&mut ids);
        let mut head = original.clone();
        let tail = head.split_off(ScalarOffset(3), ids.node()).unwrap();
        assert_eq!(plain(&head), "甲方签");
        assert_eq!(plain(&tail), "字⏎日期");
        assert_eq!(run_lengths(&head), vec![2, 1]);
        assert_eq!(run_lengths(&tail), vec![1, 1, 2]);
        assert_ne!(tail.id(), head.id());
        head.append(tail);
        assert_eq!(plain(&head), plain(&original));
        assert_eq!(head.inlines(), original.inlines());
    }
}
