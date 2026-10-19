//! Section geometry, explicit page breaks, and header and footer stories.
//!
//! Page size is `w:pgSz/@w` and `@h` (ECMA-376 §17.6.13). `orient` is stored
//! with them and does not swap the axes. `w:type` places the section on the
//! next page, the same page, or the next even or odd page (§17.6.22).
//! `titlePg` paints the First story on the first page of that section.
//! `evenAndOddHeaders` paints the Even story on even displayed page numbers
//! and the Default story on odd ones. The displayed number is `PAGE` after
//! `w:pgNumType/@w:start` (§17.6.12). The section's first page still counts,
//! and `titlePg` wins there. An omitted reference inherits the nearest earlier
//! story of that same kind. An explicit story, including an empty one, does
//! not inherit and is not replaced by another kind. Equal-width `w:cols`
//! share the content box; a column break advances one column, or one page
//! when the section has a single column (§17.3.3.1, §17.6.4).

use std::collections::BTreeSet;

use joyword_foundation::Emu;
use joyword_model::{
    BreakKind, Document, HeaderFooterKind, InlineKind, NumberFormat, OBJECT_MARKER, PageField,
    PageFieldKind, Paragraph, SectionBreak, Segment, StoryId, StoryKind,
};

use crate::FaceSet;
use crate::document::dominant_slot;
use crate::document::{
    LayoutCancellation, LayoutError, LayoutNote, LayoutSnapshot, PageGeometry, SingleFontProfile,
    unsupported,
};
use crate::line::{LayoutLine, LineError, LineLimits, SourceRange, layout_paragraph_auto};
use crate::scene::{GlyphRun, decoration_rects};
use crate::shape::paint_underline;
use crate::style_paint;
use crate::view::{self, ViewOptions};

/// How the first block of a section meets the previous section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum SectionEdge {
    #[default]
    Same,
    NextPage,
    EvenPage,
    OddPage,
    Continuous,
}

impl SectionEdge {
    pub(crate) fn forces_page(self) -> bool {
        matches!(self, Self::NextPage | Self::EvenPage | Self::OddPage)
    }
}

/// Equal columns, or one column the width of the content box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColumnSpec {
    pub count: u16,
    pub gap: i64,
    pub width: i64,
}

pub(crate) struct SectionPlan {
    pub geometry: PageGeometry,
    /// Index of the section's last body block.
    pub end: usize,
    pub section: usize,
    /// Break stored on this section. The first block of the document ignores it.
    pub edge: SectionEdge,
    pub columns: ColumnSpec,
    /// Line width: one equal column, or the full content box.
    pub fit_width: Emu,
}

pub(crate) struct TextPiece {
    pub text: String,
    pub scalar_origin: usize,
    pub byte_origin: usize,
    pub breaks_before: u32,
}

pub(crate) struct PageBreakSplit {
    pub pieces: Vec<TextPiece>,
    /// Breaks after the last text piece. Zero when the paragraph has no
    /// explicit page break, or when the paragraph is only page breaks
    /// (those live in [`Self::pieces`] as one empty piece's `breaks_before`).
    pub trailing_breaks: u32,
}

pub(crate) fn section_plans(document: &Document) -> Result<Vec<SectionPlan>, LayoutError> {
    let blocks = &document.body().blocks;
    if blocks.is_empty() {
        return Err(LayoutError::InvalidDocument);
    }
    let sections = document.sections();
    if sections.is_empty() {
        return Err(LayoutError::InvalidDocument);
    }
    let mut plans = Vec::with_capacity(sections.len());
    let mut cursor = 0usize;
    for (index, section) in sections.iter().enumerate() {
        if section.preserved.is_some() {
            return Err(unsupported(
                Some(section.id),
                "preserved section properties",
            ));
        }
        let geometry = PageGeometry::from_setup(&section.page)?;
        let columns = column_spec(section.id, geometry, section.columns.as_ref())?;
        let edge = match section.break_kind {
            SectionBreak::NextPage => SectionEdge::NextPage,
            SectionBreak::Continuous => SectionEdge::Continuous,
            SectionBreak::EvenPage => SectionEdge::EvenPage,
            SectionBreak::OddPage => SectionEdge::OddPage,
        };
        let end = match section.last_block {
            Some(id) => {
                if index + 1 == sections.len() {
                    return Err(LayoutError::InvalidDocument);
                }
                let found = blocks
                    .iter()
                    .position(|block| block.id() == id)
                    .ok_or(LayoutError::InvalidDocument)?;
                if found < cursor {
                    return Err(LayoutError::InvalidDocument);
                }
                found
            }
            None => {
                if index + 1 != sections.len() {
                    return Err(LayoutError::InvalidDocument);
                }
                blocks.len() - 1
            }
        };
        plans.push(SectionPlan {
            geometry,
            end,
            section: index,
            edge,
            fit_width: Emu::new(columns.width),
            columns,
        });
        cursor = end + 1;
    }
    if cursor != blocks.len() {
        return Err(LayoutError::InvalidDocument);
    }
    Ok(plans)
}

