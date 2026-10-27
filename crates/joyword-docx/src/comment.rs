//! Comment import and conservative export (ECMA-376 Part 1 §17.13.4;
//! [MS-DOCX] commentsExtended, commentsIds and commentsExtensible).
//!
//! Existing `w:comment` elements are not reserialized as a whole. Resolve
//! patches `w15:done` in place. A reply or a new root comment is spliced
//! before the root end tag. Text leaves of an edited comment paragraph are
//! patched in place; bookmark markers in that part are spliced by the
//! bookmark writer. Unknown attributes and non-paragraph children stay in
//! the original bytes, and the model keeps a copy so a rewrite that dropped
//! them would fail the semantic comparison.
//!
//! Range markers use `w:commentRangeStart` / `w:commentRangeEnd`. A reply
//! normally has no markers; its anchor is the parent's. People.xml is not
//! imported. This slice does not claim a Word or WPS exchange.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use joyoffice_opc::{Budget, Package, Part, PartName, Target};
use joyword_model::{
    Anchor, AnnotationId, AnnotationKind, Bias, Block, Comment, CommentAttribute, Document, NodeId,
    Paragraph, Position, ScalarOffset, StoryId, StoryKind,
};

use crate::body::paragraph;
use crate::export::{ExportError, unsupported};
use crate::import::{Importer, Scope, xml_error};
use crate::ns::{
    REL_COMMENTS, REL_COMMENTS_EXTENDED, REL_COMMENTS_EXTENSIBLE, REL_COMMENTS_IDS, W, W14, W15,
    W16CEX, W16CID,
};
use crate::result::{DiagnosticKind, ImportError, Severity};
use crate::xml::{Element, XmlReader};

/// A range marker seen while reading a story paragraph.
#[derive(Clone, Debug)]
pub(crate) struct CommentMark {
    pub source_id: u32,
    pub end: bool,
    pub paragraph: NodeId,
    pub offset: usize,
}

struct Draft {
    source_id: u32,
    author: Option<Arc<str>>,
    date: Option<Arc<str>>,
    initials: Option<Arc<str>>,
    para_id: Option<Arc<str>>,
    parent_para_id: Option<Arc<str>>,
    resolved: bool,
    durable_id: Option<Arc<str>>,
    date_utc: Option<Arc<str>>,
    extra_attributes: Vec<CommentAttribute>,
    extra_markup: Option<Arc<str>>,
    body: StoryId,
    anchor: Option<Anchor>,
}

pub(crate) fn record_marker(
    importer: &mut Importer<'_>,
    element: &Element,
    paragraph: &Paragraph,
) -> bool {
    let end = match element.local.as_ref() {
        "commentRangeStart" => false,
        "commentRangeEnd" => true,
        _ => return false,
    };
    if element.namespace.as_deref() != Some(W) {
        return false;
    }
    if !importer.record_comment_anchors {
        return true;
    }
    let Some(source_id) = comment_id(element) else {
        return false;
    };
    importer.comment_marks.push(CommentMark {
        source_id,
        end,
        paragraph: paragraph.id(),
        offset: paragraph.len(),
    });
    true
}

/// A well-formed `w:commentReference` occupies no text position.
pub(crate) fn skip_reference(element: &Element) -> bool {
    element.is(W, "commentReference") && comment_id(element).is_some()
}

/// Comment ids inside an opaque table stay detached. The table bytes are not
/// edited, including when a later export strips body markers.
pub(crate) fn note_unmapped(importer: &mut Importer<'_>, element: &Element) {
    if !importer.record_comment_anchors {
        return;
    }
    fn walk(importer: &mut Importer<'_>, element: &Element) {
        if matches!(
            element.local.as_ref(),
            "commentRangeStart" | "commentRangeEnd" | "commentReference"
        ) && element.namespace.as_deref() == Some(W)
            && let Some(id) = comment_id(element)
        {
            importer.unmapped_comment_ids.insert(id);
        }
        for child in element.elements() {
            walk(importer, child);
        }
    }
    walk(importer, element);
}

pub(crate) fn has_markup(element: &Element) -> bool {
    if element.is(W, "commentRangeStart")
        || element.is(W, "commentRangeEnd")
        || element.is(W, "commentReference")
    {
        return true;
    }
    element.elements().any(has_markup)
}

