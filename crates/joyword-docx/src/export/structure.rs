//! Plain body paragraph structure in edited export (ECMA-376 Part 1 §17.2.2
//! body, §17.3.1.22 p, §17.6.17 sectPr; Part 2 for the unchanged parts).
//!
//! The engine keeps a split paragraph's identity and source on the head and
//! gives the tail a new identity without a source; a merge keeps the first
//! paragraph and ends the others. Identities, never text similarity, decide
//! which source element a paragraph belongs to, and surviving source blocks
//! must keep their order.
//!
//! Every paragraph that a split, merge, insertion or removal touches must be
//! plain: default paragraph, mark and character properties, only text and
//! tabs, and no preserved fragment, comment, processing instruction, unknown
//! attribute or section break. A new paragraph is written after the nearest
//! preceding surviving source block; a removed one loses exactly its element
//! range. All other bytes of the part stay as they were.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use joyword_model::{Block, Document, NodeId, ParaProps, Paragraph, SourceRef};

use super::{ExportError, Names, bounded_text_xml, text_slots, unsupported};
use crate::xml::{Element, Node};
use crate::{PackageBacking, W};

/// Where a group of new paragraphs is written.
#[derive(Clone, Copy)]
pub(super) enum Anchor<'a> {
    /// After this surviving source block.
    After(&'a Block),
    /// Before this block, the first of the source body; no surviving source
    /// block precedes the group.
    Before(&'a Block),
}

/// Consecutive new paragraphs and where they go.
pub(super) struct Insertion<'a> {
    pub anchor: Anchor<'a>,
    pub paragraphs: Vec<&'a Paragraph>,
}

/// How the current body relates to the imported one.
#[derive(Default)]
pub(super) struct Alignment<'a> {
    /// Surviving source paragraphs whose content changed: (imported, current).
    pub edited: Vec<(&'a Paragraph, &'a Paragraph)>,
    /// Surviving source paragraphs (as imported) next to a new paragraph or
    /// followed by removed ones. They must be plain even if unchanged.
    pub involved: Vec<&'a Paragraph>,
    /// Source paragraphs that no longer exist.
    pub removed: Vec<&'a Paragraph>,
    pub inserted: Vec<Insertion<'a>>,
}

impl Alignment<'_> {
    pub fn is_unchanged(&self) -> bool {
        self.edited.is_empty() && self.removed.is_empty() && self.inserted.is_empty()
    }
}

/// Matches current body blocks to the imported ones by identity.
pub(super) fn align<'a>(
    original: &'a Document,
    current: &'a Document,
) -> Result<Alignment<'a>, ExportError> {
    let before: Vec<&Block> = original.body().blocks.iter().collect();
    let index: HashMap<NodeId, usize> = before
        .iter()
        .enumerate()
        .map(|(i, block)| (block.id(), i))
        .collect();
    let mut survived = vec![false; before.len()];
    let mut involved = vec![false; before.len()];
    let mut alignment = Alignment::default();
    // The last surviving source block seen, and what the previous block was.
    let mut previous: Option<usize> = None;
    let mut previous_new = false;
    for after in current.body().blocks.iter() {
        let Some(&i) = index.get(&after.id()) else {
            let Block::Paragraph(paragraph) = after else {
                return Err(unsupported(Some(after.id()), "new non-paragraph block"));
            };
            if paragraph.source.is_some() {
                return Err(unsupported(
                    Some(after.id()),
                    "new paragraph claims an original source",
                ));
            }
            if previous_new {
                let group = alignment.inserted.last_mut().expect("open group");
                group.paragraphs.push(paragraph);
            } else {
                if let Some(i) = previous {
                    involved[i] = true;
                }
                let anchor = match previous {
                    Some(i) => Anchor::After(before[i]),
                    None => Anchor::Before(before[0]),
                };
                alignment.inserted.push(Insertion {
                    anchor,
                    paragraphs: vec![paragraph],
                });
            }
            previous_new = true;
            continue;
        };
        // Strictly increasing source order also rejects duplicates.
        if previous.is_some_and(|p| i <= p) {
            return Err(unsupported(
                Some(after.id()),
                "body structure or order changed",
            ));
        }
        match (before[i], after) {
            (Block::Paragraph(b), Block::Paragraph(a)) => {
                if b.as_ref() != a.as_ref() {
                    alignment.edited.push((b.as_ref(), a.as_ref()));
                }
            }
            (Block::Opaque(b), Block::Opaque(a))
                if b.id == a.id && b.preserved == a.preserved && b.source == a.source => {}
            _ => {
                return Err(unsupported(
                    Some(after.id()),
                    "body structure or order changed",
                ));
            }
        }
        survived[i] = true;
        previous = Some(i);
        previous_new = false;
    }
    // A removed block was merged into, or simply follows, the preceding
    // survivor; treat that survivor as touched either way.
    let mut survivor = None;
    for (i, block) in before.iter().enumerate() {
        if survived[i] {
            survivor = Some(i);
            continue;
        }
        let Block::Paragraph(paragraph) = block else {
            return Err(unsupported(Some(block.id()), "non-paragraph block removed"));
        };
        alignment.removed.push(paragraph);
        if let Some(s) = survivor {
            involved[s] = true;
        }
    }
    alignment.involved = before
        .iter()
        .zip(involved)
        .filter_map(|(block, involved)| involved.then(|| block.as_paragraph()).flatten())
        .collect();
    Ok(alignment)
}

