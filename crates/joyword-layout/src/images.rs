//! Still images in the shared page scene (LAYOUT-006).
//!
//! Inline images are not shaped. Preceding text may share a line when the
//! image fits the remaining width. Text after an image stays on that line when
//! the first fitting span fits the width that remains; the rest starts the next
//! line. An extent larger than the content box is scaled for display.
//!
//! A page anchor (`ImageAnchor`) is painted at page coordinates. `wp:wrapNone`
//! does not take line width or line height. `wp:wrapSquare` keeps the same
//! painted rectangle and asks placement to keep glyphs outside the exclusion.
//! `wp:wrapTopAndBottom` keeps that rectangle and asks placement to move a
//! line that meets the vertical band entirely below it, with no side gap.
//! None of these is scaled into the content box. The stored bytes stay unchanged.

use std::ops::Range;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_model::{
    AnchorWrap, Document, InlineKind, Measure, OBJECT_MARKER, Paragraph, Segment, SquareWrap,
    TopBottomWrap, WrapSide,
};

use crate::document::unsupported;
use crate::line::{
    ClusterRange, LayoutLine, LineDiagnostic, LineImage, LineRevision, OverflowKind, SourceRange,
};
use crate::{LayoutError, LoadedFont, ParagraphLines, ShapedRun};

#[allow(clippy::too_many_arguments)]
pub(crate) fn fit_inline_images(
    document: &Document,
    paragraph: &Paragraph,
    font: &LoadedFont,
    size: Emu,
    content_width: Emu,
    available_height: Emu,
    line_box: Emu,
    descent: Emu,
    fit_text: &mut impl FnMut(&str, usize, Emu) -> Result<ParagraphLines, LayoutError>,
) -> Result<ParagraphLines, LayoutError> {
    let max_image_height = Emu::new((available_height.raw() - descent.raw()).max(1));
    let items = items(document, paragraph, content_width, max_image_height)?;
    let mut lines = Vec::new();
    let mut diagnostics = Vec::new();
    let mut omitted = 0usize;
    let mut stats = crate::LineStats::default();
    let mut index = 0usize;
    while index < items.len() {
        match &items[index] {
            Item::Text(piece) => {
                if piece.text.is_empty() {
                    index += 1;
                    continue;
                }
                let mut part = fit_text(&piece.text, piece.byte, content_width)?;
                shift_lines(&mut part.lines, piece.scalar, piece.byte);
                diagnostics.append(&mut part.diagnostics);
                omitted += part.omitted_diagnostics;
                stats.shape_calls += part.stats.shape_calls;
                stats.shaped_bytes += part.stats.shaped_bytes;
                let attach = items.get(index + 1).and_then(|item| match item {
                    Item::Image(image) if image.anchor.is_none() => Some(image.clone()),
                    Item::Image(_) | Item::Text(_) => None,
                });
                let mut produced = part.lines;
                let attached = if let Some(image) = attach {
                    produced.last_mut().is_some_and(|line| {
                        try_attach(line, &image, content_width, line_box, descent)
                    })
                } else {
                    false
                };
                if attached {
                    if let Some(Item::Image(image)) = items.get(index + 1)
                        && image.scaled
                    {
                        diagnostics.push(overflow(image, content_width));
                    }
                    index += 2;
                } else {
                    index += 1;
                }
                lines.append(&mut produced);
                if attached {
                    try_follow_text(
                        &mut lines,
                        &items,
                        &mut index,
                        content_width,
                        fit_text,
                        &mut diagnostics,
                        &mut omitted,
                        &mut stats,
                    )?;
                }
            }
            Item::Image(image) => {
                if image.anchor.is_some() {
                    if let Some(line) = lines.last_mut() {
                        attach_anchor(line, image);
                    } else {
                        lines.push(anchor_line(font, size, image));
                    }
                    index += 1;
                    try_follow_text(
                        &mut lines,
                        &items,
                        &mut index,
                        content_width,
                        fit_text,
                        &mut diagnostics,
                        &mut omitted,
                        &mut stats,
                    )?;
                    continue;
                }
                if image.scaled {
                    diagnostics.push(overflow(image, content_width));
                }
                lines.push(image_line(font, size, image, line_box, descent));
                index += 1;
                try_follow_text(
                    &mut lines,
                    &items,
                    &mut index,
                    content_width,
                    fit_text,
                    &mut diagnostics,
                    &mut omitted,
                    &mut stats,
                )?;
            }
        }
    }
    if lines.is_empty() {
        return Err(unsupported(
            Some(paragraph.id()),
            "inline image with numbering, tabs or a page break",
        ));
    }
    Ok(ParagraphLines {
        lines,
        diagnostics,
        omitted_diagnostics: omitted,
        stats,
    })
}