pub(crate) fn plan_for(plans: &[SectionPlan], index: usize) -> Option<&SectionPlan> {
    plans.iter().find(|plan| index <= plan.end)
}

pub(crate) fn section_edge(plans: &[SectionPlan], index: usize) -> SectionEdge {
    if index == 0 {
        return SectionEdge::Same;
    }
    match (plan_for(plans, index - 1), plan_for(plans, index)) {
        (Some(previous), Some(here)) if previous.section != here.section => here.edge,
        _ => SectionEdge::Same,
    }
}

pub(crate) fn section_break_before(plans: &[SectionPlan], index: usize) -> bool {
    section_edge(plans, index).forces_page()
}

fn column_spec(
    section: joyword_model::NodeId,
    geometry: PageGeometry,
    columns: Option<&joyword_model::EqualColumns>,
) -> Result<ColumnSpec, LayoutError> {
    let total = geometry.content_width().raw();
    let Some(columns) = columns else {
        return Ok(ColumnSpec {
            count: 1,
            gap: 0,
            width: total,
        });
    };
    let count = i64::from(columns.count);
    if count < 2 {
        return Ok(ColumnSpec {
            count: 1,
            gap: 0,
            width: total,
        });
    }
    let gap = columns
        .space
        .checked_mul(Emu::PER_TWIP)
        .ok_or(LayoutError::GeometryOverflow)?;
    let gaps = gap
        .checked_mul(count - 1)
        .ok_or(LayoutError::GeometryOverflow)?;
    if gaps >= total {
        return Err(unsupported(Some(section), "columns do not fit the page"));
    }
    let width = (total - gaps) / count;
    if width <= 0 {
        return Err(unsupported(Some(section), "columns do not fit the page"));
    }
    let count = u16::try_from(count).unwrap_or(1);
    Ok(ColumnSpec { count, gap, width })
}

/// Header and footer stories must be referenced. Anything else is still an
/// unsupported extra story. Non-paragraph blocks in a referenced story are
/// rejected rather than skipped.
pub(crate) fn accept_stories(document: &Document) -> Result<(), LayoutError> {
    let mut referenced = std::collections::BTreeSet::new();
    for section in document.sections() {
        for (_, id) in section.headers.iter().chain(section.footers.iter()) {
            referenced.insert(*id);
        }
    }
    for story in document.stories() {
        if story.id == document.body_id() {
            continue;
        }
        if !referenced.contains(&story.id)
            || !matches!(story.kind, StoryKind::Header | StoryKind::Footer)
        {
            return Err(unsupported(None, "additional stories"));
        }
        for block in story.blocks.iter() {
            if block.as_paragraph().is_none() {
                return Err(unsupported(Some(block.id()), "non-paragraph block"));
            }
        }
    }
    Ok(())
}

