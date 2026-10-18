//! Read-only, versioned single-section document flow (T105, C05).
//!
//! The explicit single-font profile is a preview contract, not a Word layout
//! compatibility profile. Unsupported formatting/stories/objects are rejected
//! before shaping; no apparently successful snapshot drops them. All geometry
//! stays in EMU. The engine remains the only owner of mutable document state.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use joyword_foundation::Emu;
use joyword_model::{
    Bias, DocDefaults, Document, DocumentSnapshot, DocumentVersion, Justification, LineSpacing,
    Measure, NodeId, NumberLabel, NumberingProblem, Paragraph, Position, ScalarOffset, StoryId,
    displayed_paragraph,
};
use sha2::{Digest, Sha256};

use crate::cjk::{self, CjkIssue};
use crate::line::{LineFit, layout_paragraph_adjusted, layout_paragraph_auto_cancellable_adjusted};
use crate::shape::ShapedRun;
use crate::style_paint;
use crate::{
    FaceSet, FeatureSet, FontKey, FontSlot, LayoutLine, LineDiagnostic, LineError, LineLimits,
    LoadedFont, PageScene, SourceRange, adjust_cjk_break, slot_for, wrap_mixed,
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
}
impl PageGeometry {
    fn read(document: &Document) -> Result<Self, LayoutError> {
        if document.sections().len() != 1 {
            return Err(unsupported(None, "multiple sections"));
        }
        let section = &document.sections()[0];
        if !section.headers.is_empty() || !section.footers.is_empty() || section.preserved.is_some()
        {
            return Err(unsupported(
                None,
                "headers, footers or preserved section properties",
            ));
        }
        let page = &section.page;
        let abs = |m: Measure| m.absolute().ok_or(LayoutError::InvalidPage);
        let geometry = Self {
            width: abs(page.width)?,
            height: abs(page.height)?,
            left: add(abs(page.left)?, abs(page.gutter)?)?,
            top: abs(page.top)?,
            right: abs(page.right)?,
            bottom: abs(page.bottom)?,
        };
        if geometry.width.raw() <= 0
            || geometry.height.raw() <= 0
            || [page.left, page.right, page.top, page.bottom, page.gutter]
                .iter()
                .any(|m| m.absolute().is_none_or(|n| n.raw() < 0))
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
    lines: Vec<PlacedLine>,
    paragraphs: Vec<ParagraphMap>,
    by_id: BTreeMap<NodeId, usize>,
    diagnostics: Vec<LayoutDiagnostic>,
    omitted: usize,
    stats: LayoutStats,
    limits: LayoutLimits,
}

fn unsupported(node: Option<NodeId>, feature: &'static str) -> LayoutError {
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
    hash.update(format!("{:?}", document.sections()[0].page).as_bytes());
    hash.update((document.body().blocks.len() as u64).to_le_bytes());
    for block in document.body().blocks.iter() {
        let paragraph = block.as_paragraph().expect("preflight paragraphs");
        let text = paragraph.plain_text(|_| unreachable!("preflight objects"));
        hash.update(paragraph.id().get().to_le_bytes());
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
        hash.update(format!("{:?}", paragraph.props).as_bytes());
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
        for block in document.body().blocks.iter() {
            let paragraph = block.as_paragraph().expect("preflight paragraphs");
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
        || document.body().blocks.iter().any(|block| {
            block.as_paragraph().is_some_and(|paragraph| {
                paragraph.style.is_some() || paragraph.runs().iter().any(|run| run.style.is_some())
            })
        })
}

fn check_paragraph(
    document: &Document,
    paragraph: &Paragraph,
    profile: SingleFontProfile,
) -> Result<(), LayoutError> {
    if !paragraph.inlines().is_empty() {
        return Err(unsupported(
            Some(paragraph.id()),
            "inline objects or breaks",
        ));
    }
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
    if document.stories().count() != 1 {
        return Err(unsupported(None, "additional stories"));
    }
    if document.sections().len() != 1 {
        return Err(unsupported(None, "multiple sections"));
    }
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
    if document.settings().preserved.is_some() || document.settings().track_revisions == Some(true)
    {
        return Err(unsupported(
            None,
            "preserved settings or revision projection",
        ));
    }
    let mut total = 0usize;
    let mut runs = 0usize;
    for block in document.body().blocks.iter() {
        cancellation.check()?;
        let p = block
            .as_paragraph()
            .ok_or_else(|| unsupported(Some(block.id()), "non-paragraph block"))?;
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
    if document.validate().has_errors() {
        return Err(LayoutError::InvalidDocument);
    }
    let geometry = PageGeometry::read(document)?;
    if profile.leading.raw() > geometry.bottom_edge().raw() - geometry.top.raw() {
        return Err(LayoutError::LineTooTall);
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
            && !paragraphs[end + 1].page_break_before
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
            if resolved.length.raw() <= 0 || profile.baseline.raw() > resolved.length.raw() {
                return Err(LayoutError::LineTooTall);
            }
            Ok(resolved.length)
        }
        Some(_) => Err(unsupported(Some(paragraph), "paragraph line spacing")),
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
    let mut result = open_snapshot(document, faces, profile, limits, geometry)?;
    let marks = joyword_model::list_marks(document);
    let mut prepared = Vec::new();
    let mut total_lines = 0usize;
    let mut charged_shapes = 0usize;
    let mut charged_bytes = 0usize;
    let mut charged_glyphs = 0usize;
    for block in document.body().blocks.iter() {
        cancellation.check()?;
        let paragraph = block.as_paragraph().expect("preflight checked all blocks");
        let fitted = fit_paragraph(
            document,
            paragraph,
            faces,
            profile,
            geometry,
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
    assemble(result, prepared, cancellation, None)
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
        lines: Vec::new(),
        paragraphs: Vec::new(),
        by_id: BTreeMap::new(),
        diagnostics: Vec::new(),
        omitted: 0,
        stats: LayoutStats::default(),
        limits,
    };
    result.new_page()?;
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
    let text = paragraph.plain_text(|_| unreachable!("objects rejected"));
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
    let width = geometry.content_width();
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
    let mut fitted = if crate::list_layout::needs_list_layout(&props, &text, label) {
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
        fitted
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
    if add(before, line_box)?.raw() > geometry.bottom_edge().raw() - geometry.top.raw() {
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
    let height = line_box
        .raw()
        .checked_mul(i64::try_from(fitted.lines.len()).map_err(|_| LayoutError::GeometryOverflow)?)
        .and_then(|height| height.checked_add(before.raw()))
        .ok_or(LayoutError::GeometryOverflow)?;
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
                pages.push(PageScene {
                    width: result.geometry.width,
                    height: result.geometry.height,
                    runs: Vec::new(),
                });
            }
            if pages[line.page].runs.len() != line.run {
                return false;
            }
            pages[line.page]
                .runs
                .push(stop.previous.pages[line.page].runs[line.run].clone());
            lines.push(line.clone());
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
    result.pages = pages;
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
    let geometry = result.geometry;
    let lines = std::mem::take(&mut prepared.lines);
    let id = prepared.id;
    let len = prepared.len;
    let before = prepared.before;
    let after = prepared.after;
    let line_box = prepared.line_box;
    let height = prepared.height;
    let group_height = prepared.group_height;
    let keep_lines = prepared.keep_lines;
    let page_break_before = prepared.page_break_before;
    let justification = prepared.justification;
    let baseline_offset = prepared.baseline;
    let remaining = geometry.bottom_edge().raw() - top.raw();
    let needs_page = page_break_before
        || group_height.is_some_and(|height| height.raw() > remaining)
        || (keep_lines && height.raw() > remaining);
    // An oversized group/paragraph starts on a fresh page once, then
    // flows forward. Never retry an impossible constraint or emit an
    // empty leading page. Empty paragraphs still have a scene run.
    let page_has_runs = result
        .pages
        .last()
        .is_some_and(|page| !page.runs.is_empty());
    if needs_page && page_has_runs {
        result.new_page()?;
        *top = geometry.top;
    }
    *top = add(*top, before)?;
    let first_line = result.lines.len();
    for line in lines {
        cancellation.check()?;
        if add(*top, line_box)?.raw() > geometry.bottom_edge().raw() {
            result.new_page()?;
            *top = if result.lines.len() == first_line {
                add(geometry.top, before)?
            } else {
                geometry.top
            };
            if add(*top, line_box)?.raw() > geometry.bottom_edge().raw() {
                return Err(LayoutError::LineTooTall);
            }
        }
        let slack = (geometry.content_width().raw() - line.run.width.raw()).max(0);
        let shift = match justification {
            Some(Justification::Center) => slack / 2,
            Some(Justification::End) => slack,
            _ => 0,
        };
        let x = add(geometry.left, Emu::new(shift))?;
        let body_x = match line.clusters.first() {
            Some(cluster) => line
                .run
                .glyphs
                .get(cluster.glyphs.start)
                .map(|glyph| glyph.x)
                .unwrap_or(Emu::new(0)),
            None => line.run.width,
        };
        let caret_x = add(x, body_x)?;
        let baseline = add(*top, baseline_offset)?;
        let mut carets = vec![CaretStop {
            offset: ScalarOffset(line.source.scalars.start),
            x: caret_x,
        }];
        for cluster in &line.clusters {
            let glyph = &line.run.glyphs[cluster.glyphs.end - 1];
            let right = add(x, add(glyph.x, glyph.advance)?)?;
            carets.push(CaretStop {
                offset: ScalarOffset(cluster.source.scalars.end),
                x: right,
            });
        }
        // Validate every coordinate consumed by screen/PDF before placing.
        for glyph in &line.run.glyphs {
            add(x, add(glyph.x, glyph.x_offset)?)?;
            baseline
                .raw()
                .checked_sub(glyph.y_offset.raw())
                .ok_or(LayoutError::GeometryOverflow)?;
        }
        let page_index = result.pages.len() - 1;
        let page = match result.pages.last_mut() {
            Some(page) => page,
            None => return Err(LayoutError::GeometryOverflow),
        };
        let run = page.runs.len();
        page.runs.push(line.to_glyph_run(x, baseline));
        result.lines.push(PlacedLine {
            paragraph: id,
            source: line.source,
            page: page_index,
            run,
            top: *top,
            baseline,
            height: line_box,
            carets,
        });
        *top = add(*top, line_box)?;
    }
    let map_index = result.paragraphs.len();
    result.by_id.insert(id, map_index);
    result.paragraphs.push(ParagraphMap {
        len,
        lines: first_line..result.lines.len(),
    });
    *top = add(*top, after)?;
    Ok(())
}
impl LayoutSnapshot {
    fn new_page(&mut self) -> Result<(), LayoutError> {
        if self.pages.len() >= self.limits.max_pages {
            return Err(LayoutError::LimitExceeded(LayoutLimit::Pages));
        }
        self.pages.push(PageScene {
            width: self.geometry.width,
            height: self.geometry.height,
            runs: Vec::new(),
        });
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
    pub fn diagnostics(&self) -> &[LayoutDiagnostic] {
        &self.diagnostics
    }
    pub fn omitted_diagnostics(&self) -> usize {
        self.omitted
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
