//! DOCX bookmark import (ECMA-376 Part 1 §17.13.6.1 `bookmarkEnd`,
//! §17.13.6.2 `bookmarkStart`).
//!
//! A range with one start and one end in the body, a header, a footer, or a
//! comment story becomes a model bookmark. The name is kept exactly,
//! including case: PDF named destinations and `w:anchor` compare that string
//! and do not fold it. The checks match `joyword_pdf::navigation`:
//! trim-empty, any control character, more than
//! [`MAX_IMPORTED_BOOKMARK_NAME_BYTES`] UTF-8 bytes, a repeated name, or more
//! than [`MAX_IMPORTED_BOOKMARKS`] accepted bookmarks. Those bookmarks are
//! diagnosed and not inserted. Inserting them would make
//! `navigation_from_document` refuse the whole PDF. Bookmark ids are paired
//! inside one part: the same `w:id` in the body and in a header is two
//! bookmarks. ECMA-376 numbers bookmarks per story, not per package.
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
//! Accepted markers stay in the package as fragments, but they do not lock
//! the paragraph. An unchanged export copies the original package. An edit
//! that keeps every bookmark a range in the part it was imported from reuses
//! those marker bytes: a host paragraph that did not change keeps them in
//! place, and a host that changed (or a range that moved) is rewritten by
//! splicing the same `bookmarkStart` / `bookmarkEnd` bytes at the model
//! offsets. Body markers are spliced into `document.xml`. Header, footer,
//! and comment markers are spliced into the part named by their source, not
//! into `document.xml`. Removing, renaming, detaching, reversing, changing
//! the imported bias, or pointing a bookmark at a paragraph outside that
//! part is refused.
//!
//! A comment bookmark is accepted only when that comment's anchor is a range
//! in the body, a header, or a footer: layout places every offset of the
//! comment story on that anchor caret, because comment text is not painted.
//! A detached comment has no page point, so its bookmark is diagnosed and
//! not inserted. Export refuses a comment bookmark whose comment is no
//! longer placeable, so the marker is not written back as a destination that
//! import would drop. A footnote, endnote, or text-box story is not placed.
//!
//! A `colFirst` / `colLast` pair in mapped paragraphs is inserted like any
//! other range bookmark, but its host paragraph stays locked: the column
//! attributes are not written back, so an edited export is `Restricted`
//! rather than silently dropping them.
//!
//! Markers seen only inside markup this slice does not map (an opaque table,
//! a block content control, a non-paragraph header child) cannot anchor a
//! range: the preserved block holds no paragraphs. A well-formed named pair
//! still becomes a bookmark with a point destination beside the block, at
//! the end of the nearest preceding paragraph, or at the start of the
//! story's first paragraph when the block leads the story. The marker bytes
//! stay inside the preserved markup and no source is set, so write-back
//! never moves them. Such a pair keeps its `BookmarkAnchorUnmapped`
//! diagnostic as an informational note that the anchor is synthesized. A
//! pair that is not well-formed (a half pair, reversed order, unsupported
//! attributes, a column marker, a bad name, a duplicate, or the budget) is
//! diagnosed and not inserted.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use joyoffice_opc::{Budget, PartName};
use joyword_model::{
    Anchor, AnnotationId, AnnotationKind, Bias, Block, BlockSeq, Document, NodeId, Paragraph,
    Position, ScalarOffset, SourceRef, Story, StoryId, StoryKind,
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
    unmapped: BTreeMap<(PartName, u32), Vec<Located>>,
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
    span: Range<usize>,
    end: bool,
    name: Option<Arc<str>>,
    column: bool,
    unsupported: bool,
    /// The nearest paragraph imported before the preserved block, if any.
    after: Option<NodeId>,
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

/// Whether the pair was inserted. A rejected marker locks its paragraph.
/// An accepted marker, in any story this slice writes back, does not.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Admission {
    Rejected,
    Accepted,
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
///
/// `after` is the nearest paragraph imported before `element`; it anchors a
/// point destination for a well-formed pair, because the preserved block
/// holds no paragraphs of its own.
pub(crate) fn note_unmapped(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    after: Option<NodeId>,
) {
    fn walk(
        importer: &mut Importer<'_>,
        part: &PartName,
        element: &Element,
        after: Option<NodeId>,
    ) {
        if is_bookmark(element) {
            match marker_id(element) {
                Some(id) => {
                    let class = classify(element);
                    let end = element.local.as_ref() == "bookmarkEnd";
                    let name = if end {
                        None
                    } else {
                        element.attribute(Some(W), "name").map(Arc::<str>::from)
                    };
                    importer
                        .bookmarks
                        .unmapped
                        .entry((part.clone(), id))
                        .or_default()
                        .push(Located {
                            span: element.span.clone(),
                            end,
                            name,
                            column: class.column,
                            unsupported: class.unsupported,
                            after,
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
            walk(importer, part, child, after);
        }
    }
    walk(importer, part, element, after);
}

pub(crate) fn bind(importer: &mut Importer<'_>) {
    let marks = std::mem::take(&mut importer.bookmarks.marks);
    let unmapped = std::mem::take(&mut importer.bookmarks.unmapped);
    let mut unwritable = std::mem::take(&mut importer.bookmarks.unwritable);
    let mut groups: BTreeMap<(PartName, u32), Vec<Mark>> = BTreeMap::new();
    let mut order = Vec::new();
    for mark in marks {
        let key = (mark.part.clone(), mark.id);
        if !groups.contains_key(&key) {
            order.push(key.clone());
        }
        groups.entry(key).or_default().push(mark);
    }
    let mut accepted = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for key in order {
        seen.insert(key.clone());
        let Some(group) = groups.remove(&key) else {
            continue;
        };
        let id = key.1;
        if unmapped.contains_key(&key) {
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
        if bind_group(importer, id, &group, &mut accepted) == Admission::Rejected
            || group.iter().any(|mark| mark.column)
        {
            // A rejected marker locks its paragraph. So does an accepted
            // column bookmark: colFirst/colLast are not written back, so an
            // edited export is Restricted rather than silently dropping them.
            remember_unwritable(&mut unwritable, &group);
        }
    }
    for (key, located) in unmapped {
        if seen.contains(&key) {
            continue;
        }
        bind_unmapped(importer, &key, located, &mut accepted);
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
) -> Admission {
    let (part, span) = locate_diag(group);
    if group.iter().any(|mark| mark.unsupported) {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkUnsupported { id },
        );
        return Admission::Rejected;
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
        return Admission::Rejected;
    }
    let start = starts[0];
    let end = ends[0];
    let Some(start_at) = place(&importer.document, start.paragraph) else {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return Admission::Rejected;
    };
    let Some(end_at) = place(&importer.document, end.paragraph) else {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return Admission::Rejected;
    };
    if start_at.story != end_at.story {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return Admission::Rejected;
    }
    if !story_admits_bookmarks(start_at.kind) {
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkOutsideBody { id },
        );
        return Admission::Rejected;
    }
    if start_at.kind == StoryKind::Comment
        && !comment_anchor_placeable(&importer.document, start_at.story)
    {
        // A detached comment has no page caret. Inserting the bookmark would
        // make navigation refuse the whole PDF.
        warn(
            importer,
            &part,
            span,
            DiagnosticKind::BookmarkAnchorInvalid { id },
        );
        return Admission::Rejected;
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
        return Admission::Rejected;
    }
    let name = start.name.clone().unwrap_or_else(|| Arc::from(""));
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
    let Some(annotation) = admit_bookmark(importer, &part, &span, id, &name, anchor, accepted)
    else {
        return Admission::Rejected;
    };
    let source = Arc::new(SourceRef {
        part: Arc::from(start.part.as_str()),
        locator: Arc::from(format!("@{}..{}", start.span.start, start.span.end)),
    });
    if let Some(stored) = importer.document.annotation_mut(annotation) {
        stored.source = Some(source);
    }
    Admission::Accepted
}

/// Name checks and insertion shared by mapped ranges and preserved-markup
/// pairs. A refusal is diagnosed with the PDF name rules; the checks match
/// `joyword_pdf::navigation` so an accepted bookmark never poisons export.
/// Returns the inserted annotation.
fn admit_bookmark(
    importer: &mut Importer<'_>,
    part: &PartName,
    span: &Range<usize>,
    id: u32,
    name: &Arc<str>,
    anchor: Anchor,
    accepted: &mut BTreeSet<Arc<str>>,
) -> Option<AnnotationId> {
    match classify_name(name) {
        Some(NameProblem::Empty) => {
            warn(
                importer,
                part,
                span.clone(),
                DiagnosticKind::BookmarkNameEmpty { id },
            );
            return None;
        }
        Some(NameProblem::Control) => {
            warn(
                importer,
                part,
                span.clone(),
                DiagnosticKind::BookmarkNameInvalid { id },
            );
            return None;
        }
        Some(NameProblem::Long) => {
            warn(
                importer,
                part,
                span.clone(),
                DiagnosticKind::BookmarkBudget { id },
            );
            return None;
        }
        None => {}
    }
    if accepted.contains(name.as_ref()) {
        warn(
            importer,
            part,
            span.clone(),
            DiagnosticKind::BookmarkNameDuplicate { id },
        );
        return None;
    }
    if accepted.len() >= MAX_IMPORTED_BOOKMARKS {
        warn(
            importer,
            part,
            span.clone(),
            DiagnosticKind::BookmarkBudget { id },
        );
        return None;
    }
    let annotation = importer.document.insert_annotation(
        AnnotationKind::Bookmark {
            name: Arc::clone(name),
        },
        anchor,
    );
    accepted.insert(Arc::clone(name));
    Some(annotation)
}

/// Admit markers that were only seen inside preserved markup (an opaque
/// table, a block content control, a non-paragraph header child).
///
/// The block holds no paragraphs, so a well-formed named pair cannot anchor
/// its real range. It becomes a point destination beside the block instead;
/// the marker bytes stay in the preserved markup and no source is set, so
/// write-back never moves them. The pair keeps its
/// [`DiagnosticKind::BookmarkAnchorUnmapped`] diagnostic as an informational
/// note that the anchor is synthesized. Everything that does not form a
/// valid named pair is diagnosed and not inserted.
fn bind_unmapped(
    importer: &mut Importer<'_>,
    key: &(PartName, u32),
    located: Vec<Located>,
    accepted: &mut BTreeSet<Arc<str>>,
) {
    let (part, id) = key;
    let Some(first) = located.first() else {
        return;
    };
    let span = first.span.clone();
    if located.iter().any(|mark| mark.unsupported) {
        warn(
            importer,
            part,
            span,
            DiagnosticKind::BookmarkUnsupported { id: *id },
        );
        return;
    }
    if located.iter().any(|mark| mark.column) {
        // A column marker inside preserved markup has no text range to
        // point at; it names a column of a table this slice does not map.
        warn(
            importer,
            part,
            span,
            DiagnosticKind::BookmarkColumn { id: *id },
        );
        return;
    }
    let starts: Vec<&Located> = located.iter().filter(|mark| !mark.end).collect();
    let ends: Vec<&Located> = located.iter().filter(|mark| mark.end).collect();
    if starts.len() != 1 || ends.len() != 1 {
        warn(
            importer,
            part,
            span,
            DiagnosticKind::BookmarkAnchorUnmapped { id: *id },
        );
        return;
    }
    let (start, end) = (starts[0], ends[0]);
    if start.span.start >= end.span.start {
        warn(
            importer,
            part,
            span,
            DiagnosticKind::BookmarkAnchorUnmapped { id: *id },
        );
        return;
    }
    let Some(anchor) = point_anchor(&importer.document, part, start.after) else {
        warn(
            importer,
            part,
            span,
            DiagnosticKind::BookmarkAnchorUnmapped { id: *id },
        );
        return;
    };
    let name = start.name.clone().unwrap_or_else(|| Arc::from(""));
    if admit_bookmark(importer, part, &span, *id, &name, anchor, accepted).is_some() {
        warn(
            importer,
            part,
            span,
            DiagnosticKind::BookmarkAnchorUnmapped { id: *id },
        );
    }
}

/// A point range beside preserved markup: the end of the nearest preceding
/// paragraph layout can place, or the start of the first paragraph of the
/// story that owns `part` when the preserved block leads the story.
fn point_anchor(document: &Document, part: &PartName, after: Option<NodeId>) -> Option<Anchor> {
    let (story, paragraph, offset) = after
        .and_then(|paragraph| {
            let at = place(document, paragraph)?;
            if !story_admits_bookmarks(at.kind) {
                return None;
            }
            let len = document.find_paragraph(at.story, paragraph)?.len();
            Some((at.story, paragraph, len))
        })
        .or_else(|| {
            let story = document.stories().find(|story| {
                story_admits_bookmarks(story.kind)
                    && story
                        .blocks
                        .iter()
                        .filter_map(Block::as_paragraph)
                        .any(|paragraph| paragraph_part(paragraph) == Some(part.as_str()))
            })?;
            let paragraph = story.blocks.iter().find_map(Block::as_paragraph)?.id();
            Some((story.id, paragraph, 0))
        })?;
    Some(Anchor::Range {
        start: Position {
            story,
            paragraph,
            offset: ScalarOffset(offset),
            bias: Bias::After,
        },
        end: Position {
            story,
            paragraph,
            offset: ScalarOffset(offset),
            bias: Bias::Before,
        },
    })
}

fn story_admits_bookmarks(kind: StoryKind) -> bool {
    matches!(
        kind,
        StoryKind::Body | StoryKind::Header | StoryKind::Footer | StoryKind::Comment
    )
}

/// The comment anchor is a range layout can turn into a page caret.
fn comment_anchor_placeable(document: &Document, story: StoryId) -> bool {
    let Some(annotation) = document.annotations().find(|annotation| {
        matches!(
            &annotation.kind,
            AnnotationKind::Comment(comment) if comment.body == story
        )
    }) else {
        return false;
    };
    let Anchor::Range { start, end } = &annotation.anchor else {
        return false;
    };
    if start.story != end.story {
        return false;
    }
    let Some(anchor_at) = place(document, start.paragraph) else {
        return false;
    };
    anchor_at.story == start.story
        && matches!(
            anchor_at.kind,
            StoryKind::Body | StoryKind::Header | StoryKind::Footer
        )
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

/// A range this slice can write with the imported marker bytes of `part`.
///
/// The bias is the one import stores. The start and end must be top-level
/// paragraphs of one body, header, footer, or placeable comment story, in
/// order, and that story must live in `part`. A body bookmark may land on a
/// new top-level body paragraph that has no source of its own; the part is
/// the body's. A header, footer, or comment bookmark may not: its paragraphs
/// have to name `part`, so the marker is not invented into `document.xml` or
/// another story part. A detached anchor, a reversed range, a table cell, or
/// an offset past the paragraph is not representable.
pub(crate) fn anchor_writable(document: &Document, part: &str, anchor: &Anchor) -> bool {
    let Anchor::Range { start, end } = anchor else {
        return false;
    };
    if start.bias != Bias::After || end.bias != Bias::Before || start.story != end.story {
        return false;
    }
    let Some(story) = document.story(start.story) else {
        return false;
    };
    if !story_admits_bookmarks(story.kind) {
        return false;
    }
    if story.kind == StoryKind::Comment && !comment_anchor_placeable(document, story.id) {
        return false;
    }
    let paragraphs: Vec<&Paragraph> = story
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
    if start_at > end_at || (start_at == end_at && start.offset.0 > end.offset.0) {
        return false;
    }
    if story.kind == StoryKind::Body {
        return story.id == document.body_id() && body_part(document) == Some(part);
    }
    paragraph_part(paragraphs[start_at]) == Some(part)
        && paragraph_part(paragraphs[end_at]) == Some(part)
}

fn body_part(document: &Document) -> Option<&str> {
    document.body().blocks.iter().find_map(|block| {
        block
            .as_paragraph()
            .and_then(|paragraph| paragraph_part(paragraph))
    })
}

fn paragraph_part(paragraph: &Paragraph) -> Option<&str> {
    paragraph
        .source
        .as_deref()
        .map(|source| source.part.as_ref())
}

struct SavedBookmark {
    id: u32,
    start: Vec<u8>,
    end: Vec<u8>,
}

/// Splice imported bookmark markers into `patched` when an edit moved them
/// or rewrote their host paragraph.
///
/// Only bookmarks whose source part is `part` are considered. `None` means
/// every such bookmark still sits on an unedited host, so the patched part
/// already contains the original marker bytes. Markers this slice did not
/// import are refused rather than deleted. Body markers stay in
/// `document.xml`; header, footer, and comment markers stay in their own part.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_back(
    part: &str,
    original_xml: &[u8],
    patched: &[u8],
    original: &Document,
    current: &Document,
    edited: &BTreeSet<NodeId>,
    budget: &Budget,
) -> Result<Option<Vec<u8>>, ExportError> {
    let saved = saved_bookmarks(part, original_xml, original, budget)?;
    if saved.is_empty() {
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
    let root = parse_part(part, patched, budget)?;
    let mut strip = Vec::new();
    collect_markers(&root, &moving_ids, &known_ids, &mut strip)?;
    strip.sort_by_key(|range| range.start);
    for pair in strip.windows(2) {
        if pair[1].start < pair[0].end {
            return Err(unsupported(None, "bookmark markup cannot be dropped"));
        }
    }
    let stripped = apply_edits(patched, &strip, &[]);
    let root = parse_part(part, &stripped, budget)?;
    let needed = needed_paragraphs(current, &saved, &moving);
    let aligned = align_part(part, &root, current, &needed)?;
    let mut events: BTreeMap<usize, Vec<MarkerEvent>> = BTreeMap::new();
    for (index, saved_index) in moving.iter().enumerate() {
        let (annotation_id, marker) = &saved[*saved_index];
        let annotation = current
            .annotation(*annotation_id)
            .ok_or_else(|| unsupported(None, "a bookmark was removed"))?;
        let AnnotationKind::Bookmark { .. } = &annotation.kind else {
            return Err(unsupported(None, "a bookmark changed"));
        };
        if !anchor_writable(current, part, &annotation.anchor) {
            return Err(unsupported(None, "a bookmark changed"));
        }
        let Anchor::Range { start, end } = &annotation.anchor else {
            return Err(unsupported(None, "a bookmark changed"));
        };
        let Some(start_at) = aligned
            .iter()
            .position(|(paragraph, _)| *paragraph == start.paragraph)
        else {
            return Err(unsupported(None, "bookmark range cannot be written"));
        };
        let Some(end_at) = aligned
            .iter()
            .position(|(paragraph, _)| *paragraph == end.paragraph)
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
            aligned[index].1,
            &paragraph_events,
        )?);
    }
    let bytes = apply_edits(&stripped, &[], &edits);
    if bytes.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: part.into(),
            limit: budget.max_part_bytes,
        });
    }
    Ok(Some(bytes))
}

fn needed_paragraphs(
    document: &Document,
    saved: &[(AnnotationId, SavedBookmark)],
    moving: &[usize],
) -> BTreeSet<NodeId> {
    let mut needed = BTreeSet::new();
    for index in moving {
        let Some(annotation) = document.annotation(saved[*index].0) else {
            continue;
        };
        let Anchor::Range { start, end } = &annotation.anchor else {
            continue;
        };
        needed.insert(start.paragraph);
        needed.insert(end.paragraph);
    }
    needed
}

fn align_part<'a>(
    part: &str,
    root: &'a Element,
    document: &Document,
    needed: &BTreeSet<NodeId>,
) -> Result<Vec<(NodeId, &'a Element)>, ExportError> {
    let fail = || unsupported(None, "bookmark range cannot be written");
    if root.is(W, "document") {
        let body = root.child(W, "body").ok_or_else(fail)?;
        let model = top_paragraphs(document.body());
        let xml: Vec<&Element> = body.elements().filter(|child| child.is(W, "p")).collect();
        return zip_ids(&model, &xml);
    }
    if root.is(W, "hdr") || root.is(W, "ftr") {
        let story = story_for_part(document, part, root.is(W, "hdr"))?;
        let model = top_paragraphs(story);
        let xml: Vec<&Element> = root.elements().filter(|child| child.is(W, "p")).collect();
        return zip_ids(&model, &xml);
    }
    if root.is(W, "comments") {
        let mut aligned = Vec::new();
        for element in root.elements().filter(|child| child.is(W, "comment")) {
            let Some(id) = marker_id(element) else {
                continue;
            };
            let Some(story_id) = comment_body(document, id) else {
                continue;
            };
            let Some(story) = document.story(story_id) else {
                continue;
            };
            let model = top_paragraphs(story);
            let xml: Vec<&Element> = element
                .elements()
                .filter(|child| child.is(W, "p"))
                .collect();
            let targets = model.iter().any(|id| needed.contains(id));
            if model.len() != xml.len() {
                if targets {
                    return Err(fail());
                }
                continue;
            }
            aligned.extend(model.into_iter().zip(xml));
        }
        if needed
            .iter()
            .any(|id| aligned.iter().all(|(paragraph, _)| paragraph != id))
        {
            return Err(fail());
        }
        return Ok(aligned);
    }
    Err(fail())
}

