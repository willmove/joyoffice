//! Section geometry, explicit page breaks, and header and footer stories.
//!
//! Page size is `w:pgSz/@w` and `@h` (ECMA-376 §17.6.13). `orient` is stored
//! with them and does not swap the axes. `w:type` places the section on the
//! next page, the same page, the next column, or the next even or odd page
//! (§17.6.22). `titlePg` paints the First story on the first page of that
//! section. `evenAndOddHeaders` paints the Even story on even displayed page
//! numbers and the Default story on odd ones. The displayed number is `PAGE`
//! after `w:pgNumType/@w:start` (§17.6.12). The section's first page still
//! counts, and `titlePg` wins there. An omitted reference inherits the nearest
//! earlier story of that same kind. An explicit story, including an empty one,
//! does not inherit and is not replaced by another kind. Equal-width `w:cols`,
//! and unequal columns with an explicit `w:col` width, share the content box.
//! A column break advances one column, or one page when the section has a
//! single column (§17.3.3.1, §17.6.3, §17.6.4). `w:cols/@w:sep` true paints a
//! separator rule between adjacent columns of that section (§17.6.4).

use std::collections::BTreeSet;

use joyword_foundation::Emu;
use joyword_model::{
    Anchor, AnnotationKind, BreakKind, Document, HeaderFooterKind, InlineKind, NodeId,
    NumberFormat, OBJECT_MARKER, PageField, PageFieldKind, Paragraph, Position, ScalarOffset,
    Section, SectionBreak, Segment, StoryId, StoryKind,
};

use crate::FaceSet;
use crate::document::dominant_slot;
use crate::document::{
    CaretStop, LayoutCancellation, LayoutError, LayoutNote, LayoutSnapshot, PageGeometry,
    PlacedLine, SingleFontProfile, unsupported,
};
use crate::line::{
    LayoutLine, LineError, LineImage, LineLimits, SourceRange, layout_paragraph_auto,
};
use crate::scene::{GlyphRun, SceneImage, SceneRect, decoration_rects};
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
    /// `w:type="nextColumn"`: the section starts in the next column of the
    /// current page when the column flow is unchanged.
    NextColumn,
}

impl SectionEdge {
    pub(crate) fn forces_page(self) -> bool {
        matches!(self, Self::NextPage | Self::EvenPage | Self::OddPage)
    }
}

/// Newspaper columns, or one column the width of the content box.
///
/// Widths and the gap after each column are EMU. The last gap is zero.
/// Equal columns share `width` and `gap`. Unequal columns keep one width
/// and one trailing gap per `w:col` (ECMA-376 §17.6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColumnSpec {
    pub count: u16,
    /// Gap used when every inter-column gap is the same. The first gap
    /// otherwise.
    pub gap: i64,
    /// Equal column width, or the first column's width.
    pub width: i64,
    widths: [i64; 45],
    after: [i64; 45],
}

impl ColumnSpec {
    pub(crate) fn single(width: i64) -> Self {
        let mut widths = [0; 45];
        widths[0] = width;
        Self {
            count: 1,
            gap: 0,
            width,
            widths,
            after: [0; 45],
        }
    }

    fn equal(count: u16, gap: i64, width: i64) -> Self {
        let mut widths = [0; 45];
        let mut after = [0; 45];
        let slots = usize::from(count).min(widths.len());
        for index in 0..slots {
            widths[index] = width;
            if index + 1 < slots {
                after[index] = gap;
            }
        }
        Self {
            count,
            gap,
            width,
            widths,
            after,
        }
    }

    pub(crate) fn uneven(&self) -> bool {
        let slots = usize::from(self.count).min(self.widths.len());
        if slots <= 1 {
            return false;
        }
        let width = self.widths[0];
        let gap = self.after[0];
        (0..slots).any(|index| self.widths[index] != width)
            || (0..slots - 1).any(|index| self.after[index] != gap)
    }

    pub(crate) fn min_width(&self) -> i64 {
        let slots = usize::from(self.count).min(self.widths.len());
        self.widths[..slots]
            .iter()
            .copied()
            .filter(|width| *width > 0)
            .min()
            .unwrap_or(self.width)
    }

    /// Width passed to the line fitter when every line uses one measure.
    /// Uneven columns use the narrowest column so a line can sit in any of
    /// them. A column break still fits the following piece to that column.
    pub(crate) fn fit_width_raw(&self) -> i64 {
        if self.uneven() {
            self.min_width()
        } else {
            self.width
        }
    }

    pub(crate) fn widths(&self) -> &[i64] {
        let slots = usize::from(self.count).min(self.widths.len());
        &self.widths[..slots]
    }

