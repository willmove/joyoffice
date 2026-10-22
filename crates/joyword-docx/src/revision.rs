//! Typed import of text, move and format revisions (ECMA-376 Part 1
//! §17.13.5, T300/T301).
//!
//! `w:ins`, `w:del`, `w:moveFrom` and `w:moveTo` become [`TextRevision`] marks
//! on runs when the wrapper is a plain run sequence. `w:rPrChange`
//! (§17.13.5.30) and `w:pPrChange` (§17.13.5.29) in body paragraphs become
//! [`FormatRevision`] records holding the previous run or paragraph
//! properties. Anything outside those subsets is preserved and locks the
//! paragraph; it does not make the whole document read-only. Table structure
//! revisions and format revisions inside tables are refused by
//! [`crate::ns::blocks_for_review`].

use std::collections::BTreeMap;
use std::sync::Arc;

use joyword_model::{
    Block, BreakKind, FormatRevision, InlineKind, InlineObject, MoveMarker, MoveSide, NodeId,
    Paragraph, RevisionKind, RunProps, StoryId, TextRevision,
};

use crate::import::Importer;
use crate::ns::W;
use crate::props::{self, Findings};
use crate::result::{DiagnosticKind, Severity};
use crate::xml::Element;
use crate::xml::XML_NAMESPACE;
use joyoffice_opc::PartName;

use crate::import::Scope;

/// A move marker seen before its paragraph is in a story.
pub(crate) struct PendingMove {
    pub paragraph: NodeId,
    pub offset: usize,
    pub id: u32,
    pub name: Arc<str>,
    pub author: Arc<str>,
    pub date: Option<Arc<str>>,
    pub side: MoveSide,
    pub at_end: bool,
}

/// Imports one run-level revision. `false` means the caller preserves it.
pub(crate) fn import_run_revision(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    paragraph: &mut Paragraph,
) -> bool {
    let Some(kind) = revision_kind(&element.local) else {
        return false;
    };
    let Some(mark) = plain_revision(element, kind) else {
        importer.diagnose(
            Severity::Warning,
            Some(part),
            Some(element.span.clone()),
            DiagnosticKind::RevisionSubset {
                element: element.qname.to_string(),
            },
        );
        return false;
    };
    importer.document.note_revision_id(mark.id);
    for child in element.elements() {
        if child.is(W, "proofErr") {
            importer.hint(part, "proofErr");
            continue;
        }
        if !child.is(W, "r") {
            continue;
        }
        push_revised_run(importer, part, child, scope, paragraph, &mark);
    }
    true
}

/// Records one move range marker. `false` means the caller preserves it.
pub(crate) fn import_move_marker(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    paragraph: &Paragraph,
) -> bool {
    let Some((side, at_end)) = move_ends(&element.local) else {
        return false;
    };
    let Some(marker) = plain_marker(element, side, at_end, paragraph) else {
        importer.diagnose(
            Severity::Warning,
            Some(part),
            Some(element.span.clone()),
            DiagnosticKind::RevisionSubset {
                element: element.qname.to_string(),
            },
        );
        return false;
    };
    importer.document.note_revision_id(marker.id);
    importer.pending_moves.push(marker);
    true
}

/// Places pending markers and locks paragraphs of an incomplete or crossed pair.
pub(crate) fn flush_moves(importer: &mut Importer<'_>) {
    let pending = std::mem::take(&mut importer.pending_moves);
    let mut located = Vec::new();
    for marker in pending {
        let Some((story, _)) = find_paragraph(&importer.document, marker.paragraph) else {
            importer.diagnose(
                Severity::Warning,
                None,
                None,
                DiagnosticKind::MoveRange {
                    name: marker.name.to_string(),
                },
            );
            importer.editability.lock(marker.paragraph);
            continue;
        };
        let position =
            MoveMarker::position_for(story, marker.paragraph, marker.offset, marker.at_end);
        located.push(MoveMarker {
            id: marker.id,
            name: marker.name,
            author: marker.author,
            date: marker.date,
            side: marker.side,
            at_end: marker.at_end,
            position,
        });
    }
    *importer.document.move_markers_mut() = located;
    let order = paragraph_order(&importer.document);
    let snapshot: Vec<MoveMarker> = importer.document.move_markers().to_vec();
    let mut bad: Vec<(String, Vec<NodeId>)> = Vec::new();
    let mut by_name: BTreeMap<Arc<str>, Vec<usize>> = BTreeMap::new();
    for (index, marker) in snapshot.iter().enumerate() {
        by_name
            .entry(Arc::clone(&marker.name))
            .or_default()
            .push(index);
    }
    for (name, indexes) in by_name {
        let markers: Vec<&MoveMarker> = indexes.iter().map(|index| &snapshot[*index]).collect();
        let paragraphs: Vec<NodeId> = markers
            .iter()
            .map(|marker| marker.position.paragraph)
            .collect();
        if !pair_is_complete(&markers, &order) {
            bad.push((name.to_string(), paragraphs));
        }
    }
    for (name, paragraphs) in bad {
        importer.diagnose(
            Severity::Warning,
            None,
            None,
            DiagnosticKind::MoveRange { name },
        );
        for paragraph in paragraphs {
            importer.editability.lock(paragraph);
        }
    }
}