impl Importer<'_> {
    pub(crate) fn import_comments(&mut self, package: &Package) -> Result<(), ImportError> {
        let main = self.backing.main_part().clone();
        let Some(part) = related(package, &main, REL_COMMENTS).cloned() else {
            return Ok(());
        };
        if package.part(part.name.as_str()).is_none() {
            return Ok(());
        }
        let bytes = part.data.clone();
        let root = XmlReader::new(part.name.as_str(), &bytes, self.budget)
            .and_then(XmlReader::whole)
            .map_err(xml_error(&part.name))?;
        self.backing.interpret(part.name.clone());
        let scope = Scope::default();
        self.scan(&part.name, &root, &scope);
        if !root.is(W, "comments") {
            self.warn(&part.name, &root, DiagnosticKind::CommentPartInvalid);
            return Ok(());
        }
        let inner = scope.enter(&root);
        let mut drafts = Vec::new();
        let mut seen = BTreeSet::new();
        for child in root.elements() {
            if !child.is(W, "comment") {
                continue;
            }
            let Some(source_id) = comment_id(child) else {
                self.warn(&part.name, child, DiagnosticKind::CommentIdMissing);
                continue;
            };
            self.document.note_comment_id(source_id);
            if !seen.insert(source_id) {
                self.warn(
                    &part.name,
                    child,
                    DiagnosticKind::CommentIdDuplicate { id: source_id },
                );
                continue;
            }
            drafts.push(self.read_comment(&part.name, child, &inner, &bytes, source_id));
        }
        if let Some(extended) = related(package, &main, REL_COMMENTS_EXTENDED)
            .or_else(|| related(package, &part.name, REL_COMMENTS_EXTENDED))
            .cloned()
        {
            self.read_extended(&extended, &mut drafts)?;
        }
        if let Some(ids) = related(package, &main, REL_COMMENTS_IDS)
            .or_else(|| related(package, &part.name, REL_COMMENTS_IDS))
            .cloned()
        {
            self.read_ids(&ids, &mut drafts)?;
        }
        if let Some(extensible) = related(package, &main, REL_COMMENTS_EXTENSIBLE)
            .or_else(|| related(package, &part.name, REL_COMMENTS_EXTENSIBLE))
            .cloned()
        {
            self.read_extensible(&extensible, &mut drafts)?;
        }
        self.bind_anchors(&part.name, &mut drafts);
        for draft in drafts {
            let mut comment = Comment::new(draft.body);
            comment.source_id = draft.source_id;
            comment.author = draft.author;
            comment.date = draft.date;
            comment.initials = draft.initials;
            comment.para_id = draft.para_id;
            comment.parent_para_id = draft.parent_para_id;
            comment.resolved = draft.resolved;
            comment.durable_id = draft.durable_id;
            comment.date_utc = draft.date_utc;
            comment.extra_attributes = Arc::from(draft.extra_attributes);
            comment.extra_markup = draft.extra_markup;
            let anchor = draft.anchor.unwrap_or(Anchor::Detached);
            self.document
                .insert_annotation(AnnotationKind::Comment(comment), anchor);
        }
        Ok(())
    }

    fn read_comment(
        &mut self,
        part: &PartName,
        element: &Element,
        scope: &Scope,
        bytes: &[u8],
        source_id: u32,
    ) -> Draft {
        let author = element
            .attribute(Some(W), "author")
            .filter(|value| !value.is_empty())
            .map(Arc::<str>::from);
        if author.is_none() {
            self.warn(
                part,
                element,
                DiagnosticKind::CommentAuthorMissing { id: source_id },
            );
        }
        let date = optional_text(element, "date");
        let initials = optional_text(element, "initials");
        let mut extra_attributes = Vec::new();
        for attribute in &element.attributes {
            let local = attribute.local.as_ref();
            if attribute.namespace.as_deref() == Some(W)
                && matches!(local, "id" | "author" | "date" | "initials")
            {
                continue;
            }
            extra_attributes.push(CommentAttribute {
                namespace: attribute.namespace.clone(),
                local: Arc::from(local),
                value: Arc::from(attribute.value.as_str()),
            });
        }
        self.record_comment_anchors = false;
        let mut blocks = Vec::new();
        let mut markup = String::new();
        let mut para_id = None;
        let mut para_invalid = false;
        let inner = scope.enter(element);
        for child in element.elements() {
            if child.is(W, "p") {
                match paragraph_para_id(child) {
                    Para::Absent => {}
                    Para::Invalid => para_invalid = true,
                    Para::Value(value) => {
                        if para_id.is_none() {
                            self.document.note_para_id(&value);
                            para_id = Some(value);
                        }
                    }
                }
                let (paragraph, _) = paragraph(self, part, child, &inner);
                blocks.push(paragraph.into());
            } else if let Ok(text) = std::str::from_utf8(&bytes[child.span.clone()]) {
                markup.push_str(text);
            } else {
                para_invalid = true;
            }
        }
        self.record_comment_anchors = true;
        if para_invalid {
            self.warn(part, element, DiagnosticKind::CommentPartInvalid);
        }
        if blocks.is_empty() {
            self.warn(
                part,
                element,
                DiagnosticKind::CommentBodyEmpty { id: source_id },
            );
            blocks.push(Paragraph::new(self.document.ids().node()).into());
        }
        let body = self.document.insert_story(StoryKind::Comment, blocks);
        Draft {
            source_id,
            author,
            date,
            initials,
            para_id,
            parent_para_id: None,
            resolved: false,
            durable_id: None,
            date_utc: None,
            extra_attributes,
            extra_markup: (!markup.is_empty()).then(|| Arc::from(markup)),
            body,
            anchor: None,
        }
    }

    fn read_extended(&mut self, part: &Part, drafts: &mut [Draft]) -> Result<(), ImportError> {
        let root = self.extension_root(part, W15, "commentsEx")?;
        let Some(root) = root else {
            return Ok(());
        };
        let mut seen = BTreeSet::new();
        for child in root.elements() {
            if child.local.as_ref() != "commentEx" || child.namespace.as_deref() != Some(W15) {
                continue;
            }
            let Some(raw) = child.attribute(Some(W15), "paraId") else {
                self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                continue;
            };
            if !is_para_id(raw) {
                self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                continue;
            }
            let para_id = raw.to_ascii_uppercase();
            self.document.note_para_id(&para_id);
            if !seen.insert(para_id.clone()) {
                self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                continue;
            }
            let Some(draft) = drafts
                .iter_mut()
                .find(|draft| draft.para_id.as_deref() == Some(para_id.as_str()))
            else {
                self.warn(
                    &part.name,
                    child,
                    DiagnosticKind::CommentExtensionUnmatched { para_id },
                );
                continue;
            };
            if let Some(parent) = child.attribute(Some(W15), "paraIdParent") {
                if is_para_id(parent) {
                    let parent = parent.to_ascii_uppercase();
                    self.document.note_para_id(&parent);
                    draft.parent_para_id = Some(Arc::from(parent));
                } else {
                    self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                }
            }
            match on_off_false(child.attribute(Some(W15), "done")) {
                Ok(resolved) => draft.resolved = resolved,
                Err(()) => {
                    draft.resolved = false;
                    self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                }
            }
            for attribute in &child.attributes {
                let local = attribute.local.as_ref();
                if attribute.namespace.as_deref() == Some(W15)
                    && matches!(local, "paraId" | "paraIdParent" | "done")
                {
                    continue;
                }
                draft.extra_attributes.push(CommentAttribute {
                    namespace: attribute.namespace.clone(),
                    local: Arc::from(local),
                    value: Arc::from(attribute.value.as_str()),
                });
            }
        }
        Ok(())
    }

    fn read_ids(&mut self, part: &Part, drafts: &mut [Draft]) -> Result<(), ImportError> {
        let root = self.extension_root(part, W16CID, "commentsIds")?;
        let Some(root) = root else {
            return Ok(());
        };
        for child in root.elements() {
            if child.local.as_ref() != "commentId" || child.namespace.as_deref() != Some(W16CID) {
                continue;
            }
            let (Some(para), Some(durable)) = (
                child.attribute(Some(W16CID), "paraId"),
                child.attribute(Some(W16CID), "durableId"),
            ) else {
                self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                continue;
            };
            if !is_para_id(para) || durable.is_empty() {
                self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                continue;
            }
            let para = para.to_ascii_uppercase();
            if let Some(draft) = drafts
                .iter_mut()
                .find(|draft| draft.para_id.as_deref() == Some(para.as_str()))
                && draft.durable_id.is_none()
            {
                draft.durable_id = Some(Arc::from(durable));
            }
        }
        Ok(())
    }

    fn read_extensible(&mut self, part: &Part, drafts: &mut [Draft]) -> Result<(), ImportError> {
        let root = self.extension_root(part, W16CEX, "commentsExtensible")?;
        let Some(root) = root else {
            return Ok(());
        };
        for child in root.elements() {
            if child.local.as_ref() != "commentExtensible"
                || child.namespace.as_deref() != Some(W16CEX)
            {
                continue;
            }
            let Some(durable) = child.attribute(Some(W16CEX), "durableId") else {
                self.warn(&part.name, child, DiagnosticKind::CommentPartInvalid);
                continue;
            };
            let Some(draft) = drafts
                .iter_mut()
                .find(|draft| draft.durable_id.as_deref() == Some(durable))
            else {
                continue;
            };
            if let Some(date) = child
                .attribute(Some(W16CEX), "dateUtc")
                .filter(|value| !value.is_empty())
            {
                draft.date_utc = Some(Arc::from(date));
            }
        }
        Ok(())
    }

    /// `None` when the root element is the wrong name. XML failures still
    /// refuse the package.
    fn extension_root(
        &mut self,
        part: &Part,
        namespace: &str,
        local: &str,
    ) -> Result<Option<Element>, ImportError> {
        let root = XmlReader::new(part.name.as_str(), &part.data, self.budget)
            .and_then(XmlReader::whole)
            .map_err(xml_error(&part.name))?;
        self.backing.interpret(part.name.clone());
        let scope = Scope::default();
        self.scan(&part.name, &root, &scope);
        if root.namespace.as_deref() == Some(namespace) && root.local.as_ref() == local {
            Ok(Some(root))
        } else {
            self.warn(&part.name, &root, DiagnosticKind::CommentPartInvalid);
            Ok(None)
        }
    }

    fn bind_anchors(&mut self, part: &PartName, drafts: &mut [Draft]) {
        for draft in drafts.iter_mut() {
            draft.anchor =
                Some(self.anchor_for(part, draft.source_id, draft.parent_para_id.is_some()));
        }
        let mut guard = 0usize;
        loop {
            guard += 1;
            let known: BTreeMap<Arc<str>, Anchor> = drafts
                .iter()
                .filter_map(|draft| {
                    Some((
                        draft.para_id.clone()?,
                        draft.anchor.clone().unwrap_or(Anchor::Detached),
                    ))
                })
                .collect();
            let parents: BTreeSet<Arc<str>> = drafts
                .iter()
                .filter_map(|draft| draft.para_id.clone())
                .collect();
            let mut progress = false;
            for draft in drafts.iter_mut() {
                if draft.parent_para_id.is_none() {
                    continue;
                }
                let Some(parent) = draft.parent_para_id.clone() else {
                    continue;
                };
                let own_marks = self
                    .comment_marks
                    .iter()
                    .any(|mark| mark.source_id == draft.source_id);
                if own_marks {
                    continue;
                }
                if draft.para_id.as_ref() == Some(&parent) || !parents.contains(&parent) {
                    if draft.anchor.as_ref() != Some(&Anchor::Detached) {
                        draft.anchor = Some(Anchor::Detached);
                    }
                    continue;
                }
                if let Some(anchor) = known.get(&parent)
                    && draft.anchor.as_ref() != Some(anchor)
                {
                    draft.anchor = Some(anchor.clone());
                    progress = true;
                }
            }
            if !progress || guard > drafts.len().saturating_add(1) {
                break;
            }
        }
        for index in 0..drafts.len() {
            let Some(parent) = drafts[index].parent_para_id.clone() else {
                continue;
            };
            let source_id = drafts[index].source_id;
            let own_marks = self
                .comment_marks
                .iter()
                .any(|mark| mark.source_id == source_id);
            if own_marks {
                continue;
            }
            let parent_known = drafts
                .iter()
                .any(|other| other.para_id.as_ref() == Some(&parent));
            let broken = drafts[index].para_id.as_ref() == Some(&parent)
                || drafts[index].anchor.is_none()
                || reply_cycle(drafts, index)
                || (drafts[index].anchor.as_ref() == Some(&Anchor::Detached)
                    && drafts.iter().any(|other| {
                        other.para_id.as_ref() == Some(&parent)
                            && other.anchor.as_ref() != Some(&Anchor::Detached)
                    }));
            if !parent_known {
                self.diagnose(
                    Severity::Warning,
                    Some(part),
                    None,
                    DiagnosticKind::OrphanReply {
                        para_id: parent.to_string(),
                    },
                );
                drafts[index].anchor = Some(Anchor::Detached);
            } else if broken {
                self.diagnose(
                    Severity::Warning,
                    Some(part),
                    None,
                    DiagnosticKind::CommentAnchorInvalid { id: source_id },
                );
                drafts[index].anchor = Some(Anchor::Detached);
            }
        }
    }

    fn anchor_for(&mut self, part: &PartName, source_id: u32, reply: bool) -> Anchor {
        let marks: Vec<CommentMark> = self
            .comment_marks
            .iter()
            .filter(|mark| mark.source_id == source_id)
            .cloned()
            .collect();
        let unmapped = self.unmapped_comment_ids.contains(&source_id);
        if marks.is_empty() {
            if unmapped {
                self.diagnose(
                    Severity::Warning,
                    Some(part),
                    None,
                    DiagnosticKind::CommentAnchorUnmapped { id: source_id },
                );
            } else if !reply {
                self.diagnose(
                    Severity::Warning,
                    Some(part),
                    None,
                    DiagnosticKind::CommentAnchorMissing { id: source_id },
                );
            }
            return Anchor::Detached;
        }
        if unmapped {
            self.diagnose(
                Severity::Warning,
                Some(part),
                None,
                DiagnosticKind::CommentAnchorInvalid { id: source_id },
            );
            return Anchor::Detached;
        }
        let starts: Vec<_> = marks.iter().filter(|mark| !mark.end).cloned().collect();
        let ends: Vec<_> = marks.iter().filter(|mark| mark.end).cloned().collect();
        let Some(start) = starts.first() else {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        };
        let Some(end) = ends.first() else {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        };
        if starts.len() != 1 || ends.len() != 1 {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        }
        let Some(start_story) = story_of(&self.document, start.paragraph) else {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        };
        let Some(end_story) = story_of(&self.document, end.paragraph) else {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        };
        if start_story != end_story {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        }
        let Some(start_index) = paragraph_index(&self.document, start_story, start.paragraph)
        else {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        };
        let Some(end_index) = paragraph_index(&self.document, end_story, end.paragraph) else {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        };
        let ordered = if start.paragraph == end.paragraph {
            start.offset <= end.offset
        } else {
            start_index < end_index
        };
        if !ordered {
            self.invalid_anchor(part, source_id);
            return Anchor::Detached;
        }
        Anchor::Range {
            start: Position {
                story: start_story,
                paragraph: start.paragraph,
                offset: ScalarOffset(start.offset),
                bias: Bias::Before,
            },
            end: Position {
                story: end_story,
                paragraph: end.paragraph,
                offset: ScalarOffset(end.offset),
                bias: Bias::After,
            },
        }
    }

    fn invalid_anchor(&mut self, part: &PartName, source_id: u32) {
        self.diagnose(
            Severity::Warning,
            Some(part),
            None,
            DiagnosticKind::CommentAnchorInvalid { id: source_id },
        );
    }

    fn warn(&mut self, part: &PartName, element: &Element, kind: DiagnosticKind) {
        self.diagnose(
            Severity::Warning,
            Some(part),
            Some(element.span.clone()),
            kind,
        );
    }
}

