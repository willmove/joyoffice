//! The main document body (ECMA-376 Part 1 §17.2 document and body,
//! §17.3.1 paragraphs, §17.3.2–17.3.3 runs and run content, §17.6 sections).
//!
//! The body is read one top-level block at a time. Paragraphs are mapped;
//! every other block (tables, content controls, unknown elements) becomes an
//! [`OpaqueBlock`] whose source is preserved. Inside a paragraph, text,
//! tabs and breaks are mapped; other run content becomes a preserved inline
//! object. A simple `w:hyperlink` (`r:id` external target and/or `w:anchor`,
//! runs of text and tabs) is stored on those runs and is not fetched. Other
//! wrappers are descended into for their text and preserved. Range markers
//! such as bookmarks are preserved without a position. A paragraph or block
//! holding preserved content is locked.

use std::sync::Arc;

use joyword_model::{
    Block, BlockSeq, BreakKind, EqualColumns, HeaderFooterKind, InlineKind, InlineObject, NodeId,
    NumberFormat, OBJECT_MARKER, OpaqueBlock, Orientation, PageField, PageFieldKind, PageNumbering,
    PageSetup, Paragraph, Preserved, RunProps, Section, SectionBreak, StoryId, StoryKind, StyleId,
};

use crate::backing::PreservedReason;
use crate::import::{Importer, Scope, xml_error};
use crate::ns::{KNOWN_OFFICE, MC, R, W, W_STRICT, is_revision};
use crate::props::{self, Findings};
use crate::result::{DiagnosticKind, ImportError, ReadOnlyReason, Severity};
use crate::xml::{Element, Step, XML_NAMESPACE, XmlReader, is_xml_space};
use joyoffice_opc::PartName;
use joyoffice_opc::Target;

/// Reads `w:document` and maps its body.
pub(crate) fn read(
    importer: &mut Importer<'_>,
    part: &PartName,
    reader: &mut XmlReader<'_>,
) -> Result<(), ImportError> {
    let error = xml_error(part);
    let root = reader.root().map_err(&error)?;
    if root.element.namespace.as_deref() == Some(W_STRICT) {
        return Err(ImportError::StrictUnsupported);
    }
    let missing_body = || ImportError::UnexpectedElement {
        part: part.to_string(),
        expected: "w:document with w:body",
    };
    if !root.element.is(W, "document") || root.empty {
        return Err(missing_body());
    }
    importer.backing.interpret(part.clone());
    let base = Scope::default();
    importer.scan(part, &root.element, &base);
    let scope = base.enter(&root.element).into_owned();
    let mut body = Body::default();
    let mut seen_body = false;
    loop {
        match reader.step().map_err(&error)? {
            Step::Start(start) if start.element.is(W, "body") && !seen_body => {
                seen_body = true;
                importer.scan(part, &start.element, &scope);
                let inner = scope.enter(&start.element).into_owned();
                if start.empty {
                    continue;
                }
                loop {
                    match reader.step().map_err(&error)? {
                        Step::Start(start) => {
                            let element = reader.finish(start).map_err(&error)?;
                            body.block(importer, part, &element, &inner);
                        }
                        Step::Text(_) => {}
                        Step::End => break,
                        Step::Eof => return Err(missing_body()),
                    }
                }
            }
            Step::Start(start) => {
                // w:background and anything else outside the body.
                let element = reader.finish(start).map_err(&error)?;
                importer.scan(part, &element, &scope);
                let reason = foreign(importer, part, &element, &scope);
                importer.preserve(part, &element, &scope, reason);
            }
            Step::Text(_) => {}
            Step::End => break,
            Step::Eof => return Err(missing_body()),
        }
    }
    if !matches!(reader.step().map_err(&error)?, Step::Eof) || !seen_body {
        return Err(missing_body());
    }
    body.finish(importer, part);
    crate::revision::flush_moves(importer);
    Ok(())
}