/// Parses a `w:rPrChange` (§17.13.5.30) or `w:pPrChange` (§17.13.5.29)
/// element into a typed [`FormatRevision`]. Returns `None` when the change is
/// outside the subset JoyWord types: illegal or duplicated tracking
/// attributes, more than one previous-property child, a previous property that
/// is not fully representable, a previous character or paragraph style, a
/// previous paragraph mark, or a nested revision inside the previous property.
/// On `None` the caller must fall back (preserve the property element and lock
/// the paragraph); it never silently drops the change.
pub(crate) fn import_format_change(
    importer: &mut Importer<'_>,
    element: &Element,
    paragraph_level: bool,
) -> Option<FormatRevision> {
    let (id, author, date) = track_attrs(element, false)?;
    let child_name = if paragraph_level { "pPr" } else { "rPr" };
    let mut children = element.elements();
    let previous = match children.next() {
        None => None,
        Some(child) => {
            // §17.13.5.29/.30: exactly one optional previous-property child.
            if !child.is(W, child_name) || children.next().is_some() {
                return None;
            }
            Some(child)
        }
    };
    let revision = if paragraph_level {
        let previous_para = match previous {
            None => None,
            Some(ppr) => {
                let mut findings = Findings::default();
                let parsed = props::para(ppr, &mut findings);
                // A previous style, mark, section or nested change, or any
                // unrepresentable property, is out of subset.
                if !findings.complete
                    || parsed.style.is_some()
                    || parsed.mark.is_some()
                    || parsed.section.is_some()
                {
                    return None;
                }
                Some(Arc::new(parsed.props))
            }
        };
        FormatRevision {
            id,
            author,
            date,
            previous_run: None,
            previous_para,
        }
    } else {
        let previous_run = match previous {
            None => None,
            Some(rpr) => {
                let mut findings = Findings::default();
                // Plain `run`: a nested `w:rPrChange` gaps and fails `complete`.
                let (props, style) = props::run(rpr, &mut findings);
                // A previous character style or unrepresentable property is out
                // of subset; previous run properties carry no preserved handle.
                if !findings.complete || style.is_some() {
                    return None;
                }
                Some(Arc::new(props))
            }
        };
        FormatRevision {
            id,
            author,
            date,
            previous_run,
            previous_para: None,
        }
    };
    importer.document.note_revision_id(id);
    Some(revision)
}

pub(crate) fn revision_kind(local: &str) -> Option<RevisionKind> {    match local {
        "ins" => Some(RevisionKind::Insert),
        "del" => Some(RevisionKind::Delete),
        "moveFrom" => Some(RevisionKind::MoveFrom),
        "moveTo" => Some(RevisionKind::MoveTo),
        _ => None,
    }
}

fn move_ends(local: &str) -> Option<(MoveSide, bool)> {
    match local {
        "moveFromRangeStart" => Some((MoveSide::From, false)),
        "moveFromRangeEnd" => Some((MoveSide::From, true)),
        "moveToRangeStart" => Some((MoveSide::To, false)),
        "moveToRangeEnd" => Some((MoveSide::To, true)),
        _ => None,
    }
}

/// Plain `w:ins` / `w:del` / move wrapper for a typed table cell.
/// Anything the cell parser cannot represent returns `None`.
pub(crate) fn cell_text_revision(element: &Element, kind: RevisionKind) -> Option<TextRevision> {
    plain_revision(element, kind)
}

fn plain_revision(element: &Element, kind: RevisionKind) -> Option<TextRevision> {
    let (id, author, date) = track_attrs(element, false)?;
    if !wrapper_children_are_plain(element, kind) {
        return None;
    }
    Some(TextRevision {
        id,
        kind,
        author,
        date,
    })
}

fn plain_marker(
    element: &Element,
    side: MoveSide,
    at_end: bool,
    paragraph: &Paragraph,
) -> Option<PendingMove> {
    if element.elements().next().is_some() {
        return None;
    }
    let (id, author, date) = track_attrs(element, true)?;
    let mut name = None;
    for attribute in &element.attributes {
        if attribute.namespace.as_deref() != Some(W) || &*attribute.local != "name" {
            if matches!(&*attribute.local, "id" | "author" | "date")
                && attribute.namespace.as_deref() == Some(W)
            {
                continue;
            }
            return None;
        }
        if name.is_some() || attribute.value.is_empty() {
            return None;
        }
        name = Some(Arc::<str>::from(attribute.value.as_str()));
    }
    Some(PendingMove {
        paragraph: paragraph.id(),
        offset: paragraph.len(),
        id,
        name: name?,
        author,
        date,
        side,
        at_end,
    })
}

