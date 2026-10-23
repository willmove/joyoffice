//! Review projection and markup paint (T300).
//!
//! Final and Original drop hidden runs before shaping, so line breaks follow
//! the visible text. Final with markup shapes every run and paints insertions
//! with an underline and author colour, deletions with a strikethrough. The
//! colour is an 8-entry palette of our own, hashed from the author. The
//! underline and strike are a preview measurement, not a Word match.
//!
//! Header and footer bands are fitted by `pages`. This module paints the same
//! marks onto those bands afterwards. Final and Original already drop hidden
//! header and footer text in that fitter, so they receive no decoration.

use std::collections::BTreeSet;
use std::ops::Range;

use joyword_foundation::Emu;
use joyword_model::{
    Document, HeaderFooterKind, InlineKind, Paragraph, RevisionKind, RevisionView, Segment, StoryId,
};

use crate::document::{
    LayoutCancellation, LayoutSnapshot, PageGeometry, SingleFontProfile, dominant_slot,
};
use crate::line::{LayoutLine, LineError, LineLimits, LineRevision, layout_paragraph_auto};
use crate::scene::{GlyphRun, SceneRect};
use crate::shape::paint_underline;
use crate::{FaceSet, LayoutError, LayoutLimit, ShapedGlyph};

pub(crate) struct Projection {
    pub text: String,
    pub map: Vec<usize>,
}

/// `Ok(None)` keeps the paragraph text. `Err` refuses a projection that
/// would have to guess around an image or a page break. Plain numbered
/// paragraphs retain their label only when their projected body is nonempty.
pub(crate) fn project(
    paragraph: &Paragraph,
    view: RevisionView,
    _list_layout: bool,
) -> Result<Option<Projection>, LayoutError> {
    let hidden = paragraph.runs().iter().any(|run| {
        run.revision
            .as_ref()
            .is_some_and(|revision| !revision.kind.visible_in(view))
    });
    if !hidden {
        return Ok(None);
    }
    if !paragraph.inlines().is_empty() {
        return Err(crate::document::unsupported(
            Some(paragraph.id()),
            "revision projection with an image or break",
        ));
    }
    let mut text = String::new();
    let mut map = Vec::new();
    let mut offset = 0usize;
    let plain = paragraph.plain_text(|_| String::new());
    let mut chars = plain.chars();
    for run in paragraph.runs() {
        let show = run
            .revision
            .as_ref()
            .map(|revision| revision.kind.visible_in(view))
            .unwrap_or(true);
        for _ in 0..run.len {
            let Some(ch) = chars.next() else {
                break;
            };
            if show {
                text.push(ch);
                map.push(offset);
            }
            offset += 1;
        }
    }
    Ok(Some(Projection { text, map }))
}

pub(crate) fn remap(lines: &mut [LayoutLine], map: &[usize], model_len: usize) {
    for line in &mut *lines {
        remap_range(&mut line.source.scalars, map);
        for cluster in &mut line.clusters {
            remap_range(&mut cluster.source.scalars, map);
        }
    }
    if let Some(first) = lines.first_mut() {
        first.source.scalars.start = 0;
    }
    if let Some(last) = lines.last_mut()
        && last.source.scalars.end < model_len
    {
        last.source.scalars.end = model_len;
    }
}

fn remap_range(range: &mut Range<usize>, map: &[usize]) {
    let start = mapped(map, range.start);
    let end = mapped(map, range.end);
    *range = start..end.max(start);
}

fn mapped(map: &[usize], visible: usize) -> usize {
    if visible < map.len() {
        map[visible]
    } else {
        map.last().map(|scalar| scalar + 1).unwrap_or(0)
    }
}

