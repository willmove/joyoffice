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
    Bias, Document, DocumentSnapshot, DocumentVersion, Justification, Measure, NodeId, Paragraph,
    Position, RunProps, ScalarOffset, StoryId, StyleId, StyleKind,
};
use sha2::{Digest, Sha256};

use crate::{
    FeatureSet, FontKey, LineDiagnostic, LineError, LineLimits, LoadedFont, PageScene, SourceRange,
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
    fn capped(self) -> Self {
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
    fn check(&self) -> Result<(), LayoutError> {
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
}
impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "document layout: {self:?}")
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
    fn content_width(self) -> Emu {
        Emu::new(self.width.raw() - self.left.raw() - self.right.raw())
    }
    fn bottom_edge(self) -> Emu {
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
}

#[derive(Debug)]
struct ParagraphMap {
    len: usize,
    lines: Range<usize>,
}

#[derive(Debug)]
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
fn add(a: Emu, b: Emu) -> Result<Emu, LayoutError> {
    a.checked_add(b).ok_or(LayoutError::GeometryOverflow)
}
// Hash rendered source incrementally, not a potentially huge formatted model
// dump. Preflight checks active styles before this; unused metadata is not a
// layout input. Lengths delimit text even when it contains debug-like syntax.
fn fingerprint(document: &Document) -> [u8; 32] {
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
    hash.finalize().into()
}

fn plain_run(props: &RunProps) -> bool {
    let mut props = props.clone();
    for toggle in [
        &mut props.bold,
        &mut props.italic,
        &mut props.strike,
        &mut props.hidden,
    ] {
        if *toggle == Some(false) {
            *toggle = None;
        }
    }
    if props.underline == Some(joyword_model::Underline::None) {
        props.underline = None;
    }
    props.is_empty()
}
fn plain_style(document: &Document, id: &StyleId, kind: StyleKind) -> bool {
    let chain = document.styles().chain(id);
    chain.problem.is_none()
        && chain.styles.iter().all(|s| {
            s.kind == kind && plain_run(&s.run) && s.para.is_empty() && s.preserved.is_none()
        })
}
fn check_paragraph(document: &Document, p: &Paragraph) -> Result<(), LayoutError> {
    let error = |feature| unsupported(Some(p.id()), feature);
    if !p.inlines().is_empty() {
        return Err(error("inline objects or breaks"));
    }
    if !plain_run(&p.mark_props)
        || p.runs().iter().any(|r| {
            !plain_run(&r.props)
                || r.style
                    .as_ref()
                    .is_some_and(|id| !plain_style(document, id, StyleKind::Character))
        })
    {
        return Err(error("character formatting or font resolution"));
    }
    if p.style
        .as_ref()
        .is_some_and(|id| !plain_style(document, id, StyleKind::Paragraph))
    {
        return Err(error("paragraph style resolution"));
    }
    let mut props = (*p.props).clone();
    props.justification = None;
    props.page_break_before = None;
    props.spacing.before = None;
    props.spacing.after = None;
    for toggle in [
        &mut props.keep_next,
        &mut props.keep_lines,
        &mut props.snap_to_grid,
        &mut props.spacing.before_auto,
        &mut props.spacing.after_auto,
    ] {
        if *toggle == Some(false) {
            *toggle = None;
        }
    }
    if !props.is_empty()
        || matches!(
            p.props.justification,
            Some(Justification::Both | Justification::Distribute)
        )
    {
        return Err(error("paragraph layout properties"));
    }
    for amount in [p.props.spacing.before, p.props.spacing.after]
        .into_iter()
        .flatten()
    {
        if amount.absolute().is_none_or(|n| n.raw() < 0) {
            return Err(error("relative or negative paragraph spacing"));
        }
    }
    Ok(())
}

fn preflight(
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
        check_paragraph(document, p)?;
    }
    if document.validate().has_errors() {
        return Err(LayoutError::InvalidDocument);
    }
    if !plain_run(&document.styles().defaults.run)
        || !document.styles().defaults.para.is_empty()
        || [StyleKind::Paragraph, StyleKind::Character]
            .iter()
            .any(|&kind| {
                document
                    .styles()
                    .default_of(kind)
                    .is_some_and(|style| !plain_style(document, &style.id, kind))
            })
    {
        return Err(unsupported(None, "document default style resolution"));
    }
    let geometry = PageGeometry::read(document)?;
    if profile.leading.raw() > geometry.bottom_edge().raw() - geometry.top.raw() {
        return Err(LayoutError::LineTooTall);
    }
    Ok(geometry)
}