fn optional_text(element: &Element, local: &str) -> Option<Arc<str>> {
    element
        .attribute(Some(W), local)
        .filter(|value| !value.is_empty())
        .map(Arc::from)
}

enum Para {
    Absent,
    Invalid,
    Value(Arc<str>),
}

fn paragraph_para_id(element: &Element) -> Para {
    let Some(raw) = element.attribute(Some(W14), "paraId") else {
        return Para::Absent;
    };
    if is_para_id(raw) {
        Para::Value(Arc::from(raw.to_ascii_uppercase()))
    } else {
        Para::Invalid
    }
}

fn is_para_id(raw: &str) -> bool {
    raw.len() == 8 && raw.chars().all(|ch| ch.is_ascii_hexdigit())
}

/// Missing `w15:done` is not resolved. `props::on_off` treats a missing
/// attribute as true, which is the opposite of this flag.
fn on_off_false(value: Option<&str>) -> Result<bool, ()> {
    let Some(value) = value else {
        return Ok(false);
    };
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "on" => Ok(true),
        "0" | "false" | "off" => Ok(false),
        _ => Err(()),
    }
}

fn comment_id(element: &Element) -> Option<u32> {
    let raw = element.attribute(Some(W), "id")?;
    if raw.is_empty() || raw.starts_with('+') || raw.starts_with('-') {
        return None;
    }
    raw.parse().ok()
}

fn related<'a>(package: &'a Package, source: &PartName, kind: &str) -> Option<&'a Part> {
    for relationship in package.relationships(Some(source)) {
        if relationship.kind.as_ref() != kind {
            continue;
        }
        let Target::Internal(target) = &relationship.target else {
            continue;
        };
        if let Some(part) = package.part(target.as_str()) {
            return Some(part);
        }
    }
    None
}

fn reply_cycle(drafts: &[Draft], start: usize) -> bool {
    let mut seen = BTreeSet::new();
    let mut current = Some(start);
    while let Some(index) = current {
        if !seen.insert(index) {
            return true;
        }
        let Some(parent) = drafts[index].parent_para_id.clone() else {
            return false;
        };
        current = drafts
            .iter()
            .position(|draft| draft.para_id.as_ref() == Some(&parent));
    }
    false
}

fn story_of(document: &Document, paragraph: NodeId) -> Option<StoryId> {
    document.stories().find_map(|story| {
        document
            .find_paragraph(story.id, paragraph)
            .map(|_| story.id)
    })
}

fn paragraph_index(document: &Document, story: StoryId, paragraph: NodeId) -> Option<usize> {
    let story = document.story(story)?;
    let mut index = 0usize;
    let mut found = None;
    walk_index(&story.blocks, paragraph, &mut index, &mut found, 0);
    found
}

fn walk_index(
    blocks: &joyword_model::BlockSeq,
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
                    walk_index(&cell.blocks, needle, index, found, depth + 1);
                }
            }
            Block::Sdt(sdt) => walk_index(&sdt.blocks, needle, index, found, depth + 1),
            Block::Opaque(_) => {}
        }
    }
}

/// Comment anchors and resolved flags may change. Identity fields may not.
/// New annotations must be comments with new comment stories. An imported
/// bookmark keeps its name. Its anchor may move to another representable
/// range in the part it was imported from. Removing, renaming, detaching,
/// or pointing it at a paragraph in a different part is refused.
pub(crate) fn envelope(before: &Document, after: &Document) -> Result<(), &'static str> {
    for annotation in before.annotations() {
        let Some(other) = after.annotation(annotation.id) else {
            return Err(match &annotation.kind {
                AnnotationKind::Bookmark { .. } => "a bookmark was removed",
                AnnotationKind::Comment(_) => "a comment was removed",
                _ => "an annotation was removed",
            });
        };
        match (&annotation.kind, &other.kind) {
            (AnnotationKind::Comment(left), AnnotationKind::Comment(right)) => {
                if !same_identity(left, right) {
                    return Err("comment identity changed");
                }
                if matches!(
                    (&annotation.anchor, &other.anchor),
                    (Anchor::Detached, Anchor::Range { .. })
                ) {
                    return Err("detached comment anchor cannot be restored");
                }
            }
            (AnnotationKind::Bookmark { .. }, AnnotationKind::Bookmark { .. }) => {
                // The marker bytes belong to the part recorded at import.
                // A move has to stay inside that part. A host that disappeared
                // is a change even when the stored position bytes are the same.
                if annotation.kind != other.kind {
                    return Err("a bookmark changed");
                }
                let Some(part) = annotation
                    .source
                    .as_deref()
                    .map(|source| source.part.as_ref())
                else {
                    if annotation.anchor != other.anchor {
                        return Err("a bookmark changed");
                    }
                    continue;
                };
                let after_ok = crate::bookmark::anchor_writable(after, part, &other.anchor);
                let before_ok = crate::bookmark::anchor_writable(before, part, &annotation.anchor);
                if (annotation.anchor != other.anchor && !after_ok)
                    || (annotation.anchor == other.anchor && before_ok && !after_ok)
                {
                    return Err("a bookmark changed");
                }
            }
            _ => {
                if annotation.kind != other.kind || annotation.anchor != other.anchor {
                    return Err("non-text document content changed");
                }
            }
        }
    }
    for annotation in after.annotations() {
        if before.annotation(annotation.id).is_some() {
            continue;
        }
        let AnnotationKind::Comment(comment) = &annotation.kind else {
            return Err("non-text document content changed");
        };
        if before.story(comment.body).is_some() {
            return Err("new comment reused a story");
        }
        if after.story(comment.body).map(|story| story.kind) != Some(StoryKind::Comment) {
            return Err("new comment has no comment story");
        }
    }
    Ok(())
}

