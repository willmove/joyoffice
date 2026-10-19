//! Section geometry, explicit page breaks, and header and footer stories.
//!
//! Page size is `w:pgSz/@w` and `@h` (ECMA-376 §17.6.13). `orient` is stored
//! with them and does not swap the axes. `titlePg` paints the First story on
//! the first page of that section. `evenAndOddHeaders` paints the Even story
//! on even page numbers and the Default story on odd ones; the section's
//! first page still counts, and `titlePg` wins there. An omitted reference
//! inherits the nearest earlier story of that same kind. An explicit story,
//! including an empty one, does not inherit and is not replaced by another
//! kind. Page numbers are the same 1-based index as `PAGE`. `pgNumType` is
//! not applied.

use joyword_foundation::Emu;
use joyword_model::{
    BreakKind, Document, HeaderFooterKind, InlineKind, OBJECT_MARKER, PageFieldKind, Paragraph,
    Segment, StoryId, StoryKind,
};

use crate::FaceSet;
use crate::document::dominant_slot;
use crate::document::{
    LayoutCancellation, LayoutError, LayoutNote, LayoutSnapshot, PageGeometry, SingleFontProfile,
    unsupported,
};
use crate::line::{LayoutLine, LineError, LineLimits, SourceRange, layout_paragraph_auto};
use crate::scene::GlyphRun;
use crate::style_paint;

pub(crate) struct SectionPlan {
    pub geometry: PageGeometry,
    /// Index of the section's last body block.
    pub end: usize,
    pub section: usize,
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

pub(crate) fn section_break_before(plans: &[SectionPlan], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    match (plan_for(plans, index - 1), plan_for(plans, index)) {
        (Some(previous), Some(here)) => previous.section != here.section,
        _ => false,
    }
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
    let mut image = false;
    for inline in paragraph.inlines() {
        match &inline.kind {
            InlineKind::Break(BreakKind::Page) => page_break = true,
            InlineKind::Break(_) => {
                return Err(unsupported(Some(paragraph.id()), "line or column break"));
            }
            InlineKind::PageField(_) => {
                return Err(unsupported(
                    Some(paragraph.id()),
                    "page field outside header or footer",
                ));
            }
            InlineKind::Image { .. } => image = true,
            InlineKind::Preserved(_) => {
                return Err(unsupported(
                    Some(paragraph.id()),
                    "inline objects or breaks",
                ));
            }
        }
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
    Ok(())
}

/// Text plus markers so a page break or a page field cannot share a cache
/// key with the same visible characters.
pub(crate) fn marked_text(document: &Document, paragraph: &Paragraph) -> String {
    paragraph.plain_text(|object| match &object.kind {
        InlineKind::Break(BreakKind::Page) => format!("{OBJECT_MARKER}page"),
        InlineKind::Break(kind) => format!("{OBJECT_MARKER}{kind:?}"),
        InlineKind::PageField(kind) => format!("{OBJECT_MARKER}{kind:?}"),
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
            page_index + 1,
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
    page_number: usize,
    even_and_odd: bool,
) -> HeaderFooterKind {
    let title_page = document
        .sections()
        .get(section_index)
        .is_some_and(|section| section.title_page == Some(true));
    if first_of_section && title_page {
        HeaderFooterKind::First
    } else if even_and_odd && page_number.is_multiple_of(2) {
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
        snapshot.append_page_run(page_index, place_run(line.run, geometry.left, baseline))?;
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
        snapshot.append_page_run(page_index, place_run(line.run, geometry.left, baseline))?;
        y += line.line_box.raw();
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

fn place_run(mut run: GlyphRun, origin_x: Emu, baseline_y: Emu) -> GlyphRun {
    run.origin_x = origin_x;
    run.baseline_y = baseline_y;
    run
}

/// A fitted header or footer line, not yet positioned on the page.
struct StoryLine {
    run: GlyphRun,
    line_box: Emu,
    baseline: i64,
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
    page_count: usize,
    geometry: PageGeometry,
    story_id: StoryId,
) -> Result<Vec<StoryLine>, LayoutError> {
    let Some(story) = document.story(story_id) else {
        return Ok(Vec::new());
    };
    let mut runs = Vec::new();
    for block in story.blocks.iter() {
        cancellation.check()?;
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        let display = match story_display(paragraph, page_index + 1, page_count) {
            Some(text) => text,
            None => {
                snapshot.note(LayoutNote::StoryContentSkipped {
                    paragraph: paragraph.id(),
                });
                continue;
            }
        };
        if display.is_empty() {
            continue;
        }
        let painted = match style_paint::paint(document, paragraph, profile, None) {
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
        for mut line in fitted.lines {
            line.run.bold = painted.bold;
            line.run.italic = painted.italic;
            line.run.latin_family.clone_from(&painted.latin);
            line.run.east_asia_family.clone_from(&painted.east_asia);
            let run = line.to_glyph_run(Emu::new(0), Emu::new(0));
            runs.push(StoryLine {
                run,
                line_box,
                baseline,
            });
        }
    }
    Ok(runs)
}

fn story_display(paragraph: &Paragraph, page: usize, pages: usize) -> Option<String> {
    let mut text = String::new();
    for segment in paragraph.segments() {
        match segment {
            Segment::Text(chunk) => text.push_str(&chunk),
            Segment::Inline(object) => match object.kind {
                InlineKind::PageField(PageFieldKind::Page) => {
                    text.push_str(&page.to_string());
                }
                InlineKind::PageField(PageFieldKind::NumPages) => {
                    text.push_str(&pages.to_string());
                }
                _ => return None,
            },
        }
    }
    Some(text)
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
