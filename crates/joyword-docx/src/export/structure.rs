//! Body paragraph structure in edited export (ECMA-376 Part 1 §17.2.2 body,
//! §17.3.1.22 p, §17.3.1.26 pPr, §17.3.2.28 rPr, §17.6.17 sectPr; Part 2 for
//! the unchanged parts).
//!
//! The engine keeps a split paragraph's identity and source on the head and
//! gives the tail a new identity without a source; a merge keeps the first
//! paragraph and ends the others. Identities, never text similarity, decide
//! which source element a paragraph belongs to, and surviving source blocks
//! must keep their order.
//!
//! Every paragraph that a split, merge, insertion or removal touches must be
//! either the W05 plain subset (default paragraph, mark and character
//! properties) or a fully mapped formatted paragraph: `pPr`/`rPr` children the
//! property parser accepts, no preserved fragment, no unexplained attribute or
//! child, and no prefix that would change meaning when those elements are
//! copied beside `w:body`. New paragraphs then reuse the source property
//! element bytes; they are not rebuilt from style names. Anything that cannot
//! be proved that way is refused. A new paragraph is written after the nearest
//! preceding surviving source block; a removed one loses exactly its element
//! range. All other bytes of the part stay as they were.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use joyword_model::{
    Block, Color, Document, FontRef, Indent, Justification, Measure, NodeId, NumberingRef,
    NumberingStore, ParaProps, Paragraph, RunProps, SourceRef, Spacing, StyleId, StyleKind,
    StyleSheet, Underline,
};

use super::{ExportError, Names, bounded_text_xml, text_slots, unsupported};
use crate::props::{self, Findings};
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
    /// Prefixes bound on ancestors of `w:body`'s children (the document and
    /// the body). Copied property elements must still resolve there.
    bindings: Vec<(Arc<str>, Arc<str>)>,
    /// Property bytes of paragraphs [`Self::plain`] has accepted.
    copies: RefCell<Vec<CopySet>>,
}

impl<'a> Source<'a> {
    pub fn new(
        part: &'a str,
        xml: &'a [u8],
        body: &'a Element,
        backing: &PackageBacking,
        original: &Document,
        scope: &[&Element],
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
            bindings: ancestor_bindings(scope),
            copies: RefCell::new(Vec::new()),
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
        let copy = self.copy_set(paragraph, element).map_err(error)?;
        self.copies.borrow_mut().push(copy);
        Ok(element)
    }