#[derive(Clone)]
struct FlowImage {
    media_type: Arc<str>,
    bytes: Arc<[u8]>,
    width: Emu,
    height: Emu,
    requested_width: Emu,
    scalar: usize,
    byte: usize,
    scaled: bool,
    /// Page origin when this picture does not flow with the line.
    anchor: Option<PageAnchor>,
}

#[derive(Clone)]
struct PageAnchor {
    x: Emu,
    y: Emu,
    square: Option<SquareWrap>,
    top_bottom: Option<TopBottomWrap>,
}

struct TextPiece {
    text: String,
    scalar: usize,
    byte: usize,
}

enum Item {
    Text(TextPiece),
    Image(FlowImage),
}

fn items(
    document: &Document,
    paragraph: &Paragraph,
    max_width: Emu,
    max_height: Emu,
) -> Result<Vec<Item>, LayoutError> {
    let mut items = Vec::new();
    let mut scalar = 0usize;
    let mut byte = 0usize;
    let mut text = String::new();
    let mut text_scalar = 0usize;
    let mut text_byte = 0usize;
    let mut text_open = false;
    let flush =
        |items: &mut Vec<Item>, text: &mut String, open: &mut bool, scalar: usize, byte: usize| {
            if *open {
                items.push(Item::Text(TextPiece {
                    text: std::mem::take(text),
                    scalar,
                    byte,
                }));
                *open = false;
            }
        };
    for segment in paragraph.segments() {
        match segment {
            Segment::Text(chunk) => {
                if !text_open {
                    text_scalar = scalar;
                    text_byte = byte;
                    text_open = true;
                }
                scalar += chunk.chars().count();
                byte += chunk.len();
                text.push_str(&chunk);
            }
            Segment::Inline(object) => {
                let InlineKind::Image {
                    resource,
                    width,
                    height,
                    anchor,
                } = &object.kind
                else {
                    return Err(unsupported(
                        Some(paragraph.id()),
                        "inline image with numbering, tabs or a page break",
                    ));
                };
                flush(
                    &mut items,
                    &mut text,
                    &mut text_open,
                    text_scalar,
                    text_byte,
                );
                let resource = document
                    .resources()
                    .get(*resource)
                    .ok_or_else(|| unsupported(Some(paragraph.id()), "missing image resource"))?;
                let (width, height) = absolute(*width, *height).ok_or_else(|| {
                    unsupported(Some(paragraph.id()), "image extent is not absolute")
                })?;
                let anchor = match anchor {
                    None => None,
                    Some(place) => {
                        let x = place.x.absolute().ok_or_else(|| {
                            unsupported(Some(paragraph.id()), "floating image position")
                        })?;
                        let y = place.y.absolute().ok_or_else(|| {
                            unsupported(Some(paragraph.id()), "floating image position")
                        })?;
                        let (square, top_bottom) = match place.wrap {
                            AnchorWrap::None => (None, None),
                            AnchorWrap::Square(square) => (Some(square), None),
                            AnchorWrap::TopAndBottom(band) => (None, Some(band)),
                        };
                        Some(PageAnchor {
                            x,
                            y,
                            square,
                            top_bottom,
                        })
                    }
                };
                items.push(Item::Image(FlowImage {
                    media_type: Arc::from(resource.media_type()),
                    bytes: Arc::from(resource.data()),
                    width,
                    height,
                    requested_width: width,
                    scalar,
                    byte,
                    scaled: false,
                    anchor,
                }));
                scalar += 1;
                byte += OBJECT_MARKER.len_utf8();
            }
        }
    }
    flush(
        &mut items,
        &mut text,
        &mut text_open,
        text_scalar,
        text_byte,
    );
    for item in &mut items {
        if let Item::Image(image) = item
            && image.anchor.is_none()
        {
            scale_to(image, max_width, max_height);
        }
    }
    Ok(items)
}