/// Why an element outside the mapped subset is preserved. An element in a
/// namespace that is neither WordprocessingML, a known Office namespace nor
/// ignorable makes the document read-only (ECMA-376 Part 3: a consumer may
/// not silently process content it does not understand).
fn foreign(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
) -> PreservedReason {
    let namespace = element.namespace.as_deref().unwrap_or_default();
    if element.is(MC, "AlternateContent") {
        return PreservedReason::AlternateContent;
    }
    if namespace == W {
        return if is_revision(&element.local) {
            PreservedReason::Revision
        } else {
            PreservedReason::Unsupported
        };
    }
    if scope
        .ignorable
        .iter()
        .any(|ignorable| &**ignorable == namespace)
    {
        return PreservedReason::Ignorable;
    }
    if !KNOWN_OFFICE.contains(&namespace) {
        importer.deny(
            part,
            element,
            ReadOnlyReason::UnknownNamespace {
                namespace: namespace.to_owned(),
            },
        );
    }
    PreservedReason::Unsupported
}

/// Range markers: preserved without occupying a text position.
fn is_marker(local: &str) -> bool {
    matches!(
        local,
        "bookmarkStart"
            | "bookmarkEnd"
            | "commentRangeStart"
            | "commentRangeEnd"
            | "permStart"
            | "permEnd"
    ) || (local.starts_with("customXml")
        && (local.ends_with("RangeStart") || local.ends_with("RangeEnd")))
}

/// Inline wrappers whose run content is mapped while the wrapper itself is
/// preserved: smart tags, inline custom XML, simple fields, bidirectional
/// embeddings, and inserted or moved-to revisions. A simple hyperlink is
/// mapped instead; one this slice does not understand still uses this path.
fn is_wrapper(local: &str) -> bool {
    matches!(
        local,
        "hyperlink" | "smartTag" | "customXml" | "fldSimple" | "dir" | "bdo"
    )
}

#[derive(Default)]
struct Body {
    blocks: Vec<Block>,
    /// Sections ended by a paragraph's `w:sectPr`, in order.
    sections: Vec<Section>,
    last: Option<Section>,
}

impl Body {
    fn block(
        &mut self,
        importer: &mut Importer<'_>,
        part: &PartName,
        element: &Element,
        scope: &Scope,
    ) {
        importer.scan(part, element, scope);
        if element.is(W, "p") {
            let (paragraph, section) = paragraph(importer, part, element, scope);
            let id = paragraph.id();
            self.blocks.push(paragraph.into());
            if let Some((sect, sect_scope)) = section {
                let section = self::section(importer, part, &sect, &sect_scope, Some(id));
                self.sections.push(section);
            }
        } else if element.is(W, "sectPr") && self.last.is_none() {
            self.last = Some(section(importer, part, element, scope, None));
        } else if element.is(W, "tbl")
            && let Some(table) = crate::table::import_table(importer, part, element)
        {
            self.blocks.push(table.into());
        } else {
            let reason = foreign(importer, part, element, scope);
            let handle = importer.preserve(part, element, scope, reason);
            let id = importer.document.ids().node();
            importer.editability.lock(id);
            self.blocks.push(
                OpaqueBlock {
                    id,
                    preserved: Preserved {
                        element: Arc::from(&*element.qname),
                        handle,
                    },
                    source: Some(Importer::source(part, element)),
                }
                .into(),
            );
        }
    }

    fn finish(mut self, importer: &mut Importer<'_>, part: &PartName) {
        if self.blocks.is_empty() {
            importer.diagnose(
                Severity::Warning,
                Some(part),
                None,
                DiagnosticKind::EmptyBody,
            );
            let id = importer.document.ids().node();
            self.blocks.push(Paragraph::new(id).into());
        }
        let last = self.last.take().unwrap_or_else(|| {
            importer.diagnose(
                Severity::Warning,
                Some(part),
                None,
                DiagnosticKind::SectionDefaults {
                    missing: "w:sectPr",
                },
            );
            Section {
                id: importer.document.ids().node(),
                last_block: None,
                page: PageSetup::a4_zh_cn(),
                title_page: None,
                headers: Vec::new(),
                footers: Vec::new(),
                break_kind: SectionBreak::NextPage,
                numbering: PageNumbering::default(),
                columns: None,
                preserved: None,
            }
        });
        self.sections.push(last);
        let body = importer.document.body_id();
        importer
            .document
            .story_mut(body)
            .expect("body story")
            .blocks = BlockSeq::from_vec(self.blocks);
        *importer.document.sections_mut() = self.sections;
    }
}