    pub(super) fn bytes(&self) -> &'a [u8] {
        self.xml
    }

    /// True when `plain` accepted this paragraph, so a structural edit may
    /// rebuild its runs from copied property bytes.
    pub(super) fn touched(&self, id: NodeId) -> bool {
        self.copies.borrow().iter().any(|set| set.id == id)
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

    /// Property bytes that may be copied onto a paragraph created or rebuilt
    /// from `paragraph`. Default formatting has nothing to copy.
    fn copy_set(&self, paragraph: &Paragraph, element: &Element) -> Result<CopySet, &'static str> {
        if !paragraph.inlines().is_empty() {
            return Err(
                "paragraph structure edits require default paragraph and character properties",
            );
        }
        if default_format(paragraph) {
            plain_markup(element)?;
            text_slots(element, self.xml)?;
            return Ok(CopySet {
                id: paragraph.id(),
                style: None,
                props: paragraph.props.clone(),
                mark: paragraph.mark_props.clone(),
                ppr: element.child(W, "pPr").map(|ppr| ppr.span.clone()),
                runs: Vec::new(),
                rewrite: !blocks_rewrite(element),
            });
        }
        self.formatted_set(paragraph, element)
    }

    /// Fully mapped `pPr`/`rPr` whose bytes can be copied unchanged.
    fn formatted_set(
        &self,
        paragraph: &Paragraph,
        element: &Element,
    ) -> Result<CopySet, &'static str> {
        if paragraph.props.preserved.is_some()
            || paragraph.mark_props.preserved.is_some()
            || paragraph
                .runs()
                .iter()
                .any(|run| run.props.preserved.is_some())
        {
            return Err("paragraph holds preserved markup");
        }
        container_attributes(element)?;
        text_slots(element, self.xml)?;
        let mut ppr = None;
        let mut runs = Vec::new();
        let mut rewrite = !blocks_rewrite(element);
        let mut cursor = 0usize;
        for node in &element.children {
            match node {
                Node::Text(text) if is_space(text) => {}
                Node::Element(child) if child.namespace.as_deref() == Some(W) => {
                    match &*child.local {
                        "pPr" => {
                            if ppr.is_some() {
                                return Err("unmapped structural paragraph content");
                            }
                            self.known_ppr(paragraph, child, &mut runs)?;
                            ppr = Some(child.span.clone());
                        }
                        "proofErr" => known_proof(child)?,
                        "r" => {
                            self.known_run(paragraph, child, &mut cursor, &mut runs, &mut rewrite)?
                        }
                        _ => return Err("unmapped structural paragraph content"),
                    }
                }
                _ => return Err("unmapped structural paragraph content"),
            }
        }
        if cursor != paragraph.runs().len() {
            return Err("source text tokens do not match imported runs");
        }
        if ppr.is_none()
            && (paragraph.style.is_some()
                || !paragraph.props.is_empty()
                || !paragraph.mark_props.is_empty())
        {
            return Err("mapped paragraph properties do not match the source model");
        }
        Ok(CopySet {
            id: paragraph.id(),
            style: paragraph.style.clone(),
            props: paragraph.props.clone(),
            mark: paragraph.mark_props.clone(),
            ppr,
            runs,
            rewrite,
        })
    }

    fn known_ppr(
        &self,
        paragraph: &Paragraph,
        ppr: &Element,
        runs: &mut Vec<RunSource>,
    ) -> Result<(), &'static str> {
        if !ppr.attributes.is_empty() {
            return Err("structural paragraph markup has unknown attributes");
        }
        if ppr.child(W, "sectPr").is_some() {
            return Err("paragraph ends a section");
        }
        if !portable(ppr, self.xml, &self.bindings) {
            return Err("property markup uses a prefix that cannot be copied safely");
        }
        let mut findings = Findings::default();
        let parsed = props::para(ppr, &mut findings);
        if !findings.complete || parsed.style != paragraph.style || parsed.props != *paragraph.props
        {
            return Err("mapped paragraph properties do not match the source model");
        }
        for node in &ppr.children {
            match node {
                Node::Text(text) if is_space(text) => {}
                Node::Element(child) if child.namespace.as_deref() == Some(W) => {
                    match &*child.local {
                        "rPr" => {
                            let (mark, mark_style) = self.known_rpr(child)?;
                            if mark_style.is_some() || mark != *paragraph.mark_props {
                                return Err(
                                    "mapped paragraph properties do not match the source model",
                                );
                            }
                            if !mark.is_empty() {
                                runs.push(RunSource {
                                    style: None,
                                    props: paragraph.mark_props.clone(),
                                    rpr: Some(child.span.clone()),
                                });
                            }
                        }
                        "sectPr" => return Err("paragraph ends a section"),
                        other if property_leaf(other).is_some() => known_leaf(child)?,
                        "numPr" => known_num_pr(child)?,
                        _ => return Err("unmapped structural paragraph content"),
                    }
                }
                _ => return Err("unmapped structural paragraph content"),
            }
        }
        if parsed.mark.is_none() && !paragraph.mark_props.is_empty() {
            return Err("mapped paragraph properties do not match the source model");
        }
        Ok(())
    }

    fn known_run(
        &self,
        paragraph: &Paragraph,
        run: &Element,
        cursor: &mut usize,
        runs: &mut Vec<RunSource>,
        rewrite: &mut bool,
    ) -> Result<(), &'static str> {
        container_attributes(run)?;
        let mut rpr = None;
        let mut count = 0usize;
        for node in &run.children {
            match node {
                Node::Text(text) if is_space(text) => {}
                Node::Element(child) if child.namespace.as_deref() == Some(W) => {
                    match &*child.local {
                        "rPr" => {
                            if rpr.is_some() {
                                return Err("unmapped structural paragraph content");
                            }
                            rpr = Some(child);
                        }
                        "lastRenderedPageBreak" => {
                            if !child.attributes.is_empty() || child.elements().next().is_some() {
                                return Err("structural paragraph markup has unknown attributes");
                            }
                            *rewrite = false;
                        }
                        "t" | "tab" => {
                            if !(child.is(W, "tab") || child.elements().next().is_none()) {
                                return Err("unsupported run content");
                            }
                            let text = if child.is(W, "tab") {
                                "\t"
                            } else {
                                // text_slots already accepted the leaf; only the
                                // length is needed here.
                                ""
                            };
                            if child.is(W, "tab") || !child.text().is_empty() {
                                count += if child.is(W, "tab") {
                                    text.chars().count()
                                } else {
                                    child.text().chars().count()
                                };
                            }
                        }
                        _ => return Err("unsupported run content"),
                    }
                }
                _ => return Err("unmapped structural paragraph content"),
            }
        }
        if count == 0 {
            // An empty run is not in the model. Rebuilding would drop it.
            *rewrite = false;
            return Ok(());
        }
        let (props, style, span) = match rpr {
            Some(rpr) => {
                if !portable(rpr, self.xml, &self.bindings) {
                    return Err("property markup uses a prefix that cannot be copied safely");
                }
                let (props, style) = self.known_rpr(rpr)?;
                (props, style, Some(rpr.span.clone()))
            }
            None => (RunProps::default(), None, None),
        };
        let mut stored = None;
        let mut seen = 0usize;
        while seen < count {
            let model = paragraph
                .runs()
                .get(*cursor)
                .ok_or("source text tokens do not match imported runs")?;
            if model.style != style || *model.props != props {
                return Err("mapped run properties do not match the source model");
            }
            if stored.is_none() {
                stored = Some(model.props.clone());
            }
            seen += model.len;
            *cursor += 1;
        }
        if seen != count {
            return Err("source text tokens do not match imported runs");
        }
        if !props.is_empty() || style.is_some() {
            runs.push(RunSource {
                style,
                props: stored.expect("run stored"),
                rpr: span,
            });
        }
        Ok(())
    }

    fn known_rpr(&self, rpr: &Element) -> Result<(RunProps, Option<StyleId>), &'static str> {
        if !rpr.attributes.is_empty() {
            return Err("structural paragraph markup has unknown attributes");
        }
        for node in &rpr.children {
            match node {
                Node::Text(text) if is_space(text) => {}
                Node::Element(child) if child.namespace.as_deref() == Some(W) => known_leaf(child)?,
                _ => return Err("unmapped structural paragraph content"),
            }
        }
        let mut findings = Findings::default();
        let (props, style) = props::run(rpr, &mut findings);
        if !findings.complete {
            return Err("unmapped structural paragraph content");
        }
        Ok((props, style))
    }

    fn template_for(&self, paragraph: &Paragraph) -> Result<CopySet, &'static str> {
        let copies = self.copies.borrow();
        let same = |set: &CopySet| {
            set.style == paragraph.style
                && *set.props == *paragraph.props
                && *set.mark == *paragraph.mark_props
        };
        if let Some(set) = copies.iter().find(|set| {
            same(set)
                && Arc::ptr_eq(&set.props, &paragraph.props)
                && Arc::ptr_eq(&set.mark, &paragraph.mark_props)
        }) {
            return Ok(set.clone());
        }
        copies
            .iter()
            .find(|set| same(set))
            .cloned()
            .ok_or("new paragraphs require default paragraph and character properties")
    }

    fn run_bytes(
        &self,
        style: &Option<StyleId>,
        props: &Arc<RunProps>,
    ) -> Result<Option<String>, ExportError> {
        if style.is_none() && props.is_empty() {
            return Ok(None);
        }
        let copies = self.copies.borrow();
        let found = copies.iter().find_map(|set| {
            set.runs
                .iter()
                .find(|run| run.style == *style && Arc::ptr_eq(&run.props, props))
        });
        let found = found.or_else(|| {
            copies.iter().find_map(|set| {
                set.runs
                    .iter()
                    .find(|run| run.style == *style && *run.props == **props)
            })
        });
        let range = found.and_then(|run| run.rpr.clone()).ok_or_else(|| {
            unsupported(
                None,
                "character formatting has no fully mapped source properties",
            )
        })?;
        let bytes = std::str::from_utf8(&self.xml[range]).map_err(|_| {
            unsupported(
                None,
                "property markup uses a prefix that cannot be copied safely",
            )
        })?;
        Ok(Some(bytes.to_owned()))
    }

    fn property_markup(
        &self,
        paragraph: &Paragraph,
        names: &Names,
        limit: u64,
    ) -> Result<String, ExportError> {
        let template = self
            .template_for(paragraph)
            .map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
        let mut body = String::new();
        if let Some(range) = &template.ppr {
            let bytes = std::str::from_utf8(&self.xml[range.clone()]).map_err(|_| {
                unsupported(
                    Some(paragraph.id()),
                    "property markup uses a prefix that cannot be copied safely",
                )
            })?;
            body.push_str(bytes);
        }
        body.push_str(&self.run_markup(paragraph, names, limit)?);
        if body.is_empty() {
            return Ok(format!("{}/>", names.start("p")));
        }
        Ok(format!("{}>{body}{}", names.start("p"), names.end("p")))
    }

    fn run_markup(
        &self,
        paragraph: &Paragraph,
        names: &Names,
        limit: u64,
    ) -> Result<String, ExportError> {
        let mut body = String::new();
        for (style, props, text) in run_groups(paragraph) {
            let rpr = match self.run_bytes(&style, &props) {
                Ok(markup) => markup,
                Err(error) if style.is_none() => match synthesize_rpr(names, &props) {
                    Ok(markup) => markup,
                    Err(_) => return Err(error),
                },
                Err(error) => return Err(error),
            };
            let text = bounded_text_xml(names, &text, limit)?;
            if rpr.is_none() && text.is_empty() {
                continue;
            }
            body.push_str(&names.start("r"));
            body.push('>');
            if let Some(rpr) = rpr {
                body.push_str(&rpr);
            }
            body.push_str(&text);
            body.push_str(&names.end("r"));
        }
        Ok(body)
    }

    /// Replace one surviving paragraph's element when a merge (or another
    /// structural edit) adds run-property boundaries the text-leaf patch
    /// cannot express. Paragraph properties stay the source element's bytes.
    pub(super) fn rebuild(
        &self,
        before: &Paragraph,
        after: &Paragraph,
        element: &Element,
        names: &Names,
        limit: u64,
    ) -> Result<Vec<u8>, ExportError> {
        let template = {
            let copies = self.copies.borrow();
            copies
                .iter()
                .find(|set| set.id == before.id())
                .cloned()
                .ok_or_else(|| {
                    unsupported(Some(before.id()), "character formatting boundaries changed")
                })?
        };
        if !template.rewrite {
            return Err(unsupported(
                Some(before.id()),
                "structural run rebuild would drop proofing or cached break markup",
            ));
        }
        let mut inner = String::new();
        if let Some(range) = template.ppr {
            let bytes = std::str::from_utf8(&self.xml[range]).map_err(|_| {
                unsupported(
                    Some(before.id()),
                    "property markup uses a prefix that cannot be copied safely",
                )
            })?;
            inner.push_str(bytes);
        }
        inner.push_str(&self.run_markup(after, names, limit)?);
        let open = opened_start(self.xml, element)
            .map_err(|reason| unsupported(Some(before.id()), reason))?;
        let end = format!("</{}>", element.qname);
        let mut out = open;
        out.extend_from_slice(inner.as_bytes());
        out.extend_from_slice(end.as_bytes());
        if out.len() as u64 > limit {
            return Err(ExportError::Budget {
                part: "paragraph".into(),
                limit,
            });
        }
        Ok(out)
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

/// A paragraph whose direct formatting is the M1 subset: justification and
/// character toggles, size, color, named font slots and underline. No style,
/// numbering, spacing, indent, preserved markup or theme font.
pub(super) fn direct_m1(paragraph: &Paragraph) -> bool {
    let props = &paragraph.props;
    paragraph.style.is_none()
        && props.preserved.is_none()
        && props.numbering.is_none()
        && props.tabs.is_none()
        && props.indent == Indent::default()
        && props.spacing == Spacing::default()
        && props.keep_next.is_none()
        && props.keep_lines.is_none()
        && props.widow_control.is_none()
        && props.page_break_before.is_none()
        && props.snap_to_grid.is_none()
        && props.outline_level.is_none()
        && paragraph.mark_props.is_empty()
        && paragraph.inlines().is_empty()
        && paragraph
            .runs()
            .iter()
            .all(|run| run.style.is_none() && run_supported(&run.props).is_ok())
}

/// Paragraph and character style ids that can be written back as `w:pStyle`
/// and `w:rStyle` without rewriting the styles part. Table and numbering
/// styles, unknown ids, broken chains and direct formatting outside the M1
/// subset are refused.
pub(super) fn style_assignment_ok(
    sheet: &StyleSheet,
    paragraph: &Paragraph,
) -> Result<(), &'static str> {
    let props = &paragraph.props;
    if props.preserved.is_some()
        || props.tabs.is_some()
        || props.indent != Indent::default()
        || props.spacing != Spacing::default()
        || props.keep_next.is_some()
        || props.keep_lines.is_some()
        || props.widow_control.is_some()
        || props.page_break_before.is_some()
        || props.snap_to_grid.is_some()
        || props.outline_level.is_some()
    {
        return Err("paragraph formatting is outside the style assignment subset");
    }
    if let Some(id) = &paragraph.style {
        require_style(sheet, id, StyleKind::Paragraph)?;
    }
    if !paragraph.mark_props.is_empty() || !paragraph.inlines().is_empty() {
        return Err("paragraph mark formatting is outside the style assignment subset");
    }
    for run in paragraph.runs() {
        run_supported(&run.props)?;
        if let Some(id) = &run.style {
            require_style(sheet, id, StyleKind::Character)?;
        }
    }
    Ok(())
}