fn absolute(width: Measure, height: Measure) -> Option<(Emu, Emu)> {
    Some((width.absolute()?, height.absolute()?))
}

fn scale_to(image: &mut FlowImage, max_width: Emu, max_height: Emu) {
    let width = image.width.raw().max(1);
    let height = image.height.raw().max(1);
    let max_width = max_width.raw().max(1);
    let max_height = max_height.raw().max(1);
    if width <= max_width && height <= max_height {
        return;
    }
    let mut next_width = width;
    let mut next_height = height;
    if next_width > max_width {
        next_height = next_height.saturating_mul(max_width) / width;
        next_width = max_width;
    }
    if next_height > max_height {
        next_width = next_width.saturating_mul(max_height) / next_height.max(1);
        next_height = max_height;
    }
    image.width = Emu::new(next_width.max(1));
    image.height = Emu::new(next_height.max(1));
    image.scaled = true;
}

#[allow(clippy::too_many_arguments)]
fn try_follow_text(
    lines: &mut Vec<LayoutLine>,
    items: &[Item],
    index: &mut usize,
    content_width: Emu,
    fit_text: &mut impl FnMut(&str, usize, Emu) -> Result<ParagraphLines, LayoutError>,
    diagnostics: &mut Vec<LineDiagnostic>,
    omitted: &mut usize,
    stats: &mut crate::LineStats,
) -> Result<(), LayoutError> {
    let Some(Item::Text(piece)) = items.get(*index) else {
        return Ok(());
    };
    if piece.text.is_empty() {
        return Ok(());
    }
    let Some(line) = lines.last_mut() else {
        return Ok(());
    };
    if line.images.is_empty() {
        return Ok(());
    }
    let remain = content_width.raw().saturating_sub(line.run.width.raw());
    if remain <= 0 {
        return Ok(());
    }
    let Some(head) = fitting_head(
        &piece.text,
        piece.byte,
        Emu::new(remain),
        fit_text,
        diagnostics,
        omitted,
        stats,
    )?
    else {
        return Ok(());
    };
    let consumed_bytes = head.source.bytes.end;
    let consumed_scalars = head.source.scalars.end;
    if consumed_bytes == 0 || consumed_bytes > piece.text.len() {
        return Ok(());
    }
    merge_text(line, head, piece.scalar, piece.byte);
    if consumed_bytes < piece.text.len() {
        let mut rest = fit_text(
            &piece.text[consumed_bytes..],
            piece.byte + consumed_bytes,
            content_width,
        )?;
        shift_lines(
            &mut rest.lines,
            piece.scalar + consumed_scalars,
            piece.byte + consumed_bytes,
        );
        for diagnostic in &mut rest.diagnostics {
            shift_diagnostic(
                diagnostic,
                piece.scalar + consumed_scalars,
                piece.byte + consumed_bytes,
            );
        }
        diagnostics.append(&mut rest.diagnostics);
        *omitted += rest.omitted_diagnostics;
        stats.shape_calls += rest.stats.shape_calls;
        stats.shaped_bytes += rest.stats.shaped_bytes;
        lines.extend(rest.lines);
    }
    *index += 1;
    Ok(())
}