fn zip_ids<'a>(
    model: &[NodeId],
    xml: &[&'a Element],
) -> Result<Vec<(NodeId, &'a Element)>, ExportError> {
    if model.len() != xml.len() {
        return Err(unsupported(None, "bookmark range cannot be written"));
    }
    Ok(model.iter().copied().zip(xml.iter().copied()).collect())
}

fn top_paragraphs(story: &Story) -> Vec<NodeId> {
    story
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph().map(|paragraph| paragraph.id()))
        .collect()
}

fn story_for_part<'a>(
    document: &'a Document,
    part: &str,
    header: bool,
) -> Result<&'a Story, ExportError> {
    let kind = if header {
        StoryKind::Header
    } else {
        StoryKind::Footer
    };
    let mut found = None;
    for story in document.stories() {
        if story.kind != kind {
            continue;
        }
        let in_part = story
            .blocks
            .iter()
            .filter_map(Block::as_paragraph)
            .any(|paragraph| paragraph_part(paragraph) == Some(part));
        if !in_part {
            continue;
        }
        if found.is_some() {
            return Err(unsupported(None, "bookmark range cannot be written"));
        }
        found = Some(story);
    }
    found.ok_or_else(|| unsupported(None, "bookmark range cannot be written"))
}

fn comment_body(document: &Document, source_id: u32) -> Option<StoryId> {
    document.annotations().find_map(|annotation| {
        let AnnotationKind::Comment(comment) = &annotation.kind else {
            return None;
        };
        (comment.source_id == source_id).then_some(comment.body)
    })
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
    part: &str,
    xml: &[u8],
    document: &Document,
    budget: &Budget,
) -> Result<Vec<(AnnotationId, SavedBookmark)>, ExportError> {
    let bookmarks: Vec<_> = current_bookmarks(document)
        .filter(|annotation| {
            annotation
                .source
                .as_deref()
                .is_some_and(|source| source.part.as_ref() == part)
        })
        .collect();
    if bookmarks.is_empty() {
        return Ok(Vec::new());
    }
    let root = parse_part(part, xml, budget)?;
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

fn parse_part(part: &str, bytes: &[u8], budget: &Budget) -> Result<Element, ExportError> {
    XmlReader::new(part, bytes, budget)
        .and_then(XmlReader::whole)
        .map_err(|error| {
            ExportError::Import(crate::ImportError::Xml {
                part: part.into(),
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