/// `w:numPr` may be patched only when the instance exists (or `numId` is 0)
/// and the level is one ECMA-376 defines. The numbering part is not rewritten.
pub(super) fn numbering_assignment_ok(
    store: &NumberingStore,
    paragraph: &Paragraph,
) -> Result<(), &'static str> {
    let Some(numbering) = paragraph.props.numbering else {
        return Ok(());
    };
    if numbering.level > joyword_model::MAX_LEVEL {
        return Err("numbering level is outside 0..=8");
    }
    if numbering.num_id != 0 && store.instance(numbering.num_id).is_none() {
        return Err("numbering instance is missing");
    }
    Ok(())
}

fn require_style(sheet: &StyleSheet, id: &StyleId, kind: StyleKind) -> Result<(), &'static str> {
    if !writable_style_id(id.as_str()) {
        return Err("style id cannot be written");
    }
    let Some(style) = sheet.get(id) else {
        return Err("unknown style");
    };
    if style.kind != kind {
        return Err("style kind does not match the assignment");
    }
    if sheet.chain(id).problem.is_some() {
        return Err("style chain cannot be exported");
    }
    Ok(())
}

fn writable_style_id(id: &str) -> bool {
    !id.is_empty() && !id.chars().any(char::is_control)
}

fn xml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