/// Maps `w:p`; returns the paragraph and its `w:sectPr`, if it ends a
/// section.
fn paragraph(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
) -> (Paragraph, Option<(Element, Scope)>) {
    let mut paragraph = Paragraph::new(importer.document.ids().node());
    paragraph.source = Some(Importer::source(part, element));
    let inner = scope.enter(element);
    let mut section = None;
    if let Some(ppr) = element.child(W, "pPr") {
        let ppr_scope = inner.enter(ppr);
        let mut findings = Findings::default();
        let para = props::para(ppr, &mut findings);
        paragraph.style = para.style;
        let mut props = para.props;
        if let Some(rpr) = para.mark {
            let mut mark_findings = Findings::default();
            let (mut mark, mark_style) = props::run(rpr, &mut mark_findings);
            if mark_style.is_some() {
                mark_findings.complete = false;
            }
            mark.preserved = importer.settle(part, rpr, &ppr_scope, mark_findings);
            paragraph.mark_props = Arc::new(mark);
        }
        if let Some(sect) = para.section {
            section = Some((sect.clone(), ppr_scope.clone().into_owned()));
        }
        props.preserved = importer.settle(part, ppr, &inner, findings);
        paragraph.props = Arc::new(props);
    }
    let mut locked = false;
    content(importer, part, element, &inner, &mut paragraph, &mut locked);
    if locked {
        importer.editability.lock(paragraph.id());
    }
    (paragraph, section)
}

/// Keeps `element` and puts an inline object for it at the paragraph end.
pub(crate) fn inline(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    reason: PreservedReason,
    paragraph: &mut Paragraph,
    format: (Option<StyleId>, Arc<RunProps>),
) {
    let handle = importer.preserve(part, element, scope, reason);
    let object = InlineObject {
        id: importer.document.ids().node(),
        kind: InlineKind::Preserved(Preserved {
            element: Arc::from(&*element.qname),
            handle,
        }),
        source: Some(Importer::source(part, element)),
    };
    paragraph.push_inline(object, format.0, format.1);
}