fn same_identity(left: &Comment, right: &Comment) -> bool {
    left.body == right.body
        && left.source_id == right.source_id
        && left.author == right.author
        && left.date == right.date
        && left.initials == right.initials
        && left.para_id == right.para_id
        && left.parent_para_id == right.parent_para_id
        && left.durable_id == right.durable_id
        && left.date_utc == right.date_utc
        && left.extra_attributes == right.extra_attributes
        && left.extra_markup == right.extra_markup
}

pub(crate) fn source_id_for_story(document: &Document, story: StoryId) -> Option<u32> {
    document.annotations().find_map(|annotation| {
        let AnnotationKind::Comment(comment) = &annotation.kind else {
            return None;
        };
        (comment.body == story).then_some(comment.source_id)
    })
}

/// Comment story text differs. Header and footer text is compared by
/// `header_footer`; this is the same check for comment stories, whose
/// paragraphs are patched in place.
pub(crate) fn stories_differ(original: &Document, current: &Document) -> bool {
    let changed = |document: &Document, other: &Document| {
        document.stories().any(|story| {
            if story.kind != StoryKind::Comment {
                return false;
            }
            match other.story(story.id) {
                Some(after) => !same_story_blocks(story, after),
                None => true,
            }
        })
    };
    changed(original, current) || changed(current, original)
}

fn same_story_blocks(before: &joyword_model::Story, after: &joyword_model::Story) -> bool {
    before.blocks.len() == after.blocks.len()
        && before
            .blocks
            .iter()
            .zip(after.blocks.iter())
            .all(
                |(left, right)| match (left.as_paragraph(), right.as_paragraph()) {
                    (Some(left), Some(right)) => left == right,
                    _ => false,
                },
            )
}

pub(crate) fn differs(before: &Document, after: &Document) -> bool {
    comment_rows(before) != comment_rows(after)
}

fn comment_rows(document: &Document) -> Vec<(AnnotationId, Comment, Anchor)> {
    document
        .annotations()
        .filter_map(|annotation| {
            let AnnotationKind::Comment(comment) = &annotation.kind else {
                return None;
            };
            Some((annotation.id, comment.clone(), annotation.anchor.clone()))
        })
        .collect()
}

pub(crate) struct CommentWrite {
    pub main: Option<Vec<u8>>,
    pub parts: Vec<(PartName, Vec<u8>)>,
    pub created: Vec<(String, Vec<u8>)>,
    pub content_types: Option<Vec<u8>>,
}

struct NewComment {
    source_id: u32,
    author: Arc<str>,
    date: Option<Arc<str>>,
    initials: Option<Arc<str>>,
    para_id: Arc<str>,
    durable_id: Option<Arc<str>>,
    parent_para_id: Option<Arc<str>>,
    resolved: bool,
    text: String,
    anchor: Anchor,
}

