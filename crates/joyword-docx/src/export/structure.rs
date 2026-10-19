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
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use joyword_model::{
    Block, BorderEdge, BorderKind, CellBorders, CellMargins, Color, Document, FontRef, FontSlots,
    Hyperlink, Indent, InlineKind, Justification, Languages, LineSpacing, Measure, MoveMarker,
    MoveSide, NodeId, NumberingRef, NumberingStore, Orientation, PageSetup, ParaProps, Paragraph,
    RevisionKind, RowHeightRule, RunProps, Section, Shading, SourceRef, Spacing, StyleId,
    StyleKind, StyleSheet, TabAlign, TabLeader, TabStop, Table, TableAlign, TableLayoutKind,
    TableLook, TableWidth, Underline, VerticalAlign, VerticalMerge,
};

use super::{ExportError, Names, bounded_text_leaf, bounded_text_xml, text_slots, unsupported};
use crate::ns::{R, REL_HYPERLINK};
use crate::props::{self, Findings};
use crate::xml::{Element, Node};
use crate::{PackageBacking, W};
use joyoffice_opc::{PartName, Target};

/// Where a group of new paragraphs is written.
#[derive(Clone, Copy)]
pub(super) enum Anchor<'a> {
    /// After this surviving source block.
    After(&'a Block),
    /// Before this block, the first of the source body; no surviving source
    /// block precedes the group.
    Before(&'a Block),
}

/// A body block created in this session, with no original element.
pub(super) enum NewBlock<'a> {
    Paragraph(&'a Paragraph),
    Table(&'a Table),
}

/// Consecutive new blocks and where they go.
pub(super) struct Insertion<'a> {
    pub anchor: Anchor<'a>,
    pub blocks: Vec<NewBlock<'a>>,
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
    /// Imported tables whose model changed. The whole `w:tbl` is rewritten.
    pub edited_tables: Vec<(&'a Table, &'a Table)>,
    /// Imported tables that were removed. Their element range is deleted.
    pub removed_tables: Vec<&'a Table>,
    pub inserted: Vec<Insertion<'a>>,
}

impl Alignment<'_> {
    pub fn is_unchanged(&self) -> bool {
        self.edited.is_empty()
            && self.removed.is_empty()
            && self.inserted.is_empty()
            && self.edited_tables.is_empty()
            && self.removed_tables.is_empty()
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
            let new_block = match after {
                Block::Paragraph(paragraph) => {
                    if paragraph.source.is_some() {
                        return Err(unsupported(
                            Some(after.id()),
                            "new paragraph claims an original source",
                        ));
                    }
                    NewBlock::Paragraph(paragraph)
                }
                Block::Table(table) => {
                    if table.source.is_some() {
                        return Err(unsupported(
                            Some(after.id()),
                            "new table claims an original source",
                        ));
                    }
                    NewBlock::Table(table)
                }
                _ => return Err(unsupported(Some(after.id()), "new non-paragraph block")),
            };
            if previous_new {
                let group = alignment.inserted.last_mut().expect("open group");
                group.blocks.push(new_block);
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
                    blocks: vec![new_block],
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
            (Block::Table(b), Block::Table(a)) => {
                if !b.same_content(a) {
                    alignment.edited_tables.push((b.as_ref(), a.as_ref()));
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
        match block {
            Block::Paragraph(paragraph) => {
                alignment.removed.push(paragraph);
                if let Some(s) = survivor {
                    involved[s] = true;
                }
            }
            Block::Table(table) if table.source.is_some() => {
                alignment.removed_tables.push(table);
            }
            _ => {
                return Err(unsupported(Some(block.id()), "non-paragraph block removed"));
            }
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
    /// Relationship ids for hyperlink targets written by this plan.
    links: RefCell<LinkAssign>,
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
            links: RefCell::new(LinkAssign::from_package(backing, part)),
        }
    }

    /// `r:id` for an external hyperlink, allocating one when the package does
    /// not already name this target. Anchor-only links return `None`.
    pub(super) fn hyperlink_id(&self, link: &Hyperlink) -> Result<Option<String>, ExportError> {
        self.links
            .borrow_mut()
            .id_for(link)
            .map_err(|reason| unsupported(None, reason))
    }

    /// Relationships this plan added. Empty when every external target was
    /// already in the package.
    pub(super) fn fresh_hyperlinks(&self) -> Vec<(String, String)> {
        self.links.borrow().fresh.clone()
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

    /// The `w:tbl` an imported table was read from.
    pub fn table(&self, table: &Table) -> Result<&'a Element, ExportError> {
        let source = table
            .source
            .as_deref()
            .ok_or_else(|| unsupported(Some(table.id), "table has no original XML"))?;
        let element = (source.part.as_ref() == self.part)
            .then(|| locator(&source.locator))
            .flatten()
            .and_then(|span| self.elements.get(&span).copied())
            .ok_or_else(|| unsupported(Some(table.id), "table source does not match backing"))?;
        if !element.is(W, "tbl") {
            return Err(unsupported(
                Some(table.id),
                "table source does not match backing",
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
        if paragraph
            .inlines()
            .iter()
            .any(|inline| matches!(inline.kind, InlineKind::Image { .. }))
        {
            return Err("inline image paragraphs are not rewritten");
        }
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
                        "r" => self.known_run(
                            paragraph,
                            child,
                            None,
                            &mut cursor,
                            &mut runs,
                            &mut rewrite,
                        )?,
                        "hyperlink" => self.known_hyperlink(
                            paragraph,
                            child,
                            &mut cursor,
                            &mut runs,
                            &mut rewrite,
                        )?,
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
                        "tabs" => known_tabs(child)?,
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

    fn known_hyperlink(
        &self,
        paragraph: &Paragraph,
        element: &Element,
        cursor: &mut usize,
        runs: &mut Vec<RunSource>,
        rewrite: &mut bool,
    ) -> Result<(), &'static str> {
        let expected = self
            .element_link(element)?
            .ok_or("unmapped structural paragraph content")?;
        let mut saw = false;
        for node in &element.children {
            match node {
                Node::Text(text) if is_space(text) => {}
                Node::Element(child) if child.is(W, "r") => {
                    saw = true;
                    self.known_run(paragraph, child, Some(&expected), cursor, runs, rewrite)?;
                }
                _ => return Err("unmapped structural paragraph content"),
            }
        }
        if saw {
            Ok(())
        } else {
            Err("unmapped structural paragraph content")
        }
    }

    /// The link `w:hyperlink` names, rebuilt from its attributes and the
    /// package relationship. `None` is not a mapped link.
    fn element_link(&self, element: &Element) -> Result<Option<Hyperlink>, &'static str> {
        let (relationship, anchor) = hyperlink_attrs(element)?;
        let external = if let Some(id) = relationship {
            let url = self
                .links
                .borrow()
                .url(id)
                .ok_or("hyperlink relationship is missing")?;
            Some(Arc::<str>::from(url))
        } else {
            None
        };
        let anchor = match anchor {
            Some(value) if anchor_token_ok(value) => Some(Arc::<str>::from(value)),
            Some(_) => return Err("structural paragraph markup has unknown attributes"),
            None => None,
        };
        Ok(Hyperlink::imported(
            relationship.map(Arc::from),
            external,
            anchor,
        ))
    }

    fn known_run(
        &self,
        paragraph: &Paragraph,
        run: &Element,
        expected: Option<&Hyperlink>,
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
            if model.link.as_deref() != expected {
                return Err("mapped run hyperlink does not match the source model");
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
        if paragraph.runs().iter().any(|run| run.revision.is_some()) {
            if paragraph.runs().iter().any(|run| run.link.is_some()) {
                return Err(unsupported(
                    Some(paragraph.id()),
                    "a revised run also has a hyperlink",
                ));
            }
            return revision_body(paragraph, &[], names, limit);
        }
        append_linked_runs(
            self,
            paragraph.id(),
            run_groups(paragraph),
            names,
            limit,
            |style, props| match self.run_bytes(style, props) {
                Ok(markup) => Ok(markup),
                Err(error) if style.is_none() => match synthesize_rpr(names, props) {
                    Ok(markup) => Ok(markup),
                    Err(_) => Err(error),
                },
                Err(error) => Err(error),
            },
        )
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
    if element.is(W, "hyperlink") {
        return plain_hyperlink(element);
    }
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

/// A paragraph whose direct formatting is the M1 subset: justification,
/// writable twip indent, spacing and tabs, plus character toggles, size,
/// color, named font slots and underline. No style, numbering, preserved
/// markup or theme font.
pub(super) fn direct_m1(paragraph: &Paragraph) -> bool {
    let props = &paragraph.props;
    paragraph.style.is_none()
        && props.preserved.is_none()
        && props.numbering.is_none()
        && geometry_supported(props).is_ok()
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
        || geometry_supported(props).is_err()
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

/// Twenty-two inches. Direct indent, spacing and tabs outside this stay in
/// the source bytes; they are not rewritten as a shorter number.
const MAX_GEOMETRY_TWIPS: i64 = 22 * 1_440;
const MAX_EXPORT_TABS: usize = 48;

/// Indent, spacing and tabs the paragraph rewriter can emit without dropping
/// a unit the importer would not read back. Character units, both first-line
/// and hanging, bar and clear stops, and heavy leaders are refused.
pub(super) fn geometry_supported(props: &ParaProps) -> Result<(), &'static str> {
    let indent = &props.indent;
    if !twip_slot(indent.start, true)
        || !twip_slot(indent.end, true)
        || !twip_slot(indent.first_line, false)
        || !twip_slot(indent.hanging, false)
        || (indent.first_line.is_some() && indent.hanging.is_some())
    {
        return Err("paragraph indent is outside the direct subset");
    }
    let spacing = &props.spacing;
    if !twip_slot(spacing.before, false)
        || !twip_slot(spacing.after, false)
        || spacing.before_auto.is_some()
        || spacing.after_auto.is_some()
    {
        return Err("paragraph spacing is outside the direct subset");
    }
    let line_ok = match spacing.line {
        None => true,
        Some(LineSpacing::Auto(value)) => (1..=MAX_GEOMETRY_TWIPS).contains(&value),
        Some(LineSpacing::Exact(Measure::Twips(value))) => {
            (1..=MAX_GEOMETRY_TWIPS).contains(&value)
        }
        Some(LineSpacing::AtLeast(Measure::Twips(value))) => {
            (0..=MAX_GEOMETRY_TWIPS).contains(&value)
        }
        Some(_) => false,
    };
    if !line_ok {
        return Err("paragraph line spacing is outside the direct subset");
    }
    if let Some(tabs) = &props.tabs
        && (tabs.len() > MAX_EXPORT_TABS || tabs.iter().any(|stop| !tab_exportable(stop)))
    {
        return Err("paragraph tabs are outside the direct subset");
    }
    Ok(())
}

fn twip_slot(measure: Option<Measure>, negative: bool) -> bool {
    match measure {
        None => true,
        Some(Measure::Twips(value)) => {
            if negative {
                (-MAX_GEOMETRY_TWIPS..=MAX_GEOMETRY_TWIPS).contains(&value)
            } else {
                (0..=MAX_GEOMETRY_TWIPS).contains(&value)
            }
        }
        Some(_) => false,
    }
}

fn tab_exportable(stop: &TabStop) -> bool {
    matches!(stop.position, Measure::Twips(value) if (0..=MAX_GEOMETRY_TWIPS).contains(&value))
        && !matches!(stop.align, TabAlign::Bar | TabAlign::Clear)
        && !matches!(stop.leader, TabLeader::MiddleDot | TabLeader::Heavy)
}

/// `w:ind`, `w:spacing` and `w:tabs` for a paragraph the caller already
/// checked with [`geometry_supported`].
fn geometry_markup(names: &Names, props: &ParaProps) -> Result<String, &'static str> {
    geometry_supported(props)?;
    let mut out = String::new();
    if let Some(markup) = indent_markup(names, &props.indent) {
        out.push_str(&markup);
    }
    if let Some(markup) = spacing_markup(names, &props.spacing)? {
        out.push_str(&markup);
    }
    if let Some(tabs) = &props.tabs {
        out.push_str(&tabs_markup(names, tabs)?);
    }
    Ok(out)
}

fn indent_markup(names: &Names, indent: &Indent) -> Option<String> {
    let mut body = String::new();
    write_twip(&mut body, names, "start", indent.start);
    write_twip(&mut body, names, "end", indent.end);
    write_twip(&mut body, names, "firstLine", indent.first_line);
    write_twip(&mut body, names, "hanging", indent.hanging);
    if body.is_empty() {
        None
    } else {
        Some(format!("{}{body}/>", names.start("ind")))
    }
}

fn write_twip(body: &mut String, names: &Names, local: &str, measure: Option<Measure>) {
    if let Some(Measure::Twips(value)) = measure {
        body.push_str(&qattr(names, local, &value.to_string()));
    }
}

fn spacing_markup(names: &Names, spacing: &Spacing) -> Result<Option<String>, &'static str> {
    let mut body = String::new();
    write_twip(&mut body, names, "before", spacing.before);
    write_twip(&mut body, names, "after", spacing.after);
    if let Some(line) = spacing.line {
        let (value, rule) = match line {
            LineSpacing::Auto(value) => (value, "auto"),
            LineSpacing::Exact(Measure::Twips(value)) => (value, "exact"),
            LineSpacing::AtLeast(Measure::Twips(value)) => (value, "atLeast"),
            _ => return Err("paragraph line spacing is outside the direct subset"),
        };
        body.push_str(&qattr(names, "line", &value.to_string()));
        body.push_str(&qattr(names, "lineRule", rule));
    }
    if body.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!("{}{body}/>", names.start("spacing"))))
    }
}

fn tabs_markup(names: &Names, tabs: &[TabStop]) -> Result<String, &'static str> {
    let mut body = String::new();
    for stop in tabs {
        let align = match stop.align {
            TabAlign::Left => "left",
            TabAlign::Center => "center",
            TabAlign::Right => "right",
            TabAlign::Decimal => "decimal",
            TabAlign::Num => "num",
            TabAlign::Bar | TabAlign::Clear => {
                return Err("paragraph tabs are outside the direct subset");
            }
        };
        let Measure::Twips(position) = stop.position else {
            return Err("paragraph tabs are outside the direct subset");
        };
        let leader = match stop.leader {
            TabLeader::None => None,
            TabLeader::Dot => Some("dot"),
            TabLeader::Hyphen => Some("hyphen"),
            TabLeader::Underscore => Some("underscore"),
            TabLeader::MiddleDot | TabLeader::Heavy => {
                return Err("paragraph tabs are outside the direct subset");
            }
        };
        body.push_str(&names.start("tab"));
        body.push_str(&qattr(names, "val", align));
        body.push_str(&qattr(names, "pos", &position.to_string()));
        if let Some(leader) = leader {
            body.push_str(&qattr(names, "leader", leader));
        }
        body.push_str("/>");
    }
    Ok(format!(
        "{}>{body}{}",
        names.start("tabs"),
        names.end("tabs")
    ))
}

pub(super) fn rewrite_represented_paragraph(
    xml: &[u8],
    element: &Element,
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
    source: &Source<'_>,
) -> Result<Vec<u8>, ExportError> {
    if paragraph.style.is_some() || paragraph.props.numbering.is_some() {
        rewrite_styled_paragraph(xml, element, paragraph, names, limit, source)
    } else {
        rewrite_direct_paragraph(xml, element, paragraph, names, limit, source)
    }
}

pub(super) fn rewrite_styled_paragraph(
    xml: &[u8],
    element: &Element,
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
    source: &Source<'_>,
) -> Result<Vec<u8>, ExportError> {
    let open =
        opened_start(xml, element).map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
    let mut inner = String::new();
    let geometry = geometry_markup(names, &paragraph.props)
        .map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
    if paragraph.style.is_some()
        || paragraph.props.justification.is_some()
        || paragraph.props.numbering.is_some()
        || !geometry.is_empty()
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
        inner.push_str(&geometry);
        if let Some(numbering) = paragraph.props.numbering {
            inner.push_str(&num_pr(names, numbering));
        }
        inner.push_str(&names.end("pPr"));
    }
    inner.push_str(&append_linked_runs(
        source,
        paragraph.id(),
        run_groups(paragraph),
        names,
        limit,
        |style, props| {
            styled_rpr(names, style.as_ref(), props)
                .map_err(|reason| unsupported(Some(paragraph.id()), reason))
        },
    )?);
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
    source: &Source<'_>,
) -> Result<Vec<u8>, ExportError> {
    let open =
        opened_start(xml, element).map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
    let mut inner = String::new();
    let geometry = geometry_markup(names, &paragraph.props)
        .map_err(|reason| unsupported(Some(paragraph.id()), reason))?;
    if paragraph.props.justification.is_some() || !geometry.is_empty() {
        inner.push_str(&names.start("pPr"));
        inner.push('>');
        if let Some(justification) = paragraph.props.justification {
            inner.push_str(&names.start("jc"));
            inner.push_str(&qattr(names, "val", justification_value(justification)));
            inner.push_str("/>");
        }
        inner.push_str(&geometry);
        inner.push_str(&names.end("pPr"));
    }
    inner.push_str(&append_linked_runs(
        source,
        paragraph.id(),
        run_groups(paragraph),
        names,
        limit,
        |style, props| {
            if style.is_some() {
                return Err(unsupported(
                    Some(paragraph.id()),
                    "character style is outside the direct formatting subset",
                ));
            }
            synthesize_rpr(names, props).map_err(|reason| unsupported(Some(paragraph.id()), reason))
        },
    )?);
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
            Color::Theme { .. } => {
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

fn look_markup(names: &Names, look: TableLook) -> String {
    let bit = |name: &str, on: bool| names.attribute(name, if on { "1" } else { "0" });
    format!(
        "{}{}{}{}{}{}{}/>",
        names.start("tblLook"),
        bit("firstRow", look.first_row),
        bit("lastRow", look.last_row),
        bit("firstColumn", look.first_column),
        bit("lastColumn", look.last_column),
        bit("noHBand", look.no_h_band),
        bit("noVBand", look.no_v_band),
    )
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
    markers: &[MoveMarker],
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
    if paragraph.runs().iter().any(|run| run.revision.is_some()) || !markers.is_empty() {
        if paragraph.runs().iter().any(|run| run.link.is_some()) {
            return Err(unsupported(
                Some(paragraph.id()),
                "a revised run also has a hyperlink",
            ));
        }
        let body = revision_body(paragraph, markers, names, limit)?;
        if body.is_empty() {
            return Ok(format!("{}/>", names.start("p")));
        }
        return Ok(format!("{}>{body}{}", names.start("p"), names.end("p")));
    }
    if paragraph.runs().iter().any(|run| run.link.is_some()) {
        let body = source.run_markup(paragraph, names, limit)?;
        if body.is_empty() {
            return Ok(format!("{}/>", names.start("p")));
        }
        return Ok(format!("{}>{body}{}", names.start("p"), names.end("p")));
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

/// Canonical `w:tbl` for a table the model fully represents. Unknown markup
/// is never rewritten: callers keep the original element unless the table
/// changed, and this writer refuses a model it cannot spell back.
pub(super) fn table_markup(
    source: &Source<'_>,
    table: &Table,
    names: &Names,
    limit: u64,
) -> Result<String, ExportError> {
    let refuse = |reason| unsupported(Some(table.id), reason);
    if table.preserved.is_some() {
        return Err(refuse("table holds preserved markup"));
    }
    let mut markup = String::new();
    markup.push_str(&names.start("tbl"));
    markup.push('>');
    if table.style.is_some()
        || table.width.is_some()
        || table.layout.is_some()
        || table.alignment.is_some()
        || table.indent.is_some()
        || table.look.is_some()
        || table.cell_margin.is_some()
    {
        markup.push_str(&names.start("tblPr"));
        markup.push('>');
        if let Some(style) = &table.style {
            if !table_style_id(style.as_str()) {
                return Err(refuse("table style id cannot be written"));
            }
            markup.push_str(&names.start("tblStyle"));
            markup.push_str(&names.attribute("val", style.as_str()));
            markup.push_str("/>");
        }
        if let Some(width) = table.width {
            markup.push_str(&width_markup(names, "tblW", width_value(width)?)?);
        }
        if let Some(layout) = table.layout {
            let value = match layout {
                TableLayoutKind::Fixed => "fixed",
                TableLayoutKind::Autofit => "autofit",
            };
            markup.push_str(&names.start("tblLayout"));
            markup.push_str(&names.attribute("type", value));
            markup.push_str("/>");
        }
        if let Some(alignment) = table.alignment {
            let value = match alignment {
                TableAlign::Left => "left",
                TableAlign::Center => "center",
                TableAlign::Right => "right",
            };
            markup.push_str(&val_element(names, "jc", value));
        }
        if let Some(indent) = table.indent {
            if indent < 0 {
                return Err(refuse("table indent is negative"));
            }
            markup.push_str(&width_markup(names, "tblInd", ("dxa", Some(indent)))?);
        }
        if let Some(look) = table.look {
            markup.push_str(&look_markup(names, look));
        }
        if let Some(margin) = table.cell_margin {
            markup.push_str(&cell_margin_markup(names, "tblCellMar", margin)?);
        }
        markup.push_str(&names.end("tblPr"));
    }
    markup.push_str(&names.start("tblGrid"));
    markup.push('>');
    if table.grid.is_empty() {
        return Err(refuse("table grid is empty"));
    }
    for column in &table.grid {
        let Measure::Twips(width) = column else {
            return Err(refuse("table grid is not in twips"));
        };
        if *width < 0 {
            return Err(refuse("table grid is not in twips"));
        }
        markup.push_str(&names.start("gridCol"));
        markup.push_str(&names.attribute("w", &width.to_string()));
        markup.push_str("/>");
    }
    markup.push_str(&names.end("tblGrid"));
    if table.rows.is_empty() {
        return Err(refuse("table has no rows"));
    }
    for row in &table.rows {
        if row.preserved.is_some() {
            return Err(refuse("table row holds preserved markup"));
        }
        markup.push_str(&names.start("tr"));
        markup.push('>');
        if row.grid_before != 0
            || row.grid_after != 0
            || row.cant_split
            || row.header
            || row.height.is_some()
        {
            markup.push_str(&names.start("trPr"));
            markup.push('>');
            if row.grid_before != 0 {
                markup.push_str(&val_element(
                    names,
                    "gridBefore",
                    &row.grid_before.to_string(),
                ));
            }
            if row.grid_after != 0 {
                markup.push_str(&val_element(
                    names,
                    "gridAfter",
                    &row.grid_after.to_string(),
                ));
            }
            if row.cant_split {
                markup.push_str(&names.start("cantSplit"));
                markup.push_str("/>");
            }
            if row.header {
                markup.push_str(&names.start("tblHeader"));
                markup.push_str("/>");
            }
            if let Some(height) = row.height {
                if matches!(height.rule, RowHeightRule::Exact) && height.twips <= 0 {
                    return Err(refuse("exact row height is not positive"));
                }
                if matches!(height.rule, RowHeightRule::AtLeast) && height.twips < 0 {
                    return Err(refuse("row height is negative"));
                }
                let rule = match height.rule {
                    RowHeightRule::Auto => "auto",
                    RowHeightRule::AtLeast => "atLeast",
                    RowHeightRule::Exact => "exact",
                };
                markup.push_str(&names.start("trHeight"));
                markup.push_str(&names.attribute("val", &height.twips.to_string()));
                markup.push_str(&names.attribute("hRule", rule));
                markup.push_str("/>");
            }
            markup.push_str(&names.end("trPr"));
        }
        if row.cells.is_empty() {
            return Err(refuse("table row has no cells"));
        }
        for cell in &row.cells {
            if cell.preserved.is_some() {
                return Err(refuse("table cell holds preserved markup"));
            }
            if cell.grid_span == 0 {
                return Err(refuse("table cell span is zero"));
            }
            markup.push_str(&names.start("tc"));
            markup.push('>');
            let needs_pr = cell.grid_span != 1
                || cell.vertical_merge != VerticalMerge::None
                || cell.width.is_some()
                || cell.shading.is_some()
                || cell.margin.is_some()
                || cell.vertical_align.is_some()
                || borders_present(&cell.borders);
            if needs_pr {
                markup.push_str(&names.start("tcPr"));
                markup.push('>');
                if let Some(width) = cell.width {
                    markup.push_str(&width_markup(names, "tcW", cell_width_value(width)?)?);
                }
                if cell.grid_span != 1 {
                    markup.push_str(&val_element(names, "gridSpan", &cell.grid_span.to_string()));
                }
                match cell.vertical_merge {
                    VerticalMerge::None => {}
                    VerticalMerge::Restart => {
                        markup.push_str(&val_element(names, "vMerge", "restart"));
                    }
                    VerticalMerge::Continue => {
                        markup.push_str(&val_element(names, "vMerge", "continue"));
                    }
                }
                if let Some(shading) = cell.shading {
                    markup.push_str(&shading_markup(names, shading)?);
                }
                if borders_present(&cell.borders) {
                    markup.push_str(&borders_markup(names, &cell.borders)?);
                }
                if let Some(margin) = cell.margin {
                    markup.push_str(&cell_margin_markup(names, "tcMar", margin)?);
                }
                if let Some(align) = cell.vertical_align {
                    let value = match align {
                        VerticalAlign::Top => "top",
                        VerticalAlign::Center => "center",
                        VerticalAlign::Bottom => "bottom",
                    };
                    markup.push_str(&val_element(names, "vAlign", value));
                }
                markup.push_str(&names.end("tcPr"));
            }
            if cell.blocks.is_empty() {
                return Err(refuse("table cell has no block"));
            }
            for block in cell.blocks.iter() {
                match block {
                    Block::Paragraph(paragraph) => {
                        markup.push_str(&table_paragraph(source, paragraph, names, limit)?);
                    }
                    Block::Table(nested) => {
                        markup.push_str(&table_markup(source, nested, names, limit)?);
                    }
                    Block::Sdt(_) | Block::Opaque(_) => {
                        return Err(refuse("table cell holds an unmapped block"));
                    }
                }
            }
            markup.push_str(&names.end("tc"));
        }
        markup.push_str(&names.end("tr"));
        if markup.len() as u64 > limit {
            return Err(ExportError::Budget {
                part: "table".into(),
                limit,
            });
        }
    }
    markup.push_str(&names.end("tbl"));
    if markup.len() as u64 > limit {
        return Err(ExportError::Budget {
            part: "table".into(),
            limit,
        });
    }
    Ok(markup)
}

fn cell_margin_markup(
    names: &Names,
    local: &str,
    margin: CellMargins,
) -> Result<String, ExportError> {
    for value in [margin.top, margin.left, margin.bottom, margin.right] {
        if value < 0 {
            return Err(unsupported(None, "table cell margin is negative"));
        }
    }
    let mut markup = names.start(local);
    markup.push('>');
    for (local, value) in [
        ("top", margin.top),
        ("left", margin.left),
        ("bottom", margin.bottom),
        ("right", margin.right),
    ] {
        markup.push_str(&width_markup(names, local, ("dxa", Some(value)))?);
    }
    markup.push_str(&names.end(local));
    Ok(markup)
}

fn width_value(width: TableWidth) -> Result<(&'static str, Option<i64>), ExportError> {
    match width {
        TableWidth::Auto => Ok(("auto", None)),
        TableWidth::Dxa(value) => Ok(("dxa", Some(value))),
        TableWidth::Pct(value) if value > 0 => Ok(("pct", Some(value))),
        TableWidth::Pct(_) => Err(unsupported(None, "table percentage width is not positive")),
    }
}

fn cell_width_value(width: Measure) -> Result<(&'static str, Option<i64>), ExportError> {
    match width {
        Measure::Auto => Ok(("auto", None)),
        Measure::Twips(value) if value >= 0 => Ok(("dxa", Some(value))),
        Measure::PercentFiftieths(value) if value > 0 => Ok(("pct", Some(value))),
        _ => Err(unsupported(None, "table cell width cannot be written")),
    }
}

fn width_markup(
    names: &Names,
    local: &str,
    value: (&'static str, Option<i64>),
) -> Result<String, ExportError> {
    let (kind, width) = value;
    let mut markup = names.start(local);
    markup.push_str(&names.attribute("type", kind));
    if let Some(width) = width {
        markup.push_str(&names.attribute("w", &width.to_string()));
    }
    markup.push_str("/>");
    Ok(markup)
}

fn val_element(names: &Names, local: &str, value: &str) -> String {
    format!("{}{}/>", names.start(local), names.attribute("val", value))
}

fn borders_present(borders: &CellBorders) -> bool {
    borders.top.is_some()
        || borders.left.is_some()
        || borders.bottom.is_some()
        || borders.right.is_some()
}

fn borders_markup(names: &Names, borders: &CellBorders) -> Result<String, ExportError> {
    let mut markup = names.start("tcBorders");
    markup.push('>');
    for (local, edge) in [
        ("top", borders.top),
        ("left", borders.left),
        ("bottom", borders.bottom),
        ("right", borders.right),
    ] {
        if let Some(edge) = edge {
            markup.push_str(&edge_markup(names, local, edge)?);
        }
    }
    markup.push_str(&names.end("tcBorders"));
    Ok(markup)
}

fn edge_markup(names: &Names, local: &str, edge: BorderEdge) -> Result<String, ExportError> {
    let kind = match edge.kind {
        BorderKind::Nil => "nil",
        BorderKind::Single => "single",
        BorderKind::Double => "double",
        BorderKind::Dotted => "dotted",
        BorderKind::Dashed => "dashed",
        BorderKind::Thick => "thick",
    };
    let (color, theme) = border_color(edge.color)
        .map_err(|_| unsupported(None, "table border colour is outside the mapped subset"))?;
    let theme = theme
        .map(|slot| names.attribute("themeColor", slot))
        .unwrap_or_default();
    Ok(format!(
        "{}{}{}{}{theme}/>",
        names.start(local),
        names.attribute("val", kind),
        names.attribute("sz", &edge.size_eighths.to_string()),
        names.attribute("color", &color)
    ))
}

fn border_color(color: Color) -> Result<(String, Option<&'static str>), ()> {
    match color {
        Color::Auto => Ok(("auto".into(), None)),
        Color::Rgb(red, green, blue) => Ok((format!("{red:02X}{green:02X}{blue:02X}"), None)),
        Color::Theme {
            slot,
            tint: None,
            shade: None,
        } => Ok(("auto".into(), Some(slot.word_name()))),
        Color::Theme { .. } => Err(()),
    }
}

fn shading_markup(names: &Names, shading: Shading) -> Result<String, ExportError> {
    let fill = color_value(shading.fill)
        .map_err(|_| unsupported(None, "table shading is outside the mapped subset"))?;
    Ok(format!(
        "{}{}{}{}/>",
        names.start("shd"),
        names.attribute("val", "clear"),
        names.attribute("color", "auto"),
        names.attribute("fill", &fill)
    ))
}

fn color_value(color: Color) -> Result<String, ()> {
    match color {
        Color::Auto => Ok("auto".into()),
        Color::Rgb(red, green, blue) => Ok(format!("{red:02X}{green:02X}{blue:02X}")),
        Color::Theme { .. } => Err(()),
    }
}

fn table_paragraph(
    source: &Source<'_>,
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
) -> Result<String, ExportError> {
    let refuse = |reason| unsupported(Some(paragraph.id()), reason);
    if !table_paragraph_ok(paragraph) {
        return Err(refuse("table cell paragraph is outside the mapped subset"));
    }
    let mut inner = String::new();
    if paragraph.style.is_some() || paragraph.props.justification.is_some() {
        inner.push_str(&names.start("pPr"));
        inner.push('>');
        if let Some(style) = &paragraph.style {
            inner.push_str(&names.start("pStyle"));
            inner.push_str(&names.attribute("val", style.as_str()));
            inner.push_str("/>");
        }
        if let Some(justification) = paragraph.props.justification {
            inner.push_str(&names.start("jc"));
            inner.push_str(&names.attribute("val", justification_value(justification)));
            inner.push_str("/>");
        }
        inner.push_str(&names.end("pPr"));
    }
    if paragraph.runs().iter().any(|run| run.revision.is_some()) {
        if paragraph.runs().iter().any(|run| run.link.is_some()) {
            return Err(refuse(
                "table cell revision with a hyperlink is outside the mapped subset",
            ));
        }
        inner.push_str(&revision_body(paragraph, &[], names, limit)?);
    } else {
        inner.push_str(&append_table_runs(source, paragraph, names, limit)?);
    }
    if inner.is_empty() {
        return Ok(format!("{}/>", names.start("p")));
    }
    Ok(format!("{}>{inner}{}", names.start("p"), names.end("p")))
}

/// Table cell runs with `w:hyperlink` wrappers around the runs that carry a
/// link. Unlike the body writer, consecutive runs are never merged: a cell
/// paragraph is rewritten only after an edit, and the round-trip verifier
/// treats run boundaries as semantic, so one model run is one `w:r`.
fn append_table_runs(
    source: &Source<'_>,
    paragraph: &Paragraph,
    names: &Names,
    limit: u64,
) -> Result<String, ExportError> {
    let id = paragraph.id();
    let refuse = |reason| unsupported(Some(id), reason);
    let text = paragraph.plain_text(|_| "\u{FFFC}".into());
    let mut chars = text.chars();
    let runs = paragraph.runs();
    let mut body = String::new();
    let mut index = 0usize;
    while index < runs.len() {
        let link = runs[index].link.clone();
        let start = index;
        index += 1;
        while index < runs.len() && runs[index].link == link {
            index += 1;
        }
        let mut inner = String::new();
        for run in &runs[start..index] {
            if run.style.is_some() {
                return Err(refuse("table cell paragraph is outside the mapped subset"));
            }
            let run_text: String = chars.by_ref().take(run.len).collect();
            let rpr = table_rpr(names, &run.props).map_err(refuse)?;
            let text = bounded_text_xml(names, &run_text, limit)?;
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
        if inner.is_empty() {
            continue;
        }
        if let Some(link) = link.as_ref() {
            body.push_str(&hyperlink_start(source, id, names, link)?);
            body.push_str(&inner);
            body.push_str(&names.end("hyperlink"));
        } else {
            body.push_str(&inner);
        }
    }
    Ok(body)
}

fn table_paragraph_ok(paragraph: &Paragraph) -> bool {
    let props = &paragraph.props;
    paragraph.inlines().is_empty()
        && paragraph.mark_props.is_empty()
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
        && paragraph
            .style
            .as_ref()
            .is_none_or(|style| table_style_id(style.as_str()))
        && !paragraph
            .plain_text(|_| "\u{FFFC}".into())
            .contains('\u{FFFC}')
        && paragraph.runs().iter().all(|run| {
            run.style.is_none() && run.props.preserved.is_none() && table_run_ok(&run.props)
        })
}

fn table_style_id(id: &str) -> bool {
    writable_style_id(id) && !id.chars().any(|ch| matches!(ch, '<' | '>' | '&' | '"'))
}

fn table_run_ok(props: &RunProps) -> bool {
    props.strike.is_none()
        && props.hidden.is_none()
        && props.underline.is_none()
        && props.fonts == FontSlots::default()
        && props.languages == Languages::default()
        && props
            .size
            .is_none_or(|size| matches!(size, Measure::HalfPoints(value) if value >= 0))
        && props
            .color
            .is_none_or(|color| matches!(color, Color::Auto | Color::Rgb(_, _, _)))
}

fn table_rpr(names: &Names, props: &RunProps) -> Result<Option<String>, &'static str> {
    if !table_run_ok(props) {
        return Err("table cell paragraph is outside the mapped subset");
    }
    if props.is_empty() {
        return Ok(None);
    }
    let mut body = String::new();
    body.push_str(&toggle(names, "b", props.bold));
    body.push_str(&toggle(names, "i", props.italic));
    if let Some(Measure::HalfPoints(size)) = props.size {
        body.push_str(&names.start("sz"));
        body.push_str(&names.attribute("val", &size.to_string()));
        body.push_str("/>");
    }
    if let Some(color) = props.color {
        let value =
            color_value(color).map_err(|_| "theme colours are outside the direct subset")?;
        body.push_str(&names.start("color"));
        body.push_str(&names.attribute("val", &value));
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
        if child.is(W, "hyperlink") {
            return blocks_rewrite(child);
        }
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

fn known_tabs(element: &Element) -> Result<(), &'static str> {
    if !element.attributes.is_empty() {
        return Err("structural paragraph markup has unknown attributes");
    }
    for node in &element.children {
        match node {
            Node::Text(text) if is_space(text) => {}
            Node::Element(child) if child.is(W, "tab") => {
                if !child.attributes.iter().all(|attribute| {
                    attribute.namespace.as_deref() == Some(W)
                        && matches!(attribute.local.as_ref(), "val" | "pos" | "leader")
                }) {
                    return Err("structural paragraph markup has unknown attributes");
                }
                if child.elements().next().is_some() {
                    return Err("unmapped structural paragraph content");
                }
            }
            _ => return Err("unmapped structural paragraph content"),
        }
    }
    Ok(())
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

type LinkedGroup = (
    Option<StyleId>,
    Arc<RunProps>,
    Option<Arc<Hyperlink>>,
    String,
);

fn run_groups(paragraph: &Paragraph) -> Vec<LinkedGroup> {
    let text = paragraph.plain_text(|_| "\u{FFFC}".into());
    let mut chars = text.chars();
    let mut groups: Vec<LinkedGroup> = Vec::new();
    for run in paragraph.runs() {
        let text: String = chars.by_ref().take(run.len).collect();
        if let Some((style, props, link, existing)) = groups.last_mut()
            && *style == run.style
            && *props == run.props
            && *link == run.link
        {
            existing.push_str(&text);
        } else {
            groups.push((run.style.clone(), run.props.clone(), run.link.clone(), text));
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

/// Replaces one paragraph's children, keeping the start tag and `w:pPr`.
/// Text and move revisions are written from the model (ECMA-376 §17.13.5).
pub(super) fn rewrite_revisions(
    xml: &[u8],
    element: &Element,
    paragraph: &Paragraph,
    markers: &[MoveMarker],
    names: &Names,
    limit: u64,
) -> Result<Vec<u8>, ExportError> {
    let refuse = |reason| unsupported(Some(paragraph.id()), reason);
    let mut out = opened_start(xml, element).map_err(refuse)?;
    if let Some(properties) = element.child(W, "pPr") {
        out.extend_from_slice(&xml[properties.span.clone()]);
    }
    out.extend(revision_body(paragraph, markers, names, limit)?.into_bytes());
    out.extend(names.end("p").into_bytes());
    if out.len() as u64 > limit {
        return Err(ExportError::Budget {
            part: "paragraph".into(),
            limit,
        });
    }
    Ok(out)
}

struct RevisionSegment {
    offset: usize,
    revision: Option<joyword_model::TextRevision>,
    style: Option<StyleId>,
    props: Arc<RunProps>,
    text: String,
}

fn revision_body(
    paragraph: &Paragraph,
    markers: &[MoveMarker],
    names: &Names,
    limit: u64,
) -> Result<String, ExportError> {
    let refuse = |reason| unsupported(Some(paragraph.id()), reason);
    let full = paragraph.plain_text(|_| "\u{FFFC}".into());
    if full.contains('\u{FFFC}') {
        return Err(refuse("revision export refuses inline objects"));
    }
    let cuts: BTreeSet<usize> = markers
        .iter()
        .map(|marker| marker.position.offset.0)
        .collect();
    let mut chars = full.chars();
    let mut offset = 0usize;
    let mut segments = Vec::new();
    for run in paragraph.runs() {
        let run_text: String = chars.by_ref().take(run.len).collect();
        let run_chars: Vec<char> = run_text.chars().collect();
        let mut start = 0usize;
        while start < run_chars.len() {
            let abs = offset + start;
            let end_abs = cuts
                .range((abs + 1)..)
                .next()
                .copied()
                .unwrap_or(offset + run_chars.len())
                .min(offset + run_chars.len());
            let end = start + (end_abs - abs);
            let text: String = run_chars[start..end].iter().collect();
            let continues = segments.last().is_some_and(|last: &RevisionSegment| {
                last.revision == run.revision
                    && last.style == run.style
                    && last.props == run.props
                    && last.offset + last.text.chars().count() == abs
                    && !cuts.contains(&abs)
            });
            if continues {
                if let Some(last) = segments.last_mut() {
                    last.text.push_str(&text);
                }
            } else if !text.is_empty() {
                segments.push(RevisionSegment {
                    offset: abs,
                    revision: run.revision.clone(),
                    style: run.style.clone(),
                    props: Arc::clone(&run.props),
                    text,
                });
            }
            start = end;
        }
        offset += run.len;
    }
    let mut xml = String::new();
    let mut open: Option<joyword_model::TextRevision> = None;
    let mut emitted = BTreeSet::new();
    let mut emit = |xml: &mut String, at: usize| {
        if !emitted.insert(at) {
            return;
        }
        let mut here: Vec<&MoveMarker> = markers
            .iter()
            .filter(|marker| marker.position.offset.0 == at)
            .collect();
        here.sort_by_key(|marker| (!marker.at_end, marker.id));
        for marker in here {
            xml.push_str(&marker_xml(names, marker));
        }
    };
    if segments.is_empty() {
        emit(&mut xml, 0);
        return Ok(xml);
    }
    for segment in &segments {
        if markers
            .iter()
            .any(|marker| marker.position.offset.0 == segment.offset)
        {
            close_revision(&mut xml, names, &mut open);
            emit(&mut xml, segment.offset);
        }
        if open.as_ref() != segment.revision.as_ref() {
            close_revision(&mut xml, names, &mut open);
            if let Some(revision) = &segment.revision {
                xml.push_str(&names.start(revision_element(revision.kind)));
                xml.push_str(&track_attributes(names, revision));
                xml.push('>');
                open = Some(revision.clone());
            }
        }
        let leaf = match segment.revision.as_ref().map(|revision| revision.kind) {
            Some(RevisionKind::Delete | RevisionKind::MoveFrom) => "delText",
            _ => "t",
        };
        let text = bounded_text_leaf(names, &segment.text, leaf, limit)?;
        let properties =
            direct_rpr(names, segment.style.as_ref(), &segment.props).map_err(refuse)?;
        if properties.is_none() && text.is_empty() {
            continue;
        }
        xml.push_str(&names.start("r"));
        xml.push('>');
        if let Some(properties) = properties {
            xml.push_str(&properties);
        }
        xml.push_str(&text);
        xml.push_str(&names.end("r"));
        if xml.len() as u64 > limit {
            return Err(ExportError::Budget {
                part: "paragraph".into(),
                limit,
            });
        }
    }
    let end = paragraph.len();
    if markers.iter().any(|marker| marker.position.offset.0 == end) {
        close_revision(&mut xml, names, &mut open);
        emit(&mut xml, end);
    }
    close_revision(&mut xml, names, &mut open);
    Ok(xml)
}

fn close_revision(xml: &mut String, names: &Names, open: &mut Option<joyword_model::TextRevision>) {
    if let Some(revision) = open.take() {
        xml.push_str(&names.end(revision_element(revision.kind)));
    }
}

fn revision_element(kind: RevisionKind) -> &'static str {
    match kind {
        RevisionKind::Insert => "ins",
        RevisionKind::Delete => "del",
        RevisionKind::MoveFrom => "moveFrom",
        RevisionKind::MoveTo => "moveTo",
    }
}

fn track_attributes(names: &Names, revision: &joyword_model::TextRevision) -> String {
    let mut attributes = names.attribute("id", &revision.id.to_string());
    attributes.push_str(&names.attribute("author", revision.author.as_ref()));
    if let Some(date) = &revision.date {
        attributes.push_str(&names.attribute("date", date));
    }
    attributes
}

fn marker_element(marker: &MoveMarker) -> &'static str {
    match (marker.side, marker.at_end) {
        (MoveSide::From, false) => "moveFromRangeStart",
        (MoveSide::From, true) => "moveFromRangeEnd",
        (MoveSide::To, false) => "moveToRangeStart",
        (MoveSide::To, true) => "moveToRangeEnd",
    }
}

fn marker_xml(names: &Names, marker: &MoveMarker) -> String {
    let mut attributes = names.attribute("id", &marker.id.to_string());
    attributes.push_str(&names.attribute("author", marker.author.as_ref()));
    if let Some(date) = &marker.date {
        attributes.push_str(&names.attribute("date", date));
    }
    attributes.push_str(&names.attribute("name", marker.name.as_ref()));
    format!("{}{attributes}/>", names.start(marker_element(marker)))
}

fn direct_rpr(
    names: &Names,
    style: Option<&StyleId>,
    props: &RunProps,
) -> Result<Option<String>, &'static str> {
    let mut extra = String::new();
    if let Some(style) = style {
        extra.push_str(&names.start("rStyle"));
        extra.push_str(&names.attribute("val", style.as_str()));
        extra.push_str("/>");
    }
    let synthesized = synthesize_rpr(names, props)?;
    match (extra.is_empty(), synthesized) {
        (true, None) => Ok(None),
        (true, Some(markup)) => Ok(Some(markup)),
        (false, None) => Ok(Some(format!(
            "{}>{extra}{}",
            names.start("rPr"),
            names.end("rPr")
        ))),
        (false, Some(markup)) => {
            let Some(split) = markup.find('>') else {
                return Err("character formatting is outside the direct subset");
            };
            let mut combined = String::new();
            combined.push_str(&markup[..=split]);
            combined.push_str(&extra);
            combined.push_str(&markup[split + 1..]);
            Ok(Some(combined))
        }
    }
}

/// `r:id` and/or `w:anchor`, and nothing else.
fn hyperlink_attrs(element: &Element) -> Result<(Option<&str>, Option<&str>), &'static str> {
    let mut relationship = None;
    let mut anchor = None;
    for attribute in &element.attributes {
        if attribute.namespace.as_deref() == Some(R) && attribute.local.as_ref() == "id" {
            if relationship.is_some() {
                return Err("structural paragraph markup has unknown attributes");
            }
            relationship = Some(attribute.value.as_str());
        } else if attribute.namespace.as_deref() == Some(W) && attribute.local.as_ref() == "anchor"
        {
            if anchor.is_some() {
                return Err("structural paragraph markup has unknown attributes");
            }
            anchor = Some(attribute.value.as_str());
        } else {
            return Err("structural paragraph markup has unknown attributes");
        }
    }
    if relationship.is_none() && anchor.is_none() {
        return Err("unmapped structural paragraph content");
    }
    Ok((relationship, anchor))
}

fn anchor_token_ok(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 255
        && !value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\''))
}

fn plain_hyperlink(element: &Element) -> Result<(), &'static str> {
    hyperlink_attrs(element)?;
    let mut runs = 0usize;
    for node in &element.children {
        match node {
            Node::Text(text) if is_space(text) => {}
            Node::Element(child) if child.is(W, "r") => {
                runs += 1;
                plain_markup(child)?;
            }
            _ => return Err("unmapped structural paragraph content"),
        }
    }
    if runs == 0 {
        Err("unmapped structural paragraph content")
    } else {
        Ok(())
    }
}

fn target_writable(url: &str) -> bool {
    !url.is_empty()
        && url.chars().count() <= joyword_model::MAX_HYPERLINK_CHARS
        && !url
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | '"'))
}

fn append_linked_runs(
    source: &Source<'_>,
    id: NodeId,
    groups: Vec<LinkedGroup>,
    names: &Names,
    limit: u64,
    mut properties: impl FnMut(&Option<StyleId>, &Arc<RunProps>) -> Result<Option<String>, ExportError>,
) -> Result<String, ExportError> {
    let mut body = String::new();
    let mut index = 0usize;
    while index < groups.len() {
        let link = groups[index].2.clone();
        let start = index;
        index += 1;
        while index < groups.len() && groups[index].2 == link {
            index += 1;
        }
        let mut inner = String::new();
        for (style, props, _, text) in &groups[start..index] {
            let rpr = properties(style, props)?;
            let text = bounded_text_xml(names, text, limit)?;
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
        if inner.is_empty() {
            continue;
        }
        if let Some(link) = link.as_ref() {
            body.push_str(&hyperlink_start(source, id, names, link)?);
            body.push_str(&inner);
            body.push_str(&names.end("hyperlink"));
        } else {
            body.push_str(&inner);
        }
    }
    Ok(body)
}

fn hyperlink_start(
    source: &Source<'_>,
    id: NodeId,
    names: &Names,
    link: &Hyperlink,
) -> Result<String, ExportError> {
    let relationship = source.hyperlink_id(link)?;
    if relationship.is_none() && link.anchor.is_none() {
        return Err(unsupported(
            Some(id),
            "hyperlink structure is not rewritten",
        ));
    }
    let mut tag = names.start("hyperlink");
    if let Some(relationship) = relationship.as_deref() {
        if relationship
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | '"' | '&' | ' '))
        {
            return Err(unsupported(
                Some(id),
                "hyperlink relationship id cannot be written",
            ));
        }
        tag.push_str(" xmlns:r=\"");
        tag.push_str(R);
        tag.push('"');
        tag.push_str(" r:id=\"");
        tag.push_str(relationship);
        tag.push('"');
    }
    if let Some(anchor) = link.anchor.as_deref() {
        tag.push_str(&names.attribute("anchor", anchor));
    }
    tag.push('>');
    Ok(tag)
}

/// Relationship ids already in the main part, plus ids this plan allocates
/// for external targets that are not there yet.
struct LinkAssign {
    /// Hyperlink id to external target.
    urls: HashMap<String, String>,
    /// First id that already names a target.
    by_url: HashMap<String, String>,
    /// Every relationship id on the part, so a fresh id cannot collide.
    taken: HashSet<String>,
    fresh: Vec<(String, String)>,
    next: u32,
}

impl LinkAssign {
    fn from_package(backing: &crate::PackageBacking, part: &str) -> Self {
        let mut assign = Self {
            urls: HashMap::new(),
            by_url: HashMap::new(),
            taken: HashSet::new(),
            fresh: Vec::new(),
            next: 1,
        };
        let Ok(name) = PartName::new(part) else {
            return assign;
        };
        for relationship in backing.package().relationships(Some(&name)) {
            assign.taken.insert(relationship.id.to_string());
            if relationship.kind.as_ref() != REL_HYPERLINK {
                continue;
            }
            let Target::External { raw, .. } = &relationship.target else {
                continue;
            };
            if raw.is_empty() {
                continue;
            }
            assign
                .by_url
                .entry(raw.to_string())
                .or_insert_with(|| relationship.id.to_string());
            assign
                .urls
                .insert(relationship.id.to_string(), raw.to_string());
        }
        assign
    }

    fn url(&self, id: &str) -> Option<String> {
        self.urls.get(id).cloned()
    }

    fn id_for(&mut self, link: &Hyperlink) -> Result<Option<String>, &'static str> {
        let Some(url) = link.external.as_deref() else {
            return Ok(None);
        };
        if !target_writable(url) {
            return Err("hyperlink target cannot be written");
        }
        if let Some(id) = link.relationship.as_deref()
            && self.urls.get(id).is_some_and(|existing| existing == url)
        {
            return Ok(Some(id.to_string()));
        }
        if let Some(id) = self.by_url.get(url) {
            return Ok(Some(id.clone()));
        }
        let id = self.allocate()?;
        self.fresh.push((id.clone(), url.to_string()));
        self.by_url.insert(url.to_string(), id.clone());
        self.urls.insert(id.clone(), url.to_string());
        self.taken.insert(id.clone());
        Ok(Some(id))
    }

    fn allocate(&mut self) -> Result<String, &'static str> {
        for _ in 0..10_000 {
            let id = format!("rIdH{}", self.next);
            self.next = self.next.saturating_add(1);
            if self.next == 0 {
                return Err("hyperlink relationship ids exhausted");
            }
            if !self.taken.contains(&id) {
                return Ok(id);
            }
        }
        Err("hyperlink relationship ids exhausted")
    }
}