pub(crate) fn accept_body_inlines(
    document: &Document,
    paragraph: &Paragraph,
) -> Result<(), LayoutError> {
    let mut page_break = false;
    let mut column_break = false;
    let mut image = false;
    let mut field = false;
    for inline in paragraph.inlines() {
        match &inline.kind {
            InlineKind::Break(BreakKind::Page) => page_break = true,
            InlineKind::Break(BreakKind::Column) => column_break = true,
            InlineKind::Break(BreakKind::Line) => {
                return Err(unsupported(Some(paragraph.id()), "line break"));
            }
            InlineKind::PageField(_) => field = true,
            InlineKind::Image { .. } => image = true,
            InlineKind::Preserved(_) => {
                return Err(unsupported(
                    Some(paragraph.id()),
                    "inline objects or breaks",
                ));
            }
        }
    }
    if field && (page_break || column_break || image) {
        return Err(unsupported(
            Some(paragraph.id()),
            "page field with a break or an image",
        ));
    }
    if column_break && (page_break || image) {
        return Err(unsupported(
            Some(paragraph.id()),
            "column break with a page break or an image",
        ));
    }
    if image {
        let props = joyword_model::displayed_paragraph(document, paragraph);
        let text = paragraph.plain_text(|_| String::new());
        let tabs = props.tabs.as_ref().is_some_and(|stops| {
            stops
                .iter()
                .any(|stop| stop.align != joyword_model::TabAlign::Clear)
        });
        let indent =
            joyword_model::indent_edges(&props.indent).is_some_and(|edges| edges.moves_text());
        if page_break || props.numbering.is_some() || tabs || indent || text.contains('\t') {
            return Err(unsupported(
                Some(paragraph.id()),
                "inline image with numbering, tabs or a page break",
            ));
        }
    }
    if page_break {
        let props = joyword_model::displayed_paragraph(document, paragraph);
        let text = paragraph.plain_text(|_| String::new());
        let tabs = props.tabs.as_ref().is_some_and(|stops| {
            stops
                .iter()
                .any(|stop| stop.align != joyword_model::TabAlign::Clear)
        });
        let indent =
            joyword_model::indent_edges(&props.indent).is_some_and(|edges| edges.moves_text());
        if props.numbering.is_some() || tabs || indent || text.contains('\t') {
            return Err(unsupported(
                Some(paragraph.id()),
                "page break with numbering or tabs",
            ));
        }
    }
    if column_break || field {
        let props = joyword_model::displayed_paragraph(document, paragraph);
        let text = paragraph.plain_text(|_| String::new());
        let tabs = props.tabs.as_ref().is_some_and(|stops| {
            stops
                .iter()
                .any(|stop| stop.align != joyword_model::TabAlign::Clear)
        });
        let indent =
            joyword_model::indent_edges(&props.indent).is_some_and(|edges| edges.moves_text());
        if props.numbering.is_some() || tabs || indent || text.contains('\t') {
            return Err(unsupported(
                Some(paragraph.id()),
                "page field or column break with numbering or tabs",
            ));
        }
    }
    Ok(())
}

/// Text plus markers so a page break or a page field cannot share a cache
/// key with the same visible characters.
pub(crate) fn marked_text(document: &Document, paragraph: &Paragraph) -> String {
    paragraph.plain_text(|object| match &object.kind {
        InlineKind::Break(BreakKind::Page) => format!("{OBJECT_MARKER}page"),
        InlineKind::Break(kind) => format!("{OBJECT_MARKER}{kind:?}"),
        InlineKind::PageField(field) => format!(
            "{OBJECT_MARKER}{:?}:{}",
            field.kind,
            field.cached.as_deref().unwrap_or("")
        ),
        InlineKind::Image {
            resource,
            width,
            height,
        } => {
            let hash = document
                .resources()
                .get(*resource)
                .map(|item| item.hash().to_string())
                .unwrap_or_else(|| "missing".into());
            format!("{OBJECT_MARKER}image:{hash}:{width}x{height}")
        }
        InlineKind::Preserved(_) => OBJECT_MARKER.to_string(),
    })
}