fn track_attrs(element: &Element, allow_name: bool) -> Option<(u32, Arc<str>, Option<Arc<str>>)> {
    let mut id = None;
    let mut author = None;
    let mut date = None;
    for attribute in &element.attributes {
        if attribute.namespace.as_deref() != Some(W) {
            return None;
        }
        match &*attribute.local {
            "id" => {
                if id.is_some() {
                    return None;
                }
                id = attribute.value.parse::<u32>().ok();
                id?;
            }
            "author" => {
                if author.is_some() || attribute.value.is_empty() {
                    return None;
                }
                author = Some(Arc::<str>::from(attribute.value.as_str()));
            }
            "date" => {
                if date.is_some() || attribute.value.is_empty() {
                    return None;
                }
                date = Some(Arc::<str>::from(attribute.value.as_str()));
            }
            "name" if allow_name => {}
            _ => return None,
        }
    }
    Some((id?, author?, date))
}

fn wrapper_children_are_plain(element: &Element, kind: RevisionKind) -> bool {
    let deleted = matches!(kind, RevisionKind::Delete | RevisionKind::MoveFrom);
    for child in element.elements() {
        if child.is(W, "proofErr") {
            continue;
        }
        if !child.is(W, "r") || !run_is_plain(child, deleted) {
            return false;
        }
    }
    true
}

fn run_is_plain(element: &Element, deleted: bool) -> bool {
    for child in element.elements() {
        if !child.is(W, "rPr") && !leaf_is_plain(child, deleted) {
            return false;
        }
    }
    true
}

fn leaf_is_plain(child: &Element, deleted: bool) -> bool {
    match &*child.local {
        "t" if !deleted => text_leaf(child),
        "delText" if deleted => text_leaf(child),
        "tab" => child.attributes.is_empty() && child.elements().next().is_none(),
        "br" | "cr" => break_is_plain(child),
        _ => false,
    }
}

fn text_leaf(child: &Element) -> bool {
    if child.elements().next().is_some() || child.text().contains('\u{FFFC}') {
        return false;
    }
    child.attributes.iter().all(|attribute| {
        attribute.namespace.as_deref() == Some(XML_NAMESPACE) && &*attribute.local == "space"
    })
}

fn break_is_plain(child: &Element) -> bool {
    let clear = child.attribute(Some(W), "clear").unwrap_or("none");
    let kind = match (&*child.local, child.attribute(Some(W), "type"), clear) {
        ("cr", _, _) => Some(BreakKind::Line),
        (_, None | Some("textWrapping"), "none") => Some(BreakKind::Line),
        (_, Some("page"), "none") => Some(BreakKind::Page),
        (_, Some("column"), "none") => Some(BreakKind::Column),
        _ => None,
    };
    kind.is_some()
        && child.attributes.iter().all(|attribute| {
            attribute.namespace.as_deref() == Some(W)
                && matches!(&*attribute.local, "type" | "clear")
        })
}

fn push_revised_run(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    paragraph: &mut Paragraph,
    mark: &TextRevision,
) {
    let (props, style) = match element.child(W, "rPr") {
        Some(rpr) => {
            let mut findings = Findings::default();
            let (mut props, style) = props::run(rpr, &mut findings);
            props.preserved_layout_neutral = findings.layout_neutral();
            props.preserved = importer.settle(part, rpr, scope, findings);
            (props, style)
        }
        None => (RunProps::default(), None),
    };
    let props = Arc::new(props);
    let deleted = matches!(mark.kind, RevisionKind::Delete | RevisionKind::MoveFrom);
    for child in element.elements() {
        if child.is(W, "rPr") {
            continue;
        }
        match &*child.local {
            "t" | "delText"
                if (deleted && child.local.as_ref() == "delText")
                    || (!deleted && child.local.as_ref() == "t") =>
            {
                if paragraph
                    .push_text_marked(&child.text(), style.clone(), props.clone(), mark.clone())
                    .is_err()
                {
                    importer.diagnose(
                        Severity::Warning,
                        Some(part),
                        Some(child.span.clone()),
                        DiagnosticKind::RevisionSubset {
                            element: child.qname.to_string(),
                        },
                    );
                }
            }
            "tab" => {
                if paragraph
                    .push_text_marked("\t", style.clone(), props.clone(), mark.clone())
                    .is_err()
                {
                    importer.diagnose(
                        Severity::Warning,
                        Some(part),
                        Some(child.span.clone()),
                        DiagnosticKind::RevisionSubset {
                            element: child.qname.to_string(),
                        },
                    );
                }
            }
            "br" | "cr" => {
                let clear = child.attribute(Some(W), "clear").unwrap_or("none");
                let kind = match (&*child.local, child.attribute(Some(W), "type"), clear) {
                    ("cr", _, _) => BreakKind::Line,
                    (_, Some("page"), _) => BreakKind::Page,
                    (_, Some("column"), _) => BreakKind::Column,
                    _ => BreakKind::Line,
                };
                paragraph.push_inline(
                    InlineObject {
                        id: importer.document.ids().node(),
                        kind: InlineKind::Break(kind),
                        source: Some(Importer::source(part, child)),
                    },
                    style.clone(),
                    props.clone(),
                );
                paragraph.set_last_run_revision(Some(mark.clone()));
            }
            _ => {}
        }
    }
}

