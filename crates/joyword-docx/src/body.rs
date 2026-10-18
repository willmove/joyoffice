//! The main document body (ECMA-376 Part 1 §17.2 document and body,
//! §17.3.1 paragraphs, §17.3.2–17.3.3 runs and run content, §17.6 sections).
//!
//! The body is read one top-level block at a time. Paragraphs are mapped;
//! every other block (tables, content controls, unknown elements) becomes an
//! [`OpaqueBlock`] whose source is preserved. Inside a paragraph, text,
//! tabs and breaks are mapped; other run content becomes a preserved inline
//! object, wrappers such as hyperlinks are descended into for their text and
//! preserved, and range markers such as bookmarks are preserved without a
//! position. A paragraph or block holding preserved content is locked.

use std::sync::Arc;

use joyword_model::{
    Block, BlockSeq, BreakKind, InlineKind, InlineObject, NodeId, OBJECT_MARKER, OpaqueBlock,
    Orientation, PageSetup, Paragraph, Preserved, RunProps, Section, StyleId,
};

use crate::backing::PreservedReason;
use crate::import::{Importer, Scope, xml_error};
use crate::ns::{KNOWN_OFFICE, MC, W, W_STRICT, is_revision};
use crate::props::{self, Findings};
use crate::result::{DiagnosticKind, ImportError, ReadOnlyReason, Severity};
use crate::xml::{Element, Step, XML_NAMESPACE, XmlReader, is_xml_space};
use joyoffice_opc::PartName;

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
            | "moveFromRangeStart"
            | "moveFromRangeEnd"
            | "moveToRangeStart"
            | "moveToRangeEnd"
    ) || (local.starts_with("customXml")
        && (local.ends_with("RangeStart") || local.ends_with("RangeEnd")))
}

/// Inline wrappers whose run content is mapped while the wrapper itself is
/// preserved: hyperlinks, smart tags, inline custom XML, simple fields,
/// bidirectional embeddings, and inserted or moved-to revisions.
fn is_wrapper(local: &str) -> bool {
    matches!(
        local,
        "hyperlink" | "smartTag" | "customXml" | "fldSimple" | "dir" | "bdo" | "ins" | "moveTo"
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
fn inline(
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
    for child in parent.elements() {
        let local = &*child.local;
        if child.namespace.as_deref() != Some(W) {
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
            "proofErr" => importer.hint(part, "proofErr"),
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
            _ => {
                let reason = foreign(importer, part, child, scope);
                inline(importer, part, child, scope, reason, paragraph, format());
                *locked = true;
            }
        }
    }
}

/// `w:sectPr` (§17.6.17): page size, margins and title page. Header and
/// footer references and every other child are preserved, not imported.
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
        headers: Vec::new(),
        footers: Vec::new(),
        preserved,
    }
}
