//! Inline still images in the shared page scene (LAYOUT-006, first slice).
//!
//! Images are not shaped. Preceding text may share a line when the image
//! fits the remaining width. Text after an image stays on that line when the
//! first fitting span fits the width that remains; the rest starts the next
//! line.
//! An extent larger than the content box is scaled for display. The stored
//! bytes stay unchanged.

use std::ops::Range;
use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_model::{Document, InlineKind, Measure, OBJECT_MARKER, Paragraph, Segment};

use crate::document::unsupported;
use crate::line::{LayoutLine, LineDiagnostic, LineImage, OverflowKind, SourceRange};
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
    fit_text: &mut impl FnMut(&str, Emu) -> Result<ParagraphLines, LayoutError>,
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
                let mut part = fit_text(&piece.text, content_width)?;
                shift_lines(&mut part.lines, piece.scalar, piece.byte);
                diagnostics.append(&mut part.diagnostics);
                omitted += part.omitted_diagnostics;
                stats.shape_calls += part.stats.shape_calls;
                stats.shaped_bytes += part.stats.shaped_bytes;
                let attach = items.get(index + 1).and_then(|item| match item {
                    Item::Image(image) => Some(image.clone()),
                    Item::Text(_) => None,
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
                items.push(Item::Image(FlowImage {
                    media_type: Arc::from(resource.media_type()),
                    bytes: Arc::from(resource.data()),
                    width,
                    height,
                    requested_width: width,
                    scalar,
                    byte,
                    scaled: false,
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
        if let Item::Image(image) = item {
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
    fit_text: &mut impl FnMut(&str, Emu) -> Result<ParagraphLines, LayoutError>,
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
        let mut rest = fit_text(&piece.text[consumed_bytes..], content_width)?;
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
    width: Emu,
    fit_text: &mut impl FnMut(&str, Emu) -> Result<ParagraphLines, LayoutError>,
    diagnostics: &mut Vec<LineDiagnostic>,
    omitted: &mut usize,
    stats: &mut crate::LineStats,
) -> Result<Option<LayoutLine>, LayoutError> {
    let mut prefix_len = floor_char(text, text.len().min(64));
    if prefix_len == 0 {
        return Ok(None);
    }
    loop {
        let mut fitted = fit_text(&text[..prefix_len], width)?;
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

fn line_image(image: &FlowImage, x: Emu) -> LineImage {
    LineImage {
        media_type: image.media_type.clone(),
        bytes: image.bytes.clone(),
        x,
        width: image.width,
        height: image.height,
        scalar: image.scalar,
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