pub(super) fn rewrite_styled_paragraph(
    xml: &[u8],
    element: &Element,
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
) -> Result<Vec<u8>, ExportError> {
    let open =
        opened_start(xml, element).map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
    let mut inner = String::new();
    if paragraph.style.is_some()
        || paragraph.props.justification.is_some()
        || paragraph.props.numbering.is_some()
    {
        inner.push_str(&names.start("pPr"));
        inner.push('>');
        if let Some(style) = &paragraph.style {
            inner.push_str(&names.start("pStyle"));
            inner.push_str(&qattr(names, "val", &xml_escape(style.as_str())));
            inner.push_str("/>");
        }
        if let Some(justification) = paragraph.props.justification {
            inner.push_str(&names.start("jc"));
            inner.push_str(&qattr(names, "val", justification_value(justification)));
            inner.push_str("/>");
        }
        if let Some(numbering) = paragraph.props.numbering {
            inner.push_str(&num_pr(names, numbering));
        }
        inner.push_str(&names.end("pPr"));
    }
    for (style, props, text) in run_groups(paragraph) {
        let rpr = styled_rpr(names, style.as_ref(), &props)
            .map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
        let text = bounded_text_xml(names, &text, limit)?;
        if rpr.is_none() && text.is_empty() {
            continue;
        }
        inner.push_str(&names.start("r"));
        inner.push('>');
        if let Some(rpr) = rpr {
            inner.push_str(&rpr);
        }
        inner.push_str(&text);
        inner.push_str(&names.end("r"));
    }
    let mut out = open;
    out.extend_from_slice(inner.as_bytes());
    out.extend_from_slice(names.end("p").as_bytes());
    if out.len() as u64 > limit {
        return Err(ExportError::Budget {
            part: "paragraph".into(),
            limit,
        });
    }
    Ok(out)
}