pub(crate) fn annotate(lines: &mut [LayoutLine], paragraph: &Paragraph, view: RevisionView) {
    if view != RevisionView::FinalWithMarkup {
        return;
    }
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for run in paragraph.runs() {
        let end = offset + run.len;
        if let Some(revision) = &run.revision {
            spans.push((offset..end, revision.kind, author_color(&revision.author)));
        }
        offset = end;
    }
    if spans.is_empty() {
        return;
    }
    for line in lines {
        for (range, kind, color) in &spans {
            let start = range.start.max(line.source.scalars.start);
            let end = range.end.min(line.source.scalars.end);
            if start < end {
                line.revisions.push(LineRevision {
                    scalars: start..end,
                    kind: *kind,
                    red: color.0,
                    green: color.1,
                    blue: color.2,
                });
            }
        }
    }
}

pub(crate) fn author_color(author: &str) -> (u8, u8, u8) {
    const PALETTE: [(u8, u8, u8); 8] = [
        (0xC0, 0x39, 0x2B),
        (0x29, 0x80, 0xB9),
        (0x27, 0xAE, 0x60),
        (0x8E, 0x44, 0xAD),
        (0xD3, 0x54, 0x00),
        (0x16, 0xA0, 0x85),
        (0x2C, 0x3E, 0x50),
        (0xB7, 0x95, 0x0B),
    ];
    let mut hash = 0x811c_9dc5u32;
    for byte in author.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    PALETTE[(hash as usize) % PALETTE.len()]
}

/// A vertical bar in the page's left margin beside a changed line.
///
/// The bar uses the line's first author colour. It sits in the margin, short
/// of the content edge, so screen and PDF paint the same indicator. It is a
/// JoyWord mark, not a comment balloon.
pub(crate) fn margin_bar(
    geometry: crate::document::PageGeometry,
    line: &LayoutLine,
    top: Emu,
    height: Emu,
) -> Option<SceneRect> {
    let mark = line.revisions.first()?;
    if height.raw() <= 0 {
        return None;
    }
    let margin = geometry.left.raw();
    let gap = 4 * Emu::PER_TWIP;
    let width = Emu::PER_POINT.min(margin / 3).max(2 * Emu::PER_TWIP);
    if margin < width.saturating_add(gap) {
        return None;
    }
    Some(SceneRect {
        x: Emu::new(margin - gap - width),
        y: top,
        width: Emu::new(width),
        height,
        red: mark.red,
        green: mark.green,
        blue: mark.blue,
    })
}