pub(crate) fn split_page_breaks(paragraph: &Paragraph) -> PageBreakSplit {
    let marker_bytes = OBJECT_MARKER.len_utf8();
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut cursor_scalar = 0usize;
    let mut cursor_byte = 0usize;
    let mut pending = 0u32;
    let mut origin_scalar = 0usize;
    let mut origin_byte = 0usize;
    let mut started = false;
    for segment in paragraph.segments() {
        match segment {
            Segment::Text(chunk) => {
                if !started {
                    origin_scalar = cursor_scalar;
                    origin_byte = cursor_byte;
                    started = true;
                }
                cursor_scalar += chunk.chars().count();
                cursor_byte += chunk.len();
                text.push_str(&chunk);
            }
            Segment::Inline(_) => {
                if started {
                    pieces.push(TextPiece {
                        text: std::mem::take(&mut text),
                        scalar_origin: origin_scalar,
                        byte_origin: origin_byte,
                        breaks_before: pending,
                    });
                    pending = 1;
                    started = false;
                } else {
                    pending = pending.saturating_add(1);
                }
                cursor_scalar += 1;
                cursor_byte += marker_bytes;
            }
        }
    }
    let trailing_breaks = if started {
        pieces.push(TextPiece {
            text,
            scalar_origin: origin_scalar,
            byte_origin: origin_byte,
            breaks_before: pending,
        });
        0
    } else {
        pending
    };
    PageBreakSplit {
        pieces,
        trailing_breaks,
    }
}

pub(crate) fn shift_piece(lines: &mut [LayoutLine], piece: &TextPiece) {
    let marker_bytes = OBJECT_MARKER.len_utf8();
    for line in lines.iter_mut() {
        shift_range(&mut line.source, piece.scalar_origin, piece.byte_origin);
        for cluster in &mut line.clusters {
            shift_range(&mut cluster.source, piece.scalar_origin, piece.byte_origin);
        }
    }
    if let Some(first) = lines.first_mut() {
        let breaks = piece.breaks_before as usize;
        first.source.scalars.start = first.source.scalars.start.saturating_sub(breaks);
        first.source.bytes.start = first
            .source
            .bytes
            .start
            .saturating_sub(breaks.saturating_mul(marker_bytes));
    }
}

pub(crate) fn extend_trailing(lines: &mut [LayoutLine], trailing: u32) {
    let Some(last) = lines.last_mut() else {
        return;
    };
    let extra = trailing as usize;
    last.source.scalars.end = last.source.scalars.end.saturating_add(extra);
    last.source.bytes.end = last
        .source
        .bytes
        .end
        .saturating_add(extra.saturating_mul(OBJECT_MARKER.len_utf8()));
}

fn shift_range(range: &mut SourceRange, scalars: usize, bytes: usize) {
    range.scalars.start = range.scalars.start.saturating_add(scalars);
    range.scalars.end = range.scalars.end.saturating_add(scalars);
    range.bytes.start = range.bytes.start.saturating_add(bytes);
    range.bytes.end = range.bytes.end.saturating_add(bytes);
}

/// Paint the header and footer for each page after the body page count has
/// settled. Painting does not add pages. A change in the page count is unstable.
pub(crate) fn paint_stories(
    snapshot: &mut LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    cancellation: &LayoutCancellation,
) -> Result<(), LayoutError> {
    cancellation.check()?;
    let page_count = snapshot.page_count();
    let even_and_odd = document.settings().even_and_odd_headers == Some(true);
    let mut exceeded = std::collections::BTreeSet::new();
    for page_index in 0..page_count {
        cancellation.check()?;
        let Some((geometry, section)) = snapshot.page_box(page_index) else {
            return Err(LayoutError::GeometryOverflow);
        };
        if section >= document.sections().len() {
            return Err(LayoutError::InvalidDocument);
        }
        let kind = story_kind(
            document,
            section,
            opens_section(snapshot, page_index, section),
            snapshot.displayed_page(page_index),
            even_and_odd,
        );
        if let Some(story) = linked_story(document, section, true, kind) {
            paint_header(
                snapshot,
                document,
                faces,
                profile,
                cancellation,
                page_index,
                page_count,
                geometry,
                story,
                &mut exceeded,
            )?;
        }
        if let Some(story) = linked_story(document, section, false, kind) {
            paint_footer(
                snapshot,
                document,
                faces,
                profile,
                cancellation,
                page_index,
                page_count,
                geometry,
                story,
                &mut exceeded,
            )?;
        }
    }
    if snapshot.page_count() != page_count {
        return Err(LayoutError::Unstable);
    }
    Ok(())
}