/// Appends `Relationship` elements before the single `</Relationships>` end
/// tag. An XML declaration at the start is kept. Comments, extra processing
/// instructions and a missing or repeated end tag are refused.
pub(super) fn append_hyperlink_relationships(
    xml: &[u8],
    fresh: &[(String, String)],
) -> Result<Vec<u8>, ExportError> {
    let text = std::str::from_utf8(xml)
        .map_err(|_| unsupported(None, "hyperlink relationships part is not rewritten"))?;
    if text.contains("<!--") {
        return Err(unsupported(
            None,
            "hyperlink relationships part is not rewritten",
        ));
    }
    let mut body_at = 0usize;
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<?xml") {
        let lead = text.len() - trimmed.len();
        if text[..lead].contains("<?") {
            return Err(unsupported(
                None,
                "hyperlink relationships part is not rewritten",
            ));
        }
        let Some(end) = rest.find("?>") else {
            return Err(unsupported(
                None,
                "hyperlink relationships part is not rewritten",
            ));
        };
        body_at = lead + "<?xml".len() + end + 2;
    }
    if text[body_at..].contains("<?") {
        return Err(unsupported(
            None,
            "hyperlink relationships part is not rewritten",
        ));
    }
    let end_tag = "</Relationships>";
    if text.matches(end_tag).count() != 1 {
        return Err(unsupported(
            None,
            "hyperlink relationships part is not rewritten",
        ));
    }
    let Some(pos) = text.rfind(end_tag) else {
        return Err(unsupported(
            None,
            "hyperlink relationships part is not rewritten",
        ));
    };
    let mut extra = String::new();
    for (id, target) in fresh {
        if id
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | '"' | '&' | ' '))
            || !target_writable(target)
        {
            return Err(unsupported(None, "hyperlink target cannot be written"));
        }
        extra.push_str("<Relationship Id=\"");
        extra.push_str(id);
        extra.push_str("\" Type=\"");
        extra.push_str(REL_HYPERLINK);
        extra.push_str("\" Target=\"");
        extra.push_str(&xml_escape(target));
        extra.push_str("\" TargetMode=\"External\"/>");
    }
    let mut out = String::with_capacity(text.len() + extra.len());
    out.push_str(&text[..pos]);
    out.push_str(&extra);
    out.push_str(&text[pos..]);
    Ok(out.into_bytes())
}