/// Splits one fitted line into glyph runs. Insertions and move destinations
/// are coloured and underlined. Deletions and move sources are struck through.
pub(crate) fn marked_runs(
    line: &LayoutLine,
    origin_x: Emu,
    baseline_y: Emu,
) -> (Vec<GlyphRun>, Vec<SceneRect>) {
    if line.revisions.is_empty() || line.run.glyphs.is_empty() {
        return (vec![line.to_glyph_run(origin_x, baseline_y)], Vec::new());
    }
    let mut groups: Vec<(Range<usize>, Option<usize>)> = Vec::new();
    for (index, cluster) in line.clusters.iter().enumerate() {
        let mark = line.revisions.iter().position(|revision| {
            cluster.source.scalars.start < revision.scalars.end
                && revision.scalars.start < cluster.source.scalars.end
        });
        if let Some((range, current)) = groups.last_mut()
            && *current == mark
        {
            range.end = index + 1;
        } else {
            groups.push((index..index + 1, mark));
        }
    }
    if groups.is_empty() {
        return (vec![line.to_glyph_run(origin_x, baseline_y)], Vec::new());
    }
    let mut runs = Vec::new();
    let mut rects = Vec::new();
    // List decorations have no source cluster. Keep the rendered label before
    // the marked body pieces; otherwise a markup heading loses its number.
    let first_body = line
        .clusters
        .first()
        .map(|cluster| cluster.glyphs.start)
        .unwrap_or(0);
    if first_body > 0 && first_body <= line.run.glyphs.len() {
        let end = line
            .run
            .glyphs
            .get(first_body)
            .map(|glyph| glyph.cluster)
            .unwrap_or(line.run.text.len());
        if line.run.text.is_char_boundary(end) {
            let mut label = line.run.clone();
            label.text = line.run.text[..end].to_owned();
            label.glyphs = line.run.glyphs[..first_body].to_vec();
            if line.label_size.raw() > 0 {
                label.size = line.label_size;
            }
            label.bold = line.label_bold;
            runs.push(GlyphRun::place(&label, origin_x, baseline_y));
        }
    }
    for (clusters, mark) in groups {
        let glyphs =
            line.clusters[clusters.start].glyphs.start..line.clusters[clusters.end - 1].glyphs.end;
        if glyphs.start >= glyphs.end || glyphs.end > line.run.glyphs.len() {
            continue;
        }
        let piece = &line.run.glyphs[glyphs.clone()];
        let byte_start = piece[0].cluster;
        let byte_end = line
            .run
            .glyphs
            .get(glyphs.end)
            .map(|glyph| glyph.cluster)
            .unwrap_or(line.run.text.len())
            .max(byte_start)
            .min(line.run.text.len());
        if !line.run.text.is_char_boundary(byte_start) || !line.run.text.is_char_boundary(byte_end)
        {
            continue;
        }
        let first_x = piece[0].x.raw();
        let mut glyphs_out: Vec<ShapedGlyph> = piece.to_vec();
        for glyph in &mut glyphs_out {
            glyph.cluster = glyph.cluster.saturating_sub(byte_start);
            glyph.x = Emu::new(glyph.x.raw().saturating_sub(first_x));
        }
        let mark = mark.and_then(|index| line.revisions.get(index));
        let color = mark
            .map(|mark| [mark.red, mark.green, mark.blue])
            .unwrap_or([0, 0, 0]);
        let mut run = GlyphRun::place(&line.run, Emu::new(origin_x.raw() + first_x), baseline_y);
        run.text = line.run.text[byte_start..byte_end].to_string();
        run.glyphs = glyphs_out;
        run.color = color;
        let width = piece
            .last()
            .map(|glyph| glyph.x.raw() + glyph.advance.raw() - first_x)
            .unwrap_or(0);
        if let Some(mark) = mark
            && width > 0
        {
            let thickness = (line.run.size.raw() / 16).max(Emu::PER_TWIP);
            let (y, draw) = match mark.kind {
                RevisionKind::Insert | RevisionKind::MoveTo => {
                    (baseline_y.raw() + line.run.size.raw() / 10, true)
                }
                RevisionKind::Delete | RevisionKind::MoveFrom => {
                    (baseline_y.raw() - line.run.size.raw() / 3, true)
                }
            };
            if draw {
                rects.push(SceneRect {
                    x: Emu::new(origin_x.raw() + first_x),
                    y: Emu::new(y),
                    width: Emu::new(width),
                    height: Emu::new(thickness),
                    red: mark.red,
                    green: mark.green,
                    blue: mark.blue,
                });
            }
        }
        runs.push(run);
    }
    if runs.is_empty() {
        (vec![line.to_glyph_run(origin_x, baseline_y)], Vec::new())
    } else {
        (runs, rects)
    }
}

/// Paint header and footer revision marks onto bands `pages` already placed.
///
/// Story choice matches `pages::paint_stories` (`titlePg`, even and odd
/// headers, nearest earlier reference of the same kind). Text is shaped again
/// from the same visible string so the placed run can be found and replaced.
/// A paragraph that also holds an image keeps the image fitter's line boxes
/// and is not marked: that fitter does not project revisions.
pub(crate) fn decorate_stories(
    snapshot: &mut LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    cancellation: &LayoutCancellation,
) -> Result<(), LayoutError> {
    cancellation.check()?;
    if snapshot.key().revision_view != RevisionView::FinalWithMarkup {
        return Ok(());
    }
    if !referenced_revision(document) {
        return Ok(());
    }
    let page_count = snapshot.page_count();
    let even_and_odd = document.settings().even_and_odd_headers == Some(true);
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
            let placed = place_story(
                snapshot,
                document,
                faces,
                profile,
                cancellation,
                page_index,
                geometry,
                story,
                true,
            )?;
            apply_markup(snapshot, page_index, placed)?;
        }
        if let Some(story) = linked_story(document, section, false, kind) {
            let placed = place_story(
                snapshot,
                document,
                faces,
                profile,
                cancellation,
                page_index,
                geometry,
                story,
                false,
            )?;
            apply_markup(snapshot, page_index, placed)?;
        }
    }
    Ok(())
}