fn find_paragraph(
    document: &joyword_model::Document,
    paragraph: NodeId,
) -> Option<(StoryId, usize)> {
    for story in document.stories() {
        if let Some(index) = paragraph_index(&story.blocks, paragraph) {
            return Some((story.id, index));
        }
    }
    None
}

fn paragraph_index(blocks: &joyword_model::BlockSeq, paragraph: NodeId) -> Option<usize> {
    for (index, block) in blocks.iter().enumerate() {
        if block_contains(block, paragraph) {
            return Some(index);
        }
    }
    None
}

fn block_contains(block: &Block, paragraph: NodeId) -> bool {
    match block {
        Block::Paragraph(found) => found.id() == paragraph,
        Block::Table(table) => table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .any(|cell| {
                cell.blocks
                    .iter()
                    .any(|inner| block_contains(inner, paragraph))
            }),
        Block::Sdt(sdt) => sdt
            .blocks
            .iter()
            .any(|inner| block_contains(inner, paragraph)),
        Block::Opaque(_) => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    story: usize,
    block: usize,
    offset: usize,
}

fn paragraph_order(document: &joyword_model::Document) -> BTreeMap<NodeId, (usize, usize)> {
    let mut order = BTreeMap::new();
    for (story_index, story) in document.stories().enumerate() {
        let mut sequence = 0usize;
        fn walk(
            block: &Block,
            story_index: usize,
            sequence: &mut usize,
            order: &mut BTreeMap<NodeId, (usize, usize)>,
        ) {
            match block {
                Block::Paragraph(paragraph) => {
                    order.insert(paragraph.id(), (story_index, *sequence));
                    *sequence += 1;
                }
                Block::Table(table) => {
                    for cell in table.rows.iter().flat_map(|row| row.cells.iter()) {
                        for inner in cell.blocks.iter() {
                            walk(inner, story_index, sequence, order);
                        }
                    }
                }
                Block::Sdt(sdt) => {
                    for inner in sdt.blocks.iter() {
                        walk(inner, story_index, sequence, order);
                    }
                }
                Block::Opaque(_) => {}
            }
        }
        for block in story.blocks.iter() {
            walk(block, story_index, &mut sequence, &mut order);
        }
    }
    order
}

fn place_of(order: &BTreeMap<NodeId, (usize, usize)>, marker: &MoveMarker) -> Option<Place> {
    let (story, block) = order.get(&marker.position.paragraph).copied()?;
    Some(Place {
        story,
        block,
        offset: marker.position.offset.0,
    })
}

fn pair_is_complete(markers: &[&MoveMarker], order: &BTreeMap<NodeId, (usize, usize)>) -> bool {
    let mut from_start = None;
    let mut from_end = None;
    let mut to_start = None;
    let mut to_end = None;
    for marker in markers {
        let slot = match (marker.side, marker.at_end) {
            (MoveSide::From, false) => &mut from_start,
            (MoveSide::From, true) => &mut from_end,
            (MoveSide::To, false) => &mut to_start,
            (MoveSide::To, true) => &mut to_end,
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(*marker);
    }
    let (Some(from_start), Some(from_end), Some(to_start), Some(to_end)) =
        (from_start, from_end, to_start, to_end)
    else {
        return false;
    };
    let Some(from_a) = place_of(order, from_start) else {
        return false;
    };
    let Some(from_b) = place_of(order, from_end) else {
        return false;
    };
    let Some(to_a) = place_of(order, to_start) else {
        return false;
    };
    let Some(to_b) = place_of(order, to_end) else {
        return false;
    };
    if from_a > from_b || to_a > to_b {
        return false;
    }
    !ranges_overlap(from_a, from_b, to_a, to_b)
}

fn ranges_overlap(a0: Place, a1: Place, b0: Place, b1: Place) -> bool {
    a0 < b1 && b0 < a1
}