/// Source elements of the main part's body and what they hold.
pub(super) struct Source<'a> {
    part: &'a str,
    xml: &'a [u8],
    /// Direct children of `w:body`, by byte range.
    elements: HashMap<(usize, usize), &'a Element>,
    /// Preserved fragment ranges in this part, by start.
    fragments: Vec<Range<usize>>,
    section_ends: HashSet<NodeId>,
}

impl<'a> Source<'a> {
    pub fn new(
        part: &'a str,
        xml: &'a [u8],
        body: &'a Element,
        backing: &PackageBacking,
        original: &Document,
    ) -> Self {
        let mut fragments: Vec<_> = backing
            .fragments()
            .filter(|(_, fragment)| fragment.part.as_str() == part)
            .map(|(_, fragment)| fragment.range.clone())
            .collect();
        fragments.sort_by_key(|range| (range.start, range.end));
        Self {
            part,
            xml,
            elements: body
                .elements()
                .map(|element| ((element.span.start, element.span.end), element))
                .collect(),
            fragments,
            section_ends: original
                .sections()
                .iter()
                .filter_map(|section| section.last_block)
                .collect(),
        }
    }

    fn element(&self, id: NodeId, source: Option<&SourceRef>) -> Result<&'a Element, ExportError> {
        let source =
            source.ok_or_else(|| unsupported(Some(id), "paragraph has no original XML"))?;
        (source.part.as_ref() == self.part)
            .then(|| locator(&source.locator))
            .flatten()
            .and_then(|span| self.elements.get(&span).copied())
            .ok_or_else(|| unsupported(Some(id), "paragraph source does not match backing"))
    }

    /// The `w:p` an imported paragraph was read from.
    pub fn paragraph(&self, paragraph: &Paragraph) -> Result<&'a Element, ExportError> {
        let element = self.element(paragraph.id(), paragraph.source.as_deref())?;
        if !element.is(W, "p") {
            return Err(unsupported(
                Some(paragraph.id()),
                "paragraph source does not match backing",
            ));
        }
        Ok(element)
    }

    /// Byte offset where an insertion group is written.
    pub fn insertion_point(&self, anchor: Anchor<'_>) -> Result<usize, ExportError> {
        let (block, after) = match anchor {
            Anchor::After(block) => (block, true),
            Anchor::Before(block) => (block, false),
        };
        let element = self.element(block.id(), block_source(block))?;
        Ok(if after {
            element.span.end
        } else {
            element.span.start
        })
    }

    /// Checks that a split, merge or removal may touch this imported
    /// paragraph without moving or dropping anything the model does not
    /// represent. Returns its source element.
    pub fn plain(&self, paragraph: &Paragraph) -> Result<&'a Element, ExportError> {
        let error = |reason| unsupported(Some(paragraph.id()), reason);
        if !default_format(paragraph) {
            return Err(error(
                "paragraph structure edits require default paragraph and character properties",
            ));
        }
        let element = self.paragraph(paragraph)?;
        if self.section_ends.contains(&paragraph.id())
            || element
                .child(W, "pPr")
                .is_some_and(|ppr| ppr.child(W, "sectPr").is_some())
        {
            return Err(error("paragraph ends a section"));
        }
        let raw = &self.xml[element.span.clone()];
        if raw.windows(4).any(|w| w == b"<!--") || raw.windows(2).any(|w| w == b"<?") {
            return Err(error(
                "paragraph contains comments or processing instructions",
            ));
        }
        if self.holds_fragment(&element.span) {
            return Err(error("paragraph holds preserved markup"));
        }
        plain_markup(element).map_err(error)?;
        text_slots(element, self.xml).map_err(error)?;
        Ok(element)
    }

    fn holds_fragment(&self, span: &Range<usize>) -> bool {
        // Fragments are whole elements and none encloses a body paragraph,
        // so any overlap starts inside the span.
        let first = self
            .fragments
            .partition_point(|range| range.start < span.start);
        self.fragments
            .get(first)
            .is_some_and(|range| range.start < span.end)
    }
}

