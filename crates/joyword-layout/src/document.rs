//! Read-only document flow (T105, C05, T202).
//!
//! The explicit single-font profile is a preview contract, not a Word layout
//! compatibility profile. Unsupported formatting, stories and content
//! controls are rejected before shaping. An opaque body block is skipped,
//! counted, and joined to the fingerprint; it takes no geometry. All
//! geometry stays in EMU. The engine remains the only owner of mutable
//! document state.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use joyword_foundation::Emu;
use joyword_model::{
    Bias, Block, BorderEdge, BorderKind, CellMargins, Color, DocDefaults, Document,
    DocumentSnapshot, DocumentVersion, Justification, LineSpacing, Measure, NodeId, NumberLabel,
    NumberingProblem, PageSetup, Paragraph, Position, ScalarOffset, StoryId, Table, VerticalMerge,
    displayed_paragraph,
};
use sha2::{Digest, Sha256};

use crate::cjk::{self, CjkIssue};
use crate::line::{LineFit, layout_paragraph_adjusted, layout_paragraph_auto_cancellable_adjusted};
use crate::shape::ShapedRun;
use crate::style_paint;
use crate::{
    FaceSet, FeatureSet, FontKey, FontSlot, GlyphRun, LayoutLine, LineDiagnostic, LineError,
    LineLimits, LoadedFont, PageScene, SceneImage, SceneRect, SourceRange, adjust_cjk_break,
    slot_for, wrap_mixed,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SingleFontProfile {
    pub size: Emu,
    pub leading: Emu,
    pub baseline: Emu,
    pub features: FeatureSet,
}

impl SingleFontProfile {
    /// Fixed line boxes, with baseline measured from the line-box top.
    pub fn validate(self) -> Result<(), LayoutError> {
        if !(1..=1_000_000_000).contains(&self.size.raw())
            || self.leading.raw() <= 0
            || self.baseline.raw() < 0
            || self.baseline.raw() > self.leading.raw()
        {
            return Err(LayoutError::InvalidProfile);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutLimit {
    Paragraphs,
    Runs,
    Metadata,
    TextBytes,
    Lines,
    Glyphs,
    Pages,
    ShapeCalls,
    ShapedBytes,
    SelectionRects,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutLimits {
    pub max_paragraphs: usize,
    pub max_runs: usize,
    pub max_metadata_items: usize,
    pub max_text_bytes: usize,
    pub max_lines: usize,
    pub max_glyphs: usize,
    pub max_pages: usize,
    pub max_shape_calls: usize,
    pub max_shaped_bytes: usize,
    pub max_diagnostics: usize,
    pub max_selection_rects: usize,
}
impl Default for LayoutLimits {
    fn default() -> Self {
        Self {
            max_paragraphs: 10_000,
            max_runs: 100_000,
            max_metadata_items: 10_000,
            max_text_bytes: 16 << 20,
            max_lines: 100_000,
            max_glyphs: 2_000_000,
            max_pages: 4096,
            max_shape_calls: 131_072,
            max_shaped_bytes: 128 << 20,
            max_diagnostics: 256,
            max_selection_rects: 100_000,
        }
    }
}
impl LayoutLimits {
    pub(crate) fn capped(self) -> Self {
        let h = Self::default();
        Self {
            max_paragraphs: self.max_paragraphs.min(h.max_paragraphs),
            max_runs: self.max_runs.min(h.max_runs),
            max_metadata_items: self.max_metadata_items.min(h.max_metadata_items),
            max_text_bytes: self.max_text_bytes.min(h.max_text_bytes),
            max_lines: self.max_lines.min(h.max_lines),
            max_glyphs: self.max_glyphs.min(h.max_glyphs),
            max_pages: self.max_pages.min(h.max_pages),
            max_shape_calls: self.max_shape_calls.min(h.max_shape_calls),
            max_shaped_bytes: self.max_shaped_bytes.min(h.max_shaped_bytes),
            max_diagnostics: self.max_diagnostics.min(h.max_diagnostics),
            max_selection_rects: self.max_selection_rects.min(h.max_selection_rects),
        }
    }
}

#[derive(Clone, Default)]
pub struct LayoutCancellation(Arc<AtomicBool>);
impl LayoutCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub(crate) fn check(&self) -> Result<(), LayoutError> {
        if self.is_cancelled() {
            Err(LayoutError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StaleReason {
    Version,
    Content,
    Font,
    Profile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    InvalidDocument,
    InvalidProfile,
    InvalidPage,
    Unsupported {
        node: Option<NodeId>,
        feature: &'static str,
    },
    LimitExceeded(LayoutLimit),
    Line {
        paragraph: NodeId,
        error: LineError,
    },
    LineTooTall,
    GeometryOverflow,
    Cancelled,
    StaleSnapshot(StaleReason),
    UnknownPage(usize),
    UnknownPosition(Position),
    InsideCluster(Position),
    ReversedSelection,
    /// Pagination feedback (page numbers, footnotes, wrap) repeated a state
    /// or exhausted its pass budget. The temporary pages are not a snapshot.
    Unstable,
}
impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Feedback did not settle. Callers keep the previous snapshot.
            LayoutError::Unstable => f.write_str("布局未稳定"),
            other => write!(f, "document layout: {other:?}"),
        }
    }
}
impl std::error::Error for LayoutError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutKey {
    pub version: DocumentVersion,
    pub font: FontKey,
    pub profile: SingleFontProfile,
    content: [u8; 32],
}
impl LayoutKey {
    pub fn content_hash(&self) -> [u8; 32] {
        self.content
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageGeometry {
    pub width: Emu,
    pub height: Emu,
    pub left: Emu,
    pub top: Emu,
    pub right: Emu,
    pub bottom: Emu,
    /// Distance from the top edge of the page to the header (`w:header`).
    pub header: Emu,
    /// Distance from the bottom edge of the page to the footer (`w:footer`).
    pub footer: Emu,
}

/// A bounded notice that did not fail pagination. Callers still receive the
/// snapshot; the note says what was not applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutNote {
    /// A keep-next chain or keep-lines paragraph is taller than the page.
    /// It starts on a fresh page and then flows; placement does not retry.
    KeepTallerThanPage { paragraph: NodeId },
    /// A header or footer paragraph was not painted. Its cached field result
    /// is not shown as a live page number.
    StoryContentSkipped { paragraph: NodeId },
    /// Header or footer ink crosses into the body area. The body was not
    /// pushed, because that would change the page count this slice treats
    /// as already settled.
    StoryExceedsMargin { story: StoryId },
    /// A table row is taller than the page body. A `cantSplit` row stays on
    /// one page and may pass the body edge. A splittable row still breaks;
    /// a single line that itself exceeds the body is placed once.
    RowTallerThanPage { row: NodeId },
    /// The leading `tblHeader` rows are taller than the page body, so they
    /// are drawn once and not repeated on later pages.
    HeaderTallerThanPage { table: NodeId },
}
impl PageGeometry {
    /// `w` and `h` are the page size (ECMA-376 §17.6.13). `orient` is stored
    /// with them and is not used to swap the axes here.
    pub(crate) fn from_setup(page: &PageSetup) -> Result<Self, LayoutError> {
        let abs = |measure: Measure| measure.absolute().ok_or(LayoutError::InvalidPage);
        let non_negative = |measure: Measure| {
            let length = abs(measure)?;
            if length.raw() < 0 {
                Err(LayoutError::InvalidPage)
            } else {
                Ok(length)
            }
        };
        let geometry = Self {
            width: abs(page.width)?,
            height: abs(page.height)?,
            left: add(non_negative(page.left)?, non_negative(page.gutter)?)?,
            top: non_negative(page.top)?,
            right: non_negative(page.right)?,
            bottom: non_negative(page.bottom)?,
            header: non_negative(page.header)?,
            footer: non_negative(page.footer)?,
        };
        // Header and footer distances may sit outside the body box. Painting
        // reports that; it does not reject a page the body can still flow on.
        if geometry.width.raw() <= 0
            || geometry.height.raw() <= 0
            || add(geometry.left, geometry.right)?.raw() >= geometry.width.raw()
            || add(geometry.top, geometry.bottom)?.raw() >= geometry.height.raw()
        {
            return Err(LayoutError::InvalidPage);
        }
        Ok(geometry)
    }
    pub(crate) fn content_width(self) -> Emu {
        Emu::new(self.width.raw() - self.left.raw() - self.right.raw())
    }
    pub(crate) fn bottom_edge(self) -> Emu {
        Emu::new(self.height.raw() - self.bottom.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaretStop {
    pub offset: ScalarOffset,
    pub x: Emu,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedLine {
    pub paragraph: NodeId,
    pub source: SourceRange,
    pub page: usize,
    pub run: usize,
    pub top: Emu,
    pub baseline: Emu,
    pub height: Emu,
    pub carets: Vec<CaretStop>,
    /// Images this line painted into `pages[page].images`.
    pub images: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaretGeometry {
    pub page: usize,
    pub x: Emu,
    pub top: Emu,
    pub height: Emu,
    pub baseline: Emu,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRect {
    pub page: usize,
    pub x: Emu,
    pub top: Emu,
    pub width: Emu,
    pub height: Emu,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutDiagnostic {
    pub paragraph: NodeId,
    pub diagnostic: LineDiagnostic,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutStats {
    pub shape_calls: usize,
    pub shaped_bytes: usize,
    pub glyphs: usize,
    /// Fitted paragraphs reused from the paragraph cache (T208).
    pub cache_hits: usize,
    /// Paragraph index whose placement, and everything after it, was copied
    /// because the page boundary matched the previous snapshot.
    pub stable_boundary: Option<usize>,
}

#[derive(Clone, Debug)]
struct ParagraphMap {
    len: usize,
    lines: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct LayoutSnapshot {
    key: LayoutKey,
    story: StoryId,
    geometry: PageGeometry,
    pages: Vec<PageScene>,
    /// One entry per page: the section's page box, including margins.
    page_geometry: Vec<PageGeometry>,
    /// Section index owning each page.
    page_section: Vec<usize>,
    notes: Vec<LayoutNote>,
    lines: Vec<PlacedLine>,
    paragraphs: Vec<ParagraphMap>,
    by_id: BTreeMap<NodeId, usize>,
    diagnostics: Vec<LayoutDiagnostic>,
    /// Line diagnostics dropped after the diagnostic cap. Not a block count.
    omitted: usize,
    /// Body `Block::Opaque` values skipped by this layout. They occupy no
    /// geometry and are not diagnostics.
    omitted_blocks: usize,
    stats: LayoutStats,
    limits: LayoutLimits,
}

pub(crate) fn unsupported(node: Option<NodeId>, feature: &'static str) -> LayoutError {
    LayoutError::Unsupported { node, feature }
}

fn default_tab_interval(document: &Document) -> Result<i64, LayoutError> {
    match document.settings().default_tab_stop {
        None => Ok(720 * Emu::PER_TWIP),
        Some(measure) => measure
            .absolute()
            .map(|length| length.raw())
            .filter(|length| *length > 0)
            .ok_or(unsupported(None, "relative default tab stop")),
    }
}
fn add(a: Emu, b: Emu) -> Result<Emu, LayoutError> {
    a.checked_add(b).ok_or(LayoutError::GeometryOverflow)
}
// Hash rendered source incrementally, not a potentially huge formatted model
// dump. Preflight checks active styles before this; unused metadata is not a
// layout input. Lengths delimit text even when it contains debug-like syntax.
pub(crate) fn fingerprint(document: &Document) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(document.body_id().get().to_le_bytes());
    for section in document.sections() {
        hash.update(section.id.get().to_le_bytes());
        hash.update(format!("{:?}", section.page).as_bytes());
        hash.update(format!("{:?}", section.title_page).as_bytes());
        hash.update(format!("{:?}", section.headers).as_bytes());
        hash.update(format!("{:?}", section.footers).as_bytes());
    }
    hash.update(format!("{:?}", document.settings().even_and_odd_headers).as_bytes());
    for story in document.stories() {
        if story.id == document.body_id() {
            continue;
        }
        hash.update(story.id.get().to_le_bytes());
        hash.update(format!("{:?}", story.kind).as_bytes());
        for block in story.blocks.iter() {
            let Some(paragraph) = block.as_paragraph() else {
                continue;
            };
            let text = crate::pages::marked_text(document, paragraph);
            hash.update((text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
        }
    }
    hash.update((document.body().blocks.len() as u64).to_le_bytes());
    for block in document.body().blocks.iter() {
        match block {
            Block::Paragraph(paragraph) => hash_body_paragraph(&mut hash, document, paragraph),
            Block::Table(table) => hash_table(&mut hash, document, table),
            Block::Opaque(opaque) => {
                // No geometry, but a different opaque block must not reuse a
                // snapshot of the paragraphs around it.
                hash.update(b"opaque");
                hash.update(opaque.id.get().to_le_bytes());
                hash.update((opaque.preserved.element.len() as u64).to_le_bytes());
                hash.update(opaque.preserved.element.as_bytes());
                hash.update(opaque.preserved.handle.get().to_le_bytes());
            }
            Block::Sdt(_) => {}
        }
    }
    // An empty sheet keeps the historical fingerprint. A style, a default or
    // a theme changes what the scene paints, so it joins the hash.
    if stylesheet_is_layout_input(document) {
        let styles = document.styles();
        hash.update(b"styles");
        hash.update(format!("{:?}", styles.defaults).as_bytes());
        hash.update(format!("{:?}", styles.theme()).as_bytes());
        for style in styles.iter() {
            hash.update(style.id.as_str().as_bytes());
            hash.update(format!("{:?}", style.kind).as_bytes());
            hash.update(
                style
                    .based_on
                    .as_ref()
                    .map(|id| id.as_str())
                    .unwrap_or("")
                    .as_bytes(),
            );
            hash.update(
                style
                    .link
                    .as_ref()
                    .map(|id| id.as_str())
                    .unwrap_or("")
                    .as_bytes(),
            );
            hash.update([u8::from(style.is_default)]);
            hash.update(format!("{:?}", style.para).as_bytes());
            hash.update(format!("{:?}", style.run).as_bytes());
        }
        for paragraph in flow_paragraphs(document) {
            hash.update(
                paragraph
                    .style
                    .as_ref()
                    .map(|id| id.as_str())
                    .unwrap_or("")
                    .as_bytes(),
            );
            for run in paragraph.runs() {
                hash.update(
                    run.style
                        .as_ref()
                        .map(|id| id.as_str())
                        .unwrap_or("")
                        .as_bytes(),
                );
                hash.update(format!("{:?}", run.props).as_bytes());
            }
        }
    }
    if !document.numbering().is_empty() {
        hash.update(b"numbering");
        hash.update(format!("{:?}", document.numbering()).as_bytes());
    }
    if let Some(tab) = document.settings().default_tab_stop {
        hash.update(b"defaultTab");
        hash.update(format!("{tab}").as_bytes());
    }
    hash.finalize().into()
}

fn stylesheet_is_layout_input(document: &Document) -> bool {
    let styles = document.styles();
    !styles.is_empty()
        || styles.defaults != DocDefaults::default()
        || !styles.theme().is_empty()
        || flow_paragraphs(document).iter().any(|paragraph| {
            paragraph.style.is_some() || paragraph.runs().iter().any(|run| run.style.is_some())
        })
}

fn hash_body_paragraph(hash: &mut Sha256, document: &Document, paragraph: &Paragraph) {
    let text = crate::pages::marked_text(document, paragraph);
    hash.update(paragraph.id().get().to_le_bytes());
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
    hash.update(format!("{:?}", paragraph.props).as_bytes());
}

fn hash_table(hash: &mut Sha256, document: &Document, table: &Table) {
    hash.update(b"table");
    hash.update(table.id.get().to_le_bytes());
    hash.update((table.grid.len() as u64).to_le_bytes());
    for column in &table.grid {
        hash.update(format!("{column}").as_bytes());
    }
    hash.update(format!("{:?}", table.width).as_bytes());
    hash.update(format!("{:?}", table.cell_margin).as_bytes());
    for row in &table.rows {
        hash.update(row.grid_before.to_le_bytes());
        hash.update(row.grid_after.to_le_bytes());
        hash.update([u8::from(row.header)]);
        hash.update([u8::from(row.cant_split)]);
        for cell in &row.cells {
            hash.update(cell.grid_span.to_le_bytes());
            hash.update(format!("{:?}", cell.vertical_merge).as_bytes());
            hash.update(format!("{:?}", cell.borders).as_bytes());
            hash.update(format!("{:?}", cell.shading).as_bytes());
            for block in cell.blocks.iter() {
                if let Some(paragraph) = block.as_paragraph() {
                    hash_body_paragraph(hash, document, paragraph);
                    for run in paragraph.runs() {
                        hash.update(format!("{:?}", run.props).as_bytes());
                    }
                }
            }
        }
    }
}

/// Body paragraphs, then each table cell's paragraphs, in document order.
/// A paragraph-only body yields the same sequence the fingerprint used before
/// tables existed.
fn flow_paragraphs(document: &Document) -> Vec<&Paragraph> {
    let mut paragraphs = Vec::new();
    for block in document.body().blocks.iter() {
        match block {
            Block::Paragraph(paragraph) => paragraphs.push(paragraph.as_ref()),
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                    for inner in cell.blocks.iter() {
                        if let Some(paragraph) = inner.as_paragraph() {
                            paragraphs.push(paragraph);
                        }
                    }
                }
            }
            Block::Sdt(_) | Block::Opaque(_) => {}
        }
    }
    paragraphs
}

fn table_paragraphs(table: &Table) -> Result<Vec<&Paragraph>, LayoutError> {
    let mut paragraphs = Vec::new();
    for row in &table.rows {
        for cell in &row.cells {
            for block in cell.blocks.iter() {
                let Some(paragraph) = block.as_paragraph() else {
                    return Err(unsupported(
                        Some(block.id()),
                        "nested table or unsupported cell content",
                    ));
                };
                if !paragraph.inlines().is_empty() {
                    return Err(unsupported(
                        Some(paragraph.id()),
                        "inline objects or breaks in a table cell",
                    ));
                }
                paragraphs.push(paragraph);
            }
        }
    }
    Ok(paragraphs)
}

fn check_paragraph(
    document: &Document,
    paragraph: &Paragraph,
    profile: SingleFontProfile,
) -> Result<(), LayoutError> {
    crate::pages::accept_body_inlines(document, paragraph)?;
    style_paint::paint(document, paragraph, profile).map(|_| ())
}

pub(crate) fn preflight(
    document: &Document,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    cancellation: &LayoutCancellation,
) -> Result<PageGeometry, LayoutError> {
    if document.body().blocks.len() > limits.max_paragraphs {
        return Err(LayoutError::LimitExceeded(LayoutLimit::Paragraphs));
    }
    let plans = crate::pages::section_plans(document)?;
    let metadata = document
        .styles()
        .len()
        .saturating_add(document.resources().len())
        .saturating_add(document.annotations().count());
    if metadata > limits.max_metadata_items {
        return Err(LayoutError::LimitExceeded(LayoutLimit::Metadata));
    }
    if document.annotations().next().is_some() {
        return Err(unsupported(None, "annotation or field projection"));
    }
    if document.settings().preserved.is_some() {
        return Err(unsupported(None, "preserved settings"));
    }
    let mut total = 0usize;
    let mut runs = 0usize;
    let mut seen = 0usize;
    for block in document.body().blocks.iter() {
        cancellation.check()?;
        let paragraphs = match block {
            Block::Paragraph(paragraph) => vec![paragraph.as_ref()],
            Block::Table(table) => table_paragraphs(table)?,
            Block::Opaque(_) => continue,
            Block::Sdt(_) => {
                return Err(unsupported(Some(block.id()), "non-paragraph block"));
            }
        };
        for p in paragraphs {
            seen += 1;
            if seen > limits.max_paragraphs {
                return Err(LayoutError::LimitExceeded(LayoutLimit::Paragraphs));
            }
            total = total
                .checked_add(p.text().summary().bytes)
                .ok_or(LayoutError::LimitExceeded(LayoutLimit::TextBytes))?;
            if total > limits.max_text_bytes {
                return Err(LayoutError::LimitExceeded(LayoutLimit::TextBytes));
            }
            runs = runs.saturating_add(p.runs().len());
            if runs > limits.max_runs {
                return Err(LayoutError::LimitExceeded(LayoutLimit::Runs));
            }
            check_paragraph(document, p, profile)?;
        }
    }
    crate::pages::accept_stories(document)?;
    if document.validate().has_errors() {
        return Err(LayoutError::InvalidDocument);
    }
    let geometry = plans
        .first()
        .map(|plan| plan.geometry)
        .ok_or(LayoutError::InvalidDocument)?;
    for plan in &plans {
        if profile.leading.raw() > plan.geometry.bottom_edge().raw() - plan.geometry.top.raw() {
            return Err(LayoutError::LineTooTall);
        }
    }
    Ok(geometry)
}

// Two bounded passes: fit each paragraph once under shared budgets, then
// annotate only the start of each maximal keep-next chain. Placement consumes
// the fitted lines (no reshaping, rollback, or quadratic suffix lookahead).
#[derive(Clone, Debug)]
pub(crate) struct FittedParagraph {
    pub id: NodeId,
    pub len: usize,
    pub lines: Vec<LayoutLine>,
    pub diagnostics: Vec<LineDiagnostic>,
    pub omitted: usize,
    pub shape_calls: usize,
    pub shaped_bytes: usize,
    pub glyphs: usize,
    pub before: Emu,
    pub after: Emu,
    pub line_box: Emu,
    /// Before + all line boxes, excluding trailing after-spacing.
    pub height: Emu,
    pub group_height: Option<Emu>,
    pub keep_next: bool,
    pub keep_lines: bool,
    pub page_break_before: bool,
    pub justification: Option<Justification>,
    /// Baseline offset inside the line box, after style size scaling.
    pub baseline: Emu,
    pub geometry: PageGeometry,
    pub section_index: usize,
    /// Start a new page before this paragraph when the current page already
    /// holds body runs. Set for the first paragraph of a later section.
    pub section_break_before: bool,
    /// Explicit `w:br w:type="page"` counts before each piece. Empty means
    /// the paragraph has no explicit page breaks.
    pub piece_breaks: Vec<u32>,
    pub piece_lines: Vec<usize>,
    /// Page breaks after the last text piece. One trailing break moves the
    /// next paragraph; further breaks insert blank pages.
    pub trailing_breaks: u32,
    /// Page x of the text frame. Body paragraphs use the content left edge.
    pub frame_x: Emu,
    /// Width available for this paragraph's lines. Body paragraphs use the
    /// section content width; a cell uses its inner width.
    pub frame_width: Emu,
    /// Keep the caller's y. Cell paragraphs set this so a row is not moved
    /// back to the page top and is not split onto the next page.
    pub pinned: bool,
}

/// Where a paragraph began in a previous snapshot. `top` is the y before
/// that paragraph's spacing-before is applied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParagraphAnchor {
    pub line: usize,
    pub page: usize,
    pub top: i64,
}

/// Suffix/prefix of a previous snapshot that incremental placement may copy
/// once a page boundary matches.
pub(crate) struct PlaceStop<'a> {
    pub previous: &'a LayoutSnapshot,
    pub starts: &'a [ParagraphAnchor],
    /// `same[i]` is true when paragraph i has the same identity and fit key
    /// as the previous snapshot, and was not invalidated.
    pub same: &'a [bool],
    pub place_from: usize,
}

fn prepare_keep_groups(
    paragraphs: &mut [FittedParagraph],
    cancellation: &LayoutCancellation,
) -> Result<(), LayoutError> {
    let mut start = 0;
    while start < paragraphs.len() {
        cancellation.check()?;
        let mut end = start;
        let mut height = paragraphs[start].height;
        while end + 1 < paragraphs.len()
            && paragraphs[end].keep_next
            && paragraphs[end].trailing_breaks == 0
            && !paragraphs[end + 1].page_break_before
            && !paragraphs[end + 1].section_break_before
            && paragraphs[end + 1]
                .piece_breaks
                .first()
                .copied()
                .unwrap_or(0)
                == 0
        {
            cancellation.check()?;
            height = add(
                add(height, paragraphs[end].after)?,
                paragraphs[end + 1].height,
            )?;
            end += 1;
        }
        if end > start {
            paragraphs[start].group_height = Some(height);
        }
        start = end + 1;
    }
    Ok(())
}

fn line_box_for(
    paragraph: NodeId,
    font: &LoadedFont,
    profile: SingleFontProfile,
    line: Option<LineSpacing>,
    snap_to_grid: Option<bool>,
) -> Result<Emu, LayoutError> {
    match line {
        None => Ok(profile.leading),
        Some(LineSpacing::Auto(240)) => {
            if cjk::face_cjk_family(font).is_none_or(|family| !cjk::measured_line_family(family)) {
                return Err(unsupported(
                    Some(paragraph),
                    "single spacing for an unmeasured font",
                ));
            }
            let metrics = cjk::metric_set(font)
                .map_err(|_| unsupported(Some(paragraph), "single-spacing font metrics"))?;
            let snap = snap_to_grid != Some(false);
            let grid = cjk::measured_line_grid(snap).map_err(|_| LayoutError::GeometryOverflow)?;
            let resolved =
                cjk::single_line_height(profile.size, cjk::word_line_span(&metrics), grid)
                    .map_err(|_| unsupported(Some(paragraph), "single-spacing font metrics"))?;
            fit_line_box(profile.baseline, resolved.length)
        }
        // Other multiples scale this profile's leading by n/240 with integer
        // division. That is not a measured Word line grid.
        Some(LineSpacing::Auto(multiple)) if multiple > 0 => {
            let length = scale_leading(profile.leading.raw(), multiple, 240)?;
            fit_line_box(profile.baseline, Emu::new(length))
        }
        Some(LineSpacing::Exact(measure)) => {
            let length = absolute_line(paragraph, measure)?;
            fit_line_box(profile.baseline, length)
        }
        Some(LineSpacing::AtLeast(measure)) => {
            let floor = absolute_line(paragraph, measure)?;
            let length = if floor.raw() > profile.leading.raw() {
                floor
            } else {
                profile.leading
            };
            fit_line_box(profile.baseline, length)
        }
        Some(_) => Err(unsupported(Some(paragraph), "paragraph line spacing")),
    }
}

fn fit_line_box(baseline: Emu, length: Emu) -> Result<Emu, LayoutError> {
    if length.raw() <= 0 || baseline.raw() > length.raw() {
        Err(LayoutError::LineTooTall)
    } else {
        Ok(length)
    }
}

fn scale_leading(leading: i64, multiple: i64, unit: i64) -> Result<i64, LayoutError> {
    let product = i128::from(leading)
        .checked_mul(i128::from(multiple))
        .ok_or(LayoutError::GeometryOverflow)?;
    let value = product / i128::from(unit);
    i64::try_from(value).map_err(|_| LayoutError::GeometryOverflow)
}

fn absolute_line(paragraph: NodeId, measure: Measure) -> Result<Emu, LayoutError> {
    match measure.absolute() {
        Some(length) if length.raw() >= 0 => Ok(length),
        _ => Err(unsupported(Some(paragraph), "paragraph line spacing")),
    }
}

fn r3_mark(ch: char) -> bool {
    !ch.is_ascii()
        && (cjk::prohibits_line_start(ch)
            || cjk::prohibits_line_end(ch)
            || matches!(ch, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}'))
}

fn paragraph_is_pure_chinese(lines: &[LayoutLine]) -> bool {
    let mut han = false;
    for line in lines {
        for ch in line.run.text.chars() {
            if cjk::script_run(ch) == cjk::ScriptRun::Han {
                han = true;
            } else if !r3_mark(ch) {
                return false;
            }
        }
    }
    han
}

fn justify_pure_chinese(
    lines: &mut [LayoutLine],
    content_width: Emu,
    paragraph: NodeId,
) -> Result<(), LayoutError> {
    if !paragraph_is_pure_chinese(lines) {
        return Err(unsupported(
            Some(paragraph),
            "inter-character justification",
        ));
    }
    let last = lines.len().saturating_sub(1);
    for (index, line) in lines.iter_mut().enumerate() {
        if line.run.text.is_empty() {
            continue;
        }
        let chars: Vec<usize> = line.run.text.char_indices().map(|(byte, _)| byte).collect();
        if chars.len() != line.run.glyphs.len()
            || chars
                .iter()
                .zip(&line.run.glyphs)
                .any(|(byte, glyph)| glyph.cluster != *byte)
        {
            return Err(unsupported(
                Some(paragraph),
                "inter-character justification",
            ));
        }
        let advances: Vec<Emu> = line.run.glyphs.iter().map(|glyph| glyph.advance).collect();
        let gaps = match cjk::justify_intercharacter(&advances, content_width, index == last) {
            Ok(justified) => justified.gaps,
            Err(CjkIssue::UnverifiedJustify | CjkIssue::DoesNotFit) => continue,
            Err(CjkIssue::BudgetExceeded) => {
                return Err(unsupported(Some(paragraph), "justification budget"));
            }
            Err(_) => return Err(LayoutError::GeometryOverflow),
        };
        let mut pen = 0i64;
        let glyph_count = line.run.glyphs.len();
        for (glyph_index, glyph) in line.run.glyphs.iter_mut().enumerate() {
            let gap = if glyph_index + 1 == glyph_count {
                0
            } else {
                gaps[glyph_index].raw()
            };
            let advance = glyph
                .advance
                .raw()
                .checked_add(gap)
                .ok_or(LayoutError::GeometryOverflow)?;
            glyph.x = Emu::new(pen);
            glyph.advance = Emu::new(advance);
            pen = pen
                .checked_add(advance)
                .ok_or(LayoutError::GeometryOverflow)?;
        }
        line.run.width = Emu::new(pen);
    }
    Ok(())
}

/// Lay out a complete supported document; any failure discards all candidate
/// pages. Callers retain their previous snapshot and keep editing in engine.
/// Single-font defaults are explicit, not guessed from a system font name.
///
/// Direct keep-next chains include the terminal paragraph and end at an
/// explicit page break (which takes priority). A chain that fits is kept
/// whole; an oversized chain starts on a fresh page and flows forward, still
/// respecting each member's keep-lines. Oversized keep-lines paragraphs also
/// start fresh and split. At page top neither rule inserts another empty page.
/// A final keep-next without a successor does nothing. Spacing keeps the C05
/// additive preview policy: before once, after between paragraphs, no reserved
/// trailing after-space in a keep group. These bounded fallback rules are
/// tested independently; they are not a claim of Word/WPS interoperability.
pub fn layout_document(
    document: &DocumentSnapshot,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    cancellation: &LayoutCancellation,
) -> Result<LayoutSnapshot, LayoutError> {
    cancellation.check()?;
    profile.validate()?;
    let limits = limits.capped();
    let geometry = preflight(document, profile, limits, cancellation)?;
    let plans = crate::pages::section_plans(document)?;
    let mut result = open_snapshot(document, faces, profile, limits, geometry)?;
    let marks = joyword_model::list_marks(document);
    if document
        .body()
        .blocks
        .iter()
        .any(|block| matches!(block, Block::Table(_)))
    {
        return layout_mixed(
            result,
            document,
            faces,
            profile,
            limits,
            cancellation,
            &marks,
            &plans,
        );
    }
    let mut prepared = Vec::new();
    let mut total_lines = 0usize;
    let mut charged_shapes = 0usize;
    let mut charged_bytes = 0usize;
    let mut charged_glyphs = 0usize;
    for (index, block) in document.body().blocks.iter().enumerate() {
        cancellation.check()?;
        let Some(paragraph) = block.as_paragraph() else {
            if matches!(block, Block::Opaque(_)) {
                result.omit_opaque_block();
                continue;
            }
            return Err(unsupported(Some(block.id()), "non-paragraph block"));
        };
        let plan = crate::pages::plan_for(&plans, index).ok_or(LayoutError::InvalidDocument)?;
        let mut fitted = fit_paragraph(
            document,
            paragraph,
            faces,
            profile,
            plan.geometry,
            plan.geometry.content_width(),
            limits,
            WorkSpent {
                shapes: charged_shapes,
                bytes: charged_bytes,
                glyphs: charged_glyphs,
                lines: total_lines,
            },
            cancellation,
            marks.get(&paragraph.id()),
        )?;
        fitted.geometry = plan.geometry;
        fitted.section_index = plan.section;
        fitted.section_break_before = crate::pages::section_break_before(&plans, index);
        record_fit(&mut result, &fitted);
        charged_shapes += fitted.shape_calls;
        charged_bytes += fitted.shaped_bytes;
        charged_glyphs += fitted.glyphs;
        total_lines += fitted.lines.len();
        result.stats.shape_calls += fitted.shape_calls;
        result.stats.shaped_bytes += fitted.shaped_bytes;
        result.stats.glyphs += fitted.glyphs;
        prepared.push(fitted);
    }
    let mut snapshot = assemble(result, prepared, cancellation, None)?;
    crate::pages::paint_stories(&mut snapshot, document, faces, profile, cancellation)?;
    Ok(snapshot)
}

/// Paragraph batches keep the page and header assembler. Table rows share
/// that page stream: a row may split, a `cantSplit` row moves whole, and a
/// vertical merge is one visual span.
#[allow(clippy::too_many_arguments)]
fn layout_mixed(
    mut result: LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    cancellation: &LayoutCancellation,
    marks: &BTreeMap<NodeId, Result<NumberLabel, NumberingProblem>>,
    plans: &[crate::pages::SectionPlan],
) -> Result<LayoutSnapshot, LayoutError> {
    let mut spent = WorkSpent {
        shapes: 0,
        bytes: 0,
        glyphs: 0,
        lines: 0,
    };
    let mut batch = Vec::new();
    let mut top = result.geometry.top;
    for (index, block) in document.body().blocks.iter().enumerate() {
        cancellation.check()?;
        let plan = crate::pages::plan_for(plans, index).ok_or(LayoutError::InvalidDocument)?;
        match block {
            Block::Paragraph(paragraph) => {
                let mut fitted = fit_paragraph(
                    document,
                    paragraph,
                    faces,
                    profile,
                    plan.geometry,
                    plan.geometry.content_width(),
                    limits,
                    WorkSpent {
                        shapes: spent.shapes,
                        bytes: spent.bytes,
                        glyphs: spent.glyphs,
                        lines: spent.lines,
                    },
                    cancellation,
                    marks.get(&paragraph.id()),
                )?;
                fitted.geometry = plan.geometry;
                fitted.section_index = plan.section;
                fitted.section_break_before = crate::pages::section_break_before(plans, index);
                note_fit(&mut result, &fitted, &mut spent);
                batch.push(fitted);
            }
            Block::Table(table) => {
                place_paragraph_batch(&mut result, &mut top, &mut batch, cancellation)?;
                place_table(
                    &mut result,
                    &mut top,
                    document,
                    table,
                    faces,
                    profile,
                    plan.geometry,
                    plan.section,
                    crate::pages::section_break_before(plans, index),
                    limits,
                    &mut spent,
                    cancellation,
                    marks,
                )?;
            }
            Block::Opaque(_) => result.omit_opaque_block(),
            Block::Sdt(_) => {
                return Err(unsupported(Some(block.id()), "non-paragraph block"));
            }
        }
    }
    place_paragraph_batch(&mut result, &mut top, &mut batch, cancellation)?;
    crate::pages::paint_stories(&mut result, document, faces, profile, cancellation)?;
    cancellation.check()?;
    Ok(result)
}

fn note_fit(result: &mut LayoutSnapshot, fitted: &FittedParagraph, spent: &mut WorkSpent) {
    record_fit(result, fitted);
    spent.shapes += fitted.shape_calls;
    spent.bytes += fitted.shaped_bytes;
    spent.glyphs += fitted.glyphs;
    spent.lines += fitted.lines.len();
    result.stats.shape_calls += fitted.shape_calls;
    result.stats.shaped_bytes += fitted.shaped_bytes;
    result.stats.glyphs += fitted.glyphs;
}

fn place_paragraph_batch(
    result: &mut LayoutSnapshot,
    top: &mut Emu,
    batch: &mut Vec<FittedParagraph>,
    cancellation: &LayoutCancellation,
) -> Result<(), LayoutError> {
    if batch.is_empty() {
        return Ok(());
    }
    prepare_keep_groups(batch, cancellation)?;
    for paragraph in batch.iter_mut() {
        place_one(result, top, paragraph, cancellation)?;
    }
    batch.clear();
    Ok(())
}

const DEFAULT_CELL_MARGIN_LEFT: i64 = 108;
const DEFAULT_CELL_MARGIN_RIGHT: i64 = 108;

/// Omitted `w:tblCellMar` uses the application default from slice 1: left and
/// right 108 twips, top and bottom 0. ECMA-376 §17.4.43 leaves an omitted
/// element to the table style or the application. Table styles are not
/// resolved here, and 108 twips is not a Word observation.
fn cell_pad(
    margin: Option<CellMargins>,
    table: NodeId,
) -> Result<(i64, i64, i64, i64), LayoutError> {
    let Some(margin) = margin else {
        return Ok((0, DEFAULT_CELL_MARGIN_LEFT, 0, DEFAULT_CELL_MARGIN_RIGHT));
    };
    for value in [margin.top, margin.left, margin.bottom, margin.right] {
        if value < 0 {
            return Err(unsupported(Some(table), "table cell margin is negative"));
        }
    }
    Ok((margin.top, margin.left, margin.bottom, margin.right))
}

struct CellFlow {
    paragraphs: Vec<FittedParagraph>,
    content: i64,
    top_border: i64,
    bottom_border: i64,
    left_border: i64,
    right_border: i64,
    margin_top: i64,
    margin_bottom: i64,
    outer_x: i64,
    outer_width: i64,
    merge: VerticalMerge,
    start_row: usize,
    span_rows: usize,
    line_box: i64,
    shading: Option<(u8, u8, u8)>,
    border_top: Option<BorderEdge>,
    border_bottom: Option<BorderEdge>,
    border_left: Option<BorderEdge>,
    border_right: Option<BorderEdge>,
}

struct RowFlow {
    cells: Vec<CellFlow>,
    height: i64,
    cant_split: bool,
    header: bool,
    id: NodeId,
}

#[derive(Clone)]
struct LineJob {
    row: usize,
    cell: usize,
    paragraph: usize,
    line: usize,
    page: usize,
    top: i64,
}

struct HeaderStamp {
    valid: bool,
    page: usize,
    origin_y: i64,
    height: i64,
    rects: Vec<SceneRect>,
    runs: Vec<GlyphRun>,
    images: Vec<SceneImage>,
}

#[allow(clippy::too_many_arguments)]
fn place_table(
    result: &mut LayoutSnapshot,
    top: &mut Emu,
    document: &Document,
    table: &Table,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    geometry: PageGeometry,
    section: usize,
    section_break_before: bool,
    limits: LayoutLimits,
    spent: &mut WorkSpent,
    cancellation: &LayoutCancellation,
    marks: &BTreeMap<NodeId, Result<NumberLabel, NumberingProblem>>,
) -> Result<(), LayoutError> {
    let mut rows = build_row_flows(
        result,
        document,
        table,
        faces,
        profile,
        geometry,
        section,
        limits,
        spent,
        cancellation,
        marks,
    )?;
    assign_row_heights(&mut rows)?;
    let tops = row_tops(&rows);
    let page_has_ink = result.pages.last().is_some_and(scene_has_ink);
    if !page_has_ink {
        result.retarget_open_page(geometry, section);
        *top = geometry.top;
    } else if section_break_before {
        result.new_page(geometry, section)?;
        *top = geometry.top;
    }
    let mut y = top.raw();
    let mut jobs = Vec::new();
    let mut stamp = HeaderStamp {
        valid: false,
        page: 0,
        origin_y: 0,
        height: 0,
        rects: Vec::new(),
        runs: Vec::new(),
        images: Vec::new(),
    };
    let mut repeats = Vec::new();
    flow_rows(
        result,
        table.id,
        &rows,
        &tops,
        &mut y,
        &mut jobs,
        &mut stamp,
        &mut repeats,
        geometry,
        section,
    )?;
    flush_jobs(result, &rows, &jobs, &mut stamp, &repeats)?;
    *top = Emu::new(y);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_row_flows(
    result: &mut LayoutSnapshot,
    document: &Document,
    table: &Table,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    geometry: PageGeometry,
    section: usize,
    limits: LayoutLimits,
    spent: &mut WorkSpent,
    cancellation: &LayoutCancellation,
    marks: &BTreeMap<NodeId, Result<NumberLabel, NumberingProblem>>,
) -> Result<Vec<RowFlow>, LayoutError> {
    let (pad_top, pad_left, pad_bottom, pad_right) = cell_pad(table.cell_margin, table.id)?;
    let mut edges = Vec::with_capacity(table.grid.len() + 1);
    let mut cursor = 0i64;
    edges.push(0);
    for column in &table.grid {
        cursor = cursor
            .checked_add(grid_twips(table.id, *column)?)
            .ok_or(LayoutError::GeometryOverflow)?;
        edges.push(cursor);
    }
    let margin_top = twips_to_emu(pad_top)?;
    let margin_bottom = twips_to_emu(pad_bottom)?;
    let margin_left = twips_to_emu(pad_left)?;
    let mut rows = Vec::with_capacity(table.rows.len());
    for (row_index, row) in table.rows.iter().enumerate() {
        cancellation.check()?;
        let mut column = row.grid_before as usize;
        let mut cells = Vec::new();
        for cell in &row.cells {
            let start = column;
            column = column.saturating_add(cell.grid_span as usize);
            if start >= edges.len() || column >= edges.len() {
                return Err(unsupported(
                    Some(table.id),
                    "table cell is outside the grid",
                ));
            }
            for block in cell.blocks.iter() {
                if block.as_paragraph().is_none() {
                    return Err(unsupported(
                        Some(block.id()),
                        "nested table or unsupported cell content",
                    ));
                }
            }
            let outer = edges[column] - edges[start];
            let inner = outer - pad_left - pad_right;
            if inner <= 0 {
                return Err(unsupported(
                    Some(cell.id),
                    "cell is narrower than its margins",
                ));
            }
            let outer_x = geometry
                .left
                .raw()
                .checked_add(twips_to_emu(edges[start])?)
                .ok_or(LayoutError::GeometryOverflow)?;
            let frame_x = outer_x
                .checked_add(margin_left)
                .ok_or(LayoutError::GeometryOverflow)?;
            let span_rows = vertical_span(table, row_index, start as u32, cell.grid_span);
            if cell.vertical_merge == VerticalMerge::Continue {
                cells.push(CellFlow {
                    paragraphs: Vec::new(),
                    content: 0,
                    top_border: 0,
                    bottom_border: 0,
                    left_border: 0,
                    right_border: 0,
                    margin_top: 0,
                    margin_bottom: 0,
                    outer_x,
                    outer_width: twips_to_emu(outer)?,
                    merge: VerticalMerge::Continue,
                    start_row: row_index,
                    span_rows: 0,
                    line_box: 0,
                    shading: None,
                    border_top: None,
                    border_bottom: None,
                    border_left: None,
                    border_right: None,
                });
                continue;
            }
            let inner_width = Emu::new(twips_to_emu(inner)?);
            let mut paragraphs = Vec::new();
            for block in cell.blocks.iter() {
                let paragraph = block.as_paragraph().ok_or_else(|| {
                    unsupported(Some(block.id()), "nested table or unsupported cell content")
                })?;
                let mut fitted = fit_paragraph(
                    document,
                    paragraph,
                    faces,
                    profile,
                    geometry,
                    inner_width,
                    limits,
                    WorkSpent {
                        shapes: spent.shapes,
                        bytes: spent.bytes,
                        glyphs: spent.glyphs,
                        lines: spent.lines,
                    },
                    cancellation,
                    marks.get(&paragraph.id()),
                )?;
                fitted.geometry = geometry;
                fitted.section_index = section;
                fitted.frame_x = Emu::new(frame_x);
                fitted.frame_width = inner_width;
                fitted.pinned = true;
                fitted.page_break_before = false;
                fitted.keep_lines = false;
                fitted.keep_next = false;
                fitted.section_break_before = false;
                note_fit(result, &fitted, spent);
                paragraphs.push(fitted);
            }
            let line_box = paragraphs
                .iter()
                .map(|paragraph| paragraph.line_box.raw())
                .max()
                .unwrap_or(profile.leading.raw());
            cells.push(CellFlow {
                content: stacked(&paragraphs)?,
                paragraphs,
                top_border: border_size(cell.borders.top),
                bottom_border: border_size(cell.borders.bottom),
                left_border: border_size(cell.borders.left),
                right_border: border_size(cell.borders.right),
                margin_top,
                margin_bottom,
                outer_x,
                outer_width: twips_to_emu(outer)?,
                merge: cell.vertical_merge,
                start_row: row_index,
                span_rows,
                line_box,
                shading: match cell.shading.map(|shading| shading.fill) {
                    Some(Color::Rgb(red, green, blue)) => Some((red, green, blue)),
                    _ => None,
                },
                border_top: cell.borders.top,
                border_bottom: cell.borders.bottom,
                border_left: cell.borders.left,
                border_right: cell.borders.right,
            });
        }
        rows.push(RowFlow {
            cells,
            height: 0,
            cant_split: row.cant_split,
            header: row.header,
            id: row.id,
        });
    }
    Ok(rows)
}

fn stacked(paragraphs: &[FittedParagraph]) -> Result<i64, LayoutError> {
    let mut height = 0i64;
    for paragraph in paragraphs {
        height = height
            .checked_add(paragraph.before.raw())
            .ok_or(LayoutError::GeometryOverflow)?;
        let lines =
            i64::try_from(paragraph.lines.len()).map_err(|_| LayoutError::GeometryOverflow)?;
        let block = paragraph
            .line_box
            .raw()
            .checked_mul(lines)
            .ok_or(LayoutError::GeometryOverflow)?;
        height = height
            .checked_add(block)
            .and_then(|height| height.checked_add(paragraph.after.raw()))
            .ok_or(LayoutError::GeometryOverflow)?;
    }
    Ok(height)
}

fn vertical_span(table: &Table, row_index: usize, column: u32, span: u32) -> usize {
    let Some(cell) = cell_on(&table.rows[row_index], column, span) else {
        return 1;
    };
    if cell.vertical_merge != VerticalMerge::Restart {
        return 1;
    }
    let mut count = 1usize;
    for next in (row_index + 1)..table.rows.len() {
        match cell_on(&table.rows[next], column, span) {
            Some(found) if found.vertical_merge == VerticalMerge::Continue => count += 1,
            _ => break,
        }
    }
    count
}

fn cell_on(row: &joyword_model::Row, column: u32, span: u32) -> Option<&joyword_model::Cell> {
    let mut cursor = row.grid_before;
    for cell in &row.cells {
        if cursor == column && cell.grid_span == span {
            return Some(cell);
        }
        cursor = cursor.saturating_add(cell.grid_span);
    }
    None
}

fn assign_row_heights(rows: &mut [RowFlow]) -> Result<(), LayoutError> {
    for row_index in 0..rows.len() {
        let mut height = 0i64;
        let mut visible = false;
        for cell in &rows[row_index].cells {
            if cell.merge == VerticalMerge::Continue {
                continue;
            }
            visible = true;
            if cell.span_rows <= 1 {
                height = height.max(box_height(cell));
            } else {
                height = height.max(cell.line_box.max(cell.margin_top + cell.top_border));
            }
        }
        if !visible {
            height = height.max(covering_line(rows, row_index));
        }
        if height <= 0 {
            return Err(LayoutError::LineTooTall);
        }
        rows[row_index].height = height;
    }
    for row_index in 0..rows.len() {
        for cell_index in 0..rows[row_index].cells.len() {
            let cell = &rows[row_index].cells[cell_index];
            if cell.merge == VerticalMerge::Continue || cell.span_rows <= 1 {
                continue;
            }
            let last = row_index + cell.span_rows - 1;
            if last >= rows.len() {
                continue;
            }
            let need = box_height(cell);
            let have: i64 = rows[row_index..=last].iter().map(|row| row.height).sum();
            if need > have {
                rows[last].height = rows[last].height.saturating_add(need - have);
            }
        }
    }
    for row_index in 0..rows.len() {
        let tops = row_tops(rows);
        let row_lo = tops[row_index];
        let row_hi = row_lo + rows[row_index].height;
        let mut extra = 0i64;
        for (origin, cell_index) in visuals(rows, row_index) {
            let cell = &rows[origin].cells[cell_index];
            for (paragraph_index, paragraph) in cell.paragraphs.iter().enumerate() {
                for line_index in 0..paragraph.lines.len() {
                    let line_top = line_top(cell, &tops, paragraph_index, line_index);
                    let line_hi = line_top.saturating_add(paragraph.line_box.raw());
                    if line_top >= row_lo && line_top < row_hi && line_hi > row_hi {
                        extra = extra.max(line_hi - row_hi);
                    }
                }
            }
        }
        rows[row_index].height = rows[row_index].height.saturating_add(extra);
    }
    Ok(())
}

fn box_height(cell: &CellFlow) -> i64 {
    cell.margin_top
        .saturating_add(cell.top_border)
        .saturating_add(cell.content)
        .saturating_add(cell.bottom_border)
        .saturating_add(cell.margin_bottom)
}

fn covering_line(rows: &[RowFlow], row_index: usize) -> i64 {
    let mut line = 1i64;
    for row in rows.iter().take(row_index + 1) {
        for cell in &row.cells {
            if cell.merge == VerticalMerge::Continue {
                continue;
            }
            if cell.start_row + cell.span_rows > row_index {
                line = line.max(cell.line_box);
            }
        }
    }
    line
}

fn row_tops(rows: &[RowFlow]) -> Vec<i64> {
    let mut tops = Vec::with_capacity(rows.len());
    let mut y = 0i64;
    for row in rows {
        tops.push(y);
        y = y.saturating_add(row.height);
    }
    tops
}

fn visuals(rows: &[RowFlow], row_index: usize) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (origin, row) in rows.iter().enumerate().take(row_index + 1) {
        for (cell_index, cell) in row.cells.iter().enumerate() {
            if cell.merge == VerticalMerge::Continue || cell.span_rows == 0 {
                continue;
            }
            if origin + cell.span_rows > row_index {
                found.push((origin, cell_index));
            }
        }
    }
    found
}

fn line_top(cell: &CellFlow, tops: &[i64], paragraph_index: usize, line_index: usize) -> i64 {
    let mut y = tops
        .get(cell.start_row)
        .copied()
        .unwrap_or(0)
        .saturating_add(cell.margin_top)
        .saturating_add(cell.top_border);
    for (index, paragraph) in cell.paragraphs.iter().enumerate() {
        y = y.saturating_add(paragraph.before.raw());
        if index == paragraph_index {
            let offset = i64::try_from(line_index).unwrap_or(0);
            return y.saturating_add(paragraph.line_box.raw().saturating_mul(offset));
        }
        let count = i64::try_from(paragraph.lines.len()).unwrap_or(0);
        y = y.saturating_add(paragraph.line_box.raw().saturating_mul(count));
        y = y.saturating_add(paragraph.after.raw());
    }
    y
}

#[allow(clippy::too_many_arguments)]
fn flow_rows(
    result: &mut LayoutSnapshot,
    table: NodeId,
    rows: &[RowFlow],
    tops: &[i64],
    y: &mut i64,
    jobs: &mut Vec<LineJob>,
    stamp: &mut HeaderStamp,
    repeats: &mut Vec<(usize, i64)>,
    geometry: PageGeometry,
    section: usize,
) -> Result<(), LayoutError> {
    let header_count = rows.iter().take_while(|row| row.header).count();
    let content_box = geometry.bottom_edge().raw() - geometry.top.raw();
    let header_height: i64 = rows.iter().take(header_count).map(|row| row.height).sum();
    if header_count > 0 && header_height > content_box {
        result.note(LayoutNote::HeaderTallerThanPage { table });
    }
    let stamp_page = result.pages.len().saturating_sub(1);
    let stamp_rects = result
        .pages
        .last()
        .map(|page| page.rects.len())
        .unwrap_or(0);
    let stamp_origin = *y;
    let mut header_broken = header_count > 0 && header_height > content_box;
    let mut row_noted = vec![false; rows.len()];
    for row_index in 0..rows.len() {
        let mut consumed = 0i64;
        let mut continued = false;
        let mut steps = 0usize;
        while consumed < rows[row_index].height {
            steps += 1;
            if steps > result.limits.max_pages.saturating_add(2) {
                return Err(LayoutError::LimitExceeded(LayoutLimit::Pages));
            }
            let available = (geometry.bottom_edge().raw() - *y).max(0);
            let ink = page_ink(result);
            let remaining = rows[row_index].height - consumed;
            let take = if rows[row_index].cant_split {
                if remaining <= available {
                    remaining
                } else if ink && !continued && remaining <= content_box {
                    *y = continue_page(
                        result,
                        stamp,
                        repeats,
                        geometry,
                        section,
                        rows,
                        row_index,
                        header_count,
                        true,
                        content_box,
                    )?;
                    if row_index < header_count {
                        header_broken = true;
                    }
                    continued = true;
                    continue;
                } else {
                    note_row(result, &mut row_noted, row_index, rows[row_index].id);
                    remaining
                }
            } else {
                let snapped = snap_take(rows, tops, row_index, consumed, available);
                if snapped > 0 {
                    snapped
                } else if ink && !continued {
                    *y = continue_page(
                        result,
                        stamp,
                        repeats,
                        geometry,
                        section,
                        rows,
                        row_index,
                        header_count,
                        false,
                        content_box,
                    )?;
                    if row_index < header_count {
                        header_broken = true;
                    }
                    continued = true;
                    continue;
                } else {
                    note_row(result, &mut row_noted, row_index, rows[row_index].id);
                    next_piece(rows, tops, row_index, consumed)
                }
            };
            let take = take.max(1).min(remaining);
            paint_slice(result, rows, tops, row_index, consumed, take, *y, jobs)?;
            *y = y.saturating_add(take);
            consumed = consumed.saturating_add(take);
            continued = false;
        }
        if row_index + 1 == header_count
            && !header_broken
            && result.pages.len().saturating_sub(1) == stamp_page
        {
            let page = result.pages.last().ok_or(LayoutError::GeometryOverflow)?;
            stamp.rects = page.rects[stamp_rects.min(page.rects.len())..].to_vec();
            stamp.origin_y = stamp_origin;
            stamp.height = header_height;
            stamp.page = stamp_page;
            stamp.valid = header_height > 0 && header_height <= content_box;
        }
    }
    Ok(())
}

fn note_row(result: &mut LayoutSnapshot, noted: &mut [bool], row_index: usize, id: NodeId) {
    if noted.get(row_index).copied().unwrap_or(true) {
        return;
    }
    noted[row_index] = true;
    result.note(LayoutNote::RowTallerThanPage { row: id });
}

fn page_ink(result: &LayoutSnapshot) -> bool {
    result
        .pages
        .last()
        .is_some_and(|page| !page.runs.is_empty() || !page.rects.is_empty())
}

#[allow(clippy::too_many_arguments)]
fn continue_page(
    result: &mut LayoutSnapshot,
    stamp: &HeaderStamp,
    repeats: &mut Vec<(usize, i64)>,
    geometry: PageGeometry,
    section: usize,
    rows: &[RowFlow],
    row_index: usize,
    header_count: usize,
    cant_split: bool,
    content_box: i64,
) -> Result<i64, LayoutError> {
    result.new_page(geometry, section)?;
    let mut y = geometry.top.raw();
    if row_index >= header_count && stamp_fits(stamp, rows, row_index, cant_split, content_box) {
        paint_stamp_rects(result, stamp, y)?;
        repeats.push((result.pages.len() - 1, y));
        y = y.saturating_add(stamp.height);
    }
    Ok(y)
}

fn stamp_fits(
    stamp: &HeaderStamp,
    rows: &[RowFlow],
    row_index: usize,
    cant_split: bool,
    content_box: i64,
) -> bool {
    if !stamp.valid || stamp.height <= 0 {
        return false;
    }
    let need = if cant_split {
        rows[row_index].height
    } else {
        row_line(rows, row_index).max(1)
    };
    stamp.height.saturating_add(need) <= content_box
}

fn row_line(rows: &[RowFlow], row_index: usize) -> i64 {
    visuals(rows, row_index)
        .into_iter()
        .map(|(origin, cell)| rows[origin].cells[cell].line_box)
        .max()
        .unwrap_or(1)
}

fn paint_stamp_rects(
    result: &mut LayoutSnapshot,
    stamp: &HeaderStamp,
    origin: i64,
) -> Result<(), LayoutError> {
    let delta = origin - stamp.origin_y;
    let page = result
        .pages
        .last_mut()
        .ok_or(LayoutError::GeometryOverflow)?;
    for rect in &stamp.rects {
        page.rects.push(SceneRect {
            x: rect.x,
            y: Emu::new(rect.y.raw().saturating_add(delta)),
            width: rect.width,
            height: rect.height,
            red: rect.red,
            green: rect.green,
            blue: rect.blue,
        });
    }
    Ok(())
}

fn snap_take(
    rows: &[RowFlow],
    tops: &[i64],
    row_index: usize,
    consumed: i64,
    available: i64,
) -> i64 {
    let row_lo = tops[row_index];
    let row_hi = row_lo + rows[row_index].height;
    let slice_lo = row_lo + consumed;
    let limit = slice_lo.saturating_add(available);
    let mut boundary = slice_lo;
    let mut blocked = false;
    for (origin, cell_index) in visuals(rows, row_index) {
        let cell = &rows[origin].cells[cell_index];
        for (paragraph_index, paragraph) in cell.paragraphs.iter().enumerate() {
            for line_index in 0..paragraph.lines.len() {
                let top = line_top(cell, tops, paragraph_index, line_index);
                if top < slice_lo || top >= row_hi {
                    continue;
                }
                let line_hi = top.saturating_add(paragraph.line_box.raw());
                if top >= limit {
                    continue;
                }
                if line_hi <= limit.min(row_hi) {
                    boundary = boundary.max(line_hi);
                } else {
                    blocked = true;
                }
            }
        }
    }
    if blocked && boundary == slice_lo {
        return 0;
    }
    if !blocked {
        return (row_hi - slice_lo).min(available.max(0));
    }
    boundary - slice_lo
}

fn next_piece(rows: &[RowFlow], tops: &[i64], row_index: usize, consumed: i64) -> i64 {
    let row_lo = tops[row_index];
    let row_hi = row_lo + rows[row_index].height;
    let slice_lo = row_lo + consumed;
    let mut next = None;
    for (origin, cell_index) in visuals(rows, row_index) {
        let cell = &rows[origin].cells[cell_index];
        for (paragraph_index, paragraph) in cell.paragraphs.iter().enumerate() {
            for line_index in 0..paragraph.lines.len() {
                let top = line_top(cell, tops, paragraph_index, line_index);
                if top < slice_lo || top >= row_hi {
                    continue;
                }
                let end = top.saturating_add(paragraph.line_box.raw()) - slice_lo;
                next = Some(next.map_or(end, |found: i64| found.min(end)));
            }
        }
    }
    next.unwrap_or(row_hi - slice_lo).max(1)
}

#[allow(clippy::too_many_arguments)]
fn paint_slice(
    result: &mut LayoutSnapshot,
    rows: &[RowFlow],
    tops: &[i64],
    row_index: usize,
    consumed: i64,
    take: i64,
    page_y: i64,
    jobs: &mut Vec<LineJob>,
) -> Result<(), LayoutError> {
    let slice_lo = tops[row_index] + consumed;
    let slice_hi = slice_lo + take;
    let page = result.pages.len().saturating_sub(1);
    for (origin, cell_index) in visuals(rows, row_index) {
        let cell = &rows[origin].cells[cell_index];
        let cell_lo = tops[cell.start_row];
        let last = cell.start_row + cell.span_rows - 1;
        let cell_hi = tops.get(last).copied().unwrap_or(cell_lo)
            + rows.get(last).map(|row| row.height).unwrap_or(0);
        paint_fragment(
            result,
            cell,
            page_y,
            take,
            slice_lo == cell_lo,
            slice_hi == cell_hi,
        )?;
        for (paragraph_index, paragraph) in cell.paragraphs.iter().enumerate() {
            for line_index in 0..paragraph.lines.len() {
                let top = line_top(cell, tops, paragraph_index, line_index);
                if top < slice_lo || top >= slice_hi {
                    continue;
                }
                jobs.push(LineJob {
                    row: cell.start_row,
                    cell: cell_index,
                    paragraph: paragraph_index,
                    line: line_index,
                    page,
                    top: page_y + (top - slice_lo),
                });
            }
        }
    }
    Ok(())
}

fn paint_fragment(
    result: &mut LayoutSnapshot,
    cell: &CellFlow,
    y: i64,
    height: i64,
    first: bool,
    last: bool,
) -> Result<(), LayoutError> {
    let page = result
        .pages
        .last_mut()
        .ok_or(LayoutError::GeometryOverflow)?;
    if let Some(color) = cell.shading {
        push_rect(page, cell.outer_x, y, cell.outer_width, height, color);
    }
    if first {
        push_border(
            page,
            cell.border_top,
            cell.outer_x,
            y,
            cell.outer_width,
            cell.top_border,
        );
    }
    if last {
        push_border(
            page,
            cell.border_bottom,
            cell.outer_x,
            y.saturating_add(height).saturating_sub(cell.bottom_border),
            cell.outer_width,
            cell.bottom_border,
        );
    }
    push_border(
        page,
        cell.border_left,
        cell.outer_x,
        y,
        cell.left_border,
        height,
    );
    push_border(
        page,
        cell.border_right,
        cell.outer_x
            .saturating_add(cell.outer_width)
            .saturating_sub(cell.right_border),
        y,
        cell.right_border,
        height,
    );
    Ok(())
}

fn flush_jobs(
    result: &mut LayoutSnapshot,
    rows: &[RowFlow],
    jobs: &[LineJob],
    stamp: &mut HeaderStamp,
    repeats: &[(usize, i64)],
) -> Result<(), LayoutError> {
    let mut ordered = jobs.to_vec();
    ordered.sort_by_key(|job| (job.row, job.cell, job.paragraph, job.line));
    let header_end = ordered.iter().position(|job| !rows[job.row].header);
    let header_jobs = header_end.unwrap_or(ordered.len());
    let (run_before, image_before) = if stamp.valid {
        result
            .pages
            .get(stamp.page)
            .map(|page| (page.runs.len(), page.images.len()))
            .unwrap_or((0, 0))
    } else {
        (0, 0)
    };
    place_jobs(result, rows, &ordered[..header_jobs])?;
    if stamp.valid {
        if let Some(page) = result.pages.get(stamp.page) {
            stamp.runs = page.runs[run_before.min(page.runs.len())..].to_vec();
            stamp.images = page.images[image_before.min(page.images.len())..].to_vec();
        }
        for &(page_index, origin) in repeats {
            let delta = origin - stamp.origin_y;
            let copies: Vec<GlyphRun> = stamp
                .runs
                .iter()
                .map(|run| {
                    let mut copy = run.clone();
                    copy.baseline_y = Emu::new(copy.baseline_y.raw().saturating_add(delta));
                    copy
                })
                .collect();
            let image_copies: Vec<SceneImage> = stamp
                .images
                .iter()
                .map(|image| {
                    let mut copy = image.clone();
                    copy.y = Emu::new(copy.y.raw().saturating_add(delta));
                    copy
                })
                .collect();
            if let Some(page) = result.pages.get_mut(page_index) {
                page.runs.extend(copies);
                page.images.extend(image_copies);
            }
        }
    }
    place_jobs(result, rows, &ordered[header_jobs..])?;
    Ok(())
}

fn place_jobs(
    result: &mut LayoutSnapshot,
    rows: &[RowFlow],
    jobs: &[LineJob],
) -> Result<(), LayoutError> {
    let mut index = 0usize;
    while index < jobs.len() {
        let row = jobs[index].row;
        let cell = jobs[index].cell;
        let paragraph_index = jobs[index].paragraph;
        let fitted = &rows[row].cells[cell].paragraphs[paragraph_index];
        let id = fitted.id;
        let len = fitted.len;
        let frame_x = fitted.frame_x;
        let frame_width = fitted.frame_width;
        let line_box = fitted.line_box;
        let justification = fitted.justification;
        let baseline = fitted.baseline;
        let first = result.lines.len();
        while index < jobs.len()
            && jobs[index].row == row
            && jobs[index].cell == cell
            && jobs[index].paragraph == paragraph_index
        {
            let job = &jobs[index];
            let line = &rows[job.row].cells[job.cell].paragraphs[job.paragraph].lines[job.line];
            let mut top = Emu::new(job.top);
            place_line_on(
                result,
                job.page,
                &mut top,
                line,
                id,
                frame_x,
                frame_width,
                line_box,
                justification,
                baseline,
            )?;
            index += 1;
        }
        let map = result.paragraphs.len();
        result.by_id.insert(id, map);
        result.paragraphs.push(ParagraphMap {
            len,
            lines: first..result.lines.len(),
        });
    }
    Ok(())
}

fn push_border(
    page: &mut PageScene,
    edge: Option<BorderEdge>,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
) {
    let Some(edge) = edge else {
        return;
    };
    if edge.kind != BorderKind::Single {
        return;
    }
    let color = match edge.color {
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Auto | Color::Theme(_) => (0, 0, 0),
    };
    push_rect(page, x, y, width, height, color);
}

fn push_rect(page: &mut PageScene, x: i64, y: i64, width: i64, height: i64, color: (u8, u8, u8)) {
    if width <= 0 || height <= 0 {
        return;
    }
    page.rects.push(SceneRect {
        x: Emu::new(x),
        y: Emu::new(y),
        width: Emu::new(width),
        height: Emu::new(height),
        red: color.0,
        green: color.1,
        blue: color.2,
    });
}

fn grid_twips(table: NodeId, measure: Measure) -> Result<i64, LayoutError> {
    match measure {
        Measure::Twips(value) if value >= 0 => Ok(value),
        _ => Err(unsupported(Some(table), "table grid is not in twips")),
    }
}

fn twips_to_emu(twips: i64) -> Result<i64, LayoutError> {
    twips
        .checked_mul(Emu::PER_TWIP)
        .ok_or(LayoutError::GeometryOverflow)
}

fn border_size(edge: Option<BorderEdge>) -> i64 {
    match edge {
        Some(edge) if edge.kind == BorderKind::Single && edge.size_eighths > 0 => {
            i64::from(edge.size_eighths) * Emu::PER_POINT / 8
        }
        _ => 0,
    }
}

pub(crate) fn open_snapshot(
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    geometry: PageGeometry,
) -> Result<LayoutSnapshot, LayoutError> {
    let mut result = LayoutSnapshot {
        key: LayoutKey {
            version: document.version(),
            font: faces.ascii.key(),
            profile,
            content: fingerprint(document),
        },
        story: document.body_id(),
        geometry,
        pages: Vec::new(),
        page_geometry: Vec::new(),
        page_section: Vec::new(),
        notes: Vec::new(),
        lines: Vec::new(),
        paragraphs: Vec::new(),
        by_id: BTreeMap::new(),
        diagnostics: Vec::new(),
        omitted: 0,
        omitted_blocks: 0,
        stats: LayoutStats::default(),
        limits,
    };
    result.new_page(geometry, 0)?;
    Ok(result)
}

pub(crate) fn dominant_slot(text: &str) -> FontSlot {
    let mut chosen = FontSlot::Ascii;
    for ch in text.chars() {
        match slot_for(ch, None) {
            FontSlot::EastAsia => chosen = FontSlot::EastAsia,
            FontSlot::Complex if chosen != FontSlot::EastAsia => chosen = FontSlot::Complex,
            FontSlot::HAnsi if !matches!(chosen, FontSlot::EastAsia | FontSlot::Complex) => {
                chosen = FontSlot::HAnsi;
            }
            _ => {}
        }
    }
    chosen
}

pub(crate) fn record_fit(result: &mut LayoutSnapshot, fitted: &FittedParagraph) {
    for diagnostic in &fitted.diagnostics {
        if result.diagnostics.len() < result.limits.max_diagnostics {
            result.diagnostics.push(LayoutDiagnostic {
                paragraph: fitted.id,
                diagnostic: diagnostic.clone(),
            });
        } else {
            result.omitted += 1;
        }
    }
    result.omitted += fitted.omitted;
}

pub(crate) struct WorkSpent {
    pub shapes: usize,
    pub bytes: usize,
    pub glyphs: usize,
    pub lines: usize,
}

/// Fit one paragraph. The line fitter keeps its own diagnostic ceiling;
/// [`record_fit`] applies the document cap, so a cached fit does not depend
/// on how many diagnostics an earlier paragraph emitted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_paragraph(
    document: &Document,
    paragraph: &Paragraph,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    geometry: PageGeometry,
    available_width: Emu,
    limits: LayoutLimits,
    spent: WorkSpent,
    cancellation: &LayoutCancellation,
    mark: Option<&Result<NumberLabel, NumberingProblem>>,
) -> Result<FittedParagraph, LayoutError> {
    cancellation.check()?;
    let defaults = LineLimits::default();
    let request_limits = LineLimits {
        max_text_bytes: defaults.max_text_bytes.min(limits.max_text_bytes),
        max_shape_calls: defaults
            .max_shape_calls
            .min(limits.max_shape_calls.saturating_sub(spent.shapes)),
        max_shaped_bytes: defaults
            .max_shaped_bytes
            .min(limits.max_shaped_bytes.saturating_sub(spent.bytes)),
        max_lines: defaults
            .max_lines
            .min(limits.max_lines.saturating_sub(spent.lines)),
        max_diagnostics: defaults.max_diagnostics,
        ..defaults
    };
    // Global limits are charged before each paragraph's work, not after
    // an unbounded collection of independent paragraph requests.
    if request_limits.max_shape_calls == 0 {
        return Err(LayoutError::LimitExceeded(LayoutLimit::ShapeCalls));
    }
    if request_limits.max_shaped_bytes == 0 && !paragraph.is_empty() {
        return Err(LayoutError::LimitExceeded(LayoutLimit::ShapedBytes));
    }
    if request_limits.max_lines == 0 {
        return Err(LayoutError::LimitExceeded(LayoutLimit::Lines));
    }
    let painted = style_paint::paint(document, paragraph, profile)?;
    let shaping = SingleFontProfile {
        size: painted.size,
        leading: painted.leading,
        baseline: painted.baseline,
        features: profile.features,
    };
    let text = paragraph.plain_text(|_| String::new());
    let map_line = |error: LineError| {
        if error == LineError::Cancelled {
            LayoutError::Cancelled
        } else {
            LayoutError::Line {
                paragraph: paragraph.id(),
                error,
            }
        }
    };
    // Four-slot classification, fallback wrapping and CJK prohibitions
    // decide the breaks this page scene stores. Screen and PDF read that
    // scene; they do not break lines again. The shaping face is the slot
    // that owns the paragraph, so a CJK run is not drawn with the Latin face.
    let chosen = dominant_slot(&text);
    let shape_font = faces.face(chosen);
    let line_box = line_box_for(
        paragraph.id(),
        shape_font,
        shaping,
        painted.flow.line,
        painted.flow.snap_to_grid,
    )?;
    let family = cjk::face_cjk_family(shape_font);
    let needs_slots = chosen != FontSlot::Ascii;
    let width = available_width;
    let mut fit_slice = |slice: &str,
                         slice_width: Emu|
     -> Result<crate::ParagraphLines, LayoutError> {
        let mut adjust = |at: usize, run: &mut ShapedRun| {
            cjk::compress_shaped_run(slice, at, run, family).map_err(|_| LineError::InvalidShaping)
        };
        if needs_slots {
            let mixed = wrap_mixed(faces, slice, painted.size, slice_width, None);
            let mut breaks = Vec::new();
            let mut at = 0usize;
            for line in &mixed.lines {
                at += line.text.len();
                if at == 0 || at >= slice.len() {
                    continue;
                }
                let adjusted = adjust_cjk_break(slice, at);
                if adjusted > 0 && adjusted < slice.len() && !breaks.contains(&adjusted) {
                    breaks.push(adjusted);
                }
            }
            breaks.sort_unstable();
            match layout_paragraph_adjusted(
                shape_font,
                slice,
                painted.size,
                profile.features,
                slice_width,
                &breaks,
                LineFit {
                    limits: request_limits,
                    adjust: &mut adjust,
                },
            ) {
                Ok(fitted) => Ok(fitted),
                Err(_) => layout_paragraph_auto_cancellable_adjusted(
                    shape_font,
                    slice,
                    painted.size,
                    profile.features,
                    slice_width,
                    &cancellation.0,
                    LineFit {
                        limits: request_limits,
                        adjust: &mut adjust,
                    },
                )
                .map_err(&map_line),
            }
        } else {
            layout_paragraph_auto_cancellable_adjusted(
                shape_font,
                slice,
                painted.size,
                profile.features,
                slice_width,
                &cancellation.0,
                LineFit {
                    limits: request_limits,
                    adjust: &mut adjust,
                },
            )
            .map_err(&map_line)
        }
    };
    let props = displayed_paragraph(document, paragraph);
    let label = mark.and_then(|item| item.as_ref().ok());
    let numbering_detail = match mark {
        Some(Err(problem)) => Some(problem.to_string()),
        _ => None,
    };
    let has_image = paragraph
        .inlines()
        .iter()
        .any(|inline| matches!(inline.kind, joyword_model::InlineKind::Image { .. }));
    let mut piece_breaks = Vec::new();
    let mut piece_lines = Vec::new();
    let trailing_breaks;
    let mut fitted = if has_image {
        let available = Emu::new(
            (geometry.bottom_edge().raw() - geometry.top.raw() - painted.flow.before.raw()).max(1),
        );
        let descent = Emu::new((line_box.raw() - painted.baseline.raw()).max(0));
        trailing_breaks = 0;
        crate::images::fit_inline_images(
            document,
            paragraph,
            shape_font,
            painted.size,
            width,
            available,
            line_box,
            descent,
            &mut fit_slice,
        )?
    } else {
        let split = crate::pages::split_page_breaks(paragraph);
        let explicit =
            split.trailing_breaks > 0 || split.pieces.iter().any(|piece| piece.breaks_before > 0);
        if explicit && crate::list_layout::needs_list_layout(&props, &text, label) {
            return Err(unsupported(
                Some(paragraph.id()),
                "page break with numbering or tabs",
            ));
        }
        if explicit && split.pieces.is_empty() {
            let count = split.trailing_breaks;
            let mut fitted = fit_slice("", width)?;
            let markers = count as usize;
            let marker_bytes = markers.saturating_mul(joyword_model::OBJECT_MARKER.len_utf8());
            if let Some(line) = fitted.lines.first_mut() {
                line.source.scalars = 0..markers;
                line.source.bytes = 0..marker_bytes;
                line.clusters.clear();
            }
            piece_breaks.push(count);
            piece_lines.push(fitted.lines.len());
            trailing_breaks = 0;
            fitted
        } else if explicit {
            let mut fitted = crate::ParagraphLines {
                lines: Vec::new(),
                diagnostics: Vec::new(),
                omitted_diagnostics: 0,
                stats: crate::LineStats::default(),
            };
            for piece in &split.pieces {
                let mut part = fit_slice(&piece.text, width)?;
                if painted.flow.justification == Some(Justification::Both) {
                    justify_pure_chinese(&mut part.lines, width, paragraph.id())?;
                }
                crate::pages::shift_piece(&mut part.lines, piece);
                piece_breaks.push(piece.breaks_before);
                piece_lines.push(part.lines.len());
                fitted.lines.append(&mut part.lines);
                fitted.diagnostics.append(&mut part.diagnostics);
                fitted.omitted_diagnostics += part.omitted_diagnostics;
                fitted.stats.shape_calls += part.stats.shape_calls;
                fitted.stats.shaped_bytes += part.stats.shaped_bytes;
            }
            crate::pages::extend_trailing(&mut fitted.lines, split.trailing_breaks);
            trailing_breaks = split.trailing_breaks;
            fitted
        } else if crate::list_layout::needs_list_layout(&props, &text, label) {
            if matches!(
                painted.flow.justification,
                Some(Justification::Center | Justification::End | Justification::Both)
            ) {
                return Err(unsupported(
                    Some(paragraph.id()),
                    "justification with indent, numbering or tabs",
                ));
            }
            let interval = default_tab_interval(document)?;
            trailing_breaks = 0;
            crate::list_layout::layout_marked(
                &text,
                &props,
                label,
                width,
                interval,
                shape_font,
                painted.size,
                profile.features,
                &mut fit_slice,
            )?
        } else {
            let mut fitted = fit_slice(&text, width)?;
            if painted.flow.justification == Some(Justification::Both) {
                justify_pure_chinese(&mut fitted.lines, width, paragraph.id())?;
            }
            trailing_breaks = 0;
            fitted
        }
    };
    if let Some(detail) = numbering_detail {
        fitted
            .diagnostics
            .push(LineDiagnostic::Numbering { detail });
    }
    for line in &mut fitted.lines {
        line.run.bold = painted.bold;
        line.run.italic = painted.italic;
        line.run.latin_family.clone_from(&painted.latin);
        line.run.east_asia_family.clone_from(&painted.east_asia);
    }
    let before = painted.flow.before;
    let after = painted.flow.after;
    if before.raw() > geometry.bottom_edge().raw() - geometry.top.raw()
        || after.raw() > geometry.bottom_edge().raw() - geometry.top.raw()
    {
        return Err(unsupported(
            Some(paragraph.id()),
            "paragraph spacing taller than the page",
        ));
    }
    let tallest = fitted.lines.iter().fold(line_box.raw(), |max, line| {
        let step = if line.box_height.raw() > 0 {
            line.box_height.raw()
        } else {
            line_box.raw()
        };
        max.max(step)
    });
    if add(before, Emu::new(tallest))?.raw() > geometry.bottom_edge().raw() - geometry.top.raw() {
        return Err(LayoutError::LineTooTall);
    }
    let mut glyphs = 0usize;
    for line in &fitted.lines {
        cancellation.check()?;
        if line.run.glyphs.len() > limits.max_glyphs - spent.glyphs - glyphs {
            return Err(LayoutError::LimitExceeded(LayoutLimit::Glyphs));
        }
        glyphs += line.run.glyphs.len();
    }
    let mut height = before.raw();
    for line in &fitted.lines {
        let step = if line.box_height.raw() > 0 {
            line.box_height.raw()
        } else {
            line_box.raw()
        };
        height = height
            .checked_add(step)
            .ok_or(LayoutError::GeometryOverflow)?;
    }
    Ok(FittedParagraph {
        id: paragraph.id(),
        len: paragraph.len(),
        lines: fitted.lines,
        diagnostics: fitted.diagnostics,
        omitted: fitted.omitted_diagnostics,
        shape_calls: fitted.stats.shape_calls,
        shaped_bytes: fitted.stats.shaped_bytes,
        glyphs,
        before,
        after,
        line_box,
        height: Emu::new(height),
        group_height: None,
        keep_next: painted.flow.keep_next,
        keep_lines: painted.flow.keep_lines,
        page_break_before: painted.flow.page_break_before,
        justification: painted.flow.justification,
        baseline: painted.baseline,
        geometry,
        section_index: 0,
        section_break_before: false,
        piece_breaks,
        piece_lines,
        trailing_breaks,
        frame_x: geometry.left,
        frame_width: available_width,
        pinned: false,
    })
}

pub(crate) fn assemble(
    mut result: LayoutSnapshot,
    mut prepared: Vec<FittedParagraph>,
    cancellation: &LayoutCancellation,
    stop: Option<PlaceStop<'_>>,
) -> Result<LayoutSnapshot, LayoutError> {
    prepare_keep_groups(&mut prepared, cancellation)?;
    let geometry = result.geometry;
    let mut top = geometry.top;
    let mut index = 0usize;
    while index < prepared.len() {
        cancellation.check()?;
        let boundary_matches = stop
            .as_ref()
            .is_some_and(|stop| suffix_stable(index, top, &result, stop));
        if boundary_matches {
            let copied = stop
                .as_ref()
                .is_some_and(|stop| suffix_copies(&mut result, &prepared[index..], stop));
            if copied {
                result.stats.stable_boundary = Some(index);
                cancellation.check()?;
                return Ok(result);
            }
        }
        place_one(&mut result, &mut top, &mut prepared[index], cancellation)?;
        index += 1;
    }
    cancellation.check()?;
    Ok(result)
}

fn suffix_stable(index: usize, top: Emu, result: &LayoutSnapshot, stop: &PlaceStop<'_>) -> bool {
    if index < stop.place_from
        || index >= stop.same.len()
        || stop.same[index..].iter().any(|same| !same)
    {
        return false;
    }
    let Some(anchor) = stop.starts.get(index) else {
        return false;
    };
    let page = result.pages.len().saturating_sub(1);
    page == anchor.page && top.raw() == anchor.top && result.lines.len() == anchor.line
}

/// Copy the previous snapshot from `prepared`'s first paragraph to the end.
/// Returns false, without observing a partial copy, when the run indices do
/// not line up — the caller then places those paragraphs itself.
fn suffix_copies(
    result: &mut LayoutSnapshot,
    prepared: &[FittedParagraph],
    stop: &PlaceStop<'_>,
) -> bool {
    let Some(first) = prepared.first() else {
        return true;
    };
    let anchor_line = result.lines.len();
    let source_lines = match stop.previous.lines.get(anchor_line..) {
        Some(lines) => lines,
        None => return false,
    };
    if source_lines
        .first()
        .is_none_or(|line| line.paragraph != first.id)
        && !prepared.is_empty()
    {
        // An empty tail (no lines left) is stable only when prepared is empty.
        if source_lines.is_empty() {
            return prepared.is_empty();
        }
        return false;
    }
    let mut pages = result.pages.clone();
    let mut page_geometry = result.page_geometry.clone();
    let mut page_section = result.page_section.clone();
    let mut lines = result.lines.clone();
    let mut paragraphs = result.paragraphs.clone();
    let mut by_id = result.by_id.clone();
    let mut cursor = 0usize;
    for fitted in prepared {
        let first_line = lines.len();
        let mut taken = 0usize;
        while cursor < source_lines.len() && source_lines[cursor].paragraph == fitted.id {
            let line = &source_lines[cursor];
            if line.page >= stop.previous.pages.len()
                || line.run >= stop.previous.pages[line.page].runs.len()
            {
                return false;
            }
            while pages.len() <= line.page {
                if pages.len() >= result.limits.max_pages {
                    return false;
                }
                let geometry = stop
                    .previous
                    .page_geometry
                    .get(pages.len())
                    .copied()
                    .unwrap_or(result.geometry);
                let section = stop
                    .previous
                    .page_section
                    .get(pages.len())
                    .copied()
                    .unwrap_or(0);
                pages.push(PageScene {
                    width: geometry.width,
                    height: geometry.height,
                    runs: Vec::new(),
                    rects: Vec::new(),
                    images: Vec::new(),
                });
                page_geometry.push(geometry);
                page_section.push(section);
            }
            if pages[line.page].runs.len() != line.run {
                return false;
            }
            pages[line.page]
                .runs
                .push(stop.previous.pages[line.page].runs[line.run].clone());
            let source_images = &stop.previous.pages[line.page].images;
            if line.images.start > line.images.end || line.images.end > source_images.len() {
                return false;
            }
            let mut copied = line.clone();
            let dest_start = pages[line.page].images.len();
            pages[line.page]
                .images
                .extend(source_images[line.images.clone()].iter().cloned());
            copied.images = dest_start..pages[line.page].images.len();
            lines.push(copied);
            cursor += 1;
            taken += 1;
        }
        if taken != fitted.lines.len() {
            return false;
        }
        let index = paragraphs.len();
        by_id.insert(fitted.id, index);
        paragraphs.push(ParagraphMap {
            len: fitted.len,
            lines: first_line..lines.len(),
        });
    }
    if cursor != source_lines.len() {
        return false;
    }
    while pages.len() < stop.previous.pages.len() {
        if pages.len() >= result.limits.max_pages {
            return false;
        }
        let geometry = stop
            .previous
            .page_geometry
            .get(pages.len())
            .copied()
            .unwrap_or(result.geometry);
        let section = stop
            .previous
            .page_section
            .get(pages.len())
            .copied()
            .unwrap_or(0);
        pages.push(PageScene {
            width: geometry.width,
            height: geometry.height,
            runs: Vec::new(),
            rects: Vec::new(),
            images: Vec::new(),
        });
        page_geometry.push(geometry);
        page_section.push(section);
    }
    result.pages = pages;
    result.page_geometry = page_geometry;
    result.page_section = page_section;
    result.lines = lines;
    result.paragraphs = paragraphs;
    result.by_id = by_id;
    true
}

fn place_one(
    result: &mut LayoutSnapshot,
    top: &mut Emu,
    prepared: &mut FittedParagraph,
    cancellation: &LayoutCancellation,
) -> Result<(), LayoutError> {
    let geometry = prepared.geometry;
    let section = prepared.section_index;
    let lines = std::mem::take(&mut prepared.lines);
    let piece_breaks = prepared.piece_breaks.clone();
    let piece_lines = prepared.piece_lines.clone();
    let trailing_breaks = prepared.trailing_breaks;
    let id = prepared.id;
    let len = prepared.len;
    let before = prepared.before;
    let after = prepared.after;
    let line_box = prepared.line_box;
    let height = prepared.height;
    let group_height = prepared.group_height;
    let keep_lines = prepared.keep_lines;
    let page_break_before = prepared.page_break_before;
    let section_break_before = prepared.section_break_before;
    let justification = prepared.justification;
    let baseline_offset = prepared.baseline;
    let frame_x = prepared.frame_x;
    let frame_width = prepared.frame_width;
    let pinned = prepared.pinned;
    let content_box = geometry.bottom_edge().raw() - geometry.top.raw();
    if group_height.is_some_and(|height| height.raw() > content_box)
        || (keep_lines && height.raw() > content_box)
    {
        result.note(LayoutNote::KeepTallerThanPage { paragraph: id });
    }
    // An oversized group starts on a fresh page once, then flows. A page
    // that a previous break already opened empty is resized instead of
    // gaining another blank page.
    let page_has_ink = result.pages.last().is_some_and(scene_has_ink);
    if !pinned && !page_has_ink {
        result.retarget_open_page(geometry, section);
        *top = geometry.top;
    }
    let remaining = geometry.bottom_edge().raw() - top.raw();
    let needs_page = page_break_before
        || section_break_before
        || group_height.is_some_and(|height| height.raw() > remaining)
        || (keep_lines && height.raw() > remaining);
    if needs_page && result.pages.last().is_some_and(scene_has_ink) {
        result.new_page(geometry, section)?;
        *top = geometry.top;
    }
    let first_line = result.lines.len();
    let mut before_applied = false;
    let mut cursor = 0usize;
    let slices: Vec<(u32, usize)> = if piece_breaks.is_empty() {
        vec![(0, lines.len())]
    } else {
        piece_breaks.into_iter().zip(piece_lines).collect()
    };
    for (breaks, count) in slices {
        for _ in 0..breaks {
            result.new_page(geometry, section)?;
            *top = geometry.top;
        }
        let end = cursor.saturating_add(count).min(lines.len());
        for line in &lines[cursor..end] {
            cancellation.check()?;
            if !before_applied {
                *top = add(*top, before)?;
                before_applied = true;
            }
            let step = line_step(line, line_box);
            if add(*top, step)?.raw() > geometry.bottom_edge().raw() {
                if pinned {
                    return Err(LayoutError::LineTooTall);
                }
                result.new_page(geometry, section)?;
                *top = if result.lines.len() == first_line {
                    add(geometry.top, before)?
                } else {
                    geometry.top
                };
                if add(*top, step)?.raw() > geometry.bottom_edge().raw() {
                    return Err(LayoutError::LineTooTall);
                }
            }
            place_line(
                result,
                top,
                line,
                id,
                frame_x,
                frame_width,
                line_box,
                justification,
                baseline_offset,
            )?;
        }
        cursor = end;
    }
    let map_index = result.paragraphs.len();
    result.by_id.insert(id, map_index);
    result.paragraphs.push(ParagraphMap {
        len,
        lines: first_line..result.lines.len(),
    });
    *top = add(*top, after)?;
    for _ in 0..trailing_breaks {
        result.new_page(geometry, section)?;
        *top = geometry.top;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn place_line(
    result: &mut LayoutSnapshot,
    top: &mut Emu,
    line: &LayoutLine,
    id: NodeId,
    frame_x: Emu,
    frame_width: Emu,
    line_box: Emu,
    justification: Option<Justification>,
    baseline_offset: Emu,
) -> Result<(), LayoutError> {
    let page_index = result.pages.len() - 1;
    place_line_on(
        result,
        page_index,
        top,
        line,
        id,
        frame_x,
        frame_width,
        line_box,
        justification,
        baseline_offset,
    )
}

#[allow(clippy::too_many_arguments)]
fn place_line_on(
    result: &mut LayoutSnapshot,
    page_index: usize,
    top: &mut Emu,
    line: &LayoutLine,
    id: NodeId,
    frame_x: Emu,
    frame_width: Emu,
    line_box: Emu,
    justification: Option<Justification>,
    baseline_offset: Emu,
) -> Result<(), LayoutError> {
    let slack = (frame_width.raw() - line.run.width.raw()).max(0);
    let shift = match justification {
        Some(Justification::Center) => slack / 2,
        Some(Justification::End) => slack,
        _ => 0,
    };
    let x = add(frame_x, Emu::new(shift))?;
    let step = line_step(line, line_box);
    let image_height = line
        .images
        .iter()
        .map(|image| image.height.raw())
        .max()
        .unwrap_or(0);
    let baseline = if image_height > baseline_offset.raw() {
        add(*top, Emu::new(image_height))?
    } else {
        add(*top, baseline_offset)?
    };
    let carets = line_carets(line, x)?;
    for glyph in &line.run.glyphs {
        add(x, add(glyph.x, glyph.x_offset)?)?;
        baseline
            .raw()
            .checked_sub(glyph.y_offset.raw())
            .ok_or(LayoutError::GeometryOverflow)?;
    }
    let (run, images) = {
        let page = result
            .pages
            .get_mut(page_index)
            .ok_or(LayoutError::GeometryOverflow)?;
        let run = page.runs.len();
        page.runs.push(line.to_glyph_run(x, baseline));
        let image_start = page.images.len();
        for image in &line.images {
            let y = baseline
                .raw()
                .checked_sub(image.height.raw())
                .ok_or(LayoutError::GeometryOverflow)?;
            page.images.push(SceneImage {
                media_type: image.media_type.clone(),
                bytes: image.bytes.clone(),
                x: add(x, image.x)?,
                y: Emu::new(y),
                width: image.width,
                height: image.height,
            });
        }
        (run, image_start..page.images.len())
    };
    result.lines.push(PlacedLine {
        paragraph: id,
        source: line.source.clone(),
        page: page_index,
        run,
        top: *top,
        baseline,
        height: step,
        carets,
        images,
    });
    *top = add(*top, step)?;
    Ok(())
}

fn line_step(line: &LayoutLine, line_box: Emu) -> Emu {
    if line.box_height.raw() > 0 {
        line.box_height
    } else {
        line_box
    }
}

fn scene_has_ink(page: &PageScene) -> bool {
    !page.runs.is_empty() || !page.rects.is_empty() || !page.images.is_empty()
}

fn line_carets(line: &LayoutLine, x: Emu) -> Result<Vec<CaretStop>, LayoutError> {
    let first = line
        .clusters
        .first()
        .map(|cluster| cluster.source.scalars.start)
        .unwrap_or(line.source.scalars.end);
    let left = match line.clusters.first() {
        Some(cluster) => line
            .run
            .glyphs
            .get(cluster.glyphs.start)
            .map(|glyph| glyph.x)
            .unwrap_or(Emu::new(0)),
        None => Emu::new(0),
    };
    let left = add(x, left)?;
    let mut carets = Vec::new();
    let mut offset = line.source.scalars.start;
    loop {
        carets.push(CaretStop {
            offset: ScalarOffset(offset),
            x: left,
        });
        if offset >= first {
            break;
        }
        offset += 1;
    }
    let mut last_x = left;
    for cluster in &line.clusters {
        if cluster.glyphs.end == 0 || cluster.glyphs.end > line.run.glyphs.len() {
            return Err(LayoutError::GeometryOverflow);
        }
        let glyph = &line.run.glyphs[cluster.glyphs.end - 1];
        let right = add(x, add(glyph.x, glyph.advance)?)?;
        if cluster.source.scalars.end > first {
            carets.push(CaretStop {
                offset: ScalarOffset(cluster.source.scalars.end),
                x: right,
            });
        }
        last_x = right;
    }
    let covered = carets.last().map(|stop| stop.offset.0).unwrap_or(first);
    let mut extra = covered.saturating_add(1);
    while extra <= line.source.scalars.end {
        carets.push(CaretStop {
            offset: ScalarOffset(extra),
            x: last_x,
        });
        extra += 1;
    }
    Ok(carets)
}

impl LayoutSnapshot {
    fn new_page(&mut self, geometry: PageGeometry, section: usize) -> Result<(), LayoutError> {
        if self.pages.len() >= self.limits.max_pages {
            return Err(LayoutError::LimitExceeded(LayoutLimit::Pages));
        }
        self.pages.push(PageScene {
            width: geometry.width,
            height: geometry.height,
            runs: Vec::new(),
            rects: Vec::new(),
            images: Vec::new(),
        });
        self.page_geometry.push(geometry);
        self.page_section.push(section);
        Ok(())
    }

    fn retarget_open_page(&mut self, geometry: PageGeometry, section: usize) {
        let Some(page) = self.pages.last_mut() else {
            return;
        };
        if scene_has_ink(page) {
            return;
        }
        page.width = geometry.width;
        page.height = geometry.height;
        if let Some(slot) = self.page_geometry.last_mut() {
            *slot = geometry;
        }
        if let Some(slot) = self.page_section.last_mut() {
            *slot = section;
        }
    }

    pub(crate) fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub(crate) fn page_box(&self, page: usize) -> Option<(PageGeometry, usize)> {
        Some((
            *self.page_geometry.get(page)?,
            *self.page_section.get(page)?,
        ))
    }

    pub(crate) fn note(&mut self, note: LayoutNote) {
        if self.notes.len() < self.limits.max_diagnostics {
            self.notes.push(note);
        }
    }

    pub(crate) fn append_page_run(
        &mut self,
        page: usize,
        run: crate::GlyphRun,
    ) -> Result<(), LayoutError> {
        let scene = self
            .pages
            .get_mut(page)
            .ok_or(LayoutError::GeometryOverflow)?;
        scene.runs.push(run);
        Ok(())
    }
    pub fn key(&self) -> &LayoutKey {
        &self.key
    }
    pub fn pages(&self) -> &[PageScene] {
        &self.pages
    }
    pub fn lines(&self) -> &[PlacedLine] {
        &self.lines
    }
    pub fn geometry(&self) -> PageGeometry {
        self.geometry
    }
    pub fn page_geometry(&self) -> &[PageGeometry] {
        &self.page_geometry
    }
    pub fn notes(&self) -> &[LayoutNote] {
        &self.notes
    }
    pub fn diagnostics(&self) -> &[LayoutDiagnostic] {
        &self.diagnostics
    }
    pub fn omitted_diagnostics(&self) -> usize {
        self.omitted
    }
    /// Opaque body blocks this snapshot skipped. They join the fingerprint
    /// and take no line or page geometry.
    pub fn omitted_blocks(&self) -> usize {
        self.omitted_blocks
    }

    pub(crate) fn omit_opaque_block(&mut self) {
        self.omitted_blocks = self.omitted_blocks.saturating_add(1);
    }
    pub fn stats(&self) -> LayoutStats {
        self.stats
    }
    pub(crate) fn stats_mut(&mut self) -> &mut LayoutStats {
        &mut self.stats
    }

    /// Check at a background-result handoff boundary. This also fingerprints
    /// content, so a manually built model with a reused version is not accepted.
    /// Do not recompute it on every glyph paint; retain the checked key.
    pub fn ensure_current(
        &self,
        document: &DocumentSnapshot,
        font: &LoadedFont,
        profile: SingleFontProfile,
    ) -> Result<(), LayoutError> {
        if self.key.version != document.version() {
            return Err(LayoutError::StaleSnapshot(StaleReason::Version));
        }
        if self.key.font != font.key() {
            return Err(LayoutError::StaleSnapshot(StaleReason::Font));
        }
        if self.key.profile != profile {
            return Err(LayoutError::StaleSnapshot(StaleReason::Profile));
        }
        if preflight(
            document,
            profile,
            self.limits,
            &LayoutCancellation::default(),
        )
        .is_err()
            || self.key.content != fingerprint(document)
        {
            return Err(LayoutError::StaleSnapshot(StaleReason::Content));
        }
        Ok(())
    }
    fn paragraph(&self, position: Position) -> Result<(usize, &ParagraphMap), LayoutError> {
        if position.story != self.story {
            return Err(LayoutError::UnknownPosition(position));
        }
        let index = *self
            .by_id
            .get(&position.paragraph)
            .ok_or(LayoutError::UnknownPosition(position))?;
        let paragraph = &self.paragraphs[index];
        if position.offset.0 > paragraph.len {
            return Err(LayoutError::UnknownPosition(position));
        }
        Ok((index, paragraph))
    }
    /// Before selects the upstream visual line at a wrap boundary, After the
    /// downstream line. Interior shaping-cluster positions are not fabricated.
    pub fn caret(&self, position: Position) -> Result<CaretGeometry, LayoutError> {
        let (_, p) = self.paragraph(position)?;
        let candidates = &self.lines[p.lines.clone()];
        let line = if position.bias == Bias::Before {
            candidates.iter().find(|line| {
                line.source.scalars.start <= position.offset.0
                    && position.offset.0 <= line.source.scalars.end
            })
        } else {
            candidates.iter().rev().find(|line| {
                line.source.scalars.start <= position.offset.0
                    && position.offset.0 <= line.source.scalars.end
            })
        }
        .ok_or(LayoutError::UnknownPosition(position))?;
        let stop = line
            .carets
            .iter()
            .find(|stop| stop.offset == position.offset)
            .ok_or(LayoutError::InsideCluster(position))?;
        Ok(CaretGeometry {
            page: line.page,
            x: stop.x,
            top: line.top,
            height: line.height,
            baseline: line.baseline,
        })
    }
    /// Nearest safe cluster boundary on the nearest line of this physical
    /// page. Outside-page points clamp to a line/endpoint; invalid pages fail.
    /// Zero-advance boundaries share a coordinate and have a deterministic tie.
    pub fn hit_test(&self, page: usize, x: Emu, y: Emu) -> Result<Position, LayoutError> {
        if page >= self.pages.len() {
            return Err(LayoutError::UnknownPage(page));
        }
        let line = self
            .lines
            .iter()
            .filter(|line| line.page == page)
            .min_by_key(|line| {
                let y = i128::from(y.raw());
                let start = i128::from(line.top.raw());
                let end = start + i128::from(line.height.raw());
                if y < start {
                    start - y
                } else if y >= end {
                    y - end + 1
                } else {
                    0
                }
            })
            .ok_or(LayoutError::UnknownPage(page))?;
        let stop = line
            .carets
            .iter()
            .min_by_key(|stop| (i128::from(stop.x.raw()) - i128::from(x.raw())).abs())
            .expect("every line has a caret");
        Ok(Position {
            story: self.story,
            paragraph: line.paragraph,
            offset: stop.offset,
            bias: if stop.offset.0 == line.source.scalars.end && !line.source.scalars.is_empty() {
                Bias::Before
            } else {
                Bias::After
            },
        })
    }
    /// Physical line-box rectangles for an ordered logical selection. Cluster
    /// interiors fail. Crossing a paragraph includes its mark, including an
    /// empty paragraph; zero-advance selected text gets a one-EMU rectangle.
    pub fn selection_rects(
        &self,
        start: Position,
        end: Position,
    ) -> Result<Vec<SelectionRect>, LayoutError> {
        let (first, _) = self.paragraph(start)?;
        let (last, _) = self.paragraph(end)?;
        self.caret(start)?;
        self.caret(end)?;
        if (first, start.offset) > (last, end.offset) {
            return Err(LayoutError::ReversedSelection);
        }
        if (first, start.offset) == (last, end.offset) {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for (index, p) in self
            .paragraphs
            .iter()
            .enumerate()
            .take(last + 1)
            .skip(first)
        {
            let from = if index == first { start.offset.0 } else { 0 };
            let to = if index == last { end.offset.0 } else { p.len };
            for (line_index, line) in self.lines[p.lines.clone()].iter().enumerate() {
                let lo = from.max(line.source.scalars.start);
                let hi = to.min(line.source.scalars.end);
                let mark = index < last && line_index + 1 == p.lines.len();
                if lo >= hi && !mark {
                    continue;
                }
                let left = if lo < hi {
                    line.carets.iter().find(|c| c.offset.0 == lo).map(|c| c.x)
                } else {
                    line.carets.last().map(|c| c.x)
                }
                .ok_or(LayoutError::InsideCluster(start))?;
                let mut right = if lo < hi {
                    line.carets.iter().find(|c| c.offset.0 == hi).map(|c| c.x)
                } else {
                    Some(left)
                }
                .ok_or(LayoutError::InsideCluster(end))?;
                if mark {
                    right = add(
                        *line.carets.last().map(|c| &c.x).expect("caret"),
                        Emu::new((self.key.profile.size.raw() / 2).max(1)),
                    )?;
                }
                if result.len() >= self.limits.max_selection_rects {
                    return Err(LayoutError::LimitExceeded(LayoutLimit::SelectionRects));
                }
                result.push(SelectionRect {
                    page: line.page,
                    x: left,
                    top: line.top,
                    width: Emu::new((right.raw() - left.raw()).max(1)),
                    height: line.height,
                });
            }
        }
        Ok(result)
    }
}