fn fitting_head(
    text: &str,
    base: usize,
    width: Emu,
    fit_text: &mut impl FnMut(&str, usize, Emu) -> Result<ParagraphLines, LayoutError>,
    diagnostics: &mut Vec<LineDiagnostic>,
    omitted: &mut usize,
    stats: &mut crate::LineStats,
) -> Result<Option<LayoutLine>, LayoutError> {
    let mut prefix_len = floor_char(text, text.len().min(64));
    if prefix_len == 0 {
        return Ok(None);
    }
    loop {
        let mut fitted = fit_text(&text[..prefix_len], base, width)?;
        stats.shape_calls += fitted.stats.shape_calls;
        stats.shaped_bytes += fitted.stats.shaped_bytes;
        *omitted += fitted.omitted_diagnostics;
        let Some(first) = fitted.lines.first() else {
            return Ok(None);
        };
        if first.run.width.raw() > width.raw()
            || first.source.bytes.start != 0
            || first.source.bytes.end == 0
        {
            return Ok(None);
        }
        let consumed = first.source.bytes.end;
        if consumed == prefix_len && prefix_len < text.len() {
            let grown = floor_char(text, text.len().min(prefix_len.saturating_mul(2)));
            prefix_len = if grown <= prefix_len {
                text.len()
            } else {
                grown
            };
            continue;
        }
        for diagnostic in &mut fitted.diagnostics {
            match diagnostic {
                LineDiagnostic::Overflow { source, .. }
                | LineDiagnostic::MissingGlyph { source }
                    if source.bytes.end <= consumed =>
                {
                    diagnostics.push(diagnostic.clone());
                }
                LineDiagnostic::Numbering { .. } => diagnostics.push(diagnostic.clone()),
                _ => {}
            }
        }
        return Ok(fitted.lines.into_iter().next());
    }
}

fn merge_text(line: &mut LayoutLine, mut head: LayoutLine, scalar: usize, byte: usize) {
    let glyph_base = line.run.glyphs.len();
    let text_base = line.run.text.len();
    let x_base = line.run.width.raw();
    for glyph in &mut head.run.glyphs {
        glyph.cluster += text_base;
        glyph.x = Emu::new(glyph.x.raw().saturating_add(x_base));
    }
    line.run.glyphs.append(&mut head.run.glyphs);
    for cluster in &mut head.clusters {
        shift_range(&mut cluster.source.bytes, byte);
        shift_range(&mut cluster.source.scalars, scalar);
        cluster.glyphs.start += glyph_base;
        cluster.glyphs.end += glyph_base;
    }
    line.clusters.append(&mut head.clusters);
    line.run.missing.extend(
        head.run
            .missing
            .into_iter()
            .map(|offset| offset.saturating_add(text_base)),
    );
    line.run.text.push_str(&head.run.text);
    line.run.width = Emu::new(line.run.width.raw().saturating_add(head.run.width.raw()));
    line.source.bytes.end = byte + head.source.bytes.end;
    line.source.scalars.end = scalar + head.source.scalars.end;
}