/// First page of a section uses First when `titlePg` is on. Otherwise an even
/// page number uses Even when odd and even headers are on. Every other page
/// uses Default. A missing flag is off, matching an omitted on/off element.
fn story_kind(
    document: &Document,
    section_index: usize,
    first_of_section: bool,
    page_number: i64,
    even_and_odd: bool,
) -> HeaderFooterKind {
    let title_page = document
        .sections()
        .get(section_index)
        .is_some_and(|section| section.title_page == Some(true));
    if first_of_section && title_page {
        HeaderFooterKind::First
    } else if even_and_odd && page_number.rem_euclid(2) == 0 {
        HeaderFooterKind::Even
    } else {
        HeaderFooterKind::Default
    }
}

fn opens_section(snapshot: &LayoutSnapshot, page_index: usize, section: usize) -> bool {
    page_index == 0
        || snapshot
            .page_box(page_index - 1)
            .is_none_or(|(_, previous)| previous != section)
}

/// Nearest explicit reference of `kind` at or before this section. A later
/// empty story replaces an earlier one; it does not fall through.
fn linked_story(
    document: &Document,
    section_index: usize,
    header: bool,
    kind: HeaderFooterKind,
) -> Option<StoryId> {
    let mut found = None;
    for section in document.sections().iter().take(section_index + 1) {
        let refs = if header {
            &section.headers
        } else {
            &section.footers
        };
        if let Some((_, id)) = refs.iter().find(|(existing, _)| *existing == kind) {
            found = Some(*id);
        }
    }
    found
}