fn num_pr(names: &Names, numbering: NumberingRef) -> String {
    format!(
        "{}>{}{}/>{}{}/>{}",
        names.start("numPr"),
        names.start("ilvl"),
        qattr(names, "val", &numbering.level.to_string()),
        names.start("numId"),
        qattr(names, "val", &numbering.num_id.to_string()),
        names.end("numPr")
    )
}

fn styled_rpr(
    names: &Names,
    style: Option<&StyleId>,
    props: &RunProps,
) -> Result<Option<String>, &'static str> {
    let rpr = synthesize_rpr(names, props)?;
    let Some(style) = style else {
        return Ok(rpr);
    };
    let tag = format!(
        "{}{}/>",
        names.start("rStyle"),
        qattr(names, "val", &xml_escape(style.as_str()))
    );
    match rpr {
        None => Ok(Some(format!(
            "{}>{tag}{}",
            names.start("rPr"),
            names.end("rPr")
        ))),
        Some(existing) => {
            let open = format!("{}>", names.start("rPr"));
            let Some(rest) = existing.strip_prefix(&open) else {
                return Err("character style is outside the direct formatting subset");
            };
            Ok(Some(format!("{open}{tag}{rest}")))
        }
    }
}

pub(super) fn rewrite_direct_paragraph(
    xml: &[u8],
    element: &Element,
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
) -> Result<Vec<u8>, ExportError> {
    let open =
        opened_start(xml, element).map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
    let mut inner = String::new();
    if let Some(justification) = paragraph.props.justification {
        inner.push_str(&names.start("pPr"));
        inner.push('>');
        inner.push_str(&names.start("jc"));
        inner.push_str(&qattr(names, "val", justification_value(justification)));
        inner.push_str("/>");
        inner.push_str(&names.end("pPr"));
    }
    for (style, props, text) in run_groups(paragraph) {
        if style.is_some() {
            return Err(unsupported(
                Some(paragraph.id()),
                "character style is outside the direct formatting subset",
            ));
        }
        let rpr = synthesize_rpr(names, &props)
            .map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
        let text = bounded_text_xml(names, &text, limit)?;
        if rpr.is_none() && text.is_empty() {
            continue;
        }
        inner.push_str(&names.start("r"));
        inner.push('>');
        if let Some(rpr) = rpr {
            inner.push_str(&rpr);
        }
        inner.push_str(&text);
        inner.push_str(&names.end("r"));
    }
    let mut out = open;
    out.extend_from_slice(inner.as_bytes());
    out.extend_from_slice(names.end("p").as_bytes());
    if out.len() as u64 > limit {
        return Err(ExportError::Budget {
            part: "paragraph".into(),
            limit,
        });
    }
    Ok(out)
}

fn run_supported(props: &RunProps) -> Result<(), &'static str> {
    if props.hidden.is_some()
        || props.preserved.is_some()
        || props.languages != joyword_model::Languages::default()
    {
        return Err("character formatting is outside the direct subset");
    }
    for font in [
        &props.fonts.ascii,
        &props.fonts.h_ansi,
        &props.fonts.east_asia,
        &props.fonts.complex_script,
    ] {
        if matches!(font, Some(FontRef::Theme(_))) {
            return Err("theme fonts are outside the direct subset");
        }
    }
    if props
        .size
        .is_some_and(|size| !matches!(size, Measure::HalfPoints(value) if value >= 0))
    {
        return Err("only half-point font sizes can be written directly");
    }
    Ok(())
}