/// Paragraph-level content of `parent` (a paragraph or an inline wrapper).
fn content(
    importer: &mut Importer<'_>,
    part: &PartName,
    parent: &Element,
    scope: &Scope,
    paragraph: &mut Paragraph,
    locked: &mut bool,
) {
    let children: Vec<&Element> = parent.elements().collect();
    let mut index = 0;
    while index < children.len() {
        if let Some((taken, field)) = complex_page_field(&children[index..]) {
            paragraph.push_inline(
                InlineObject {
                    id: importer.document.ids().node(),
                    kind: InlineKind::PageField(field),
                    source: Some(Importer::source(part, children[index])),
                },
                None,
                Arc::default(),
            );
            index += taken;
            continue;
        }
        let child = children[index];
        index += 1;
        let local = &*child.local;
        if child.namespace.as_deref() != Some(W) {
            if child.is(MC, "AlternateContent") {
                match crate::images::import_container(
                    importer,
                    part,
                    child,
                    scope,
                    paragraph,
                    Default::default(),
                ) {
                    crate::images::PictureImport::Imported => {}
                    crate::images::PictureImport::Kept => *locked = true,
                    crate::images::PictureImport::Pass => {
                        let reason = foreign(importer, part, child, scope);
                        inline(
                            importer,
                            part,
                            child,
                            scope,
                            reason,
                            paragraph,
                            Default::default(),
                        );
                        *locked = true;
                    }
                }
                continue;
            }
            let reason = foreign(importer, part, child, scope);
            inline(
                importer,
                part,
                child,
                scope,
                reason,
                paragraph,
                Default::default(),
            );
            *locked = true;
            continue;
        }
        match local {
            "pPr" => {}
            "r" => run(
                importer,
                part,
                child,
                &scope.enter(child),
                paragraph,
                locked,
            ),
            "hyperlink" => {
                if let Some(link) = crate::hyperlink::mapped(importer, part, child) {
                    let before = paragraph.runs().len();
                    let was_locked = *locked;
                    let inner = scope.enter(child);
                    for run_element in child.elements() {
                        run(
                            importer,
                            part,
                            run_element,
                            &inner.enter(run_element),
                            paragraph,
                            locked,
                        );
                    }
                    if *locked == was_locked && paragraph.runs().len() > before {
                        paragraph.stamp_runs_from(before, Arc::new(link));
                    } else {
                        // The preflight did not hold. Keep the wrapper bytes and
                        // do not read the children again.
                        importer.preserve(part, child, scope, PreservedReason::Unsupported);
                        *locked = true;
                    }
                } else {
                    importer.preserve(part, child, scope, PreservedReason::Unsupported);
                    *locked = true;
                    content(
                        importer,
                        part,
                        child,
                        &scope.enter(child),
                        paragraph,
                        locked,
                    );
                }
            }
            "proofErr" => importer.hint(part, "proofErr"),
            "fldSimple" => {
                if let Some(kind) = page_field_kind(child.attribute(Some(W), "instr").unwrap_or(""))
                {
                    paragraph.push_inline(
                        InlineObject {
                            id: importer.document.ids().node(),
                            kind: InlineKind::PageField(PageField {
                                kind,
                                cached: simple_cache(child),
                            }),
                            source: Some(Importer::source(part, child)),
                        },
                        None,
                        Arc::default(),
                    );
                } else {
                    // An unknown simple field stays preserved. Do not import its
                    // cached result as text that could be mistaken for a page number.
                    inline(
                        importer,
                        part,
                        child,
                        scope,
                        PreservedReason::Unsupported,
                        paragraph,
                        Default::default(),
                    );
                    *locked = true;
                }
            }
            "ins" | "del" | "moveFrom" | "moveTo" => {
                if !crate::revision::import_run_revision(importer, part, child, scope, paragraph) {
                    *locked = true;
                    if local == "ins" || local == "moveTo" {
                        // Outside the typed subset, inserted and move-to text
                        // still belongs in the imported projection.
                        importer.preserve(part, child, scope, PreservedReason::Revision);
                        content(
                            importer,
                            part,
                            child,
                            &scope.enter(child),
                            paragraph,
                            locked,
                        );
                    } else {
                        // Deleted and move-from text stays out of that
                        // projection. The wrapper is one preserved inline,
                        // not walked for `w:t`.
                        inline(
                            importer,
                            part,
                            child,
                            scope,
                            PreservedReason::Revision,
                            paragraph,
                            Default::default(),
                        );
                    }
                }
            }
            "moveFromRangeStart" | "moveFromRangeEnd" | "moveToRangeStart" | "moveToRangeEnd" => {
                if !crate::revision::import_move_marker(importer, part, child, paragraph) {
                    let reason = foreign(importer, part, child, scope);
                    importer.preserve(part, child, scope, reason);
                    *locked = true;
                }
            }
            _ if is_wrapper(local) => {
                let reason = if is_revision(local) {
                    PreservedReason::Revision
                } else {
                    PreservedReason::Unsupported
                };
                importer.preserve(part, child, scope, reason);
                *locked = true;
                content(
                    importer,
                    part,
                    child,
                    &scope.enter(child),
                    paragraph,
                    locked,
                );
            }
            "sdt" => {
                importer.preserve(part, child, scope, PreservedReason::Unsupported);
                *locked = true;
                let inner = scope.enter(child);
                if let Some(sdt_content) = child.child(W, "sdtContent") {
                    let content_scope = inner.enter(sdt_content);
                    content(
                        importer,
                        part,
                        sdt_content,
                        &content_scope,
                        paragraph,
                        locked,
                    );
                }
            }
            _ if is_marker(local) => {
                let reason = foreign(importer, part, child, scope);
                importer.preserve(part, child, scope, reason);
                *locked = true;
            }
            _ => {
                let reason = foreign(importer, part, child, scope);
                inline(
                    importer,
                    part,
                    child,
                    scope,
                    reason,
                    paragraph,
                    Default::default(),
                );
                *locked = true;
            }
        }
    }
}

