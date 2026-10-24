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
//! Accepted body markers stay in the package as fragments, but they do not
//! lock the paragraph. An unchanged export copies the original package. An
//! edit that keeps every bookmark a body range reuses those marker bytes:
//! a host paragraph that did not change keeps them in place, and a host that
//! changed (or a range that moved) is rewritten by splicing the same
//! `bookmarkStart` / `bookmarkEnd` bytes at the model offsets. Removing,
//! renaming, detaching, reversing, or pointing a bookmark outside a top-level
//! body paragraph is refused. Header, footer and comment ranges are not body
//! carets, so they are not annotations; their paragraphs stay locked.
//! A bookmark inside an opaque table, or a `colFirst` / `colLast` column
//! bookmark, is diagnosed and not inserted, and its paragraph stays locked.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use joyoffice_opc::{Budget, PartName};
use joyword_model::{
    Anchor, AnnotationId, AnnotationKind, Bias, Block, BlockSeq, Document, NodeId, Paragraph,
    Position, ScalarOffset, SourceRef, StoryId, StoryKind,
};

use crate::backing::{Fragment, PreservedReason};
use crate::export::{ExportError, unsupported};
use crate::import::{Importer, Scope};
use crate::ns::W;
use crate::result::{DiagnosticKind, Severity};
use crate::xml::{Element, XmlReader};

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
    /// Paragraphs whose markers this slice will not write back.
    unwritable: BTreeSet<NodeId>,
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
/// marker occupies no text position. Locking is decided in [`bind`]: only a
/// marker this slice will not write back locks its paragraph.
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
        importer.bookmarks.unwritable.insert(paragraph.id());
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
    let mut unwritable = std::mem::take(&mut importer.bookmarks.unwritable);
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
            remember_unwritable(&mut unwritable, &group);
            continue;
        }
        if !bind_group(importer, id, &group, &mut accepted) {
            remember_unwritable(&mut unwritable, &group);
        }
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
    for id in unwritable {
        importer.editability.lock(id);
    }
}