pub(super) fn section_page_changes(
    original: &Document,
    current: &Document,
) -> Result<Vec<usize>, ExportError> {
    if original.sections().len() != current.sections().len() {
        return Err(unsupported(None, "section count changed"));
    }
    let mut changed = Vec::new();
    for (index, (before, after)) in original
        .sections()
        .iter()
        .zip(current.sections())
        .enumerate()
    {
        if !super::compare::same_section_envelope(before, after) {
            return Err(unsupported(
                None,
                "section properties outside page setup export",
            ));
        }
        if before.page != after.page {
            if after.preserved.is_some() || !page_setup_exportable(&after.page) {
                return Err(unsupported(
                    None,
                    "section page setup outside export subset",
                ));
            }
            changed.push(index);
        }
    }
    Ok(changed)
}

fn page_setup_exportable(page: &PageSetup) -> bool {
    let twip = |measure: Measure| matches!(measure, Measure::Twips(value) if value >= 0);
    twip(page.width)
        && twip(page.height)
        && twip(page.top)
        && twip(page.right)
        && twip(page.bottom)
        && twip(page.left)
        && twip(page.header)
        && twip(page.footer)
        && twip(page.gutter)
}

impl<'a> Source<'a> {
    pub(super) fn patch_section_page(
        &self,
        document: &Document,
        section: &Section,
        page: &PageSetup,
        names: &Names,
    ) -> Result<(Range<usize>, Vec<u8>), ExportError> {
        let element = self.section_sectpr(document, section)?;
        let bytes = rewrite_sectpr_page(self.xml, element, page, names)?;
        Ok((element.span.clone(), bytes))
    }