/// `w:r` (§17.3.2.25) and its content (§17.3.3).
fn run(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    paragraph: &mut Paragraph,
    locked: &mut bool,
) {
    let (props, style) = match element.child(W, "rPr") {
        Some(rpr) => {
            let mut findings = Findings::default();
            let (mut props, style) = props::run(rpr, &mut findings);
            props.preserved = importer.settle(part, rpr, scope, findings);
            (props, style)
        }
        None => (RunProps::default(), None),
    };
    let props = Arc::new(props);
    let format = || (style.clone(), props.clone());
    for child in element.elements() {
        let mapped = child.namespace.as_deref() == Some(W);
        match (mapped, &*child.local) {
            (true, "rPr") => {}
            (true, "t") if child.elements().next().is_none() => {
                let text = child.text();
                // Every character is kept. Without xml:space="preserve",
                // ECMA-376 §17.3.3.31 defers to XML 1.0 §2.10, which leaves
                // edge white space to the application; that case is
                // reported until a Word/WPS reference settles it (T004).
                if child.attribute(Some(XML_NAMESPACE), "space") != Some("preserve")
                    && (text.starts_with(is_xml_space) || text.ends_with(is_xml_space))
                {
                    importer.diagnose(
                        Severity::Info,
                        Some(part),
                        Some(child.span.clone()),
                        DiagnosticKind::UnpreservedWhitespace,
                    );
                }
                if text.contains(OBJECT_MARKER) {
                    inline(
                        importer,
                        part,
                        child,
                        scope,
                        PreservedReason::ReservedText,
                        paragraph,
                        format(),
                    );
                    *locked = true;
                } else {
                    paragraph
                        .push_text(&text, style.clone(), props.clone())
                        .expect("text without the object marker");
                }
            }
            (true, "tab") if child.attributes.is_empty() => {
                paragraph
                    .push_text("\t", style.clone(), props.clone())
                    .expect("tab is ordinary text");
            }
            (true, "br" | "cr") => {
                let clear = child.attribute(Some(W), "clear").unwrap_or("none");
                let kind = match (&*child.local, child.attribute(Some(W), "type"), clear) {
                    ("cr", _, _) => Some(BreakKind::Line),
                    (_, None | Some("textWrapping"), "none") => Some(BreakKind::Line),
                    (_, Some("page"), "none") => Some(BreakKind::Page),
                    (_, Some("column"), "none") => Some(BreakKind::Column),
                    _ => None,
                };
                match kind {
                    Some(kind) => paragraph.push_inline(
                        InlineObject {
                            id: importer.document.ids().node(),
                            kind: InlineKind::Break(kind),
                            source: Some(Importer::source(part, child)),
                        },
                        style.clone(),
                        props.clone(),
                    ),
                    None => {
                        inline(
                            importer,
                            part,
                            child,
                            scope,
                            PreservedReason::Unsupported,
                            paragraph,
                            format(),
                        );
                        *locked = true;
                    }
                }
            }
            (true, "lastRenderedPageBreak") => importer.hint(part, "lastRenderedPageBreak"),
            (true, "drawing" | "pict" | "object") => {
                match crate::images::import_container(
                    importer,
                    part,
                    child,
                    scope,
                    paragraph,
                    format(),
                ) {
                    crate::images::PictureImport::Imported => {}
                    crate::images::PictureImport::Kept => *locked = true,
                    crate::images::PictureImport::Pass => {
                        let reason = foreign(importer, part, child, scope);
                        inline(importer, part, child, scope, reason, paragraph, format());
                        *locked = true;
                    }
                }
            }
            (false, _) if child.is(MC, "AlternateContent") => {
                match crate::images::import_container(
                    importer,
                    part,
                    child,
                    scope,
                    paragraph,
                    format(),
                ) {
                    crate::images::PictureImport::Imported => {}
                    crate::images::PictureImport::Kept => *locked = true,
                    crate::images::PictureImport::Pass => {
                        let reason = foreign(importer, part, child, scope);
                        inline(importer, part, child, scope, reason, paragraph, format());
                        *locked = true;
                    }
                }
            }
            _ => {
                let reason = foreign(importer, part, child, scope);
                inline(importer, part, child, scope, reason, paragraph, format());
                *locked = true;
            }
        }
    }
}

/// A `PAGE`, `NUMPAGES` or `SECTIONPAGES` instruction, optionally with
/// `\* MERGEFORMAT`. Any other switch stays on the preserved-field path.
fn page_field_kind(instr: &str) -> Option<PageFieldKind> {
    let mut tokens = instr.split_whitespace();
    let kind = match tokens.next()?.to_ascii_uppercase().as_str() {
        "PAGE" => PageFieldKind::Page,
        "NUMPAGES" => PageFieldKind::NumPages,
        "SECTIONPAGES" => PageFieldKind::SectionPages,
        _ => return None,
    };
    let rest: Vec<&str> = tokens.collect();
    if rest.is_empty()
        || (rest.len() == 2
            && rest[0].eq_ignore_ascii_case("\\*")
            && rest[1].eq_ignore_ascii_case("MERGEFORMAT"))
    {
        Some(kind)
    } else {
        None
    }
}