    /// Offset from the content left and the column width, both in EMU.
    pub(crate) fn frame(&self, index: u16) -> (i64, i64) {
        let slots = usize::from(self.count).min(self.widths.len());
        if slots == 0 {
            return (0, self.width);
        }
        let index = usize::from(index).min(slots - 1);
        let mut x = 0i64;
        for slot in 0..index {
            x = x
                .saturating_add(self.widths[slot])
                .saturating_add(self.after[slot]);
        }
        (x, self.widths[index])
    }

    pub(crate) fn same(&self, other: &Self) -> bool {
        self.count == other.count
            && self.widths() == other.widths()
            && self.after[..usize::from(self.count).min(self.after.len())]
                == other.after[..usize::from(other.count).min(other.after.len())]
    }
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
        if section
            .numbering
            .chapter_style
            .is_some_and(|level| !(1..=9).contains(&level))
        {
            return Err(unsupported(
                Some(section.id),
                "chapter heading outside Heading1..9",
            ));
        }
        if section.preserved.is_some() {
            return Err(unsupported(
                Some(section.id),
                "preserved section properties",
            ));
        }
        let geometry = PageGeometry::from_setup(&section.page)?;
        let columns = column_spec(
            section.id,
            geometry,
            section.columns.as_ref(),
            section.unequal_columns.as_ref(),
        )?;
        let edge = match section.break_kind {
            SectionBreak::NextPage => SectionEdge::NextPage,
            SectionBreak::Continuous => SectionEdge::Continuous,
            SectionBreak::EvenPage => SectionEdge::EvenPage,
            SectionBreak::OddPage => SectionEdge::OddPage,
            SectionBreak::NextColumn => SectionEdge::NextColumn,
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
            fit_width: Emu::new(columns.fit_width_raw()),
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

pub(crate) fn column_mix(columns: &ColumnSpec) -> i64 {
    if !columns.uneven() {
        return 0;
    }
    let mut mixed = i64::from(columns.count);
    let slots = usize::from(columns.count).min(columns.widths.len());
    for index in 0..slots {
        mixed = mixed
            .wrapping_mul(0x9E37)
            .wrapping_add(columns.widths[index])
            .wrapping_add(columns.after[index]);
    }
    if mixed == 0 { 1 } else { mixed }
}

fn column_spec(
    section: joyword_model::NodeId,
    geometry: PageGeometry,
    equal: Option<&joyword_model::EqualColumns>,
    unequal: Option<&joyword_model::UnequalColumns>,
) -> Result<ColumnSpec, LayoutError> {
    let total = geometry.content_width().raw();
    if equal.is_some() && unequal.is_some() {
        return Err(unsupported(Some(section), "conflicting column definitions"));
    }
    if let Some(columns) = unequal {
        return unequal_spec(section, total, columns);
    }
    let Some(columns) = equal else {
        return Ok(ColumnSpec::single(total));
    };
    let count = i64::from(columns.count);
    if count < 2 {
        return Ok(ColumnSpec::single(total));
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
    Ok(ColumnSpec::equal(count, gap, width))
}

fn unequal_spec(
    section: joyword_model::NodeId,
    total: i64,
    columns: &joyword_model::UnequalColumns,
) -> Result<ColumnSpec, LayoutError> {
    let count = columns.columns.len();
    if !(2..=45).contains(&count) {
        return Err(unsupported(Some(section), "columns do not fit the page"));
    }
    let mut widths = [0; 45];
    let mut after = [0; 45];
    let mut used = 0i64;
    for (index, column) in columns.columns.iter().enumerate() {
        if column.width <= 0 || column.space < 0 {
            return Err(unsupported(Some(section), "columns do not fit the page"));
        }
        let width = column
            .width
            .checked_mul(Emu::PER_TWIP)
            .ok_or(LayoutError::GeometryOverflow)?;
        let space = column
            .space
            .checked_mul(Emu::PER_TWIP)
            .ok_or(LayoutError::GeometryOverflow)?;
        widths[index] = width;
        if index + 1 == count {
            used = used
                .checked_add(width)
                .ok_or(LayoutError::GeometryOverflow)?;
        } else {
            after[index] = space;
            used = used
                .checked_add(width)
                .and_then(|sum| sum.checked_add(space))
                .ok_or(LayoutError::GeometryOverflow)?;
        }
    }
    if used > total || widths[..count].iter().any(|width| *width <= 0) {
        return Err(unsupported(Some(section), "columns do not fit the page"));
    }
    Ok(ColumnSpec {
        count: u16::try_from(count).unwrap_or(45),
        gap: after[0],
        width: widths[0],
        widths,
        after,
    })
}

/// Header and footer stories must be referenced. Comment stories are
/// paragraph metadata: they are not painted, and a bookmark inside one is
/// placed on the comment anchor. Anything else is still an unsupported extra
/// story. Non-paragraph blocks in an accepted story are rejected rather than
/// skipped.
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
        let allowed = match story.kind {
            StoryKind::Header | StoryKind::Footer => referenced.contains(&story.id),
            StoryKind::Comment => true,
            _ => false,
        };
        if !allowed {
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
            InlineKind::Ruby(_)
            | InlineKind::Combine(_)
            | InlineKind::Emphasis { .. }
            | InlineKind::Enclosed(_) => {}
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
            anchor,
        } => {
            let hash = document
                .resources()
                .get(*resource)
                .map(|item| item.hash().to_string())
                .unwrap_or_else(|| "missing".into());
            let placed = anchor
                .map(|anchor| format!("@{}x{}", anchor.x, anchor.y))
                .unwrap_or_default();
            format!("{OBJECT_MARKER}image:{hash}:{width}x{height}{placed}")
        }
        InlineKind::Preserved(_) => OBJECT_MARKER.to_string(),
        InlineKind::Ruby(ruby) => format!("{OBJECT_MARKER}ruby:{}[{}]", ruby.base, ruby.text),
        InlineKind::Combine(combine) => {
            format!(
                "{OBJECT_MARKER}combine:{:?}:{}",
                combine.brackets, combine.text
            )
        }
        InlineKind::Emphasis { text, mark } => {
            format!("{OBJECT_MARKER}emphasis:{mark:?}:{text}")
        }
        InlineKind::Enclosed(enclosed) => {
            format!(
                "{OBJECT_MARKER}enclosed:{:?}:{}",
                enclosed.shape, enclosed.text
            )
        }
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
            Segment::Inline(inline) => {
                if matches!(
                    inline.kind,
                    InlineKind::Break(BreakKind::Page | BreakKind::Column)
                ) {
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
                } else {
                    if !started {
                        origin_scalar = cursor_scalar;
                        origin_byte = cursor_byte;
                        started = true;
                    }
                    let inline_text = match &inline.kind {
                        InlineKind::Ruby(ruby) => ruby.base.as_str(),
                        InlineKind::Combine(combine) => combine.text.as_str(),
                        InlineKind::Emphasis { text, .. } => text.as_str(),
                        InlineKind::Enclosed(enclosed) => enclosed.text.as_str(),
                        _ => "",
                    };
                    text.push_str(inline_text);
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
    let mut recorded = BTreeSet::new();
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
            let record = recorded.insert(story);
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
                true,
                record,
            )?;
        }
        if let Some(story) = linked_story(document, section, false, kind) {
            let record = recorded.insert(story);
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
                true,
                record,
            )?;
        }
    }
    if snapshot.page_count() != page_count {
        return Err(LayoutError::Unstable);
    }
    // A referenced variant that this section does not show (a First header
    // while title pages are off) still needs a page caret, or a bookmark in
    // it would fail the whole PDF export. The glyphs stay unpainted.
    if page_count > 0
        && let Some((geometry, _)) = snapshot.page_box(0)
    {
        for story in document.stories() {
            if !matches!(story.kind, StoryKind::Header | StoryKind::Footer) {
                continue;
            }
            if recorded.contains(&story.id) {
                continue;
            }
            let referenced = document.sections().iter().any(|section| {
                let refs = if story.kind == StoryKind::Header {
                    &section.headers
                } else {
                    &section.footers
                };
                refs.iter().any(|(_, id)| *id == story.id)
            });
            if !referenced {
                continue;
            }
            let header = story.kind == StoryKind::Header;
            if header {
                paint_header(
                    snapshot,
                    document,
                    faces,
                    profile,
                    cancellation,
                    0,
                    page_count,
                    geometry,
                    story.id,
                    &mut exceeded,
                    false,
                    true,
                )?;
            } else {
                paint_footer(
                    snapshot,
                    document,
                    faces,
                    profile,
                    cancellation,
                    0,
                    page_count,
                    geometry,
                    story.id,
                    &mut exceeded,
                    false,
                    true,
                )?;
            }
        }
    }
    Ok(())
}

