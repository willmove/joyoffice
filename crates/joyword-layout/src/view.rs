//! View options that participate in the layout snapshot key (PLATFORM-003).
//!
//! Formatting marks are drawn after line breaking and do not change advances.
//! Hidden text (`w:vanish`) and field codes change the string that is shaped,
//! so they change breaks. A hidden paragraph mark is still refused: this slice
//! does not merge paragraphs. A field that crosses paragraphs is refused.
//! Showing codes replaces a page field's number with `PAGE` or `NUMPAGES`
//! and, for a complex field, keeps the instruction and drops the result.
//! Showing results does the opposite. A field with no separator has no result,
//! so the result projection omits it.

use std::collections::BTreeSet;
use std::ops::Range;

use joyword_model::{Anchor, AnnotationKind, Document, NodeId, OBJECT_MARKER, Paragraph, RunProps};

use crate::LayoutError;

fn unsupported(node: Option<NodeId>, feature: &'static str) -> LayoutError {
    LayoutError::Unsupported { node, feature }
}

/// Options stored on [`crate::LayoutKey`]. Defaults match the previous scene:
/// marks off, hidden text omitted, field results shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ViewOptions {
    pub formatting_marks: bool,
    /// Paint runs whose resolved `w:vanish` is on. Off omits those runs.
    pub show_hidden: bool,
    /// Paint field instructions. Off paints the cached result.
    pub field_codes: bool,
}

pub(crate) struct Projection {
    pub text: String,
    /// `None` means the fitted offsets are already the paragraph offsets the
    /// fitter used before this projection existed.
    pub map: Option<Vec<usize>>,
}

pub(crate) fn project_paragraph(
    document: &Document,
    paragraph: &Paragraph,
    view: ViewOptions,
) -> Result<Projection, LayoutError> {
    let original = paragraph.plain_text(|_| String::new());
    let hidden = hidden_scalars(document, paragraph, view)?;
    let fields = field_omissions(document, paragraph, view)?;
    if hidden.is_empty() && fields.is_empty() {
        return Ok(Projection {
            text: original,
            map: None,
        });
    }
    if paragraph.inlines().iter().any(|inline| {
        matches!(
            inline.kind,
            joyword_model::InlineKind::Break(_) | joyword_model::InlineKind::Image { .. }
        )
    }) {
        return Err(unsupported(
            Some(paragraph.id()),
            "hidden text or field projection with a page break or image",
        ));
    }
    let chars: Vec<char> = paragraph
        .plain_text(|_| OBJECT_MARKER.to_string())
        .chars()
        .collect();
    let mut text = String::new();
    let mut map = Vec::new();
    for (index, ch) in chars.iter().enumerate() {
        if *ch == OBJECT_MARKER || hidden.contains(&index) || covered(index, &fields) {
            continue;
        }
        map.push(index);
        text.push(*ch);
    }
    map.push(chars.len());
    Ok(Projection {
        text,
        map: Some(map),
    })
}

pub(crate) fn remap_range(range: Range<usize>, map: &[usize]) -> Range<usize> {
    let start = map.get(range.start).copied().unwrap_or(range.start);
    let end = map.get(range.end).copied().unwrap_or(range.end);
    start..end.max(start)
}

fn covered(index: usize, ranges: &[Range<usize>]) -> bool {
    ranges.iter().any(|range| range.contains(&index))
}

pub(crate) fn run_is_hidden(props: &RunProps) -> bool {
    props.hidden == Some(true)
}

fn hidden_scalars(
    document: &Document,
    paragraph: &Paragraph,
    view: ViewOptions,
) -> Result<BTreeSet<usize>, LayoutError> {
    let mut hidden = BTreeSet::new();
    if view.show_hidden {
        return Ok(hidden);
    }
    let mut start = 0usize;
    for run in paragraph.runs() {
        let end = start.saturating_add(run.len);
        let resolved =
            document
                .styles()
                .resolve_run(paragraph.style.as_ref(), run.style.as_ref(), &run.props);
        if resolved.problems.is_empty() && run_is_hidden(&resolved.props) {
            hidden.extend(start..end);
        }
        start = end;
    }
    Ok(hidden)
}

fn field_omissions(
    document: &Document,
    paragraph: &Paragraph,
    view: ViewOptions,
) -> Result<Vec<Range<usize>>, LayoutError> {
    let mut ranges = Vec::new();
    for annotation in document.annotations() {
        let AnnotationKind::Field { separator } = &annotation.kind else {
            continue;
        };
        let Anchor::Range { start, end } = &annotation.anchor else {
            return Err(unsupported(
                Some(paragraph.id()),
                "annotation or field projection",
            ));
        };
        if start.paragraph != paragraph.id() && end.paragraph != paragraph.id() {
            continue;
        }
        if start.paragraph != end.paragraph || start.story != end.story {
            return Err(unsupported(Some(paragraph.id()), "field spans paragraphs"));
        }
        if start.paragraph != paragraph.id() {
            continue;
        }
        let from = start.offset.0;
        let to = end.offset.0;
        if view.field_codes {
            if let Some(separator) = separator {
                if separator.paragraph != paragraph.id() || separator.story != start.story {
                    return Err(unsupported(Some(paragraph.id()), "field spans paragraphs"));
                }
                ranges.push(separator.offset.0..to);
            }
        } else if let Some(separator) = separator {
            if separator.paragraph != paragraph.id() || separator.story != start.story {
                return Err(unsupported(Some(paragraph.id()), "field spans paragraphs"));
            }
            ranges.push(from..separator.offset.0);
        } else {
            ranges.push(from..to);
        }
    }
    Ok(ranges)
}

pub(crate) fn omitted_indices(
    document: &Document,
    paragraph: &Paragraph,
    view: ViewOptions,
) -> Result<BTreeSet<usize>, LayoutError> {
    let mut omitted = hidden_scalars(document, paragraph, view)?;
    for range in field_omissions(document, paragraph, view)? {
        omitted.extend(range);
    }
    Ok(omitted)
}

pub(crate) fn accept_annotations(document: &Document) -> Result<(), LayoutError> {
    for annotation in document.annotations() {
        match &annotation.kind {
            // Bookmarks and comments do not project text. Model validation
            // checks ranges. Header, footer, and comment bookmark carets are
            // recorded after the body is placed; the PDF resolver checks
            // names and those carets. Comment text itself is not painted.
            AnnotationKind::Bookmark { .. } | AnnotationKind::Comment(_) => {}
            AnnotationKind::Field { separator } => {
                let Anchor::Range { start, end } = &annotation.anchor else {
                    return Err(unsupported(None, "annotation or field projection"));
                };
                if start.paragraph != end.paragraph || start.story != end.story {
                    return Err(unsupported(Some(start.paragraph), "field spans paragraphs"));
                }
                if let Some(separator) = separator
                    && (separator.paragraph != start.paragraph || separator.story != start.story)
                {
                    return Err(unsupported(Some(start.paragraph), "field spans paragraphs"));
                }
            }
            _ => return Err(unsupported(None, "annotation or field projection")),
        }
    }
    Ok(())
}
