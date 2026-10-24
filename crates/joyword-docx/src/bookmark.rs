//! DOCX bookmark import (ECMA-376 Part 1 §17.13.6.1 `bookmarkEnd`,
//! §17.13.6.2 `bookmarkStart`).
//!
//! A body range with one start and one end becomes a model bookmark. The name
//! is kept exactly, including case: PDF named destinations and `w:anchor`
//! compare that string and do not fold it. The checks match
//! `joyword_pdf::navigation`: trim-empty, any control character, more than
//! [`MAX_IMPORTED_BOOKMARK_NAME_BYTES`] UTF-8 bytes, a repeated name, or more
//! than [`MAX_IMPORTED_BOOKMARKS`] accepted bookmarks. Those bookmarks are
//! diagnosed and not inserted. Inserting them would make
//! `navigation_from_document` refuse the whole PDF.
//!
//! XML 1.0 attribute-value normalization turns a literal tab, line feed or
//! carriage return into a space. A character reference such as `&#9;` is the
//! referenced character: a name that is only that character is trim-empty,
//! and a name that also contains other characters is a control character.
//! Both are refused. The classifier still rejects a control character that
//! cannot occur in a well-formed XML 1.0 attribute.
//!
//! ECMA-376 keeps the first bookmark when names collide and ignores the rest.
//! This importer does the same, case-sensitively, so the PDF writer does not
//! see a duplicate.
//!
//! The marker bytes stay in the package and still lock the paragraph. This
//! slice does not write bookmarks back; an edited export of a package that
//! contains one is refused, and removing or renaming the annotation is
//! refused as well. Header, footer and comment ranges are not body carets, so
//! they are not annotations. A bookmark inside an opaque table, or a
//! `colFirst` / `colLast` column bookmark, is diagnosed and not inserted.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use joyoffice_opc::PartName;
use joyword_model::{
    Anchor, AnnotationKind, Bias, Block, BlockSeq, Document, NodeId, Paragraph, Position,
    ScalarOffset, SourceRef, StoryId, StoryKind,
};

use crate::backing::{Fragment, PreservedReason};
use crate::import::{Importer, Scope};
use crate::ns::W;
use crate::result::{DiagnosticKind, Severity};
use crate::xml::Element;

/// Upper bound of bookmarks inserted into the model.
///
/// This is `joyword_pdf::MAX_PDF_BOOKMARKS`. One more valid bookmark would
/// make `navigation_from_document` refuse the whole export.
pub const MAX_IMPORTED_BOOKMARKS: usize = 512;

/// UTF-8 byte budget for one bookmark name.
///
/// This is `joyword_pdf::MAX_PDF_NAME_BYTES`.
pub const MAX_IMPORTED_BOOKMARK_NAME_BYTES: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NameProblem {
    Empty,
    Control,
    Long,
}

fn classify_name(name: &str) -> Option<NameProblem> {
    if name.len() > MAX_IMPORTED_BOOKMARK_NAME_BYTES {
        return Some(NameProblem::Long);
    }
    if name.trim().is_empty() {
        return Some(NameProblem::Empty);
    }
    if name.chars().any(char::is_control) {
        return Some(NameProblem::Control);
    }
    None
}

#[derive(Default)]
pub(crate) struct State {
    marks: Vec<Mark>,
    unmapped: BTreeMap<u32, Located>,
    preserved: usize,
}

impl State {
    pub(crate) fn preserved(&self) -> usize {
        self.preserved
    }
}

struct Mark {
    id: u32,
    end: bool,
    name: Option<Arc<str>>,
    column: bool,
    unsupported: bool,
    part: PartName,
    span: Range<usize>,
    paragraph: NodeId,
    offset: usize,
}

struct Located {
    part: PartName,
    span: Range<usize>,
}

struct Class {
    column: bool,
    unsupported: bool,
}

struct Place {
    story: StoryId,
    kind: StoryKind,
    order: usize,
}

fn is_bookmark(element: &Element) -> bool {
    element.namespace.as_deref() == Some(W)
        && matches!(element.local.as_ref(), "bookmarkStart" | "bookmarkEnd")
}

fn marker_id(element: &Element) -> Option<u32> {
    let raw = element.attribute(Some(W), "id")?;
    if raw.is_empty() || raw.starts_with('+') || raw.starts_with('-') {
        return None;
    }
    raw.parse().ok()
}

fn has_payload(element: &Element) -> bool {
    element.elements().next().is_some() || element.text().chars().any(|ch| !ch.is_whitespace())
}