pub(crate) fn apply(
    package: &Package,
    main_name: &PartName,
    main_bytes: &[u8],
    rels_bytes: Option<&[u8]>,
    original: &Document,
    current: &Document,
    budget: &Budget,
) -> Result<Option<CommentWrite>, ExportError> {
    if !differs(original, current) {
        return Ok(None);
    }
    let mut strip = BTreeSet::new();
    let mut resolved_updates = Vec::new();
    for annotation in original.annotations() {
        let AnnotationKind::Comment(before) = &annotation.kind else {
            continue;
        };
        let Some(other) = current.annotation(annotation.id) else {
            return Err(unsupported(None, "a comment was removed"));
        };
        let AnnotationKind::Comment(after) = &other.kind else {
            return Err(unsupported(None, "comment identity changed"));
        };
        if matches!(
            (&annotation.anchor, &other.anchor),
            (Anchor::Range { .. }, Anchor::Detached)
        ) {
            strip.insert(before.source_id);
        }
        if before.resolved != after.resolved {
            let Some(para_id) = after.para_id.clone() else {
                return Err(unsupported(None, "comment extension cannot be updated"));
            };
            resolved_updates.push((para_id, after.resolved));
        }
    }
    let mut fresh = Vec::new();
    for annotation in current.annotations() {
        if original.annotation(annotation.id).is_some() {
            continue;
        }
        let AnnotationKind::Comment(comment) = &annotation.kind else {
            return Err(unsupported(None, "non-text document content changed"));
        };
        let Some(author) = comment.author.clone() else {
            return Err(unsupported(None, "comment author is missing"));
        };
        let Some(para_id) = comment.para_id.clone() else {
            return Err(unsupported(None, "comment thread cannot be written"));
        };
        if !is_para_id(&para_id) {
            return Err(unsupported(None, "comment thread cannot be written"));
        }
        if let Some(parent) = &comment.parent_para_id
            && !is_para_id(parent)
        {
            return Err(unsupported(None, "comment thread cannot be written"));
        }
        fresh.push(NewComment {
            source_id: comment.source_id,
            author,
            date: comment.date.clone(),
            initials: comment.initials.clone(),
            para_id,
            durable_id: comment.durable_id.clone(),
            parent_para_id: comment.parent_para_id.clone(),
            resolved: comment.resolved,
            text: comment_plain(current, comment.body)?,
            anchor: annotation.anchor.clone(),
        });
    }
    if strip.is_empty() && resolved_updates.is_empty() && fresh.is_empty() {
        return Ok(None);
    }
    let main = if strip.is_empty() && !fresh.iter().any(|comment| comment.parent_para_id.is_none())
    {
        None
    } else {
        Some(rewrite_main(main_bytes, current, &strip, &fresh, budget)?)
    };
    let mut parts = Vec::new();
    let mut created = Vec::new();
    let mut content_types = None;
    let mut rels = rels_bytes.map(|bytes| bytes.to_vec()).or_else(|| {
        package
            .part(main_name.relationships_part().as_str())
            .map(|part| part.data.to_vec())
    });
    if !fresh.is_empty() || !resolved_updates.is_empty() {
        let comments_existing = related(package, main_name, REL_COMMENTS);
        let extended_existing = related(package, main_name, REL_COMMENTS_EXTENDED).or_else(|| {
            comments_existing.and_then(|part| related(package, &part.name, REL_COMMENTS_EXTENDED))
        });
        if !fresh.is_empty() {
            let (name, bytes, is_new) = write_comments(comments_existing, &fresh)?;
            if bytes.len() as u64 > budget.max_part_bytes {
                return Err(ExportError::Budget {
                    part: name.to_string(),
                    limit: budget.max_part_bytes,
                });
            }
            if is_new {
                created.push((name.item_name().to_owned(), bytes));
                content_types = Some(add_override(
                    content_types
                        .as_deref()
                        .unwrap_or(package.content_types_xml()),
                    name.as_str(),
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
                )?);
                rels = Some(add_relationship(
                    rels.as_deref().ok_or_else(|| {
                        unsupported(None, "comment relationships part is missing")
                    })?,
                    REL_COMMENTS,
                    "comments.xml",
                )?);
            } else {
                parts.push((name, bytes));
            }
        }
        let (ext_name, ext_bytes, ext_new) =
            write_extended(extended_existing, &fresh, &resolved_updates)?;
        if ext_bytes.len() as u64 > budget.max_part_bytes {
            return Err(ExportError::Budget {
                part: ext_name.to_string(),
                limit: budget.max_part_bytes,
            });
        }
        if ext_new {
            created.push((ext_name.item_name().to_owned(), ext_bytes));
            content_types = Some(add_override(
                content_types
                    .as_deref()
                    .unwrap_or(package.content_types_xml()),
                ext_name.as_str(),
                "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml",
            )?);
            rels = Some(add_relationship(
                rels.as_deref()
                    .ok_or_else(|| unsupported(None, "comment relationships part is missing"))?,
                REL_COMMENTS_EXTENDED,
                "commentsExtended.xml",
            )?);
        } else if ext_bytes
            != extended_existing
                .map(|part| part.data.as_ref())
                .unwrap_or(&[])
        {
            parts.push((ext_name, ext_bytes));
        }
        // commentsIds and commentsExtensible change only with new comments:
        // resolving toggles `w15:done`, which neither part carries.
        if !fresh.is_empty() {
            let ids_existing = related(package, main_name, REL_COMMENTS_IDS).or_else(|| {
                comments_existing.and_then(|part| related(package, &part.name, REL_COMMENTS_IDS))
            });
            let extensible_existing =
                related(package, main_name, REL_COMMENTS_EXTENSIBLE).or_else(|| {
                    comments_existing
                        .and_then(|part| related(package, &part.name, REL_COMMENTS_EXTENSIBLE))
                });
            let durables = durable_plan(ids_existing, extensible_existing, &fresh)?;
            let (ids_name, ids_bytes, ids_new) = write_ids(ids_existing, &fresh, &durables)?;
            if ids_bytes.len() as u64 > budget.max_part_bytes {
                return Err(ExportError::Budget {
                    part: ids_name.to_string(),
                    limit: budget.max_part_bytes,
                });
            }
            if ids_new {
                created.push((ids_name.item_name().to_owned(), ids_bytes));
                content_types = Some(add_override(
                    content_types
                        .as_deref()
                        .unwrap_or(package.content_types_xml()),
                    ids_name.as_str(),
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsIds+xml",
                )?);
                rels = Some(add_relationship(
                    rels.as_deref().ok_or_else(|| {
                        unsupported(None, "comment relationships part is missing")
                    })?,
                    REL_COMMENTS_IDS,
                    "commentsIds.xml",
                )?);
            } else if ids_bytes != ids_existing.map(|part| part.data.as_ref()).unwrap_or(&[]) {
                parts.push((ids_name, ids_bytes));
            }
            if let Some((ex_name, ex_bytes, ex_new)) =
                write_extensible(extensible_existing, &fresh, &durables)?
            {
                if ex_bytes.len() as u64 > budget.max_part_bytes {
                    return Err(ExportError::Budget {
                        part: ex_name.to_string(),
                        limit: budget.max_part_bytes,
                    });
                }
                if ex_new {
                    created.push((ex_name.item_name().to_owned(), ex_bytes));
                    content_types = Some(add_override(
                        content_types
                            .as_deref()
                            .unwrap_or(package.content_types_xml()),
                        ex_name.as_str(),
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtensible+xml",
                    )?);
                    rels = Some(add_relationship(
                        rels.as_deref().ok_or_else(|| {
                            unsupported(None, "comment relationships part is missing")
                        })?,
                        REL_COMMENTS_EXTENSIBLE,
                        "commentsExtensible.xml",
                    )?);
                } else if ex_bytes
                    != extensible_existing
                        .map(|part| part.data.as_ref())
                        .unwrap_or(&[])
                {
                    parts.push((ex_name, ex_bytes));
                }
            }
        }
    }
    if let Some(bytes) = rels
        && rels_bytes != Some(bytes.as_slice())
        && package
            .part(main_name.relationships_part().as_str())
            .is_none_or(|part| part.data.as_ref() != bytes.as_slice())
    {
        parts.push((main_name.relationships_part(), bytes));
    }
    let changed = main.is_some() || !parts.is_empty() || !created.is_empty();
    if !changed {
        return Ok(None);
    }
    Ok(Some(CommentWrite {
        main,
        parts,
        created,
        content_types,
    }))
}

fn comment_plain(document: &Document, story: StoryId) -> Result<String, ExportError> {
    let story = document
        .story(story)
        .ok_or_else(|| unsupported(None, "comment story is missing"))?;
    let mut paragraphs = story.blocks.iter().filter_map(Block::as_paragraph);
    let Some(paragraph) = paragraphs.next() else {
        return Err(unsupported(None, "comment body is empty"));
    };
    if paragraphs.next().is_some() || !paragraph.inlines().is_empty() {
        return Err(unsupported(None, "comment body is not a single paragraph"));
    }
    Ok(paragraph.plain_text(|_| String::new()))
}

fn rewrite_main(
    xml: &[u8],
    current: &Document,
    strip: &BTreeSet<u32>,
    fresh: &[NewComment],
    budget: &Budget,
) -> Result<Vec<u8>, ExportError> {
    let mut bytes = xml.to_vec();
    if !strip.is_empty() {
        let root = parse_main(&bytes, budget)?;
        let mut ranges = Vec::new();
        strip_ranges(&root, strip, &mut ranges);
        ranges.sort_by_key(|range| range.start);
        for pair in ranges.windows(2) {
            if pair[1].start < pair[0].end {
                return Err(unsupported(None, "comment markers overlap"));
            }
        }
        bytes = apply_ranges(&bytes, &ranges, &[]);
    }
    let roots: Vec<&NewComment> = fresh
        .iter()
        .filter(|comment| comment.parent_para_id.is_none())
        .filter(|comment| matches!(comment.anchor, Anchor::Range { .. }))
        .collect();
    if roots.is_empty() {
        return Ok(bytes);
    }
    let root = parse_main(&bytes, budget)?;
    let prefix =
        ns_prefix(&root, W).ok_or_else(|| unsupported(None, "comment range cannot be written"))?;
    let body = root
        .child(W, "body")
        .ok_or_else(|| unsupported(None, "missing source body"))?;
    let model: Vec<NodeId> = current
        .body()
        .blocks
        .iter()
        .filter_map(|block| block.as_paragraph().map(|paragraph| paragraph.id()))
        .collect();
    let xml_paragraphs: Vec<&Element> = body.elements().filter(|child| child.is(W, "p")).collect();
    if model.len() != xml_paragraphs.len() {
        return Err(unsupported(None, "comment range cannot be written"));
    }
    let mut events: BTreeMap<usize, Vec<MarkerEvent>> = BTreeMap::new();
    for comment in roots {
        let Anchor::Range { start, end } = &comment.anchor else {
            continue;
        };
        let Some(start_at) = model.iter().position(|id| *id == start.paragraph) else {
            return Err(unsupported(None, "comment range cannot be written"));
        };
        let Some(end_at) = model.iter().position(|id| *id == end.paragraph) else {
            return Err(unsupported(None, "comment range cannot be written"));
        };
        events.entry(start_at).or_default().push(MarkerEvent {
            offset: start.offset.0,
            kind: 0,
            source_id: comment.source_id,
            xml: format!(
                "<{prefix}:commentRangeStart {prefix}:id=\"{}\"/>",
                comment.source_id
            ),
        });
        events.entry(end_at).or_default().push(MarkerEvent {
            offset: end.offset.0,
            kind: 1,
            source_id: comment.source_id,
            xml: format!(
                "<{prefix}:commentRangeEnd {prefix}:id=\"{id}\"/><{prefix}:r><{prefix}:commentReference {prefix}:id=\"{id}\"/></{prefix}:r>",
                id = comment.source_id
            ),
        });
    }
    let mut edits = Vec::new();
    for (index, paragraph_events) in events {
        let paragraph = xml_paragraphs[index];
        edits.extend(paragraph_edits(&bytes, paragraph, &paragraph_events)?);
    }
    edits.sort_by(|left, right| {
        right
            .0
            .start
            .cmp(&left.0.start)
            .then(right.0.end.cmp(&left.0.end))
    });
    let bytes = apply_ranges(&bytes, &[], &edits);
    if bytes.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: "word/document.xml".into(),
            limit: budget.max_part_bytes,
        });
    }
    Ok(bytes)
}