    fn section_sectpr(
        &self,
        document: &Document,
        section: &Section,
    ) -> Result<&'a Element, ExportError> {
        if let Some(last) = section.last_block {
            let paragraph = document
                .body()
                .blocks
                .iter()
                .filter_map(Block::as_paragraph)
                .find(|paragraph| paragraph.id() == last)
                .ok_or_else(|| unsupported(None, "section boundary block is missing"))?;
            let element = self.paragraph(paragraph)?;
            let ppr = element
                .child(W, "pPr")
                .ok_or_else(|| unsupported(None, "section paragraph has no pPr"))?;
            return ppr
                .child(W, "sectPr")
                .ok_or_else(|| unsupported(None, "section paragraph has no sectPr"));
        }
        self.elements
            .values()
            .find(|element| element.is(W, "sectPr"))
            .copied()
            .ok_or_else(|| unsupported(None, "final section properties are missing"))
    }
}

fn rewrite_sectpr_page(
    xml: &[u8],
    sectpr: &Element,
    page: &PageSetup,
    names: &Names,
) -> Result<Vec<u8>, ExportError> {
    let children: Vec<_> = sectpr.elements().collect();
    let page_markup = format!(
        "{}{}",
        pg_sz_markup(names, page)?,
        pg_mar_markup(names, page)?
    );
    let open_end = children
        .first()
        .map(|child| child.span.start)
        .unwrap_or_else(|| {
            xml[sectpr.span.start..]
                .iter()
                .position(|&byte| byte == b'>')
                .map(|offset| sectpr.span.start + offset + 1)
                .unwrap_or(sectpr.span.end)
        });
    let close_start = children
        .last()
        .map(|child| child.span.end)
        .unwrap_or(open_end);
    let mut out = Vec::new();
    out.extend_from_slice(&xml[sectpr.span.start..open_end]);
    let mut wrote_page = false;
    for child in children {
        let mapped = child.namespace.as_deref() == Some(W);
        let local = child.local.as_ref();
        if mapped && (local == "pgSz" || local == "pgMar") {
            if !wrote_page {
                out.extend_from_slice(page_markup.as_bytes());
                wrote_page = true;
            }
            continue;
        }
        out.extend_from_slice(&xml[child.span.clone()]);
    }
    if !wrote_page {
        out.extend_from_slice(page_markup.as_bytes());
    }
    out.extend_from_slice(&xml[close_start..sectpr.span.end]);
    Ok(out)
}