/// Cached result text that can be shown when a number cannot be formatted.
/// Tabs, nested markup and long or non-character text are not a cache.
fn cache_text(text: &str) -> Option<Arc<str>> {
    if text.is_empty() || text.chars().count() > 32 {
        return None;
    }
    if text.chars().any(|ch| {
        ch == OBJECT_MARKER
            || matches!(ch, '\u{0}'..='\u{8}' | '\u{B}' | '\u{C}' | '\u{E}'..='\u{1F}')
    }) {
        return None;
    }
    Some(Arc::from(text))
}

fn simple_cache(element: &Element) -> Option<Arc<str>> {
    let mut text = String::new();
    for run in element.elements() {
        if !run.is(W, "r") {
            return None;
        }
        for leaf in run.elements() {
            if leaf.is(W, "rPr") {
                continue;
            }
            if leaf.is(W, "t") && leaf.elements().next().is_none() {
                text.push_str(&leaf.text());
            } else {
                return None;
            }
        }
    }
    cache_text(&text)
}

fn fld_char_type(run: &Element) -> Option<&str> {
    let mut found = None;
    for child in run.elements() {
        if child.is(W, "rPr") {
            continue;
        }
        if found.is_some() || !child.is(W, "fldChar") || child.elements().next().is_some() {
            return None;
        }
        if child.attributes.len() != 1 {
            return None;
        }
        found = child.attribute(Some(W), "fldCharType");
        found?;
    }
    found
}

fn instr_text(run: &Element) -> Option<String> {
    let mut text = String::new();
    let mut any = false;
    for child in run.elements() {
        if child.is(W, "rPr") {
            continue;
        }
        if !child.is(W, "instrText") || child.elements().next().is_some() {
            return None;
        }
        text.push_str(&child.text());
        any = true;
    }
    any.then_some(text)
}

fn result_text(run: &Element) -> Option<String> {
    let mut text = String::new();
    for child in run.elements() {
        if child.is(W, "rPr") {
            continue;
        }
        if !child.is(W, "t") || child.elements().next().is_some() {
            return None;
        }
        text.push_str(&child.text());
    }
    Some(text)
}

/// A complex field whose instruction is only `PAGE`, `NUMPAGES` or
/// `SECTIONPAGES`, with an optional cached result between separate and end.
/// Anything else returns `None` so the existing per-run path preserves it.
fn complex_page_field(children: &[&Element]) -> Option<(usize, PageField)> {
    if children.is_empty() || fld_char_type(children[0]) != Some("begin") {
        return None;
    }
    let mut index = 1;
    let mut instr = String::new();
    while index < children.len() {
        let kind = fld_char_type(children[index]);
        if kind == Some("separate") || kind == Some("end") {
            break;
        }
        instr.push_str(&instr_text(children[index])?);
        index += 1;
    }
    let mut cached = String::new();
    let mut saw_result = false;
    if index < children.len() && fld_char_type(children[index]) == Some("separate") {
        saw_result = true;
        index += 1;
        while index < children.len() && fld_char_type(children[index]) != Some("end") {
            cached.push_str(&result_text(children[index])?);
            index += 1;
        }
    }
    if index >= children.len() || fld_char_type(children[index]) != Some("end") {
        return None;
    }
    let kind = page_field_kind(&instr)?;
    Some((
        index + 1,
        PageField {
            kind,
            cached: saw_result.then(|| cache_text(&cached)).flatten(),
        },
    ))
}