/// Lay out a complete supported document; any failure discards all candidate
/// pages. Callers retain their previous snapshot and keep editing in engine.
/// Single-font defaults are explicit, not guessed from a system font name.
pub fn layout_document(
    document: &DocumentSnapshot,
    font: &LoadedFont,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    cancellation: &LayoutCancellation,
) -> Result<LayoutSnapshot, LayoutError> {
    cancellation.check()?;
    profile.validate()?;
    let limits = limits.capped();
    let geometry = preflight(document, profile, limits, cancellation)?;
    let mut result = LayoutSnapshot {
        key: LayoutKey {
            version: document.version(),
            font: font.key(),
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
    let mut top = geometry.top;
    for block in document.body().blocks.iter() {
        cancellation.check()?;
        let paragraph = block.as_paragraph().expect("preflight checked all blocks");
        let defaults = LineLimits::default();
        let request_limits = LineLimits {
            max_text_bytes: defaults.max_text_bytes.min(limits.max_text_bytes),
            max_shape_calls: defaults
                .max_shape_calls
                .min(limits.max_shape_calls - result.stats.shape_calls),
            max_shaped_bytes: defaults
                .max_shaped_bytes
                .min(limits.max_shaped_bytes - result.stats.shaped_bytes),
            max_lines: defaults
                .max_lines
                .min(limits.max_lines - result.lines.len()),
            max_diagnostics: defaults.max_diagnostics.min(
                limits
                    .max_diagnostics
                    .saturating_sub(result.diagnostics.len()),
            ),
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
        let text = paragraph.plain_text(|_| unreachable!("objects rejected"));
        let fitted = crate::line::layout_paragraph_auto_cancellable(
            font,
            &text,
            profile.size,
            profile.features,
            geometry.content_width(),
            request_limits,
            &cancellation.0,
        )
        .map_err(|error| {
            if error == LineError::Cancelled {
                LayoutError::Cancelled
            } else {
                LayoutError::Line {
                    paragraph: paragraph.id(),
                    error,
                }
            }
        })?;
        result.stats.shape_calls += fitted.stats.shape_calls;
        result.stats.shaped_bytes += fitted.stats.shaped_bytes;
        let before = paragraph
            .props
            .spacing
            .before
            .and_then(Measure::absolute)
            .unwrap_or(Emu::new(0));
        let after = paragraph
            .props
            .spacing
            .after
            .and_then(Measure::absolute)
            .unwrap_or(Emu::new(0));
        if before.raw() > geometry.bottom_edge().raw() - geometry.top.raw()
            || after.raw() > geometry.bottom_edge().raw() - geometry.top.raw()
        {
            return Err(unsupported(
                Some(paragraph.id()),
                "paragraph spacing taller than the page",
            ));
        }
        if paragraph.props.page_break_before == Some(true)
            && !result.pages.last().expect("one page").runs.is_empty()
        {
            result.new_page()?;
            top = geometry.top;
        }
        top = add(top, before)?;
        let first_line = result.lines.len();
        for line in fitted.lines {
            cancellation.check()?;
            if add(top, profile.leading)?.raw() > geometry.bottom_edge().raw() {
                result.new_page()?;
                top = if result.lines.len() == first_line {
                    add(geometry.top, before)?
                } else {
                    geometry.top
                };
                if add(top, profile.leading)?.raw() > geometry.bottom_edge().raw() {
                    return Err(LayoutError::LineTooTall);
                }
            }
            if line.run.glyphs.len() > limits.max_glyphs - result.stats.glyphs {
                return Err(LayoutError::LimitExceeded(LayoutLimit::Glyphs));
            }
            result.stats.glyphs += line.run.glyphs.len();
            let slack = (geometry.content_width().raw() - line.run.width.raw()).max(0);
            let shift = match paragraph.props.justification {
                Some(Justification::Center) => slack / 2,
                Some(Justification::End) => slack,
                _ => 0,
            };
            let x = add(geometry.left, Emu::new(shift))?;
            let baseline = add(top, profile.baseline)?;
            let mut carets = vec![CaretStop {
                offset: ScalarOffset(line.source.scalars.start),
                x,
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
            let page = result.pages.last_mut().expect("one page");
            let run = page.runs.len();
            page.runs.push(line.to_glyph_run(x, baseline));
            result.lines.push(PlacedLine {
                paragraph: paragraph.id(),
                source: line.source,
                page: page_index,
                run,
                top,
                baseline,
                height: profile.leading,
                carets,
            });
            top = add(top, profile.leading)?;
        }
        let index = result.paragraphs.len();
        result.by_id.insert(paragraph.id(), index);
        result.paragraphs.push(ParagraphMap {
            len: paragraph.len(),
            lines: first_line..result.lines.len(),
        });
        top = add(top, after)?;
        for diagnostic in fitted.diagnostics {
            if result.diagnostics.len() < limits.max_diagnostics {
                result.diagnostics.push(LayoutDiagnostic {
                    paragraph: paragraph.id(),
                    diagnostic,
                });
            } else {
                result.omitted += 1;
            }
        }
        result.omitted += fitted.omitted_diagnostics;
    }
    cancellation.check()?;
    Ok(result)
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