type ByteEdit = (Range<usize>, Vec<u8>);

#[derive(Clone)]
struct MarkerEvent {
    offset: usize,
    kind: u8,
    source_id: u32,
    xml: String,
}

fn paragraph_edits(
    xml: &[u8],
    paragraph: &Element,
    events: &[MarkerEvent],
) -> Result<Vec<ByteEdit>, ExportError> {
    let fail = || unsupported(None, "comment range cannot be written");
    let mut pending = events.to_vec();
    pending.sort_by_key(|event| (event.offset, event.kind, event.source_id));
    let mut edits = Vec::new();
    let mut cursor = 0usize;
    let content_at = after_start(xml, paragraph).map_err(|_| fail())?;
    let boundary = paragraph
        .child(W, "pPr")
        .map(|properties| properties.span.end)
        .unwrap_or(content_at);
    flush_events(&mut pending, 0, boundary, &mut edits);
    for child in paragraph.elements() {
        if child.is(W, "pPr") {
            continue;
        }
        if child.is(W, "commentRangeStart")
            || child.is(W, "commentRangeEnd")
            || child.is(W, "proofErr")
        {
            flush_events(&mut pending, cursor, child.span.end, &mut edits);
            continue;
        }
        if !child.is(W, "r") {
            return Err(fail());
        }
        let length = run_len(child).map_err(|_| fail())?;
        let run_start = cursor;
        let interior: Vec<MarkerEvent> = pending
            .iter()
            .filter(|event| event.offset > run_start && event.offset < run_start + length)
            .cloned()
            .collect();
        flush_events(&mut pending, cursor, child.span.start, &mut edits);
        if !interior.is_empty() {
            let replacement = split_run(xml, child, run_start, &interior).map_err(|_| fail())?;
            edits.push((child.span.clone(), replacement));
            pending
                .retain(|event| !(event.offset > run_start && event.offset < run_start + length));
        }
        cursor += length;
    }
    let end_at = end_tag_at(xml, paragraph).map_err(|_| fail())?;
    flush_events(&mut pending, cursor, end_at, &mut edits);
    if !pending.is_empty() {
        return Err(fail());
    }
    Ok(edits)
}

fn flush_events(
    pending: &mut Vec<MarkerEvent>,
    offset: usize,
    at: usize,
    edits: &mut Vec<(Range<usize>, Vec<u8>)>,
) {
    let mut xml = String::new();
    pending.retain(|event| {
        if event.offset == offset {
            xml.push_str(&event.xml);
            false
        } else {
            true
        }
    });
    if !xml.is_empty() {
        edits.push((at..at, xml.into_bytes()));
    }
}

fn run_len(run: &Element) -> Result<usize, ()> {
    let mut length = 0usize;
    for leaf in run.elements() {
        if leaf.is(W, "rPr")
            || leaf.is(W, "commentReference")
            || leaf.is(W, "lastRenderedPageBreak")
        {
            continue;
        }
        if leaf.is(W, "t") && leaf.elements().next().is_none() {
            length += leaf.text().chars().count();
        } else if leaf.is(W, "tab") && leaf.elements().next().is_none() {
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
            return Err(());
        }
    }
    let Some(text) = text else {
        return Err(());
    };
    let chars: Vec<char> = text.text().chars().collect();
    let open = open_tag(xml, run)?;
    let prefix = qname_prefix(&text.qname);
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
                    out.extend(event.xml.as_bytes());
                }
            }
        }
        if from == to {
            continue;
        }
        let piece: String = chars[from..to].iter().collect();
        out.extend(open);
        out.push(b'>');
        out.extend(&properties);
        let tag = if prefix.is_empty() {
            "t".to_string()
        } else {
            format!("{prefix}:t")
        };
        let run_end = if prefix.is_empty() {
            "</r>".to_string()
        } else {
            format!("</{prefix}:r>")
        };
        out.extend(
            format!(
                "<{tag} xml:space=\"preserve\">{}</{tag}>",
                xml_escape(&piece)
            )
            .into_bytes(),
        );
        out.extend(run_end.into_bytes());
    }
    Ok(out)
}

fn open_tag<'a>(xml: &'a [u8], element: &Element) -> Result<&'a [u8], ()> {
    let raw = xml.get(element.span.start..element.span.end).ok_or(())?;
    let gt = raw.iter().position(|byte| *byte == b'>').ok_or(())?;
    if raw[..gt].ends_with(b"/") {
        return Err(());
    }
    Ok(&raw[..gt])
}

fn qname_prefix(qname: &str) -> &str {
    qname
        .rsplit_once(':')
        .map(|(prefix, _)| prefix)
        .unwrap_or("")
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

fn strip_ranges(element: &Element, ids: &BTreeSet<u32>, ranges: &mut Vec<Range<usize>>) {
    if element.is(W, "tbl") {
        return;
    }
    if (element.is(W, "commentRangeStart") || element.is(W, "commentRangeEnd"))
        && id_in(element, ids)
    {
        ranges.push(element.span.clone());
        return;
    }
    if element.is(W, "r") {
        let mut references = Vec::new();
        let mut other = false;
        for child in element.elements() {
            if child.is(W, "rPr") {
                continue;
            }
            if child.is(W, "commentReference") && id_in(child, ids) {
                references.push(child.span.clone());
            } else {
                other = true;
            }
        }
        if !references.is_empty() && !other {
            ranges.push(element.span.clone());
        } else {
            ranges.extend(references);
        }
        return;
    }
    for child in element.elements() {
        strip_ranges(child, ids, ranges);
    }
}

fn id_in(element: &Element, ids: &BTreeSet<u32>) -> bool {
    comment_id(element).is_some_and(|id| ids.contains(&id))
}

fn apply_ranges(
    xml: &[u8],
    deletions: &[Range<usize>],
    edits: &[(Range<usize>, Vec<u8>)],
) -> Vec<u8> {
    let mut combined: Vec<(Range<usize>, Vec<u8>)> = deletions
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

fn parse_main(bytes: &[u8], budget: &Budget) -> Result<Element, ExportError> {
    XmlReader::new("word/document.xml", bytes, budget)
        .and_then(XmlReader::whole)
        .map_err(|error| {
            ExportError::Import(ImportError::Xml {
                part: "word/document.xml".into(),
                error,
            })
        })
}

fn ns_prefix(element: &Element, namespace: &str) -> Option<String> {
    element.declarations.iter().find_map(|(prefix, uri)| {
        (uri.as_ref() == namespace && !prefix.is_empty()).then(|| prefix.to_string())
    })
}

fn write_comments(
    existing: Option<&Part>,
    fresh: &[NewComment],
) -> Result<(PartName, Vec<u8>, bool), ExportError> {
    let fail = || unsupported(None, "comment part cannot be written");
    if let Some(part) = existing {
        let root = XmlReader::new(part.name.as_str(), &part.data, &Budget::default())
            .and_then(XmlReader::whole)
            .map_err(|_| fail())?;
        let prefix = ns_prefix(&root, W).ok_or_else(fail)?;
        let w14 = ns_prefix(&root, W14);
        let mut extra = String::new();
        for comment in fresh {
            extra.push_str(&comment_xml(comment, &prefix, w14.as_deref())?);
        }
        let bytes = insert_before_end(&part.data, &root, &extra).map_err(|_| fail())?;
        Ok((part.name.clone(), bytes, false))
    } else {
        let name = PartName::new("/word/comments.xml").map_err(|_| fail())?;
        let mut body = String::new();
        for comment in fresh {
            body.push_str(&comment_xml(comment, "w", None)?);
        }
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:comments xmlns:w=\"{W}\" xmlns:w14=\"{W14}\">{body}</w:comments>"
        );
        Ok((name, xml.into_bytes(), true))
    }
}

fn comment_xml(
    comment: &NewComment,
    prefix: &str,
    w14: Option<&str>,
) -> Result<String, ExportError> {
    let fail = || unsupported(None, "comment part cannot be written");
    if comment.author.chars().any(|ch| ch.is_control()) {
        return Err(fail());
    }
    let mut attrs = format!(
        " {prefix}:id=\"{}\" {prefix}:author=\"{}\"",
        comment.source_id,
        xml_escape(&comment.author)
    );
    if let Some(date) = &comment.date {
        attrs.push_str(&format!(" {prefix}:date=\"{}\"", xml_escape(date)));
    }
    if let Some(initials) = &comment.initials {
        attrs.push_str(&format!(" {prefix}:initials=\"{}\"", xml_escape(initials)));
    }
    let para = if let Some(w14) = w14 {
        format!(" {w14}:paraId=\"{}\"", comment.para_id)
    } else {
        format!(" w14:paraId=\"{}\" xmlns:w14=\"{W14}\"", comment.para_id)
    };
    let text = if comment.text.is_empty() {
        String::new()
    } else {
        format!(
            "<{prefix}:r><{prefix}:t xml:space=\"preserve\">{}</{prefix}:t></{prefix}:r>",
            xml_escape(&comment.text)
        )
    };
    Ok(format!(
        "<{prefix}:comment{attrs}><{prefix}:p{para}>{text}</{prefix}:p></{prefix}:comment>"
    ))
}

fn write_extended(
    existing: Option<&Part>,
    fresh: &[NewComment],
    updates: &[(Arc<str>, bool)],
) -> Result<(PartName, Vec<u8>, bool), ExportError> {
    let fail = || unsupported(None, "comment extension cannot be updated");
    if let Some(part) = existing {
        let root = XmlReader::new(part.name.as_str(), &part.data, &Budget::default())
            .and_then(XmlReader::whole)
            .map_err(|_| fail())?;
        let prefix = ns_prefix(&root, W15).ok_or_else(fail)?;
        let mut bytes = part.data.to_vec();
        let mut missing = Vec::new();
        for (para_id, resolved) in updates {
            if !patch_done(&mut bytes, &root, &prefix, para_id, *resolved)? {
                missing.push((para_id.clone(), *resolved));
            }
        }
        if !missing.is_empty() || !fresh.is_empty() {
            let root = XmlReader::new(part.name.as_str(), &bytes, &Budget::default())
                .and_then(XmlReader::whole)
                .map_err(|_| fail())?;
            let mut extra = String::new();
            for (para_id, resolved) in &missing {
                extra.push_str(&comment_ex(&prefix, para_id, None, *resolved));
            }
            for comment in fresh {
                extra.push_str(&comment_ex(
                    &prefix,
                    &comment.para_id,
                    comment.parent_para_id.as_deref(),
                    comment.resolved,
                ));
            }
            bytes = insert_before_end(&bytes, &root, &extra).map_err(|_| fail())?;
        }
        Ok((part.name.clone(), bytes, false))
    } else if fresh.is_empty() && updates.is_empty() {
        Err(fail())
    } else {
        let name = PartName::new("/word/commentsExtended.xml").map_err(|_| fail())?;
        let mut body = String::new();
        for (para_id, resolved) in updates {
            body.push_str(&comment_ex("w15", para_id, None, *resolved));
        }
        for comment in fresh {
            body.push_str(&comment_ex(
                "w15",
                &comment.para_id,
                comment.parent_para_id.as_deref(),
                comment.resolved,
            ));
        }
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w15:commentsEx xmlns:w15=\"{W15}\">{body}</w15:commentsEx>"
        );
        Ok((name, xml.into_bytes(), true))
    }
}