fn floor_char(text: &str, mut index: usize) -> usize {
    if index > text.len() {
        index = text.len();
    }
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn shift_diagnostic(diagnostic: &mut LineDiagnostic, scalar: usize, byte: usize) {
    match diagnostic {
        LineDiagnostic::Overflow { source, .. } | LineDiagnostic::MissingGlyph { source } => {
            shift_range(&mut source.bytes, byte);
            shift_range(&mut source.scalars, scalar);
        }
        LineDiagnostic::Numbering { .. } => {}
    }
}

fn try_attach(
    line: &mut LayoutLine,
    image: &FlowImage,
    content_width: Emu,
    line_box: Emu,
    descent: Emu,
) -> bool {
    if !line.images.is_empty() {
        return false;
    }
    if line.run.width.raw().saturating_add(image.width.raw()) > content_width.raw() {
        return false;
    }
    let x = line.run.width;
    line.run.width = Emu::new(line.run.width.raw().saturating_add(image.width.raw()));
    line.source.scalars.end = image.scalar + 1;
    line.source.bytes.end = image.byte + OBJECT_MARKER.len_utf8();
    line.images.push(line_image(image, x));
    grow(line, image.height, line_box, descent);
    true
}

fn image_line(
    font: &LoadedFont,
    size: Emu,
    image: &FlowImage,
    line_box: Emu,
    descent: Emu,
) -> LayoutLine {
    let mut line = LayoutLine {
        source: SourceRange {
            bytes: image.byte..image.byte + OBJECT_MARKER.len_utf8(),
            scalars: image.scalar..image.scalar + 1,
        },
        run: ShapedRun {
            font: font.key(),
            size,
            text: String::new(),
            glyphs: Vec::new(),
            width: image.width,
            missing: Vec::new(),
            bold: false,
            italic: false,
            latin_family: None,
            east_asia_family: None,
            color: [0, 0, 0],
            underline: None,
        },
        clusters: Vec::new(),
        images: vec![line_image(image, Emu::new(0))],
        revisions: Vec::new(),
        hyperlinks: Vec::new(),
        box_height: Emu::new(0),
        tab_gaps: Vec::new(),
        label_glyphs: 0,
        label_text_bytes: 0,
        label_size: Emu::new(0),
        label_bold: false,
    };
    grow(&mut line, image.height, line_box, descent);
    line
}

fn attach_anchor(line: &mut LayoutLine, image: &FlowImage) {
    line.source.scalars.end = image.scalar + 1;
    line.source.bytes.end = image.byte + OBJECT_MARKER.len_utf8();
    line.images.push(line_image(image, Emu::new(0)));
}

fn anchor_line(font: &LoadedFont, size: Emu, image: &FlowImage) -> LayoutLine {
    LayoutLine {
        source: SourceRange {
            bytes: image.byte..image.byte + OBJECT_MARKER.len_utf8(),
            scalars: image.scalar..image.scalar + 1,
        },
        run: ShapedRun {
            font: font.key(),
            size,
            text: String::new(),
            glyphs: Vec::new(),
            width: Emu::new(0),
            missing: Vec::new(),
            bold: false,
            italic: false,
            latin_family: None,
            east_asia_family: None,
            color: [0, 0, 0],
            underline: None,
        },
        clusters: Vec::new(),
        images: vec![line_image(image, Emu::new(0))],
        revisions: Vec::new(),
        hyperlinks: Vec::new(),
        box_height: Emu::new(0),
        tab_gaps: Vec::new(),
        label_glyphs: 0,
        label_text_bytes: 0,
        label_size: Emu::new(0),
        label_bold: false,
    }
}

fn line_image(image: &FlowImage, x: Emu) -> LineImage {
    let (x, absolute_top, square, top_bottom) = if let Some(place) = &image.anchor {
        (place.x, Some(place.y), place.square, place.top_bottom)
    } else {
        (x, None, None, None)
    };
    LineImage {
        media_type: image.media_type.clone(),
        bytes: image.bytes.clone(),
        x,
        width: image.width,
        height: image.height,
        scalar: image.scalar,
        absolute_top,
        square,
        top_bottom,
    }
}

fn grow(line: &mut LayoutLine, image_height: Emu, line_box: Emu, descent: Emu) {
    let needed = image_height.raw().saturating_add(descent.raw());
    let current = if line.box_height.raw() > 0 {
        line.box_height.raw()
    } else {
        line_box.raw()
    };
    if needed > current {
        line.box_height = Emu::new(needed);
    }
}

fn overflow(image: &FlowImage, available: Emu) -> LineDiagnostic {
    LineDiagnostic::Overflow {
        source: SourceRange {
            bytes: image.byte..image.byte + OBJECT_MARKER.len_utf8(),
            scalars: image.scalar..image.scalar + 1,
        },
        width: image.requested_width,
        available,
        kind: OverflowKind::SingleCluster,
    }
}

fn shift_lines(lines: &mut [LayoutLine], scalar: usize, byte: usize) {
    for line in lines {
        shift_range(&mut line.source.bytes, byte);
        shift_range(&mut line.source.scalars, scalar);
        for cluster in &mut line.clusters {
            shift_range(&mut cluster.source.bytes, byte);
            shift_range(&mut cluster.source.scalars, scalar);
        }
    }
}

fn shift_range(range: &mut Range<usize>, by: usize) {
    range.start += by;
    range.end += by;
}

/// One exclusion already committed on a page, in page EMU.
///
/// `full_width` is `wp:wrapTopAndBottom`: the band covers the line frame, so
/// text is not placed in a side gap. `left`, `right` and `side` are unused
/// in that case.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SquareBand {
    pub page: usize,
    pub left: i64,
    pub right: i64,
    pub top: i64,
    pub bottom: i64,
    pub side: WrapSide,
    pub full_width: bool,
}