fn pg_sz_markup(names: &Names, page: &PageSetup) -> Result<String, ExportError> {
    let (width, height) = match (page.width, page.height) {
        (Measure::Twips(width), Measure::Twips(height)) => (width, height),
        _ => return Err(unsupported(None, "page size is outside export subset")),
    };
    let mut markup = names.start("pgSz");
    markup.push_str(&names.attribute("w", &width.to_string()));
    markup.push_str(&names.attribute("h", &height.to_string()));
    if page.orientation == Orientation::Landscape {
        markup.push_str(&names.attribute("orient", "landscape"));
    }
    markup.push_str("/>");
    Ok(markup)
}

fn pg_mar_markup(names: &Names, page: &PageSetup) -> Result<String, ExportError> {
    let twip = |measure: Measure| match measure {
        Measure::Twips(value) if value >= 0 => Ok(value),
        _ => Err(unsupported(None, "page margins are outside export subset")),
    };
    let mut markup = names.start("pgMar");
    for (name, measure) in [
        ("top", page.top),
        ("right", page.right),
        ("bottom", page.bottom),
        ("left", page.left),
        ("header", page.header),
        ("footer", page.footer),
        ("gutter", page.gutter),
    ] {
        markup.push_str(&names.attribute(name, &twip(measure)?.to_string()));
    }
    markup.push_str("/>");
    Ok(markup)
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