fn remember_unwritable(unwritable: &mut BTreeSet<NodeId>, group: &[Mark]) {
    for mark in group {
        unwritable.insert(mark.paragraph);
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
) -> bool {
    let (part, span) = locate_diag(group);
    if group.iter().any(|mark| mark.unsupported) {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkUnsupported { id },
        );
        return false;
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
        return false;
    }
    let start = starts[0];
    let end = ends[0];
    if start.column {
        warn(importer, &part, span, DiagnosticKind::BookmarkColumn { id });
        return false;
    }
    let Some(start_at) = place(&importer.document, start.paragraph) else {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return false;
    };
    let Some(end_at) = place(&importer.document, end.paragraph) else {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return false;
    };
    if start_at.story != end_at.story {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return false;
    }
    if start_at.kind != StoryKind::Body {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkOutsideBody { id },
        );
        return false;
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
        return false;
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
            return false;
        }
        Some(NameProblem::Control) => {
            warn(
                importer,
                &part,
                span,
                DiagnosticKind::BookmarkNameInvalid { id },
            );
            return false;
        }
        Some(NameProblem::Long) => {
            warn(importer, &part, span, DiagnosticKind::BookmarkBudget { id });
            return false;
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
        return false;
    }
    if accepted.len() >= MAX_IMPORTED_BOOKMARKS {
        warn(importer, &part, span, DiagnosticKind::BookmarkBudget { id });
        return false;
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
    true
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

/// Whether `after` still names the same bookmarks as `before`.
///
/// An anchor that moved inside the body is a difference: the early export
/// path must not copy the original package over it.
pub(crate) fn differs(before: &Document, after: &Document) -> bool {
    bookmark_rows(before) != bookmark_rows(after)
}

fn bookmark_rows(document: &Document) -> Vec<(AnnotationId, Arc<str>, Anchor)> {
    document
        .annotations()
        .filter_map(|annotation| {
            let AnnotationKind::Bookmark { name } = &annotation.kind else {
                return None;
            };
            Some((annotation.id, Arc::clone(name), annotation.anchor.clone()))
        })
        .collect()
}

/// A body range this slice can write with the imported marker bytes.
///
/// The bias is the one import stores. A detached anchor, a header, footer,
/// comment, or table-cell paragraph, a reversed range, or an offset past the
/// paragraph is not representable.
pub(crate) fn anchor_writable(document: &Document, anchor: &Anchor) -> bool {
    let Anchor::Range { start, end } = anchor else {
        return false;
    };
    if start.bias != Bias::After || end.bias != Bias::Before {
        return false;
    }
    let body = document.body_id();
    if start.story != body || end.story != body {
        return false;
    }
    let paragraphs: Vec<&Paragraph> = document
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect();
    let Some(start_at) = paragraphs
        .iter()
        .position(|paragraph| paragraph.id() == start.paragraph)
    else {
        return false;
    };
    let Some(end_at) = paragraphs
        .iter()
        .position(|paragraph| paragraph.id() == end.paragraph)
    else {
        return false;
    };
    if start.offset.0 > paragraphs[start_at].len() || end.offset.0 > paragraphs[end_at].len() {
        return false;
    }
    start_at < end_at || (start_at == end_at && start.offset.0 <= end.offset.0)
}

struct SavedBookmark {
    id: u32,
    start: Vec<u8>,
    end: Vec<u8>,
}

/// Splice imported bookmark markers into `patched` when an edit moved them
/// or rewrote their host paragraph.
///
/// `None` means every accepted bookmark still sits on an unedited host, so
/// the patched part already contains the original marker bytes. Markers this
/// slice did not import are refused rather than deleted.
pub(crate) fn write_back(
    original_xml: &[u8],
    patched: &[u8],
    original: &Document,
    current: &Document,
    edited: &BTreeSet<NodeId>,
    budget: &Budget,
) -> Result<Option<Vec<u8>>, ExportError> {
    let saved = saved_bookmarks(original_xml, original, budget)?;
    if saved.is_empty() {
        if current_bookmarks(current).next().is_some() {
            return Err(unsupported(None, "bookmark range cannot be written"));
        }
        return Ok(None);
    }
    let moving: Vec<usize> = saved
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| bookmark_moves(original, current, *id, edited))
        .map(|(index, _)| index)
        .collect();
    if moving.is_empty() {
        return Ok(None);
    }
    let moving_ids: BTreeSet<u32> = moving.iter().map(|index| saved[*index].1.id).collect();
    let known_ids: BTreeSet<u32> = saved.iter().map(|(_, saved)| saved.id).collect();
    let root = parse_document(patched, budget)?;
    let mut strip = Vec::new();
    collect_markers(&root, &moving_ids, &known_ids, &mut strip)?;
    strip.sort_by_key(|range| range.start);
    for pair in strip.windows(2) {
        if pair[1].start < pair[0].end {
            return Err(unsupported(None, "bookmark markup cannot be dropped"));
        }
    }
    let stripped = apply_edits(patched, &strip, &[]);
    let root = parse_document(&stripped, budget)?;
    let body = root
        .child(W, "body")
        .ok_or_else(|| unsupported(None, "missing source body"))?;
    let model: Vec<&Paragraph> = current
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect();
    let xml_paragraphs: Vec<&Element> = body.elements().filter(|child| child.is(W, "p")).collect();
    if model.len() != xml_paragraphs.len() {
        return Err(unsupported(None, "bookmark range cannot be written"));
    }
    let mut events: BTreeMap<usize, Vec<MarkerEvent>> = BTreeMap::new();
    for (index, saved_index) in moving.iter().enumerate() {
        let (annotation_id, marker) = &saved[*saved_index];
        let annotation = current
            .annotation(*annotation_id)
            .ok_or_else(|| unsupported(None, "a bookmark was removed"))?;
        let AnnotationKind::Bookmark { .. } = &annotation.kind else {
            return Err(unsupported(None, "a bookmark changed"));
        };
        if !anchor_writable(current, &annotation.anchor) {
            return Err(unsupported(None, "a bookmark changed"));
        }
        let Anchor::Range { start, end } = &annotation.anchor else {
            return Err(unsupported(None, "a bookmark changed"));
        };
        let Some(start_at) = model
            .iter()
            .position(|paragraph| paragraph.id() == start.paragraph)
        else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        let Some(end_at) = model
            .iter()
            .position(|paragraph| paragraph.id() == end.paragraph)
        else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        events.entry(start_at).or_default().push(MarkerEvent {
            offset: start.offset.0,
            end: false,
            seq: index as u32,
            xml: marker.start.clone(),
        });
        events.entry(end_at).or_default().push(MarkerEvent {
            offset: end.offset.0,
            end: true,
            seq: index as u32,
            xml: marker.end.clone(),
        });
    }
    let mut edits = Vec::new();
    for (index, paragraph_events) in events {
        edits.extend(paragraph_edits(
            &stripped,
            xml_paragraphs[index],
            &paragraph_events,
        )?);
    }
    let bytes = apply_edits(&stripped, &[], &edits);
    if bytes.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: "word/document.xml".into(),
            limit: budget.max_part_bytes,
        });
    }
    Ok(Some(bytes))
}