fn classify(element: &Element) -> Class {
    let end = element.local.as_ref() == "bookmarkEnd";
    let mut class = Class {
        column: false,
        unsupported: false,
    };
    for attribute in &element.attributes {
        if attribute.namespace.as_deref() != Some(W) {
            class.unsupported = true;
            continue;
        }
        match attribute.local.as_ref() {
            "id" | "displacedByCustomXml" => {}
            "name" if !end => {}
            "colFirst" | "colLast" if !end => class.column = true,
            _ => class.unsupported = true,
        }
    }
    if has_payload(element) {
        class.unsupported = true;
    }
    class
}

fn warn(importer: &mut Importer<'_>, part: &PartName, span: Range<usize>, kind: DiagnosticKind) {
    importer.diagnose(Severity::Warning, Some(part), Some(span), kind);
}

fn keep_fragment(importer: &mut Importer<'_>, part: &PartName, element: &Element, scope: &Scope) {
    let id = importer.document.ids().preserved();
    importer.backing.insert(
        id,
        Fragment {
            part: part.clone(),
            element: Arc::from(&*element.qname),
            namespace: element.namespace.clone(),
            range: element.span.clone(),
            namespaces: scope.namespaces.clone(),
            reason: PreservedReason::Unsupported,
        },
    );
    importer.bookmarks.preserved += 1;
}

/// Record a paragraph-level or run-level bookmark marker and keep its bytes.
///
/// Returns true when `element` is a WordprocessingML bookmark marker. The
/// caller locks the paragraph. The marker occupies no text position.
pub(crate) fn take(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    paragraph: &Paragraph,
) -> bool {
    if !is_bookmark(element) {
        return false;
    }
    let class = classify(element);
    let end = element.local.as_ref() == "bookmarkEnd";
    let Some(id) = marker_id(element) else {
        warn(
            importer,
            part,
            element.span.clone(),
            DiagnosticKind::BookmarkIdMissing,
        );
        keep_fragment(importer, part, element, scope);
        return true;
    };
    let name = if end {
        None
    } else {
        element.attribute(Some(W), "name").map(Arc::<str>::from)
    };
    importer.bookmarks.marks.push(Mark {
        id,
        end,
        name,
        column: class.column,
        unsupported: class.unsupported,
        part: part.clone(),
        span: element.span.clone(),
        paragraph: paragraph.id(),
        offset: paragraph.len(),
    });
    keep_fragment(importer, part, element, scope);
    true
}

/// Bookmark markers inside markup this slice does not map (an opaque table,
/// a block content control, a non-paragraph header child).
pub(crate) fn note_unmapped(importer: &mut Importer<'_>, part: &PartName, element: &Element) {
    fn walk(importer: &mut Importer<'_>, part: &PartName, element: &Element) {
        if is_bookmark(element) {
            match marker_id(element) {
                Some(id) => {
                    importer
                        .bookmarks
                        .unmapped
                        .entry(id)
                        .or_insert_with(|| Located {
                            part: part.clone(),
                            span: element.span.clone(),
                        });
                }
                None => warn(
                    importer,
                    part,
                    element.span.clone(),
                    DiagnosticKind::BookmarkIdMissing,
                ),
            }
        }
        for child in element.elements() {
            walk(importer, part, child);
        }
    }
    walk(importer, part, element);
}

pub(crate) fn bind(importer: &mut Importer<'_>) {
    let marks = std::mem::take(&mut importer.bookmarks.marks);
    let unmapped = std::mem::take(&mut importer.bookmarks.unmapped);
    let mut groups: BTreeMap<u32, Vec<Mark>> = BTreeMap::new();
    let mut order = Vec::new();
    for mark in marks {
        if !groups.contains_key(&mark.id) {
            order.push(mark.id);
        }
        groups.entry(mark.id).or_default().push(mark);
    }
    let mut accepted = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for id in order {
        seen.insert(id);
        let Some(group) = groups.remove(&id) else {
            continue;
        };
        if unmapped.contains_key(&id) {
            let (part, span) = locate_diag(&group);
            warn(
                importer,
                &part,
                span,
                DiagnosticKind::BookmarkAnchorInvalid { id },
            );
            continue;
        }
        bind_group(importer, id, &group, &mut accepted);
    }
    for (id, located) in unmapped {
        if seen.contains(&id) {
            continue;
        }
        warn(
            importer,
            &located.part,
            located.span,
            DiagnosticKind::BookmarkAnchorUnmapped { id },
        );
    }
}

fn locate_diag(group: &[Mark]) -> (PartName, Range<usize>) {
    let mark = group.iter().find(|mark| !mark.end).unwrap_or(&group[0]);
    (mark.part.clone(), mark.span.clone())
}