/// Thickness of a column separator rule. ECMA-376 §17.6.4 records only that
/// `w:cols/@w:sep` is on; it fixes neither the rule's thickness nor its exact
/// position, so the one-point hairline centered in the inter-column gap is a
/// JoyWord assumption, not a Word observation.
pub(crate) const COLUMN_SEPARATOR_THICKNESS: i64 = Emu::PER_POINT;

/// Paint a vertical separator rule between adjacent newspaper columns of every
/// page whose section sets `w:cols/@w:sep` true and has at least two columns
/// (ECMA-376 §17.6.4). Each rule is centered in the gap between two columns
/// and spans the body box from `w:pgMar/@w:top` to the bottom edge. The
/// rectangles land in the same [`PageScene`] the screen, PDF and CPU raster
/// consume; painting never adds a page, so a change in the page count would be
/// a bug rather than instability.
/// The separator rules of one page, derived only from the section's column
/// spec and the page box: the same rectangles [`paint_column_separators`]
/// appends after assembly. Empty when the section has no separator flag, a
/// spec error (which pagination would have surfaced), fewer than two columns,
/// or a non-positive body height.
pub(crate) fn separator_rects(section: &Section, geometry: &PageGeometry) -> Vec<SceneRect> {
    if !section.column_separator {
        return Vec::new();
    }
    // The section already laid out, so its columns fit; a spec error here
    // would contradict pagination and is skipped rather than surfaced.
    let Ok(spec) = column_spec(
        section.id,
        *geometry,
        section.columns.as_ref(),
        section.unequal_columns.as_ref(),
    ) else {
        return Vec::new();
    };
    if spec.count < 2 {
        return Vec::new();
    }
    let top = geometry.top.raw();
    let height = geometry.bottom_edge().raw().saturating_sub(top);
    if height <= 0 {
        return Vec::new();
    }
    let left = geometry.left.raw();
    let thickness = COLUMN_SEPARATOR_THICKNESS.max(1);
    let mut rects = Vec::new();
    for index in 1..spec.count {
        let (prev_offset, prev_width) = spec.frame(index - 1);
        let (next_offset, _) = spec.frame(index);
        let right_edge = left.saturating_add(prev_offset).saturating_add(prev_width);
        let left_edge = left.saturating_add(next_offset);
        let center = right_edge.saturating_add(left_edge) / 2;
        let x = center.saturating_sub(thickness / 2);
        rects.push(SceneRect {
            screen_only: false,
            x: Emu::new(x),
            y: Emu::new(top),
            width: Emu::new(thickness),
            height: Emu::new(height),
            red: 0,
            green: 0,
            blue: 0,
        });
    }
    rects
}