fn current_bookmarks(document: &Document) -> impl Iterator<Item = &joyword_model::Annotation> {
    document
        .annotations()
        .filter(|annotation| matches!(annotation.kind, AnnotationKind::Bookmark { .. }))
}

fn bookmark_moves(
    original: &Document,
    current: &Document,
    id: AnnotationId,
    edited: &BTreeSet<NodeId>,
) -> bool {
    let Some(before) = original.annotation(id) else {
        return true;
    };
    let Some(after) = current.annotation(id) else {
        return true;
    };
    if before.anchor != after.anchor || before.kind != after.kind {
        return true;
    }
    let Anchor::Range { start, end } = &after.anchor else {
        return true;
    };
    edited.contains(&start.paragraph) || edited.contains(&end.paragraph)
}

fn saved_bookmarks(
    xml: &[u8],
    document: &Document,
    budget: &Budget,
) -> Result<Vec<(AnnotationId, SavedBookmark)>, ExportError> {
    let bookmarks: Vec<_> = current_bookmarks(document).collect();
    if bookmarks.is_empty() {
        return Ok(Vec::new());
    }
    let root = parse_document(xml, budget)?;
    let mut found = Vec::new();
    for annotation in bookmarks {
        let Some(source) = annotation.source.as_deref() else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        let Some(span) = source_span(source) else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        let Some(start) = find_span(&root, &span) else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        if !start.is(W, "bookmarkStart") {
            return Err(unsupported(None, "bookmark range cannot be written"));
        }
        let Some(id) = marker_id(start) else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        let Some(end) = find_end(&root, id) else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        let start_bytes = xml
            .get(start.span.clone())
            .ok_or_else(|| unsupported(None, "bookmark range cannot be written"))?
            .to_vec();
        let end_bytes = xml
            .get(end.span.clone())
            .ok_or_else(|| unsupported(None, "bookmark range cannot be written"))?
            .to_vec();
        found.push((
            annotation.id,
            SavedBookmark {
                id,
                start: start_bytes,
                end: end_bytes,
            },
        ));
    }
    Ok(found)
}

fn source_span(source: &SourceRef) -> Option<Range<usize>> {
    let locator = source.locator.strip_prefix('@')?;
    let (start, end) = locator.split_once("..")?;
    Some(start.parse().ok()?..end.parse().ok()?)
}

fn find_span<'a>(element: &'a Element, span: &Range<usize>) -> Option<&'a Element> {
    if element.span == *span {
        return Some(element);
    }
    element.elements().find_map(|child| find_span(child, span))
}

fn find_end(element: &Element, id: u32) -> Option<&Element> {
    if element.is(W, "bookmarkEnd") && marker_id(element) == Some(id) {
        return Some(element);
    }
    element.elements().find_map(|child| find_end(child, id))
}

fn collect_markers(
    element: &Element,
    moving: &BTreeSet<u32>,
    known: &BTreeSet<u32>,
    ranges: &mut Vec<Range<usize>>,
) -> Result<(), ExportError> {
    if is_bookmark(element) {
        match marker_id(element) {
            Some(id) if moving.contains(&id) => ranges.push(element.span.clone()),
            // A bookmark that did not move keeps the bytes already in `patched`.
            Some(id) if known.contains(&id) => {}
            _ => return Err(unsupported(None, "bookmark markup cannot be dropped")),
        }
        return Ok(());
    }
    for child in element.elements() {
        collect_markers(child, moving, known, ranges)?;
    }
    Ok(())
}

type ByteEdit = (Range<usize>, Vec<u8>);