fn synthesize_rpr(names: &Names, props: &RunProps) -> Result<Option<String>, &'static str> {
    run_supported(props)?;
    if props.is_empty() && props.fonts.hint.is_none() {
        return Ok(None);
    }
    let mut body = String::new();
    body.push_str(&toggle(names, "b", props.bold));
    body.push_str(&toggle(names, "i", props.italic));
    body.push_str(&toggle(names, "strike", props.strike));
    if let Some(underline) = props.underline {
        body.push_str(&names.start("u"));
        body.push_str(&qattr(names, "val", underline_value(underline)));
        body.push_str("/>");
    }
    if let Some(Measure::HalfPoints(size)) = props.size {
        body.push_str(&names.start("sz"));
        body.push_str(&qattr(names, "val", &size.to_string()));
        body.push_str("/>");
    }
    if let Some(color) = props.color {
        let value = match color {
            Color::Auto => "auto".to_owned(),
            Color::Rgb(red, green, blue) => format!("{red:02X}{green:02X}{blue:02X}"),
            Color::Theme(_) => {
                return Err("theme colours are outside the direct subset");
            }
        };
        body.push_str(&names.start("color"));
        body.push_str(&qattr(names, "val", &value));
        body.push_str("/>");
    }
    let fonts = font_attributes(names, &props.fonts);
    if !fonts.is_empty() || props.fonts.hint.is_some() {
        body.push_str(&names.start("rFonts"));
        body.push_str(&fonts);
        if let Some(hint) = props.fonts.hint {
            let value = match hint {
                joyword_model::FontHint::Default => "default",
                joyword_model::FontHint::EastAsia => "eastAsia",
                joyword_model::FontHint::ComplexScript => "cs",
            };
            body.push_str(&qattr(names, "hint", value));
        }
        body.push_str("/>");
    }
    if body.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "{}>{body}{}",
        names.start("rPr"),
        names.end("rPr")
    )))
}

fn toggle(names: &Names, local: &str, value: Option<bool>) -> String {
    match value {
        None => String::new(),
        Some(true) => format!("{}/>", names.start(local)),
        Some(false) => format!("{}{}/>", names.start(local), qattr(names, "val", "0")),
    }
}

fn font_attributes(names: &Names, fonts: &joyword_model::FontSlots) -> String {
    let mut out = String::new();
    for (local, font) in [
        ("ascii", &fonts.ascii),
        ("hAnsi", &fonts.h_ansi),
        ("eastAsia", &fonts.east_asia),
        ("cs", &fonts.complex_script),
    ] {
        if let Some(FontRef::Named(name)) = font {
            out.push_str(&qattr(names, local, name));
        }
    }
    out
}

fn qattr(names: &Names, local: &str, value: &str) -> String {
    let prefix = match names {
        Names::Bound(prefix) => format!("{prefix}:"),
        Names::Default => String::new(),
        Names::Declared => "jwe:".to_owned(),
    };
    format!(r#" {prefix}{local}="{value}""#)
}

fn justification_value(value: Justification) -> &'static str {
    match value {
        Justification::Start => "start",
        Justification::Center => "center",
        Justification::End => "end",
        Justification::Both => "both",
        Justification::Distribute => "distribute",
    }
}

fn underline_value(value: Underline) -> &'static str {
    match value {
        Underline::None => "none",
        Underline::Single => "single",
        Underline::Double => "double",
        Underline::Thick => "thick",
        Underline::Dotted => "dotted",
        Underline::Dash => "dash",
        Underline::Wave => "wave",
        Underline::Words => "words",
    }
}