fn comment_ex(prefix: &str, para_id: &str, parent: Option<&str>, resolved: bool) -> String {
    let done = if resolved { "1" } else { "0" };
    let parent = parent
        .map(|parent| format!(" {prefix}:paraIdParent=\"{parent}\""))
        .unwrap_or_default();
    format!("<{prefix}:commentEx {prefix}:paraId=\"{para_id}\"{parent} {prefix}:done=\"{done}\"/>")
}

/// FNV-1a over the paraId text plus a zero-based attempt counter: a durable
/// id for a comment this session created. JoyWord assumption (not a Word
/// algorithm): the value only has to be stable and collision-free inside the
/// part, which the deterministic retry against the ids already taken below
/// guarantees. Word writes GUID-shaped values; the importer accepts any
/// non-empty string, and `w16cid:durableId` is a 32-bit integer per
/// [MS-DOCX], so the plan emits plain decimals.
fn durable_id_for(para_id: &str, attempt: u32, taken: &BTreeSet<String>) -> String {
    let mut candidate = joyword_model::durable_id_from(para_id, attempt).to_string();
    let mut salt = 0u32;
    while taken.contains(&candidate) {
        salt += 1;
        candidate =
            joyword_model::durable_id_from(&format!("{para_id}:{salt}"), attempt).to_string();
    }
    candidate
}

/// One durable id per new comment, planned once so commentsIds and
/// commentsExtensible write the same value for the same paraId. A paraId
/// already present in the part for a comment this session claims as new is an
/// inconsistency the model should not produce; refuse instead of inventing a
/// second entry.
fn durable_plan(
    ids_existing: Option<&Part>,
    extensible_existing: Option<&Part>,
    fresh: &[NewComment],
) -> Result<Vec<String>, ExportError> {
    let fail = || unsupported(None, "comment thread cannot be written");
    let mut taken = BTreeSet::new();
    let mut para_seen = BTreeSet::new();
    for part in [ids_existing, extensible_existing].into_iter().flatten() {
        let root = XmlReader::new(part.name.as_str(), &part.data, &Budget::default())
            .and_then(XmlReader::whole)
            .map_err(|_| fail())?;
        for child in root.elements() {
            if child.namespace.as_deref() == Some(W16CID) && child.local.as_ref() == "commentId" {
                if let Some(para) = child.attribute(Some(W16CID), "paraId") {
                    para_seen.insert(para.to_ascii_uppercase());
                }
                if let Some(durable) = child.attribute(Some(W16CID), "durableId") {
                    taken.insert(durable.to_owned());
                }
            }
            if child.namespace.as_deref() == Some(W16CEX)
                && child.local.as_ref() == "commentExtensible"
                && let Some(durable) = child.attribute(Some(W16CEX), "durableId")
            {
                taken.insert(durable.to_owned());
            }
        }
    }
    let mut plan = Vec::with_capacity(fresh.len());
    for comment in fresh {
        if para_seen.contains(comment.para_id.to_ascii_uppercase().as_str()) {
            return Err(fail());
        }
        // Comments created through the engine carry the durable id they were
        // allocated with; write that value so a reimport compares equal.
        // Anything else (a comment constructed without the allocator) falls
        // back to the same shared derivation.
        let durable = match comment.durable_id.as_deref() {
            Some(durable) if !taken.contains(durable) => durable.to_owned(),
            Some(_) => return Err(fail()),
            None => durable_id_for(&comment.para_id, 0, &taken),
        };
        taken.insert(durable.clone());
        plan.push(durable);
    }
    Ok(plan)
}

/// Append one `w16cid:commentId` per new comment; existing entries keep their
/// bytes untouched, exactly like the commentsExtended writer.
fn write_ids(
    existing: Option<&Part>,
    fresh: &[NewComment],
    durables: &[String],
) -> Result<(PartName, Vec<u8>, bool), ExportError> {
    let fail = || unsupported(None, "comment thread cannot be written");
    if let Some(part) = existing {
        let root = XmlReader::new(part.name.as_str(), &part.data, &Budget::default())
            .and_then(XmlReader::whole)
            .map_err(|_| fail())?;
        let prefix = ns_prefix(&root, W16CID).ok_or_else(fail)?;
        let mut extra = String::new();
        for (comment, durable) in fresh.iter().zip(durables) {
            extra.push_str(&format!(
                "<{prefix}:commentId {prefix}:paraId=\"{}\" {prefix}:durableId=\"{durable}\"/>",
                comment.para_id
            ));
        }
        let bytes = insert_before_end(&part.data, &root, &extra).map_err(|_| fail())?;
        Ok((part.name.clone(), bytes, false))
    } else {
        let name = PartName::new("/word/commentsIds.xml").map_err(|_| fail())?;
        let mut body = String::new();
        for (comment, durable) in fresh.iter().zip(durables) {
            body.push_str(&format!(
                "<w16cid:commentId w16cid:paraId=\"{}\" w16cid:durableId=\"{durable}\"/>",
                comment.para_id
            ));
        }
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w16cid:commentsIds xmlns:w16cid=\"{W16CID}\">{body}</w16cid:commentsIds>"
        );
        Ok((name, xml.into_bytes(), true))
    }
}