fn bind_group(
    importer: &mut Importer<'_>,
    id: u32,
    group: &[Mark],
    accepted: &mut BTreeSet<Arc<str>>,
) {
    let (part, span) = locate_diag(group);
    if group.iter().any(|mark| mark.unsupported) {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkUnsupported { id },
        );
        return;
    }
    let starts: Vec<_> = group.iter().filter(|mark| !mark.end).collect();
    let ends: Vec<_> = group.iter().filter(|mark| mark.end).collect();
    if starts.len() != 1 || ends.len() != 1 {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return;
    }
    let start = starts[0];
    let end = ends[0];
    if start.column {
        warn(importer, &part, span, DiagnosticKind::BookmarkColumn { id });
        return;
    }
    let Some(start_at) = place(&importer.document, start.paragraph) else {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return;
    };
    let Some(end_at) = place(&importer.document, end.paragraph) else {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return;
    };
    if start_at.story != end_at.story {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return;
    }
    if start_at.kind != StoryKind::Body {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkOutsideBody { id },
        );
        return;
    }
    let ordered = if start.paragraph == end.paragraph {
        start.offset <= end.offset
    } else {
        start_at.order < end_at.order
    };
    if !ordered {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return;
    }
    let name = start.name.clone().unwrap_or_else(|| Arc::from(""));
    match classify_name(&name) {
        Some(NameProblem::Empty) => {
            warn(
                importer,
                &part,
                span,
                DiagnosticKind::BookmarkNameEmpty { id },
            );
            return;
        }
        Some(NameProblem::Control) => {
            warn(
                importer,
                &part,
                span,
                DiagnosticKind::BookmarkNameInvalid { id },
            );
            return;
        }
        Some(NameProblem::Long) => {
            warn(importer, &part, span, DiagnosticKind::BookmarkBudget { id });
            return;
        }
        None => {}
    }
    if accepted.contains(name.as_ref()) {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkNameDuplicate { id },
        );
        return;
    }
    if accepted.len() >= MAX_IMPORTED_BOOKMARKS {
        warn(importer, &part, span, DiagnosticKind::BookmarkBudget { id });
        return;
    }
    let anchor = Anchor::Range {
        start: Position {
            story: start_at.story,
            paragraph: start.paragraph,
            offset: ScalarOffset(start.offset),
            bias: Bias::After,
        },
        end: Position {
            story: end_at.story,
            paragraph: end.paragraph,
            offset: ScalarOffset(end.offset),
            bias: Bias::Before,
        },
    };
    let annotation = importer.document.insert_annotation(
        AnnotationKind::Bookmark {
            name: Arc::clone(&name),
        },
        anchor,
    );
    let source = Arc::new(SourceRef {
        part: Arc::from(start.part.as_str()),
        locator: Arc::from(format!("@{}..{}", start.span.start, start.span.end)),
    });
    if let Some(stored) = importer.document.annotation_mut(annotation) {
        stored.source = Some(source);
    }
    accepted.insert(name);
}

fn place(document: &Document, paragraph: NodeId) -> Option<Place> {
    for story in document.stories() {
        let mut index = 0usize;
        let mut found = None;
        walk(&story.blocks, paragraph, &mut index, &mut found, 0);
        if let Some(order) = found {
            return Some(Place {
                story: story.id,
                kind: story.kind,
                order,
            });
        }
    }
    None
}

fn walk(
    blocks: &BlockSeq,
    needle: NodeId,
    index: &mut usize,
    found: &mut Option<usize>,
    depth: usize,
) {
    if depth > joyword_model::MAX_NESTING || found.is_some() {
        return;
    }
    for block in blocks.iter() {
        match block {
            Block::Paragraph(paragraph) => {
                if paragraph.id() == needle {
                    *found = Some(*index);
                    return;
                }
                *index += 1;
            }
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                    walk(&cell.blocks, needle, index, found, depth + 1);
                }
            }
            Block::Sdt(sdt) => walk(&sdt.blocks, needle, index, found, depth + 1),
            Block::Opaque(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NameProblem, classify_name};

    #[test]
    fn control_characters_match_the_pdf_name_rule() {
        assert_eq!(classify_name("\u{1}Target"), Some(NameProblem::Control));
        assert_eq!(classify_name("a\u{7f}"), Some(NameProblem::Control));
        assert_eq!(classify_name(""), Some(NameProblem::Empty));
        assert_eq!(classify_name(" \t"), Some(NameProblem::Empty));
        assert_eq!(classify_name(" Target "), None);
        assert_eq!(classify_name("Go"), None);
        assert_eq!(classify_name("go"), None);
        assert_eq!(classify_name(&" ".repeat(2048)), Some(NameProblem::Empty));
        assert_eq!(classify_name(&" ".repeat(2049)), Some(NameProblem::Long));
        assert_eq!(classify_name(&"n".repeat(2048)), None);
        assert_eq!(classify_name(&"n".repeat(2049)), Some(NameProblem::Long));
    }
}