/// Markup of a new paragraph, named as `names` says for content of `w:body`.
/// Default formatting keeps the W05 spelling. Any other formatting is the
/// source `pPr`/`rPr` bytes recorded for a paragraph [`Source::plain`]
/// accepted; a paragraph whose formatting has no such source is refused.
pub(super) fn new_paragraph(
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
    source: &Source<'_>,
) -> Result<String, ExportError> {
    if !default_format(paragraph) {
        if paragraph.inlines().is_empty()
            && paragraph.props.preserved.is_none()
            && paragraph.mark_props.preserved.is_none()
            && paragraph
                .runs()
                .iter()
                .all(|run| run.props.preserved.is_none())
        {
            return source.property_markup(paragraph, names, limit);
        }
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

/// Source property element copied onto a new or rebuilt paragraph.
#[derive(Clone)]
struct CopySet {
    id: NodeId,
    style: Option<StyleId>,
    props: Arc<ParaProps>,
    mark: Arc<RunProps>,
    ppr: Option<Range<usize>>,
    runs: Vec<RunSource>,
    /// False when rebuilding the element would drop markup the model does
    /// not represent (`w:proofErr`, `w:lastRenderedPageBreak`, an empty run).
    rewrite: bool,
}

#[derive(Clone)]
struct RunSource {
    style: Option<StyleId>,
    props: Arc<RunProps>,
    rpr: Option<Range<usize>>,
}

fn ancestor_bindings(scope: &[&Element]) -> Vec<(Arc<str>, Arc<str>)> {
    let mut bound = Vec::new();
    for element in scope {
        for (prefix, uri) in &element.declarations {
            if let Some(existing) = bound.iter_mut().find(|(name, _)| name == prefix) {
                existing.1 = uri.clone();
            } else {
                bound.push((prefix.clone(), uri.clone()));
            }
        }
    }
    bound
}

fn is_space(text: &str) -> bool {
    text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

/// Proofing marks, cached page breaks and empty runs have no model field.
/// Leaving them in the original element is safe; rebuilding the element is not.
fn blocks_rewrite(element: &Element) -> bool {
    element.elements().any(|child| {
        child.is(W, "proofErr")
            || (child.is(W, "r")
                && (child.child(W, "lastRenderedPageBreak").is_some() || !run_has_text(child)))
    })
}

fn run_has_text(run: &Element) -> bool {
    run.elements().any(|leaf| {
        (leaf.is(W, "tab") && leaf.elements().next().is_none())
            || (leaf.is(W, "t") && leaf.elements().next().is_none() && !leaf.text().is_empty())
    })
}

fn container_attributes(element: &Element) -> Result<(), &'static str> {
    const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
    let paragraph = element.local.as_ref() == "p";
    let known = element.attributes.iter().all(|attribute| {
        (attribute.namespace.as_deref() == Some(W)
            && (matches!(attribute.local.as_ref(), "rsidDel" | "rsidR" | "rsidRPr")
                || (paragraph && matches!(attribute.local.as_ref(), "rsidP" | "rsidRDefault"))))
            || (paragraph
                && attribute.namespace.as_deref() == Some(W14)
                && matches!(attribute.local.as_ref(), "paraId" | "textId"))
    });
    if known {
        Ok(())
    } else if paragraph {
        Err("paragraph has unknown attributes")
    } else {
        Err("structural paragraph markup has unknown attributes")
    }
}

fn known_proof(element: &Element) -> Result<(), &'static str> {
    if element.attributes.len() == 1
        && element.attributes.iter().all(|attribute| {
            attribute.namespace.as_deref() == Some(W)
                && attribute.local.as_ref() == "type"
                && matches!(
                    attribute.value.as_str(),
                    "spellStart" | "spellEnd" | "gramStart" | "gramEnd"
                )
        })
        && element
            .children
            .iter()
            .all(|node| matches!(node, Node::Text(text) if is_space(text)))
    {
        Ok(())
    } else if element
        .elements()
        .any(|child| child.namespace.as_deref() != Some(W) || !child.local.is_empty())
        && element.elements().next().is_some()
    {
        Err("unmapped structural paragraph content")
    } else {
        Err("structural paragraph markup has unknown attributes")
    }
}

fn known_num_pr(element: &Element) -> Result<(), &'static str> {
    if !element.attributes.is_empty() {
        return Err("structural paragraph markup has unknown attributes");
    }
    for node in &element.children {
        match node {
            Node::Text(text) if is_space(text) => {}
            Node::Element(child)
                if child.namespace.as_deref() == Some(W)
                    && matches!(child.local.as_ref(), "numId" | "ilvl") =>
            {
                known_leaf(child)?;
            }
            _ => return Err("unmapped structural paragraph content"),
        }
    }
    Ok(())
}

fn known_leaf(element: &Element) -> Result<(), &'static str> {
    let Some(allowed) = property_leaf(&element.local) else {
        return Err("unmapped structural paragraph content");
    };
    if !element.attributes.iter().all(|attribute| {
        attribute.namespace.as_deref() == Some(W) && allowed.contains(&attribute.local.as_ref())
    }) {
        return Err("structural paragraph markup has unknown attributes");
    }
    if element.children.iter().all(|node| match node {
        Node::Text(text) => is_space(text),
        Node::Element(_) => false,
    }) {
        Ok(())
    } else {
        Err("unmapped structural paragraph content")
    }
}

/// Attributes the property parser reads. `None` means the element is not a
/// mapped leaf. A present attribute outside the list is unexplained even
/// when the parser still fills the model field.
fn property_leaf(local: &str) -> Option<&'static [&'static str]> {
    Some(match local {
        "pStyle" | "outlineLvl" | "rStyle" | "sz" | "jc" | "u" | "color" | "numId" | "ilvl" => {
            &["val"]
        }
        "keepNext" | "keepLines" | "widowControl" | "pageBreakBefore" | "snapToGrid" | "b"
        | "i" | "strike" | "vanish" => &["val"],
        "ind" => &[
            "startChars",
            "leftChars",
            "start",
            "left",
            "endChars",
            "rightChars",
            "end",
            "right",
            "firstLineChars",
            "firstLine",
            "hangingChars",
            "hanging",
        ],
        "spacing" => &[
            "beforeLines",
            "before",
            "afterLines",
            "after",
            "beforeAutospacing",
            "afterAutospacing",
            "line",
            "lineRule",
        ],
        "rFonts" => &[
            "ascii",
            "asciiTheme",
            "hAnsi",
            "hAnsiTheme",
            "eastAsia",
            "eastAsiaTheme",
            "cs",
            "cstheme",
            "hint",
        ],
        "lang" => &["val", "eastAsia", "bidi"],
        _ => return None,
    })
}