fn referenced_revision(document: &Document) -> bool {
    document.sections().iter().any(|section| {
        section
            .headers
            .iter()
            .chain(section.footers.iter())
            .any(|(_, id)| story_has_revision(document, *id))
    })
}

fn story_has_revision(document: &Document, id: StoryId) -> bool {
    document.story(id).is_some_and(|story| {
        story.blocks.iter().any(|block| {
            block
                .as_paragraph()
                .is_some_and(|paragraph| paragraph.runs().iter().any(|run| run.revision.is_some()))
        })
    })
}

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

struct LocalLine {
    line_box: Emu,
    baseline: i64,
    marked: Option<LayoutLine>,
}

struct PlacedMarkup {
    plain: GlyphRun,
    runs: Vec<GlyphRun>,
    rects: Vec<SceneRect>,
}

struct DisplayText {
    text: String,
    /// Display character `i` shows this model scalar.
    map: Vec<usize>,
}

#[allow(clippy::too_many_arguments)]
fn place_story(
    snapshot: &LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    cancellation: &LayoutCancellation,
    page_index: usize,
    geometry: PageGeometry,
    story_id: StoryId,
    header: bool,
) -> Result<Vec<PlacedMarkup>, LayoutError> {
    let Some(story) = document.story(story_id) else {
        return Ok(Vec::new());
    };
    let mut lines = Vec::new();
    for block in story.blocks.iter() {
        cancellation.check()?;
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        append_paragraph(
            snapshot, document, faces, profile, page_index, geometry, paragraph, &mut lines,
        )?;
    }
    Ok(position_lines(geometry, header, lines))
}