/// Recognizing a namespace does not mean every attribute in it is mapped.
/// Only these session/paragraph identifiers have a defined removal policy;
/// unknown attributes on every container of a touched paragraph must survive
/// via the original bytes or make the structural edit fail.
fn plain_markup(element: &Element) -> Result<(), &'static str> {
    const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
    let is_identifier = |paragraph: bool, namespace: Option<&str>, local: &str| {
        (namespace == Some(W)
            && (matches!(local, "rsidDel" | "rsidR" | "rsidRPr")
                || (paragraph && matches!(local, "rsidP" | "rsidRDefault"))))
            || (paragraph && namespace == Some(W14) && matches!(local, "paraId" | "textId"))
    };
    let attributes_known = match &*element.local {
        "p" | "r" => element.attributes.iter().all(|a| {
            is_identifier(
                element.local.as_ref() == "p",
                a.namespace.as_deref(),
                &a.local,
            )
        }),
        "pPr" | "rPr" | "lastRenderedPageBreak" => element.attributes.is_empty(),
        "proofErr" => {
            element.attributes.len() == 1
                && element.attributes.iter().all(|a| {
                    a.namespace.as_deref() == Some(W)
                        && a.local.as_ref() == "type"
                        && matches!(
                            a.value.as_str(),
                            "spellStart" | "spellEnd" | "gramStart" | "gramEnd"
                        )
                })
        }
        // The existing text-slot validator checks these leaves, including
        // xml:space and their local namespace declarations.
        "t" | "tab" => return Ok(()),
        _ => return Err("unmapped structural paragraph content"),
    };
    if !attributes_known {
        return Err(if element.local.as_ref() == "p" {
            "paragraph has unknown attributes"
        } else {
            "structural paragraph markup has unknown attributes"
        });
    }
    let empty = matches!(element.local.as_ref(), "proofErr" | "lastRenderedPageBreak");
    for node in &element.children {
        match node {
            Node::Text(text) if text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) => {}
            Node::Element(child) if !empty && child.namespace.as_deref() == Some(W) => {
                plain_markup(child)?
            }
            _ => return Err("unmapped structural paragraph content"),
        }
    }
    Ok(())
}

fn block_source(block: &Block) -> Option<&SourceRef> {
    match block {
        Block::Paragraph(paragraph) => paragraph.source.as_deref(),
        Block::Table(table) => table.source.as_deref(),
        Block::Sdt(sdt) => sdt.source.as_deref(),
        Block::Opaque(opaque) => opaque.source.as_deref(),
    }
}

/// Byte range of an importer locator (`@start..end`).
fn locator(locator: &str) -> Option<(usize, usize)> {
    let (start, end) = locator.strip_prefix('@')?.split_once("..")?;
    Some((start.parse().ok()?, end.parse().ok()?))
}

fn default_format(paragraph: &Paragraph) -> bool {
    paragraph.style.is_none()
        && *paragraph.props == ParaProps::default()
        && paragraph.mark_props.is_empty()
        && paragraph.inlines().is_empty()
        && paragraph
            .runs()
            .iter()
            .all(|run| run.style.is_none() && run.props.is_empty())
}

/// Markup of a new plain paragraph, named as `names` says for content of
/// `w:body`.
pub(super) fn new_paragraph(
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
) -> Result<String, ExportError> {
    if !default_format(paragraph) {
        return Err(unsupported(
            Some(paragraph.id()),
            "new paragraphs require default paragraph and character properties",
        ));
    }
    let text = paragraph.plain_text(|_| String::new());
    if text.is_empty() {
        return Ok(format!("{}/>", names.start("p")));
    }
    let runs = bounded_text_xml(names, &text, limit)?;
    Ok(format!(
        "{}>{}>{runs}{}{}",
        names.start("p"),
        names.start("r"),
        names.end("r"),
        names.end("p")
    ))
}

#[cfg(test)]
mod tests {
    use super::locator;

    #[test]
    fn locators_parse_only_importer_ranges() {
        assert_eq!(locator("@12..40"), Some((12, 40)));
        for bad in ["12..40", "@12", "@a..40", "@12..", "@-1..4", "w14:paraId"] {
            assert_eq!(locator(bad), None, "{bad}");
        }
    }
}