/// The frame a line is about to occupy.
pub(crate) struct CarveFrame {
    pub x: i64,
    pub width: i64,
    pub top: i64,
    pub step: i64,
    pub page: usize,
}

pub(crate) enum Carve {
    /// No square crosses this line. Place it in the full frame.
    Keep,
    /// The whole line fits in one side gap.
    Move { x: i64, width: i64 },
    /// The line is wider than the gap. `head` fits; `rest` is placed next.
    Break {
        head: Box<LayoutLine>,
        rest: Box<LayoutLine>,
        x: i64,
        width: i64,
    },
    /// The gap is empty. The next try should start at `y`.
    Below { y: i64 },
}

pub(crate) fn square_bands(page: usize, line: &LayoutLine) -> Vec<SquareBand> {
    let mut bands = Vec::new();
    for image in &line.images {
        let Some(square) = image.square else {
            continue;
        };
        let Some(top) = image.absolute_top else {
            continue;
        };
        let left = image.x.raw().saturating_sub(square.left);
        let right = image
            .x
            .raw()
            .saturating_add(image.width.raw())
            .saturating_add(square.right);
        let band_top = top.raw().saturating_sub(square.top);
        let bottom = top
            .raw()
            .saturating_add(image.height.raw())
            .saturating_add(square.bottom);
        if right <= left || bottom <= band_top {
            continue;
        }
        bands.push(SquareBand {
            page,
            left,
            right,
            top: band_top,
            bottom,
            side: square.side,
            full_width: false,
        });
    }
    for image in &line.images {
        let Some(band) = image.top_bottom else {
            continue;
        };
        let Some(top) = image.absolute_top else {
            continue;
        };
        let band_top = top.raw().saturating_sub(band.top);
        let bottom = top
            .raw()
            .saturating_add(image.height.raw())
            .saturating_add(band.bottom);
        if bottom <= band_top {
            continue;
        }
        bands.push(SquareBand {
            page,
            left: 0,
            right: 0,
            top: band_top,
            bottom,
            side: WrapSide::Larger,
            full_width: true,
        });
    }
    bands
}

/// Choose a side gap for `line`, or leave it unchanged.
///
/// `committed` holds exclusions already placed on this page. The line's own
/// square or top-and-bottom band counts even before it is committed, so text
/// on the anchor line avoids that rectangle. A gap as wide as the frame is
/// [`Carve::Keep`]. A top-and-bottom band covers the whole frame: a line with
/// glyphs moves entirely below it, and a line with no glyphs stays put so the
/// anchor marker does not insert a blank line under the picture.
pub(crate) fn carve_line(line: &LayoutLine, frame: CarveFrame, committed: &[SquareBand]) -> Carve {
    if frame.width <= 0 || frame.step <= 0 {
        return Carve::Keep;
    }
    let mut hitting = Vec::new();
    for band in committed {
        if band_hits(band, &frame) {
            hitting.push(*band);
        }
    }
    for band in square_bands(frame.page, line) {
        if band_hits(&band, &frame) {
            hitting.push(band);
        }
    }
    if hitting.is_empty() {
        return Carve::Keep;
    }
    if hitting.iter().any(|band| band.full_width) {
        if line.run.glyphs.is_empty() {
            return Carve::Keep;
        }
        let below = hitting
            .iter()
            .filter(|band| band.full_width)
            .map(|band| band.bottom)
            .max()
            .unwrap_or(frame.top);
        return Carve::Below { y: below };
    }
    let mut gaps = vec![(frame.x, frame.x.saturating_add(frame.width))];
    for band in &hitting {
        subtract_gap(&mut gaps, band.left, band.right);
    }
    let side = combined_side(&hitting);
    let union_left = hitting
        .iter()
        .map(|band| band.left)
        .min()
        .unwrap_or(frame.x);
    let union_right = hitting
        .iter()
        .map(|band| band.right)
        .max()
        .unwrap_or(frame.x);
    let Some((gap_x, gap_end)) = pick_gap(&gaps, side, union_left, union_right) else {
        let below = hitting
            .iter()
            .map(|band| band.bottom)
            .max()
            .unwrap_or(frame.top);
        return Carve::Below { y: below };
    };
    let gap_width = gap_end.saturating_sub(gap_x);
    if gap_width <= 0 {
        let below = hitting
            .iter()
            .map(|band| band.bottom)
            .max()
            .unwrap_or(frame.top);
        return Carve::Below { y: below };
    }
    if gap_x == frame.x && gap_width == frame.width {
        return Carve::Keep;
    }
    if line.run.glyphs.is_empty() || line.run.width.raw() <= gap_width {
        return Carve::Move {
            x: gap_x,
            width: gap_width,
        };
    }
    if line.label_glyphs > 0 || line.clusters.is_empty() {
        return Carve::Keep;
    }
    match split_line(line, gap_width) {
        Some((head, rest)) => Carve::Break {
            head: Box::new(head),
            rest: Box::new(rest),
            x: gap_x,
            width: gap_width,
        },
        None => Carve::Move {
            x: gap_x,
            width: gap_width,
        },
    }
}