pub(crate) fn paint_column_separators(
    snapshot: &mut LayoutSnapshot,
    document: &Document,
    cancellation: &LayoutCancellation,
) -> Result<(), LayoutError> {
    let page_count = snapshot.page_count();
    for page_index in 0..page_count {
        cancellation.check()?;
        let Some((geometry, section_index)) = snapshot.page_box(page_index) else {
            continue;
        };
        let Some(section) = document.sections().get(section_index) else {
            continue;
        };
        let rects = separator_rects(section, &geometry);
        if rects.is_empty() {
            continue;
        }
        snapshot.extend_page_rects(page_index, rects)?;
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
    paint: bool,
    record: bool,
) -> Result<(), LayoutError> {
    let pieces = story_runs(
        snapshot,
        document,
        faces,
        profile,
        cancellation,
        page_index,
        page_count,
        geometry,
        story,
        paint,
    )?;
    let mut y = geometry.header;
    let mut crossed = false;
    let mut carets = Vec::new();
    for piece in pieces {
        match piece {
            StoryPiece::Gap(gap) => {
                if record {
                    carets.push(gap_caret(gap, page_index, geometry.left, y));
                }
            }
            StoryPiece::Line(line) => {
                let baseline = y
                    .checked_add(Emu::new(line.baseline))
                    .ok_or(LayoutError::GeometryOverflow)?;
                let next = y
                    .checked_add(line.line_box)
                    .ok_or(LayoutError::GeometryOverflow)?;
                if next.raw() > geometry.top.raw() {
                    crossed = true;
                }
                if record {
                    carets.extend(line_caret(page_index, &line, y, baseline));
                }
                if paint {
                    place_story_line(snapshot, page_index, line, geometry.left, baseline)?;
                }
                y = next;
            }
        }
    }
    if record {
        snapshot.store_auxiliary(story, carets);
    }
    if paint && crossed {
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
    paint: bool,
    record: bool,
) -> Result<(), LayoutError> {
    let pieces = story_runs(
        snapshot,
        document,
        faces,
        profile,
        cancellation,
        page_index,
        page_count,
        geometry,
        story,
        paint,
    )?;
    let total: i64 = pieces
        .iter()
        .map(|piece| match piece {
            StoryPiece::Line(line) => line.line_box.raw(),
            StoryPiece::Gap(_) => 0,
        })
        .sum();
    let mut y = geometry.height.raw() - geometry.footer.raw() - total;
    let mut crossed = false;
    if y < 0 {
        y = 0;
        crossed = true;
    }
    if y < geometry.bottom_edge().raw() && total > 0 {
        crossed = true;
    }
    let mut carets = Vec::new();
    for piece in pieces {
        match piece {
            StoryPiece::Gap(gap) => {
                if record {
                    carets.push(gap_caret(gap, page_index, geometry.left, Emu::new(y)));
                }
            }
            StoryPiece::Line(line) => {
                let baseline = Emu::new(y)
                    .checked_add(Emu::new(line.baseline))
                    .ok_or(LayoutError::GeometryOverflow)?;
                let box_height = line.line_box.raw();
                if record {
                    carets.extend(line_caret(page_index, &line, Emu::new(y), baseline));
                }
                if paint {
                    place_story_line(snapshot, page_index, line, geometry.left, baseline)?;
                }
                y += box_height;
            }
        }
    }
    if record {
        snapshot.store_auxiliary(story, carets);
    }
    if paint && crossed {
        note_exceeds(snapshot, story, exceeded);
    }
    Ok(())
}

fn point_stops(x: Emu, len: usize) -> Vec<CaretStop> {
    (0..=len)
        .map(|offset| CaretStop {
            offset: ScalarOffset(offset),
            x,
        })
        .collect()
}

fn point_carets(paragraph: NodeId, len: usize, x: Emu) -> LineCarets {
    LineCarets {
        paragraph,
        len,
        source: SourceRange {
            bytes: 0..0,
            scalars: 0..len,
        },
        stops: point_stops(x, len),
    }
}

fn gap_caret(gap: GapCaret, page: usize, x: Emu, top: Emu) -> (PlacedLine, usize) {
    let carets = point_carets(gap.paragraph, gap.len, x);
    (
        PlacedLine {
            paragraph: carets.paragraph,
            source: carets.source,
            page,
            run: 0,
            top,
            baseline: top,
            height: Emu::new(0),
            carets: carets.stops,
            images: 0..0,
            tab_gaps: Vec::new(),
        },
        gap.len,
    )
}

fn line_caret(page: usize, line: &StoryLine, top: Emu, baseline: Emu) -> Vec<(PlacedLine, usize)> {
    let Some(carets) = &line.carets else {
        return Vec::new();
    };
    vec![(
        PlacedLine {
            paragraph: carets.paragraph,
            source: carets.source.clone(),
            page,
            run: 0,
            top,
            baseline,
            height: line.line_box,
            carets: carets.stops.clone(),
            images: 0..0,
            tab_gaps: Vec::new(),
        },
        carets.len,
    )]
}

/// Place every comment-story offset on the comment anchor's caret.
///
/// Comment text is not painted. A bookmark in the comment still needs a page
/// point, and that point is the anchor the comment is attached to. A detached
/// comment, or an anchor caret layout cannot resolve, is left without carets;
/// import does not insert a bookmark in that case.
pub(crate) fn record_comment_carets(snapshot: &mut LayoutSnapshot, document: &Document) {
    let mut pending = Vec::new();
    for story in document.stories() {
        if story.kind != StoryKind::Comment {
            continue;
        }
        let Some(start) = comment_anchor_start(document, story.id) else {
            continue;
        };
        let Ok(caret) = snapshot.caret(start) else {
            continue;
        };
        let mut items = Vec::new();
        for block in story.blocks.iter() {
            let Some(paragraph) = block.as_paragraph() else {
                continue;
            };
            let len = paragraph.len();
            let placed = PlacedLine {
                paragraph: paragraph.id(),
                source: SourceRange {
                    bytes: 0..0,
                    scalars: 0..len,
                },
                page: caret.page,
                run: 0,
                top: caret.top,
                baseline: caret.baseline,
                height: caret.height,
                carets: point_stops(caret.x, len),
                images: 0..0,
                tab_gaps: Vec::new(),
            };
            items.push((placed, len));
        }
        pending.push((story.id, items));
    }
    for (story, items) in pending {
        snapshot.store_auxiliary(story, items);
    }
}

fn comment_anchor_start(document: &Document, story: StoryId) -> Option<Position> {
    let annotation = document.annotations().find(|annotation| {
        matches!(
            &annotation.kind,
            AnnotationKind::Comment(comment) if comment.body == story
        )
    })?;
    let Anchor::Range { start, .. } = annotation.anchor else {
        return None;
    };
    Some(start)
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
    for image in &line.images {
        let scene = if let Some(top) = image.absolute_top {
            SceneImage {
                media_type: image.media_type.clone(),
                bytes: image.bytes.clone(),
                x: image.x,
                y: top,
                width: image.width,
                height: image.height,
            }
        } else {
            let y = baseline_y
                .raw()
                .checked_sub(image.height.raw())
                .ok_or(LayoutError::GeometryOverflow)?;
            let x = origin_x
                .checked_add(image.x)
                .ok_or(LayoutError::GeometryOverflow)?;
            SceneImage {
                media_type: image.media_type.clone(),
                bytes: image.bytes.clone(),
                x,
                y: Emu::new(y),
                width: image.width,
                height: image.height,
            }
        };
        snapshot.append_page_image(page_index, scene)?;
    }
    snapshot.extend_page_rects(page_index, rects)
}

/// A fitted header or footer line, not yet positioned on the page.
struct StoryLine {
    run: GlyphRun,
    line_box: Emu,
    baseline: i64,
    paragraph_end: bool,
    tab_gaps: Vec<(i64, i64)>,
    images: Vec<LineImage>,
    /// Model-scalar carets for this painted line. Absent on a continuation
    /// image line, which does not own a caret span.
    carets: Option<LineCarets>,
}

struct LineCarets {
    paragraph: NodeId,
    len: usize,
    source: SourceRange,
    stops: Vec<CaretStop>,
}

struct GapCaret {
    paragraph: NodeId,
    len: usize,
}

// `Line` is the common case. Boxing it would allocate every header and
// footer line only to shrink the rare `Gap` variant.
#[allow(clippy::large_enum_variant)]
enum StoryPiece {
    Line(StoryLine),
    /// A paragraph that contributes no painted line. Its bookmark still needs
    /// a caret, and the following line does not move.
    Gap(GapCaret),
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
    notes: bool,
) -> Result<Vec<StoryPiece>, LayoutError> {
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
        let gap = GapCaret {
            paragraph: paragraph.id(),
            len: paragraph.len(),
        };
        if paragraph
            .inlines()
            .iter()
            .any(|inline| matches!(inline.kind, InlineKind::Image { .. }))
        {
            match story_image_lines(snapshot, document, faces, profile, paragraph, geometry) {
                Ok(mut lines) => {
                    if let Some(first) = lines.first_mut() {
                        first.carets =
                            Some(point_carets(paragraph.id(), paragraph.len(), geometry.left));
                    }
                    runs.extend(lines.into_iter().map(StoryPiece::Line));
                }
                Err(LayoutError::Cancelled) => return Err(LayoutError::Cancelled),
                Err(LayoutError::LimitExceeded(limit)) => {
                    return Err(LayoutError::LimitExceeded(limit));
                }
                Err(_) => {
                    if notes {
                        snapshot.note(LayoutNote::StoryContentSkipped {
                            paragraph: paragraph.id(),
                        });
                    }
                    runs.push(StoryPiece::Gap(gap));
                }
            }
            continue;
        }
        let shown = match story_display(snapshot, document, paragraph, page_index, view)? {
            Some(text) => text,
            None => {
                if notes {
                    snapshot.note(LayoutNote::StoryContentSkipped {
                        paragraph: paragraph.id(),
                    });
                }
                runs.push(StoryPiece::Gap(gap));
                continue;
            }
        };
        if shown.used_cache && notes {
            let note = LayoutNote::PageFieldCached {
                paragraph: paragraph.id(),
            };
            if !snapshot.notes().contains(&note) {
                snapshot.note(note);
            }
        }
        if shown.display.is_empty() {
            runs.push(StoryPiece::Gap(gap));
            continue;
        }
        let painted = match style_paint::paint(document, paragraph, profile, view, None) {
            Ok(painted) => painted,
            Err(LayoutError::Cancelled) => return Err(LayoutError::Cancelled),
            Err(LayoutError::LimitExceeded(limit)) => {
                return Err(LayoutError::LimitExceeded(limit));
            }
            Err(_) => {
                if notes {
                    snapshot.note(LayoutNote::StoryContentSkipped {
                        paragraph: paragraph.id(),
                    });
                }
                runs.push(StoryPiece::Gap(gap));
                continue;
            }
        };
        let slot = dominant_slot(&shown.display);
        let font = faces.face(slot);
        let fitted = match layout_paragraph_auto(
            font,
            &shown.display,
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
                if notes {
                    snapshot.note(LayoutNote::StoryContentSkipped {
                        paragraph: paragraph.id(),
                    });
                }
                runs.push(StoryPiece::Gap(gap));
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
        let mut caret_lines = fitted.lines.clone();
        remap_field_sources(&mut caret_lines, &shown.sources, paragraph);
        let mut caret_stops = Vec::with_capacity(caret_lines.len());
        for caret_line in &caret_lines {
            caret_stops.push(crate::document::line_carets(caret_line, geometry.left)?);
        }
        for (index, mut line) in fitted.lines.into_iter().enumerate() {
            line.run.bold = painted.bold;
            line.run.italic = painted.italic;
            line.run.latin_family.clone_from(&painted.latin);
            line.run.east_asia_family.clone_from(&painted.east_asia);
            line.run.color = painted.color;
            line.run.underline = underline;
            let tab_gaps = line.tab_gaps.clone();
            let run = line.to_glyph_run(Emu::new(0), Emu::new(0));
            let carets = caret_lines.get(index).map(|caret_line| LineCarets {
                paragraph: paragraph.id(),
                len: paragraph.len(),
                source: caret_line.source.clone(),
                stops: caret_stops[index].clone(),
            });
            runs.push(StoryPiece::Line(StoryLine {
                run,
                line_box,
                baseline,
                paragraph_end: index == last,
                tab_gaps,
                images: Vec::new(),
                carets,
            }));
        }
    }
    Ok(runs)
}

/// Inline pictures in a header or footer, fitted like body pictures and
/// returned ready for the story band. Floating page anchors keep page
/// coordinates. A paragraph this slice cannot place is an error so the caller
/// can skip that paragraph without failing the page.
fn story_image_lines(
    snapshot: &LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    paragraph: &Paragraph,
    geometry: PageGeometry,
) -> Result<Vec<StoryLine>, LayoutError> {
    accept_body_inlines(document, paragraph)?;
    let view = snapshot.key().view;
    let painted = style_paint::paint(document, paragraph, profile, view, None)?;
    let slot = dominant_slot(&paragraph.plain_text(|_| String::new()));
    let font = faces.face(slot);
    let line_box = if painted.leading.raw() > 0 {
        painted.leading
    } else {
        profile.leading
    };
    let descent = Emu::new((line_box.raw() - painted.baseline.raw()).max(0));
    let available = Emu::new((geometry.bottom_edge().raw() - geometry.top.raw()).max(1));
    let fitted = crate::images::fit_inline_images(
        document,
        paragraph,
        font,
        painted.size,
        geometry.content_width(),
        available,
        line_box,
        descent,
        &mut |text, _byte, width| {
            layout_paragraph_auto(
                font,
                text,
                painted.size,
                profile.features,
                width,
                LineLimits::default(),
            )
            .map_err(|error| match error {
                LineError::Cancelled => LayoutError::Cancelled,
                LineError::LimitExceeded(limit) => map_line_limit(limit),
                _ => unsupported(Some(paragraph.id()), "header or footer image"),
            })
        },
    )?;
    if fitted.lines.is_empty() {
        return Err(unsupported(Some(paragraph.id()), "header or footer image"));
    }
    let underline = paint_underline(font, painted.size, painted.underline);
    let last = fitted.lines.len().saturating_sub(1);
    let mut lines = Vec::new();
    for (index, line) in fitted.lines.into_iter().enumerate() {
        let image_height = line
            .images
            .iter()
            .filter(|image| image.absolute_top.is_none())
            .map(|image| image.height.raw())
            .max()
            .unwrap_or(0);
        let box_height = if line.box_height.raw() > 0 {
            line.box_height
        } else {
            line_box
        };
        let baseline = if image_height > painted.baseline.raw() {
            image_height
        } else {
            painted.baseline.raw().clamp(0, box_height.raw())
        };
        let mut run = line.to_glyph_run(Emu::new(0), Emu::new(0));
        run.bold = painted.bold;
        run.italic = painted.italic;
        run.latin_family.clone_from(&painted.latin);
        run.east_asia_family.clone_from(&painted.east_asia);
        run.color = painted.color;
        run.underline = underline;
        lines.push(StoryLine {
            run,
            line_box: box_height,
            baseline,
            paragraph_end: index == last,
            tab_gaps: line.tab_gaps,
            images: line.images,
            carets: None,
        });
    }
    Ok(lines)
}

/// Chapter subset and assumptions:
/// - ECMA-376 pgNumType: one-based heading style, nearest numbered heading.
///   https://c-rex.net/samples/ooxml/e1/Part4/OOXML_P4_DOCX_pgNumType_topic_ID0EYWCT.html
/// - MS-OI29500 supplies the omitted separator's hyphen default:
///   https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/a3bbd188-7d1e-400c-9ea7-1d8f96eb62c4
/// - JoyWord deliberately does NOT implement ECMA's ambiguous "previous level"
///   fallback. Only a top-level body paragraph with the exact built-in heading
///   style (canonical name, not a guessed style ID/outline level) qualifies.
/// - Use the rendered numeric list label verbatim, including multilevel/literal
///   formatting but excluding its tab/space suffix. No body-text digit scraping,
///   bullets, absent labels or failed number renderings can supply a chapter.
/// - First eligible heading of the requested level starting on the physical page
///   wins for ALL its PAGE fields; otherwise use the last preceding one, across
///   sections. The last heading on this page becomes the next page's predecessor.
///   The page-owning section chooses the heading level/separator even when a
///   continuous/nextColumn section shares the page. This resolves "nearest"
///   deterministically, not a claim of Word/WPS visual parity.
/// - Actual first-line placement (not estimated page/column) and the same hidden
///   and revision projection as the body determine eligibility and activation.
pub(crate) fn chapter_prefixes(
    snapshot: &LayoutSnapshot,
    document: &Document,
    cancellation: &LayoutCancellation,
) -> Result<Vec<Option<std::sync::Arc<str>>>, LayoutError> {
    use joyword_model::{BuiltinStyle, StyleKind};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    let mut result = vec![None; snapshot.page_count()];
    if !document
        .sections()
        .iter()
        .any(|section| section.numbering.chapter_style.is_some())
    {
        return Ok(result);
    }
    // Body fields currently receive one feedback context per paragraph. A
    // chapter PAGE spanning physical pages cannot be rendered honestly by
    // that fitter; refuse rather than publish the first page's stale prefix.
    let field_paragraphs: BTreeSet<_> =
        document
            .body()
            .blocks
            .iter()
            .filter_map(|block| block.as_paragraph())
            .filter(|paragraph| {
                paragraph.inlines().iter().any(|inline| matches!(
            &inline.kind, InlineKind::PageField(field) if field.kind == PageFieldKind::Page
        ))
            })
            .map(Paragraph::id)
            .collect();
    let mut field_pages = BTreeMap::new();
    for line in snapshot.lines() {
        if field_paragraphs.contains(&line.paragraph)
            && field_pages
                .insert(line.paragraph, line.page)
                .is_some_and(|page| page != line.page)
        {
            return Err(unsupported(
                Some(line.paragraph),
                "chapter PAGE paragraph spans pages",
            ));
        }
    }
    let marks = joyword_model::list_marks(document);
    let mut candidates = BTreeMap::new();
    for block in document.body().blocks.iter() {
        cancellation.check()?;
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        let Some(style) = paragraph
            .style
            .as_ref()
            .and_then(|id| document.styles().get(id))
        else {
            continue;
        };
        let Some(BuiltinStyle::Heading(level @ 1..=9)) = style.builtin() else {
            continue;
        };
        if style.kind != StyleKind::Paragraph {
            continue;
        }
        let Some(Ok(label)) = marks.get(&paragraph.id()) else {
            continue;
        };
        if label.text.is_empty() || paragraph.is_empty() {
            continue;
        }
        let Some(reference) = joyword_model::list_reference(document, paragraph) else {
            continue;
        };
        let Ok(numbering) = document
            .numbering()
            .resolved_level(reference.num_id, reference.level)
        else {
            continue;
        };
        if matches!(
            numbering.level.format,
            NumberFormat::Bullet | NumberFormat::None
        ) {
            continue;
        }
        let mut omitted = view::omitted_indices(document, paragraph, snapshot.key().view)?;
        let mut scalar = 0;
        for run in paragraph.runs() {
            if run
                .revision
                .as_ref()
                .is_some_and(|revision| !revision.kind.visible_in(snapshot.key().revision_view))
            {
                omitted.extend(scalar..scalar + run.len);
            }
            scalar += run.len;
        }
        if (0..paragraph.len()).all(|scalar| omitted.contains(&scalar)) {
            continue;
        }
        // Fields take a separate fitter that does not paint list labels.
        if !paragraph.inlines().is_empty() {
            continue;
        }
        candidates.insert(
            paragraph.id(),
            (
                usize::from(level - 1),
                Arc::<str>::from(label.text.as_str()),
            ),
        );
    }
    let mut on_page = vec![Vec::new(); snapshot.page_count()];
    for line in snapshot.lines() {
        if let Some(candidate) = candidates.remove(&line.paragraph)
            && let Some(page) = on_page.get_mut(line.page)
        {
            page.push(candidate);
        }
    }
    let mut preceding: [Option<Arc<str>>; 9] = Default::default();
    for (page, headings) in on_page.into_iter().enumerate() {
        cancellation.check()?;
        let mut current = preceding.clone();
        let mut first = [false; 9];
        for (level, label) in headings {
            if !first[level] {
                current[level] = Some(Arc::clone(&label));
                first[level] = true;
            }
            preceding[level] = Some(label);
        }
        let Some((_, section)) = snapshot.page_box(page) else {
            continue;
        };
        let Some(numbering) = document
            .sections()
            .get(section)
            .map(|section| &section.numbering)
        else {
            continue;
        };
        let Some(level @ 1..=9) = numbering.chapter_style else {
            continue;
        };
        if let Some(label) = &current[usize::from(level - 1)] {
            result[page] = Some(Arc::from(format!(
                "{label}{}",
                numbering.chapter_separator.character()
            )));
        }
    }
    Ok(result)
}

struct StoryText {
    display: String,
    used_cache: bool,
    /// Display scalar `i` belongs to this paragraph scalar. Field digits share
    /// the one object-marker scalar, matching [`expand_page_fields`].
    sources: Vec<usize>,
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
) -> Result<Option<StoryText>, LayoutError> {
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
        chapter_prefix: snapshot.chapter_prefix(page_index),
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
    let mut sources = Vec::new();
    let mut used_cache = false;
    let mut scalar = 0usize;
    for segment in paragraph.segments() {
        match segment {
            Segment::Text(chunk) => {
                for ch in chunk.chars() {
                    if !omitted.contains(&scalar) {
                        sources.push(scalar);
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
                                render_field(field, &numbering, &ctx, view.field_codes);
                            used_cache |= cache;
                            for _ch in rendered.chars() {
                                sources.push(scalar);
                            }
                            text.push_str(&rendered);
                        }
                        _ => return Ok(None),
                    }
                }
                scalar += 1;
            }
        }
    }
    Ok(Some(StoryText {
        display: text,
        used_cache,
        sources,
    }))
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
                                render_field(field, numbering, &ctx, view.field_codes);
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
    ctx: &crate::cache::ExternalContext,
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
        if field.kind == PageFieldKind::Page
            && let Some(prefix) = &ctx.chapter_prefix
        {
            (format!("{prefix}{rendered}"), false)
        } else {
            (rendered, false)
        }
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