#[allow(clippy::too_many_arguments)]
fn append_paragraph(
    snapshot: &LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    page_index: usize,
    geometry: PageGeometry,
    paragraph: &Paragraph,
    lines: &mut Vec<LocalLine>,
) -> Result<(), LayoutError> {
    if paragraph
        .inlines()
        .iter()
        .any(|inline| matches!(inline.kind, InlineKind::Image { .. }))
    {
        if let Some(image_lines) =
            image_lines(snapshot, document, faces, profile, paragraph, geometry)?
        {
            lines.extend(image_lines);
        }
        return Ok(());
    }
    let Some((_, section)) = snapshot.page_box(page_index) else {
        return Ok(());
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
    let view = snapshot.key().view;
    let Some(display) = visible_display(
        document,
        paragraph,
        view,
        snapshot.key().revision_view,
        &numbering,
        ctx,
    )?
    else {
        return Ok(());
    };
    if display.text.is_empty() {
        return Ok(());
    }
    let painted = match crate::style_paint::paint(document, paragraph, profile, view, None) {
        Ok(painted) => painted,
        Err(LayoutError::Cancelled) => return Err(LayoutError::Cancelled),
        Err(LayoutError::LimitExceeded(limit)) => return Err(LayoutError::LimitExceeded(limit)),
        Err(_) => return Ok(()),
    };
    let font = faces.face(dominant_slot(&display.text));
    let fitted = match layout_paragraph_auto(
        font,
        &display.text,
        painted.size,
        profile.features,
        geometry.content_width(),
        LineLimits::default(),
    ) {
        Ok(fitted) => fitted,
        Err(LineError::Cancelled) => return Err(LayoutError::Cancelled),
        Err(LineError::LimitExceeded(limit)) => return Err(story_line_limit(limit)),
        Err(_) => return Ok(()),
    };
    let line_box = if painted.leading.raw() > 0 {
        painted.leading
    } else {
        profile.leading
    };
    let baseline = painted.baseline.raw().clamp(0, line_box.raw());
    let underline = paint_underline(font, painted.size, painted.underline);
    let mut fitted_lines = fitted.lines;
    if display.map.len() == display.text.chars().count() {
        remap_display(&mut fitted_lines, &display.map);
        annotate(&mut fitted_lines, paragraph, snapshot.key().revision_view);
    }
    for mut line in fitted_lines {
        line.run.bold = painted.bold;
        line.run.italic = painted.italic;
        line.run.latin_family.clone_from(&painted.latin);
        line.run.east_asia_family.clone_from(&painted.east_asia);
        line.run.color = painted.color;
        line.run.underline = underline;
        let marked = if line.revisions.is_empty() {
            None
        } else {
            Some(line)
        };
        lines.push(LocalLine {
            line_box,
            baseline,
            marked,
        });
    }
    Ok(())
}

fn remap_display(lines: &mut [LayoutLine], map: &[usize]) {
    for line in lines {
        remap_display_range(&mut line.source.scalars, map);
        for cluster in &mut line.clusters {
            remap_display_range(&mut cluster.source.scalars, map);
        }
    }
}

fn remap_display_range(range: &mut Range<usize>, map: &[usize]) {
    if map.is_empty() || range.start >= map.len() {
        let end = map
            .last()
            .copied()
            .map(|scalar| scalar.saturating_add(1))
            .unwrap_or(0);
        *range = end..end;
        return;
    }
    let exclusive = range.end.min(map.len());
    let src_start = map[range.start];
    let src_end = if exclusive <= range.start {
        src_start
    } else {
        map[exclusive - 1].saturating_add(1).max(src_start)
    };
    *range = src_start..src_end;
}

fn visible_display(
    document: &Document,
    paragraph: &Paragraph,
    view: crate::view::ViewOptions,
    revision: RevisionView,
    numbering: &joyword_model::PageNumbering,
    ctx: crate::cache::ExternalContext,
) -> Result<Option<DisplayText>, LayoutError> {
    let mut omitted = crate::view::omitted_indices(document, paragraph, view)?;
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
    let mut map = Vec::new();
    let mut scalar = 0usize;
    for segment in paragraph.segments() {
        match segment {
            Segment::Text(chunk) => {
                for ch in chunk.chars() {
                    if !omitted.contains(&scalar) {
                        text.push(ch);
                        map.push(scalar);
                    }
                    scalar += 1;
                }
            }
            Segment::Inline(object) => {
                if !omitted.contains(&scalar) {
                    match &object.kind {
                        InlineKind::PageField(field) => {
                            let (rendered, _) = crate::pages::render_field(
                                field,
                                numbering,
                                &ctx,
                                view.field_codes,
                            );
                            for _ch in rendered.chars() {
                                map.push(scalar);
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
    Ok(Some(DisplayText { text, map }))
}

fn image_lines(
    snapshot: &LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    paragraph: &Paragraph,
    geometry: PageGeometry,
) -> Result<Option<Vec<LocalLine>>, LayoutError> {
    match crate::pages::accept_body_inlines(document, paragraph) {
        Ok(()) => {}
        Err(LayoutError::Cancelled) => return Err(LayoutError::Cancelled),
        Err(LayoutError::LimitExceeded(limit)) => return Err(LayoutError::LimitExceeded(limit)),
        Err(_) => return Ok(None),
    }
    let view = snapshot.key().view;
    let painted = match crate::style_paint::paint(document, paragraph, profile, view, None) {
        Ok(painted) => painted,
        Err(LayoutError::Cancelled) => return Err(LayoutError::Cancelled),
        Err(LayoutError::LimitExceeded(limit)) => return Err(LayoutError::LimitExceeded(limit)),
        Err(_) => return Ok(None),
    };
    let font = faces.face(dominant_slot(&paragraph.plain_text(|_| String::new())));
    let line_box = if painted.leading.raw() > 0 {
        painted.leading
    } else {
        profile.leading
    };
    let descent = Emu::new((line_box.raw() - painted.baseline.raw()).max(0));
    let available = Emu::new((geometry.bottom_edge().raw() - geometry.top.raw()).max(1));
    let fitted = {
        let mut fit_text = |text: &str, _byte: usize, width: Emu| {
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
                LineError::LimitExceeded(limit) => story_line_limit(limit),
                _ => crate::document::unsupported(Some(paragraph.id()), "header or footer image"),
            })
        };
        match crate::images::fit_inline_images(
            document,
            paragraph,
            font,
            painted.size,
            geometry.content_width(),
            available,
            line_box,
            descent,
            &mut fit_text,
        ) {
            Ok(fitted) => fitted,
            Err(LayoutError::Cancelled) => return Err(LayoutError::Cancelled),
            Err(LayoutError::LimitExceeded(limit)) => {
                return Err(LayoutError::LimitExceeded(limit));
            }
            Err(_) => return Ok(None),
        }
    };
    if fitted.lines.is_empty() {
        return Ok(None);
    }
    let mut lines = Vec::new();
    for line in fitted.lines {
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
        lines.push(LocalLine {
            line_box: box_height,
            baseline,
            marked: None,
        });
    }
    Ok(Some(lines))
}

fn position_lines(
    geometry: PageGeometry,
    header: bool,
    lines: Vec<LocalLine>,
) -> Vec<PlacedMarkup> {
    let mut placed = Vec::new();
    if header {
        let mut y = geometry.header;
        for line in lines {
            let Some(baseline_y) = y.checked_add(Emu::new(line.baseline)) else {
                break;
            };
            let top = y;
            let Some(next) = y.checked_add(line.line_box) else {
                break;
            };
            push_placed(&mut placed, geometry, &line, top, baseline_y);
            y = next;
        }
    } else {
        let total: i64 = lines.iter().map(|line| line.line_box.raw()).sum();
        let mut y = geometry.height.raw() - geometry.footer.raw() - total;
        if y < 0 {
            y = 0;
        }
        for line in lines {
            let Some(baseline_y) = Emu::new(y).checked_add(Emu::new(line.baseline)) else {
                break;
            };
            let top = Emu::new(y);
            push_placed(&mut placed, geometry, &line, top, baseline_y);
            y += line.line_box.raw();
        }
    }
    placed
}

fn push_placed(
    placed: &mut Vec<PlacedMarkup>,
    geometry: PageGeometry,
    line: &LocalLine,
    top: Emu,
    baseline_y: Emu,
) {
    let Some(marked) = &line.marked else {
        return;
    };
    let plain = marked.to_glyph_run(geometry.left, baseline_y);
    let (runs, mut rects) = marked_runs(marked, geometry.left, baseline_y);
    if let Some(bar) = margin_bar(geometry, marked, top, line.line_box) {
        rects.push(bar);
    }
    if runs.is_empty() && rects.is_empty() {
        return;
    }
    let runs = if runs.is_empty() {
        vec![plain.clone()]
    } else {
        runs
    };
    placed.push(PlacedMarkup { plain, runs, rects });
}

fn apply_markup(
    snapshot: &mut LayoutSnapshot,
    page: usize,
    placed: Vec<PlacedMarkup>,
) -> Result<(), LayoutError> {
    let mut edits = {
        let Some(runs) = snapshot.page_runs(page) else {
            return Err(LayoutError::GeometryOverflow);
        };
        let mut used = BTreeSet::new();
        let mut edits = Vec::new();
        for item in placed {
            let Some(index) = runs
                .iter()
                .enumerate()
                .find(|(index, run)| !used.contains(index) && *run == &item.plain)
                .map(|(index, _)| index)
            else {
                continue;
            };
            used.insert(index);
            edits.push((index, item.runs, item.rects));
        }
        edits
    };
    edits.sort_by_key(|(index, _, _)| std::cmp::Reverse(*index));
    for (index, runs, rects) in edits {
        snapshot.replace_page_run(page, index, runs, rects)?;
    }
    Ok(())
}

fn story_line_limit(limit: crate::LineLimit) -> LayoutError {
    let kind = match limit {
        crate::LineLimit::TextBytes => LayoutLimit::TextBytes,
        crate::LineLimit::ShapeCalls => LayoutLimit::ShapeCalls,
        crate::LineLimit::ShapedBytes => LayoutLimit::ShapedBytes,
        crate::LineLimit::Lines | crate::LineLimit::Breakpoints => LayoutLimit::Lines,
    };
    LayoutError::LimitExceeded(kind)
}