fn band_hits(band: &SquareBand, frame: &CarveFrame) -> bool {
    band.page == frame.page
        && frame.top < band.bottom
        && frame.top.saturating_add(frame.step) > band.top
}

fn combined_side(bands: &[SquareBand]) -> WrapSide {
    let left = bands.iter().any(|band| band.side == WrapSide::Left);
    let right = bands.iter().any(|band| band.side == WrapSide::Right);
    if left && right {
        WrapSide::Larger
    } else if left {
        WrapSide::Left
    } else if right {
        WrapSide::Right
    } else {
        WrapSide::Larger
    }
}

fn subtract_gap(gaps: &mut Vec<(i64, i64)>, left: i64, right: i64) {
    if right <= left {
        return;
    }
    let mut next = Vec::new();
    for (start, end) in gaps.drain(..) {
        if right <= start || left >= end {
            next.push((start, end));
            continue;
        }
        if start < left {
            next.push((start, left.min(end)));
        }
        if right < end {
            next.push((right.max(start), end));
        }
    }
    *gaps = next;
}

fn pick_gap(
    gaps: &[(i64, i64)],
    side: WrapSide,
    union_left: i64,
    union_right: i64,
) -> Option<(i64, i64)> {
    let mut chosen: Option<(i64, i64)> = None;
    for &(start, end) in gaps {
        let width = end.saturating_sub(start);
        if width <= 0 {
            continue;
        }
        let allowed = match side {
            WrapSide::Left => end <= union_left,
            WrapSide::Right => start >= union_right,
            WrapSide::Larger => true,
        };
        if !allowed {
            continue;
        }
        let replace = match chosen {
            None => true,
            Some((prev_start, prev_end)) => {
                let prev = prev_end.saturating_sub(prev_start);
                width > prev || (width == prev && start < prev_start)
            }
        };
        if replace {
            chosen = Some((start, end));
        }
    }
    chosen
}