#[derive(Clone)]
struct MarkerEvent {
    offset: usize,
    end: bool,
    seq: u32,
    xml: Vec<u8>,
}

fn paragraph_edits(
    xml: &[u8],
    paragraph: &Element,
    events: &[MarkerEvent],
) -> Result<Vec<ByteEdit>, ExportError> {
    let fail = || unsupported(None, "bookmark range cannot be written");
    let mut pending = events.to_vec();
    pending.sort_by_key(|event| (event.offset, event.end, event.seq));
    let mut edits = Vec::new();
    let mut cursor = 0usize;
    let content_at = after_start(xml, paragraph).map_err(|_| fail())?;
    let boundary = paragraph
        .child(W, "pPr")
        .map(|properties| properties.span.end)
        .unwrap_or(content_at);
    flush_events(&mut pending, 0, boundary, &mut edits);
    walk_content(xml, paragraph, &mut pending, &mut cursor, &mut edits)?;
    let end_at = end_tag_at(xml, paragraph).map_err(|_| fail())?;
    flush_events(&mut pending, cursor, end_at, &mut edits);
    if !pending.is_empty() {
        return Err(fail());
    }
    Ok(edits)
}

fn walk_content(
    xml: &[u8],
    container: &Element,
    pending: &mut Vec<MarkerEvent>,
    cursor: &mut usize,
    edits: &mut Vec<ByteEdit>,
) -> Result<(), ExportError> {
    let fail = || unsupported(None, "bookmark range cannot be written");
    for child in container.elements() {
        if child.is(W, "pPr") {
            continue;
        }
        if child.is(W, "proofErr")
            || child.is(W, "commentRangeStart")
            || child.is(W, "commentRangeEnd")
            || child.is(W, "bookmarkStart")
            || child.is(W, "bookmarkEnd")
        {
            flush_events(pending, *cursor, child.span.end, edits);
            continue;
        }
        if child.is(W, "hyperlink") {
            let at = after_start(xml, child).map_err(|_| fail())?;
            flush_events(pending, *cursor, at, edits);
            walk_content(xml, child, pending, cursor, edits)?;
            let end_at = end_tag_at(xml, child).map_err(|_| fail())?;
            flush_events(pending, *cursor, end_at, edits);
            continue;
        }
        if !child.is(W, "r") {
            return Err(fail());
        }
        let length = run_len(child).map_err(|_| fail())?;
        let run_start = *cursor;
        let interior: Vec<MarkerEvent> = pending
            .iter()
            .filter(|event| event.offset > run_start && event.offset < run_start + length)
            .cloned()
            .collect();
        flush_events(pending, *cursor, child.span.start, edits);
        if !interior.is_empty() {
            let replacement = split_run(xml, child, run_start, &interior).map_err(|_| fail())?;
            edits.push((child.span.clone(), replacement));
            pending
                .retain(|event| !(event.offset > run_start && event.offset < run_start + length));
        }
        *cursor += length;
    }
    Ok(())
}

fn flush_events(
    pending: &mut Vec<MarkerEvent>,
    offset: usize,
    at: usize,
    edits: &mut Vec<ByteEdit>,
) {
    let mut xml = Vec::new();
    pending.retain(|event| {
        if event.offset == offset {
            xml.extend_from_slice(&event.xml);
            false
        } else {
            true
        }
    });
    if !xml.is_empty() {
        edits.push((at..at, xml));
    }
}

fn run_len(run: &Element) -> Result<usize, ()> {
    let mut length = 0usize;
    for leaf in run.elements() {
        if leaf.is(W, "rPr")
            || leaf.is(W, "commentReference")
            || leaf.is(W, "lastRenderedPageBreak")
            || leaf.is(W, "bookmarkStart")
            || leaf.is(W, "bookmarkEnd")
            || leaf.is(W, "proofErr")
        {
            continue;
        }
        if leaf.is(W, "t") && leaf.elements().next().is_none() {
            length += leaf.text().chars().count();
        } else if leaf.elements().next().is_none()
            && (leaf.is(W, "tab") || leaf.is(W, "br") || leaf.is(W, "cr"))
        {
            length += 1;
        } else {
            return Err(());
        }
    }
    Ok(length)
}