/// Append one `w16cex:commentExtensible` per new comment carrying the
/// creation timestamp as `dateUtc`. `dateUtc` is only written when every new
/// comment's `w:date` already matches the strict UTC shape seen in the wild
/// (`YYYY-MM-DDTHH:MM:SSZ`, the shape this repository writes); anything else
/// leaves the part untouched rather than inventing a timestamp. `None` means
/// "nothing to write".
fn write_extensible(
    existing: Option<&Part>,
    fresh: &[NewComment],
    durables: &[String],
) -> Result<Option<(PartName, Vec<u8>, bool)>, ExportError> {
    let fail = || unsupported(None, "comment thread cannot be written");
    let mut dated = Vec::with_capacity(fresh.len());
    for comment in fresh {
        let Some(date) = comment.date.as_deref() else {
            return Ok(None);
        };
        if !joyword_model::is_strict_utc_date(date) {
            return Ok(None);
        }
        dated.push(date);
    }
    let Some(part) = existing else {
        let name = PartName::new("/word/commentsExtensible.xml").map_err(|_| fail())?;
        let mut body = String::new();
        for (durable, date) in durables.iter().zip(&dated) {
            body.push_str(&format!(
                "<w16cex:commentExtensible w16cex:durableId=\"{durable}\" w16cex:dateUtc=\"{date}\"/>"
            ));
        }
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w16cex:commentsExtensible xmlns:w16cex=\"{W16CEX}\">{body}</w16cex:commentsExtensible>"
        );
        return Ok(Some((name, xml.into_bytes(), true)));
    };
    let root = XmlReader::new(part.name.as_str(), &part.data, &Budget::default())
        .and_then(XmlReader::whole)
        .map_err(|_| fail())?;
    let prefix = ns_prefix(&root, W16CEX).ok_or_else(fail)?;
    let mut taken = BTreeSet::new();
    for child in root.elements() {
        if child.local.as_ref() == "commentExtensible"
            && let Some(durable) = child.attribute(Some(W16CEX), "durableId")
        {
            taken.insert(durable.to_owned());
        }
    }
    if durables.iter().any(|durable| taken.contains(durable)) {
        return Err(fail());
    }
    let mut extra = String::new();
    for (durable, date) in durables.iter().zip(&dated) {
        extra.push_str(&format!(
            "<{prefix}:commentExtensible {prefix}:durableId=\"{durable}\" {prefix}:dateUtc=\"{date}\"/>"
        ));
    }
    let bytes = insert_before_end(&part.data, &root, &extra).map_err(|_| fail())?;
    Ok(Some((part.name.clone(), bytes, false)))
}

fn patch_done(
    bytes: &mut Vec<u8>,
    root: &Element,
    prefix: &str,
    para_id: &str,
    resolved: bool,
) -> Result<bool, ExportError> {
    let fail = || unsupported(None, "comment extension cannot be updated");
    let Some(element) = root.elements().find(|child| {
        child.local.as_ref() == "commentEx"
            && child
                .attribute(Some(W15), "paraId")
                .is_some_and(|value| value.eq_ignore_ascii_case(para_id))
    }) else {
        return Ok(false);
    };
    let span = element.span.clone();
    let tag_end = bytes
        .get(span.start..span.end)
        .and_then(|raw| raw.iter().position(|byte| *byte == b'>'))
        .ok_or_else(fail)?;
    let tag =
        std::str::from_utf8(&bytes[span.start..span.start + tag_end + 1]).map_err(|_| fail())?;
    let value = if resolved { "1" } else { "0" };
    let replacement = if let Some((start, end)) = attr_value(tag, "done") {
        let mut next = String::new();
        next.push_str(&tag[..start]);
        next.push_str(value);
        next.push_str(&tag[end..]);
        next
    } else {
        let attribute = format!(" {prefix}:done=\"{value}\"");
        insert_attr(tag, &attribute).ok_or_else(fail)?
    };
    let mut next = Vec::new();
    next.extend_from_slice(&bytes[..span.start]);
    next.extend(replacement.into_bytes());
    next.extend_from_slice(&bytes[span.start + tag_end + 1..]);
    *bytes = next;
    Ok(true)
}

fn attr_value(tag: &str, local: &str) -> Option<(usize, usize)> {
    let needle = format!("{local}=");
    let mut search = 0usize;
    while let Some(pos) = tag[search..].find(&needle) {
        let abs = search + pos;
        let boundary = abs == 0
            || tag.as_bytes()[abs - 1] == b':'
            || tag.as_bytes()[abs - 1].is_ascii_whitespace();
        if boundary {
            let after = abs + needle.len();
            let quote = *tag.as_bytes().get(after)?;
            if quote == b'"' || quote == b'\'' {
                let start = after + 1;
                let end = tag[start..].find(quote as char)? + start;
                return Some((start, end));
            }
        }
        search = abs + local.len();
        if search >= tag.len() {
            break;
        }
    }
    None
}

fn insert_attr(tag: &str, attribute: &str) -> Option<String> {
    if let Some(stripped) = tag.strip_suffix("/>") {
        Some(format!("{stripped}{attribute}/>"))
    } else {
        tag.strip_suffix('>')
            .map(|stripped| format!("{stripped}{attribute}>"))
    }
}

fn insert_before_end(xml: &[u8], root: &Element, extra: &str) -> Result<Vec<u8>, ()> {
    let text = std::str::from_utf8(xml).map_err(|_| ())?;
    if text.contains("<!--") {
        return Err(());
    }
    let raw = text.get(root.span.clone()).ok_or(())?;
    if let Some(head) = raw.strip_suffix("/>") {
        let mut out = String::new();
        out.push_str(&text[..root.span.start]);
        out.push_str(head);
        out.push('>');
        out.push_str(extra);
        out.push_str(&format!("</{}>", root.qname));
        out.push_str(&text[root.span.end..]);
        return Ok(out.into_bytes());
    }
    let end = format!("</{}>", root.qname);
    let at = root.span.end.checked_sub(end.len()).ok_or(())?;
    if text.get(at..root.span.end) != Some(end.as_str()) {
        return Err(());
    }
    let mut out = String::with_capacity(text.len() + extra.len());
    out.push_str(&text[..at]);
    out.push_str(extra);
    out.push_str(&text[at..]);
    Ok(out.into_bytes())
}

fn add_override(xml: &[u8], part: &str, content_type: &str) -> Result<Vec<u8>, ExportError> {
    let fail = || unsupported(None, "content types are not rewritten");
    let text = std::str::from_utf8(xml).map_err(|_| fail())?;
    if text.contains(&format!("PartName=\"{part}\"")) {
        return Ok(xml.to_vec());
    }
    let end = "</Types>";
    let Some(pos) = text.rfind(end) else {
        return Err(fail());
    };
    let extra = format!("<Override PartName=\"{part}\" ContentType=\"{content_type}\"/>");
    let mut out = String::with_capacity(text.len() + extra.len());
    out.push_str(&text[..pos]);
    out.push_str(&extra);
    out.push_str(&text[pos..]);
    Ok(out.into_bytes())
}

fn add_relationship(xml: &[u8], kind: &str, target: &str) -> Result<Vec<u8>, ExportError> {
    let fail = || unsupported(None, "comment relationships part is missing");
    let text = std::str::from_utf8(xml).map_err(|_| fail())?;
    if text.contains(kind) {
        return Ok(xml.to_vec());
    }
    let end = "</Relationships>";
    let Some(pos) = text.rfind(end) else {
        return Err(fail());
    };
    let id = next_relationship_id(text);
    let extra = format!("<Relationship Id=\"rId{id}\" Type=\"{kind}\" Target=\"{target}\"/>");
    let mut out = String::with_capacity(text.len() + extra.len());
    out.push_str(&text[..pos]);
    out.push_str(&extra);
    out.push_str(&text[pos..]);
    Ok(out.into_bytes())
}

fn next_relationship_id(xml: &str) -> u32 {
    let mut max = 0u32;
    let mut rest = xml;
    while let Some(pos) = rest.find("rId") {
        let after = &rest[pos + 3..];
        let digits: String = after.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        if let Ok(value) = digits.parse::<u32>() {
            max = max.max(value);
        }
        rest = &rest[pos + 3..];
    }
    max.saturating_add(1)
}

fn xml_escape(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
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