/// `w:sectPr` (§17.6.17): page size, margins, title page, header or footer
/// references, section start, page numbering and equal columns. Other
/// children are preserved, not imported.
fn section(
    importer: &mut Importer<'_>,
    part: &PartName,
    element: &Element,
    scope: &Scope,
    last_block: Option<NodeId>,
) -> Section {
    let defaults = PageSetup::a4_zh_cn();
    let mut page = defaults.clone();
    let mut findings = Findings::default();
    let mut title_page = None;
    let mut size = false;
    let mut margins = false;
    let mut headers = Vec::new();
    let mut footers = Vec::new();
    let mut break_kind = SectionBreak::NextPage;
    let mut numbering = PageNumbering::default();
    let mut columns = None;
    for child in element.elements() {
        let mapped = child.namespace.as_deref() == Some(W);
        match (mapped, &*child.local) {
            (true, "pgSz") => {
                let width = props::twips(child, "w", &mut findings);
                let height = props::twips(child, "h", &mut findings);
                size = width.is_some() && height.is_some();
                page.width = width.unwrap_or(defaults.width);
                page.height = height.unwrap_or(defaults.height);
                page.orientation = match child.attribute(Some(W), "orient") {
                    Some("landscape") => Orientation::Landscape,
                    None | Some("portrait") => Orientation::Portrait,
                    Some(other) => {
                        findings.invalid(child, "w:orient", other);
                        Orientation::Portrait
                    }
                };
                if child.attribute(Some(W), "code").is_some() {
                    findings.complete = false;
                }
            }
            (true, "pgMar") => {
                margins = true;
                for (name, slot) in [
                    ("top", &mut page.top),
                    ("right", &mut page.right),
                    ("bottom", &mut page.bottom),
                    ("left", &mut page.left),
                    ("header", &mut page.header),
                    ("footer", &mut page.footer),
                    ("gutter", &mut page.gutter),
                ] {
                    match props::twips(child, name, &mut findings) {
                        Some(value) => *slot = value,
                        None => margins = false,
                    }
                }
            }
            (true, "titlePg") => title_page = props::on_off(props::val(child)),
            (true, "type") => {
                if child.elements().next().is_some()
                    || child
                        .attributes
                        .iter()
                        .any(|attribute| attribute.local.as_ref() != "val")
                {
                    findings.complete = false;
                }
                match child.attribute(Some(W), "val") {
                    None | Some("nextPage") => break_kind = SectionBreak::NextPage,
                    Some("continuous") => break_kind = SectionBreak::Continuous,
                    Some("evenPage") => break_kind = SectionBreak::EvenPage,
                    Some("oddPage") => break_kind = SectionBreak::OddPage,
                    Some(_) => findings.complete = false,
                }
            }
            (true, "pgNumType") => {
                if let Some(parsed) = page_numbering(child) {
                    numbering = parsed;
                } else {
                    findings.complete = false;
                }
            }
            (true, "cols") => match equal_columns(child) {
                Some(parsed) => columns = parsed,
                None => findings.complete = false,
            },
            (true, "headerReference" | "footerReference") => {
                if let Some((kind, story)) = story_reference(importer, part, child, &mut findings) {
                    let list = if child.local.as_ref() == "headerReference" {
                        &mut headers
                    } else {
                        &mut footers
                    };
                    if list.iter().any(|(existing, _)| *existing == kind) {
                        importer.diagnose(
                            Severity::Warning,
                            Some(part),
                            Some(child.span.clone()),
                            DiagnosticKind::InvalidValue {
                                element: child.qname.to_string(),
                                attribute: "type".into(),
                                value: "duplicate".into(),
                            },
                        );
                    } else {
                        list.push((kind, story));
                    }
                }
            }
            _ => findings.complete = false,
        }
    }
    for (present, missing) in [(size, "w:pgSz"), (margins, "w:pgMar")] {
        if !present {
            importer.diagnose(
                Severity::Warning,
                Some(part),
                Some(element.span.clone()),
                DiagnosticKind::SectionDefaults { missing },
            );
        }
    }
    let preserved = importer.settle(part, element, scope, findings);
    Section {
        id: importer.document.ids().node(),
        last_block,
        page,
        title_page,
        headers,
        footers,
        break_kind,
        numbering,
        columns,
        preserved,
    }
}

/// `w:pgNumType` (§17.6.12). Chapter prefixes stay unmodeled.
fn page_numbering(element: &Element) -> Option<PageNumbering> {
    if element.elements().next().is_some() {
        return None;
    }
    let mut numbering = PageNumbering::default();
    for attribute in &element.attributes {
        if attribute.namespace.as_deref() != Some(W) {
            return None;
        }
        match attribute.local.as_ref() {
            "fmt" => numbering.format = NumberFormat::parse(&attribute.value),
            "start" => numbering.start = Some(attribute.value.parse().ok()?),
            _ => return None,
        }
    }
    Some(numbering)
}