fn run_groups(paragraph: &Paragraph) -> Vec<(Option<StyleId>, Arc<RunProps>, String)> {
    let text = paragraph.plain_text(|_| "\u{FFFC}".into());
    let mut chars = text.chars();
    let mut groups: Vec<(Option<StyleId>, Arc<RunProps>, String)> = Vec::new();
    for run in paragraph.runs() {
        let text: String = chars.by_ref().take(run.len).collect();
        if let Some((style, props, existing)) = groups.last_mut()
            && *style == run.style
            && *props == run.props
        {
            existing.push_str(&text);
        } else {
            groups.push((run.style.clone(), run.props.clone(), text));
        }
    }
    groups
}

/// `w:p` start tag, opened so inner markup can be appended. A self-closing
/// tag loses only its slash.
fn opened_start(xml: &[u8], element: &Element) -> Result<Vec<u8>, &'static str> {
    let tag = start_tag(xml, element.span.start).ok_or("missing paragraph start tag")?;
    if let Some(empty) = tag.strip_suffix("/>") {
        let mut open = empty.trim_end().as_bytes().to_vec();
        open.push(b'>');
        Ok(open)
    } else {
        Ok(tag.as_bytes().to_vec())
    }
}

fn start_tag(xml: &[u8], start: usize) -> Option<&str> {
    let bytes = xml.get(start..)?;
    let mut quote = 0u8;
    for (index, &byte) in bytes.iter().enumerate() {
        if quote != 0 {
            if byte == quote {
                quote = 0;
            }
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            quote = byte;
            continue;
        }
        if byte == b'>' {
            return std::str::from_utf8(&bytes[..=index]).ok();
        }
    }
    None
}

fn attribute_keys(tag: &str) -> Vec<&str> {
    let bytes = tag.as_bytes();
    let mut index = usize::from(bytes.first() == Some(&b'<'));
    while index < bytes.len()
        && !bytes[index].is_ascii_whitespace()
        && bytes[index] != b'>'
        && bytes[index] != b'/'
    {
        index += 1;
    }
    let mut keys = Vec::new();
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] == b'>' || bytes[index] == b'/' {
            break;
        }
        let key_at = index;
        while index < bytes.len() && bytes[index] != b'=' && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let key = &tag[key_at..index];
        while index < bytes.len() && bytes[index] != b'=' {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'=' {
            index += 1;
        }
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < bytes.len() && (bytes[index] == b'"' || bytes[index] == b'\'') {
            let quote = bytes[index];
            index += 1;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
            if index < bytes.len() {
                index += 1;
            }
        }
        if !key.is_empty() {
            keys.push(key);
        }
    }
    keys
}

/// Copied property bytes keep the meaning they had under `w:body`: every
/// prefix resolves from declarations on the document, the body, or inside
/// the copied element itself. A prefix declared only on `w:p` or `w:r`
/// would not be in scope on a new sibling.
fn portable(element: &Element, xml: &[u8], ancestors: &[(Arc<str>, Arc<str>)]) -> bool {
    let mut layers = vec![ancestors.to_vec()];
    portable_element(element, xml, &mut layers)
}

fn portable_element(
    element: &Element,
    xml: &[u8],
    layers: &mut Vec<Vec<(Arc<str>, Arc<str>)>>,
) -> bool {
    layers.push(element.declarations.clone());
    let names_ok = prefix_matches(&element.qname, element.namespace.as_deref(), layers)
        && attributes_portable(element, xml, layers);
    let children_ok = element
        .elements()
        .all(|child| portable_element(child, xml, layers));
    layers.pop();
    names_ok && children_ok
}

fn lookup<'a>(layers: &'a [Vec<(Arc<str>, Arc<str>)>], prefix: &str) -> Option<&'a str> {
    layers.iter().rev().find_map(|layer| {
        layer
            .iter()
            .rev()
            .find(|(name, _)| &**name == prefix)
            .map(|(_, uri)| &**uri)
    })
}

fn prefix_matches(
    qname: &str,
    namespace: Option<&str>,
    layers: &[Vec<(Arc<str>, Arc<str>)>],
) -> bool {
    let prefix = qname
        .split_once(':')
        .map(|(prefix, _)| prefix)
        .unwrap_or("");
    lookup(layers, prefix) == namespace
}

fn attributes_portable(
    element: &Element,
    xml: &[u8],
    layers: &[Vec<(Arc<str>, Arc<str>)>],
) -> bool {
    let Some(tag) = start_tag(xml, element.span.start) else {
        return false;
    };
    attribute_keys(tag).into_iter().all(|key| {
        if key == "xmlns" || key.starts_with("xmlns:") {
            return true;
        }
        let (prefix, local) = key
            .split_once(':')
            .map_or((None, key), |(prefix, local)| (Some(prefix), local));
        let expected = match prefix {
            Some(prefix) => lookup(layers, prefix).map(str::to_owned),
            None => None,
        };
        element.attributes.iter().any(|attribute| {
            attribute.local.as_ref() == local
                && attribute.namespace.as_deref() == expected.as_deref()
        })
    })
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