#[allow(clippy::too_many_arguments)]
fn paint_header(
    snapshot: &mut LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    cancellation: &LayoutCancellation,
    page_index: usize,
    page_count: usize,
    geometry: PageGeometry,
    story: StoryId,
    exceeded: &mut std::collections::BTreeSet<StoryId>,
) -> Result<(), LayoutError> {
    let runs = story_runs(
        snapshot,
        document,
        faces,
        profile,
        cancellation,
        page_index,
        page_count,
        geometry,
        story,
    )?;
    let mut y = geometry.header;
    let mut crossed = false;
    for line in runs {
        let baseline = y
            .checked_add(Emu::new(line.baseline))
            .ok_or(LayoutError::GeometryOverflow)?;
        let next = y
            .checked_add(line.line_box)
            .ok_or(LayoutError::GeometryOverflow)?;
        if next.raw() > geometry.top.raw() {
            crossed = true;
        }
        place_story_line(snapshot, page_index, line, geometry.left, baseline)?;
        y = next;
    }
    if crossed {
        note_exceeds(snapshot, story, exceeded);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn paint_footer(
    snapshot: &mut LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    cancellation: &LayoutCancellation,
    page_index: usize,
    page_count: usize,
    geometry: PageGeometry,
    story: StoryId,
    exceeded: &mut std::collections::BTreeSet<StoryId>,
) -> Result<(), LayoutError> {
    let runs = story_runs(
        snapshot,
        document,
        faces,
        profile,
        cancellation,
        page_index,
        page_count,
        geometry,
        story,
    )?;
    let total: i64 = runs.iter().map(|line| line.line_box.raw()).sum();
    let mut y = geometry.height.raw() - geometry.footer.raw() - total;
    let mut crossed = false;
    if y < 0 {
        y = 0;
        crossed = true;
    }
    if y < geometry.bottom_edge().raw() && total > 0 {
        crossed = true;
    }
    for line in runs {
        let baseline = Emu::new(y)
            .checked_add(Emu::new(line.baseline))
            .ok_or(LayoutError::GeometryOverflow)?;
        let box_height = line.line_box.raw();
        place_story_line(snapshot, page_index, line, geometry.left, baseline)?;
        y += box_height;
    }
    if crossed {
        note_exceeds(snapshot, story, exceeded);
    }
    Ok(())
}

fn note_exceeds(
    snapshot: &mut LayoutSnapshot,
    story: StoryId,
    exceeded: &mut std::collections::BTreeSet<StoryId>,
) {
    if exceeded.insert(story) {
        snapshot.note(LayoutNote::StoryExceedsMargin { story });
    }
}

fn place_story_line(
    snapshot: &mut LayoutSnapshot,
    page_index: usize,
    line: StoryLine,
    origin_x: Emu,
    baseline_y: Emu,
) -> Result<(), LayoutError> {
    let mut run = line.run;
    run.origin_x = origin_x;
    run.baseline_y = baseline_y;
    let marks = snapshot.key().view.formatting_marks;
    let rects = decoration_rects(&run, marks, line.paragraph_end, &line.tab_gaps);
    snapshot.append_page_run(page_index, run)?;
    snapshot.extend_page_rects(page_index, rects)
}

/// A fitted header or footer line, not yet positioned on the page.
struct StoryLine {
    run: GlyphRun,
    line_box: Emu,
    baseline: i64,
    paragraph_end: bool,
    tab_gaps: Vec<(i64, i64)>,
}

/// Fitted header or footer lines, not yet positioned.
#[allow(clippy::too_many_arguments)]
fn story_runs(
    snapshot: &mut LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    cancellation: &LayoutCancellation,
    page_index: usize,
    _page_count: usize,
    geometry: PageGeometry,
    story_id: StoryId,
) -> Result<Vec<StoryLine>, LayoutError> {
    let Some(story) = document.story(story_id) else {
        return Ok(Vec::new());
    };
    let view = snapshot.key().view;
    let mut runs = Vec::new();
    for block in story.blocks.iter() {
        cancellation.check()?;
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        let (display, used_cache) =
            match story_display(snapshot, document, paragraph, page_index, view)? {
                Some(text) => text,
                None => {
                    snapshot.note(LayoutNote::StoryContentSkipped {
                        paragraph: paragraph.id(),
                    });
                    continue;
                }
            };
        if used_cache {
            let note = LayoutNote::PageFieldCached {
                paragraph: paragraph.id(),
            };
            if !snapshot.notes().contains(&note) {
                snapshot.note(note);
            }
        }
        if display.is_empty() {
            continue;
        }
        let painted = match style_paint::paint(document, paragraph, profile, view, None) {
            Ok(painted) => painted,
            Err(LayoutError::Cancelled) => return Err(LayoutError::Cancelled),
            Err(LayoutError::LimitExceeded(limit)) => {
                return Err(LayoutError::LimitExceeded(limit));
            }
            Err(_) => {
                snapshot.note(LayoutNote::StoryContentSkipped {
                    paragraph: paragraph.id(),
                });
                continue;
            }
        };
        let slot = dominant_slot(&display);
        let font = faces.face(slot);
        let fitted = match layout_paragraph_auto(
            font,
            &display,
            painted.size,
            profile.features,
            geometry.content_width(),
            LineLimits::default(),
        ) {
            Ok(fitted) => fitted,
            Err(LineError::Cancelled) => return Err(LayoutError::Cancelled),
            Err(LineError::LimitExceeded(limit)) => {
                return Err(map_line_limit(limit));
            }
            Err(_) => {
                snapshot.note(LayoutNote::StoryContentSkipped {
                    paragraph: paragraph.id(),
                });
                continue;
            }
        };
        let line_box = if painted.leading.raw() > 0 {
            painted.leading
        } else {
            profile.leading
        };
        let baseline = painted.baseline.raw().clamp(0, line_box.raw());
        let underline = paint_underline(font, painted.size, painted.underline);
        let last = fitted.lines.len().saturating_sub(1);
        for (index, mut line) in fitted.lines.into_iter().enumerate() {
            line.run.bold = painted.bold;
            line.run.italic = painted.italic;
            line.run.latin_family.clone_from(&painted.latin);
            line.run.east_asia_family.clone_from(&painted.east_asia);
            line.run.color = painted.color;
            line.run.underline = underline;
            let tab_gaps = line.tab_gaps.clone();
            let run = line.to_glyph_run(Emu::new(0), Emu::new(0));
            runs.push(StoryLine {
                run,
                line_box,
                baseline,
                paragraph_end: index == last,
                tab_gaps,
            });
        }
    }
    Ok(runs)
}

/// Header and footer text. Field codes print `PAGE`, `NUMPAGES` or
/// `SECTIONPAGES` instead of the number. Braces and switches are not
/// printed. Scalars omitted by hidden text or a field projection are skipped.
fn story_display(
    snapshot: &LayoutSnapshot,
    document: &Document,
    paragraph: &Paragraph,
    page_index: usize,
    view: ViewOptions,
) -> Result<Option<(String, bool)>, LayoutError> {
    let Some((_, section)) = snapshot.page_box(page_index) else {
        return Ok(None);
    };
    let numbering = document
        .sections()
        .get(section)
        .map(|section| section.numbering.clone())
        .unwrap_or_default();
    let ctx = crate::cache::ExternalContext {
        displayed_page: snapshot.displayed_page(page_index),
        num_pages: u32::try_from(snapshot.page_count()).unwrap_or(u32::MAX),
        section_pages: snapshot.section_page_count(section),
        ..crate::cache::ExternalContext::default()
    };
    let revision = snapshot.key().revision_view;
    let mut omitted = view::omitted_indices(document, paragraph, view)?;
    let mut scalar = 0usize;
    for run in paragraph.runs() {
        let show = run
            .revision
            .as_ref()
            .map(|item| item.kind.visible_in(revision))
            .unwrap_or(true);
        if !show {
            omitted.extend(scalar..scalar + run.len);
        }
        scalar += run.len;
    }
    let mut text = String::new();
    let mut used_cache = false;
    let mut scalar = 0usize;
    for segment in paragraph.segments() {
        match segment {
            Segment::Text(chunk) => {
                for ch in chunk.chars() {
                    if !omitted.contains(&scalar) {
                        text.push(ch);
                    }
                    scalar += 1;
                }
            }
            Segment::Inline(object) => {
                if !omitted.contains(&scalar) {
                    match &object.kind {
                        InlineKind::PageField(field) => {
                            let (rendered, cache) =
                                render_field(field, &numbering, ctx, view.field_codes);
                            used_cache |= cache;
                            text.push_str(&rendered);
                        }
                        _ => return Ok(None),
                    }
                }
                scalar += 1;
            }
        }
    }
    Ok(Some((text, used_cache)))
}

pub(crate) struct FieldExpansion {
    pub display: String,
    /// Display scalar `i` belongs to this paragraph scalar. Field digits share
    /// the one object-marker scalar.
    pub source_scalars: Vec<usize>,
    pub used_cache: bool,
}

/// Replace page fields with the text layout should shape. Hidden scalars and
/// field-annotation ranges in `omitted` are left out. Field codes show the
/// instruction name instead of the number. The map sends display scalars back
/// to the paragraph, where the field is one marker.
pub(crate) fn expand_page_fields(
    paragraph: &Paragraph,
    numbering: &joyword_model::PageNumbering,
    ctx: crate::cache::ExternalContext,
    view: ViewOptions,
    omitted: &BTreeSet<usize>,
) -> FieldExpansion {
    let mut display = String::new();
    let mut source_scalars = Vec::new();
    let mut source_scalar = 0usize;
    let mut used_cache = false;
    for segment in paragraph.segments() {
        match segment {
            Segment::Text(chunk) => {
                for ch in chunk.chars() {
                    if !omitted.contains(&source_scalar) {
                        source_scalars.push(source_scalar);
                        display.push(ch);
                    }
                    source_scalar += 1;
                }
            }
            Segment::Inline(object) => {
                if !omitted.contains(&source_scalar) {
                    let rendered = match &object.kind {
                        InlineKind::PageField(field) => {
                            let (rendered, cache) =
                                render_field(field, numbering, ctx, view.field_codes);
                            used_cache |= cache;
                            rendered
                        }
                        _ => OBJECT_MARKER.to_string(),
                    };
                    for _ch in rendered.chars() {
                        source_scalars.push(source_scalar);
                    }
                    display.push_str(&rendered);
                }
                source_scalar += 1;
            }
        }
    }
    FieldExpansion {
        display,
        source_scalars,
        used_cache,
    }
}

pub(crate) fn remap_field_sources(lines: &mut [LayoutLine], map: &[usize], paragraph: &Paragraph) {
    if map.is_empty() {
        let end = paragraph.len();
        let bytes = scalar_byte(paragraph, end);
        if let Some(line) = lines.first_mut() {
            line.source.scalars = 0..end;
            line.source.bytes = 0..bytes;
            line.clusters.clear();
        }
        return;
    }
    for line in lines.iter_mut() {
        remap_display_range(&mut line.source, map, paragraph);
        for cluster in &mut line.clusters {
            remap_display_range(&mut cluster.source, map, paragraph);
        }
    }
}

fn remap_display_range(range: &mut SourceRange, map: &[usize], paragraph: &Paragraph) {
    if map.is_empty() {
        range.scalars = 0..0;
        range.bytes = 0..0;
        return;
    }
    let last_slot = map.len() - 1;
    if range.scalars.start >= map.len() {
        let end = map[last_slot].saturating_add(1);
        range.scalars = end..end;
        let byte = scalar_byte(paragraph, end);
        range.bytes = byte..byte;
        return;
    }
    let start_index = range.scalars.start.min(last_slot);
    let exclusive = range.scalars.end.min(map.len());
    let src_start = map[start_index];
    let src_end = if exclusive == 0 {
        src_start
    } else {
        map[exclusive - 1].saturating_add(1).max(src_start)
    };
    range.scalars = src_start..src_end;
    range.bytes = scalar_byte(paragraph, src_start)..scalar_byte(paragraph, src_end);
}

fn scalar_byte(paragraph: &Paragraph, scalar: usize) -> usize {
    paragraph
        .text()
        .scalar_to_utf8(joyword_model::ScalarOffset(scalar))
        .map(|offset| offset.0)
        .unwrap_or(0)
}

pub(crate) fn render_field(
    field: &PageField,
    numbering: &joyword_model::PageNumbering,
    ctx: crate::cache::ExternalContext,
    codes: bool,
) -> (String, bool) {
    if codes {
        return (field_instruction(field.kind).to_owned(), false);
    }
    if ctx.force_cache {
        return (
            field.cached.as_deref().unwrap_or("").to_owned(),
            field.cached.is_some(),
        );
    }
    let value = match field.kind {
        PageFieldKind::Page => ctx.displayed_page,
        PageFieldKind::NumPages => i64::from(ctx.num_pages),
        PageFieldKind::SectionPages => i64::from(ctx.section_pages),
    };
    let format = match field.kind {
        PageFieldKind::Page => &numbering.format,
        PageFieldKind::NumPages | PageFieldKind::SectionPages => &NumberFormat::Decimal,
    };
    if let Some(rendered) = format_page(format, value) {
        (rendered, false)
    } else if let Some(cached) = &field.cached {
        (cached.to_string(), true)
    } else {
        (String::new(), false)
    }
}

fn field_instruction(kind: PageFieldKind) -> &'static str {
    match kind {
        PageFieldKind::Page => "PAGE",
        PageFieldKind::NumPages => "NUMPAGES",
        PageFieldKind::SectionPages => "SECTIONPAGES",
    }
}

fn format_page(format: &NumberFormat, value: i64) -> Option<String> {
    if matches!(format, NumberFormat::Decimal) {
        return Some(value.to_string());
    }
    format.render(i32::try_from(value).ok()?)
}

fn map_line_limit(limit: crate::LineLimit) -> LayoutError {
    use crate::line::LineLimit;
    let kind = match limit {
        LineLimit::TextBytes => crate::LayoutLimit::TextBytes,
        LineLimit::ShapeCalls => crate::LayoutLimit::ShapeCalls,
        LineLimit::ShapedBytes => crate::LayoutLimit::ShapedBytes,
        LineLimit::Lines => crate::LayoutLimit::Lines,
        LineLimit::Breakpoints => crate::LayoutLimit::Lines,
    };
    LayoutError::LimitExceeded(kind)
}