/// Equal-width `w:cols` (§17.6.4). `None` means the element is not in this
/// slice (unequal columns, a separator, or a bad value), so the section is
/// preserved. `Some(None)` is a single column.
fn equal_columns(element: &Element) -> Option<Option<EqualColumns>> {
    if element.elements().next().is_some() {
        return None;
    }
    let mut count = 1u16;
    let mut space = 720i64;
    let mut saw_space = false;
    for attribute in &element.attributes {
        if attribute.namespace.as_deref() != Some(W) {
            return None;
        }
        match attribute.local.as_ref() {
            "num" => {
                let parsed: u16 = attribute.value.parse().ok()?;
                if !(1..=45).contains(&parsed) {
                    return None;
                }
                count = parsed;
            }
            "space" => {
                let parsed: i64 = attribute.value.parse().ok()?;
                if parsed < 0 {
                    return None;
                }
                space = parsed;
                saw_space = true;
            }
            "equalWidth" => {
                if props::on_off(Some(&attribute.value)) != Some(true) {
                    return None;
                }
            }
            "sep" => {
                if props::on_off(Some(&attribute.value)) != Some(false) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    if count <= 1 {
        let _ = saw_space;
        return Some(None);
    }
    Some(Some(EqualColumns { count, space }))
}

fn story_reference(
    importer: &mut Importer<'_>,
    part: &PartName,
    child: &Element,
    findings: &mut Findings,
) -> Option<(HeaderFooterKind, StoryId)> {
    let kind = match child.attribute(Some(W), "type") {
        Some("default") => HeaderFooterKind::Default,
        Some("first") => HeaderFooterKind::First,
        Some("even") => HeaderFooterKind::Even,
        Some(other) => {
            findings.invalid(child, "w:type", other);
            return None;
        }
        None => {
            findings.complete = false;
            return None;
        }
    };
    let Some(id) = child.attribute(Some(R), "id") else {
        findings.complete = false;
        return None;
    };
    let target = {
        let package = importer.backing.package();
        package
            .relationships(Some(part))
            .iter()
            .find(|relationship| relationship.id.as_ref() == id)
            .and_then(|relationship| match &relationship.target {
                Target::Internal(name) => Some(name.clone()),
                _ => None,
            })
    };
    let Some(target) = target else {
        findings.complete = false;
        importer.diagnose(
            Severity::Warning,
            Some(part),
            Some(child.span.clone()),
            DiagnosticKind::MissingPart {
                relationship: id.to_owned(),
                part: String::new(),
            },
        );
        return None;
    };
    let header = child.local.as_ref() == "headerReference";
    match import_story(importer, &target, header) {
        Some(story) => Some((kind, story)),
        None => {
            findings.complete = false;
            None
        }
    }
}

fn import_story(importer: &mut Importer<'_>, name: &PartName, header: bool) -> Option<StoryId> {
    if let Some((_, id)) = importer
        .imported_stories
        .iter()
        .find(|(part, _)| part == name)
    {
        return Some(*id);
    }
    let part = importer.backing.package().part(name.as_str()).cloned()?;
    let expected = if header { "hdr" } else { "ftr" };
    let element = XmlReader::new(part.name.as_str(), &part.data, importer.budget)
        .and_then(XmlReader::whole)
        .ok()?;
    if !element.is(W, expected) {
        return None;
    }
    importer.backing.interpret(part.name.clone());
    let scope = Scope::default();
    importer.scan(&part.name, &element, &scope);
    let inner = scope.enter(&element).into_owned();
    let mut blocks = Vec::new();
    for child in element.elements() {
        if child.is(W, "p") {
            let (paragraph, _) = paragraph(importer, &part.name, child, &inner);
            blocks.push(paragraph.into());
        } else {
            importer.preserve(&part.name, child, &inner, PreservedReason::Unsupported);
        }
    }
    if blocks.is_empty() {
        // The part is the story's identity. A later text edit writes into this
        // root instead of inventing another header or footer part.
        let mut paragraph = Paragraph::new(importer.document.ids().node());
        paragraph.source = Some(Importer::source(&part.name, &element));
        blocks.push(paragraph.into());
    }
    let kind = if header {
        StoryKind::Header
    } else {
        StoryKind::Footer
    };
    let id = importer.document.insert_story(kind, blocks);
    importer.imported_stories.push((part.name, id));
    Some(id)
}