fn split_run(
    xml: &[u8],
    run: &Element,
    run_start: usize,
    events: &[MarkerEvent],
) -> Result<Vec<u8>, ()> {
    let mut text = None;
    let mut properties = None;
    for child in run.elements() {
        if child.is(W, "rPr") && properties.is_none() && text.is_none() {
            properties = Some(child);
        } else if child.is(W, "t") && text.is_none() && child.elements().next().is_none() {
            text = Some(child);
        } else {
            // A marker left in this run belongs to a bookmark that is not
            // being rewritten. Splitting would drop it.
            return Err(());
        }
    }
    let Some(text) = text else {
        return Err(());
    };
    let chars: Vec<char> = text.text().chars().collect();
    let open = open_tag(xml, run)?;
    let mut text_open = open_tag(xml, text)?.to_vec();
    if !text_open.windows(9).any(|window| window == b"xml:space") {
        text_open.extend_from_slice(b" xml:space=\"preserve\"");
    }
    let text_end = format!("</{}>", text.qname);
    let run_end = format!("</{}>", run.qname);
    let properties = properties
        .map(|element| xml[element.span.clone()].to_vec())
        .unwrap_or_default();
    let mut cuts: Vec<usize> = events
        .iter()
        .map(|event| event.offset - run_start)
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    if cuts.iter().any(|cut| *cut == 0 || *cut >= chars.len()) {
        return Err(());
    }
    let mut points = vec![0];
    points.extend(cuts);
    points.push(chars.len());
    let mut out = Vec::new();
    for window in points.windows(2) {
        let from = window[0];
        let to = window[1];
        if from != 0 {
            for event in events {
                if event.offset - run_start == from {
                    out.extend_from_slice(&event.xml);
                }
            }
        }
        if from == to {
            continue;
        }
        let piece: String = chars[from..to].iter().collect();
        out.extend_from_slice(open);
        out.push(b'>');
        out.extend_from_slice(&properties);
        out.extend_from_slice(&text_open);
        out.push(b'>');
        out.extend_from_slice(xml_escape(&piece).as_bytes());
        out.extend_from_slice(text_end.as_bytes());
        out.extend_from_slice(run_end.as_bytes());
    }
    Ok(out)
}

fn xml_escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn open_tag<'a>(xml: &'a [u8], element: &Element) -> Result<&'a [u8], ()> {
    let raw = xml.get(element.span.start..element.span.end).ok_or(())?;
    let gt = raw.iter().position(|byte| *byte == b'>').ok_or(())?;
    if raw[..gt].ends_with(b"/") {
        return Err(());
    }
    Ok(&raw[..gt])
}

fn after_start(xml: &[u8], element: &Element) -> Result<usize, ()> {
    let raw = xml.get(element.span.start..element.span.end).ok_or(())?;
    let gt = raw.iter().position(|byte| *byte == b'>').ok_or(())?;
    if raw[..=gt].ends_with(b"/>") {
        return Err(());
    }
    Ok(element.span.start + gt + 1)
}

fn end_tag_at(xml: &[u8], element: &Element) -> Result<usize, ()> {
    let end = format!("</{}>", element.qname);
    let at = element.span.end.checked_sub(end.len()).ok_or(())?;
    if xml.get(at..element.span.end) != Some(end.as_bytes()) {
        return Err(());
    }
    Ok(at)
}

fn apply_edits(xml: &[u8], deletions: &[Range<usize>], edits: &[ByteEdit]) -> Vec<u8> {
    let mut combined: Vec<ByteEdit> = deletions
        .iter()
        .cloned()
        .map(|range| (range, Vec::new()))
        .chain(edits.iter().cloned())
        .collect();
    combined.sort_by(|left, right| {
        right
            .0
            .start
            .cmp(&left.0.start)
            .then(right.0.end.cmp(&left.0.end))
    });
    let mut out = xml.to_vec();
    for (range, bytes) in combined {
        out.splice(range, bytes);
    }
    out
}

fn parse_document(bytes: &[u8], budget: &Budget) -> Result<Element, ExportError> {
    XmlReader::new("word/document.xml", bytes, budget)
        .and_then(XmlReader::whole)
        .map_err(|error| {
            ExportError::Import(crate::ImportError::Xml {
                part: "word/document.xml".into(),
                error,
            })
        })
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