fn split_line(line: &LayoutLine, max_width: i64) -> Option<(LayoutLine, LayoutLine)> {
    if line.clusters.is_empty() || max_width <= 0 {
        return None;
    }
    let mut keep = 0usize;
    for (index, cluster) in line.clusters.iter().enumerate() {
        if cluster_right(line, cluster) <= max_width {
            keep = index + 1;
        } else {
            break;
        }
    }
    if keep == 0 {
        keep = 1;
    }
    if keep >= line.clusters.len() {
        return None;
    }
    let glyph_cut = line.clusters[keep].glyphs.start;
    if glyph_cut == 0 || glyph_cut > line.run.glyphs.len() {
        return None;
    }
    let text_cut = line.run.glyphs[glyph_cut].cluster;
    if text_cut == 0 || text_cut > line.run.text.len() || !line.run.text.is_char_boundary(text_cut)
    {
        return None;
    }
    let shift = line.run.glyphs[glyph_cut].x.raw();
    let scalar_cut = line.clusters[keep].source.scalars.start;
    let byte_cut = line.clusters[keep].source.bytes.start;
    let mut head = line.clone();
    head.run.glyphs.truncate(glyph_cut);
    head.run.text.truncate(text_cut);
    head.run.missing.retain(|offset| *offset < text_cut);
    head.run.width = Emu::new(cluster_right(&head, &line.clusters[keep - 1]).max(0));
    head.clusters.truncate(keep);
    head.source.bytes.end = byte_cut;
    head.source.scalars.end = scalar_cut;
    head.images
        .retain(|image| head.source.scalars.contains(&image.scalar));
    head.hyperlinks = clip_ranges(&line.hyperlinks, head.source.scalars.clone());
    head.revisions = clip_revisions(&line.revisions, &head.source.scalars);
    head.tab_gaps
        .retain(|(x, width)| x.saturating_add(*width) <= shift);
    head.label_glyphs = head.label_glyphs.min(head.run.glyphs.len());

    let mut rest = line.clone();
    rest.run.glyphs = line.run.glyphs[glyph_cut..]
        .iter()
        .map(|glyph| {
            let mut glyph = *glyph;
            glyph.cluster = glyph.cluster.saturating_sub(text_cut);
            glyph.x = Emu::new(glyph.x.raw().saturating_sub(shift));
            glyph
        })
        .collect();
    rest.run.text = line.run.text[text_cut..].to_owned();
    rest.run.missing = line
        .run
        .missing
        .iter()
        .filter(|offset| **offset >= text_cut)
        .map(|offset| offset.saturating_sub(text_cut))
        .collect();
    rest.run.width = Emu::new(line.run.width.raw().saturating_sub(shift).max(0));
    rest.clusters = line.clusters[keep..]
        .iter()
        .map(|cluster| ClusterRange {
            source: cluster.source.clone(),
            glyphs: cluster.glyphs.start - glyph_cut..cluster.glyphs.end - glyph_cut,
        })
        .collect();
    rest.source.bytes.start = byte_cut;
    rest.source.scalars.start = scalar_cut;
    rest.images.retain(|image| image.scalar >= scalar_cut);
    rest.hyperlinks = clip_ranges(&line.hyperlinks, rest.source.scalars.clone());
    rest.revisions = clip_revisions(&line.revisions, &rest.source.scalars);
    rest.tab_gaps = line
        .tab_gaps
        .iter()
        .filter(|(x, _)| *x >= shift)
        .map(|(x, width)| (x.saturating_sub(shift), *width))
        .collect();
    rest.label_glyphs = 0;
    rest.label_text_bytes = 0;
    rest.label_size = Emu::new(0);
    rest.label_bold = false;
    Some((head, rest))
}

fn cluster_right(line: &LayoutLine, cluster: &ClusterRange) -> i64 {
    line.run
        .glyphs
        .get(cluster.glyphs.clone())
        .unwrap_or(&[])
        .iter()
        .map(|glyph| glyph.x.raw().saturating_add(glyph.advance.raw()))
        .max()
        .unwrap_or(0)
}

fn clip_ranges(ranges: &[Range<usize>], scalars: Range<usize>) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter_map(|range| {
            let start = range.start.max(scalars.start);
            let end = range.end.min(scalars.end);
            (start < end).then_some(start..end)
        })
        .collect()
}

fn clip_revisions(revisions: &[LineRevision], scalars: &Range<usize>) -> Vec<LineRevision> {
    revisions
        .iter()
        .filter_map(|revision| {
            let start = revision.scalars.start.max(scalars.start);
            let end = revision.scalars.end.min(scalars.end);
            (start < end).then(|| {
                let mut revision = revision.clone();
                revision.scalars = start..end;
                revision
            })
        })
        .collect()
}
