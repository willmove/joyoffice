//! Conservative edited export (ECMA-376 Part 1 §17.3.1, §17.3.3.31, Part 2).
//!
//! Text leaves change, and body paragraphs may be split, merged, inserted or
//! removed when their properties are either default or fully mapped (see
//! `structure`). Unmapped property markup is refused rather than dropped.
//! Comments outside text leaves, relationships and every other part stay
//! byte-for-byte intact. Moves, other structural edits, signed packages and
//! any imported restriction are refused. The completed output is re-imported
//! under the same budget and compared before a plan can be returned. This
//! module performs no IO.

mod compare;
mod structure;

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use joyoffice_opc::{Budget, Method, PartName, write_archive};
use joyword_model::{
    Block, BreakKind, Document, DocumentSnapshot, DocumentVersion, InlineKind, NodeId,
    NumberingStore, PageFieldKind, Paragraph, RunProps, Story, StyleId, StyleSheet,
};

use crate::xml::{Element, XML_NAMESPACE, XmlReader};
use crate::{ImportError, PackageBacking, W, import};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    Import(ImportError),
    Restricted,
    SignedPackage,
    Unsupported {
        node: Option<NodeId>,
        reason: &'static str,
    },
    Budget {
        part: String,
        limit: u64,
    },
    Write(String),
    RoundTripMismatch,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Import(error) => write!(f, "export verification: {error}"),
            Self::Restricted => f.write_str("restricted DOCX cannot be exported after editing"),
            Self::SignedPackage => f.write_str(
                "editing a signed package requires explicit signature invalidation support",
            ),
            Self::Unsupported { node, reason } => {
                write!(f, "unsupported DOCX edit at {node:?}: {reason}")
            }
            Self::Budget { part, limit } => {
                write!(f, "export of {part} exceeds byte budget {limit}")
            }
            Self::Write(error) => write!(f, "DOCX package writer: {error}"),
            Self::RoundTripMismatch => {
                f.write_str("exported DOCX does not reproduce the current document")
            }
        }
    }
}

impl std::error::Error for ExportError {}

impl From<ImportError> for ExportError {
    fn from(error: ImportError) -> Self {
        Self::Import(error)
    }
}

/// Original package kind; exact copies retain templates and macro packages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportTarget {
    Docx,
    Dotx,
    Docm,
    Dotm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportMode {
    /// Original ZIP, including its metadata, copied exactly.
    OriginalPackage,
    /// Text leaves and safe body paragraphs patched in place; ZIP container
    /// metadata is regenerated.
    TextPatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportWarning {
    /// ZIP timestamps, extra fields, directory records and comments are not
    /// retained by the current package writer. OPC part bytes are retained.
    ZipMetadataRegenerated,
}

/// An immutable, verified output for one document version. Creating a plan
/// does not write a file, change the backing, or mark a session saved.
///
/// `origin` is the package the plan was derived from. Text patches replace
/// `bytes` and leave `origin` alone.
#[derive(Clone, Debug)]
pub struct ExportPlan {
    version: DocumentVersion,
    mode: ExportMode,
    target: ExportTarget,
    bytes: Arc<[u8]>,
    changed: Vec<PartName>,
    preserved: Vec<PartName>,
    warnings: Vec<ExportWarning>,
    origin: Option<Arc<[u8]>>,
}

impl ExportPlan {
    pub fn version(&self) -> DocumentVersion {
        self.version
    }
    pub fn mode(&self) -> ExportMode {
        self.mode
    }
    pub fn target(&self) -> ExportTarget {
        self.target
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn changed_parts(&self) -> &[PartName] {
        &self.changed
    }
    /// Exact uncompressed bytes, not identical ZIP headers/compression.
    /// The content-types stream is also kept exactly in TextPatch mode.
    pub fn preserved_parts(&self) -> &[PartName] {
        &self.preserved
    }
    pub fn warnings(&self) -> &[ExportWarning] {
        &self.warnings
    }
    /// Exact package bytes of the [`PackageBacking`] passed to [`plan_export`].
    ///
    /// `Some` shares that backing's allocation and compares equal to its
    /// [`PackageBacking::original_bytes`], including after an edit rewrites
    /// [`Self::bytes`]. `None` means this plan cannot prove an origin;
    /// `save_original` must refuse it. Every successful `plan_export` returns
    /// `Some`. There is no public constructor or setter, so a caller cannot
    /// install different bytes or clear the binding. An equal
    /// [`DocumentVersion`] is not evidence that two origins match.
    pub fn origin_bytes(&self) -> Option<&[u8]> {
        self.origin.as_deref()
    }
    pub fn target_content_type(&self) -> &'static str {
        match self.target {
            ExportTarget::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
            ExportTarget::Dotx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.template"
            }
            ExportTarget::Docm => "application/vnd.ms-word.document.macroEnabled.12",
            ExportTarget::Dotm => "application/vnd.ms-word.template.macroEnabled.12",
        }
    }
}

fn unsupported(node: Option<NodeId>, reason: &'static str) -> ExportError {
    ExportError::Unsupported { node, reason }
}

/// Reconstruct the trusted baseline from the backing, rather than trusting
/// caller-supplied editability or a mutable original model. IDs and source
/// locators must still correspond to that import. No budget is lifted.
pub fn plan_export(
    backing: &PackageBacking,
    current: &DocumentSnapshot,
    budget: &Budget,
) -> Result<ExportPlan, ExportError> {
    let source = import(backing.original_bytes(), budget)?;
    plan_export_after_import(&source, backing, &source.document, current, budget)
}

/// Use an immutable baseline whose stable model IDs were retained across an
/// original save. The package is still re-imported on every call; only the
/// verified source-locator projection comes from `original`.
pub(crate) fn plan_export_with_original(
    backing: &PackageBacking,
    original: &Document,
    current: &DocumentSnapshot,
    budget: &Budget,
) -> Result<ExportPlan, ExportError> {
    let source = import(backing.original_bytes(), budget)?;
    plan_export_after_import(&source, backing, original, current, budget)
}

fn plan_export_after_import(
    source: &crate::ImportResult,
    backing: &PackageBacking,
    original: &Document,
    current: &DocumentSnapshot,
    budget: &Budget,
) -> Result<ExportPlan, ExportError> {
    check_envelope(original, current)?;
    let text_bytes = current
        .body()
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .try_fold(0u64, |sum, p| {
            sum.checked_add(p.text().summary().bytes as u64)
        });
    if text_bytes.is_none_or(|n| n > budget.max_part_bytes) {
        return Err(ExportError::Budget {
            part: source.backing.main_part().to_string(),
            limit: budget.max_part_bytes,
        });
    }
    let alignment = structure::align(original, current)?;
    let package = source.backing.package();
    let main_name = source.backing.main_part();
    let main = package
        .part(main_name.as_str())
        .expect("imported main part");
    let target = match main.content_type.as_deref() {
        Some(ty) if ty == crate::ns::MAIN_CONTENT_TYPES[0] => ExportTarget::Docx,
        Some(ty) if ty == crate::ns::MAIN_CONTENT_TYPES[1] => ExportTarget::Dotx,
        Some(ty) if ty == crate::ns::MAIN_CONTENT_TYPES[2] => ExportTarget::Docm,
        Some(ty) if ty == crate::ns::MAIN_CONTENT_TYPES[3] => ExportTarget::Dotm,
        _ => return Err(unsupported(None, "unknown package kind")),
    };
    let mut plan = ExportPlan {
        version: current.version(),
        mode: ExportMode::OriginalPackage,
        target,
        bytes: source.backing.original_bytes().into(),
        changed: Vec::new(),
        warnings: Vec::new(),
        preserved: package.parts().map(|part| part.name.clone()).collect(),
        // The caller's backing allocation, not the re-imported copy and not
        // the patched archive later stored in `bytes`.
        origin: Some(backing.original_arc()),
    };
    if alignment.is_unchanged() {
        return Ok(plan);
    }
    if !source.editability.is_unrestricted() {
        return Err(ExportError::Restricted);
    }
    if has_signatures(package) {
        return Err(ExportError::SignedPackage);
    }
    // Refuse templates and macro-enabled output until the plan can express
    // their target type and signature/macro policy explicitly.
    if main.content_type.as_deref()
        != Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml")
    {
        return Err(unsupported(
            None,
            "edited export currently requires ordinary DOCX",
        ));
    }
    let root = XmlReader::new(main_name.as_str(), &main.data, budget)
        .and_then(XmlReader::whole)
        .map_err(|error| ImportError::Xml {
            part: main_name.to_string(),
            error,
        })?;
    let body = root
        .child(W, "body")
        .ok_or_else(|| unsupported(None, "missing source body"))?;
    let xml = structure::Source::new(
        main_name.as_str(),
        &main.data,
        body,
        &source.backing,
        original,
        &[&root, body],
    );
    // Structural limits are reported before any text patch is attempted.
    let mut patches = Vec::new();
    for &paragraph in &alignment.involved {
        xml.plain(paragraph)?;
    }
    for &paragraph in &alignment.removed {
        let element = xml.plain(paragraph)?;
        patches.push((element.span.clone(), Vec::new()));
    }
    for &(before, after) in &alignment.edited {
        let element = xml.paragraph(before)?;
        patch_paragraph(
            current.styles(),
            current.numbering(),
            before,
            after,
            &[&root, body],
            element,
            &mut patches,
            budget.max_part_bytes,
            &xml,
        )?;
    }
    // New paragraphs are children of w:body.
    let names = Names::in_scope(&[&root, body]);
    let mut inserted = 0u64;
    for insertion in &alignment.inserted {
        let at = xml.insertion_point(insertion.anchor)?;
        let mut markup = String::new();
        for block in &insertion.blocks {
            let new = match block {
                structure::NewBlock::Paragraph(paragraph) => {
                    structure::new_paragraph(paragraph, &names, budget.max_part_bytes, &xml)?
                }
                structure::NewBlock::Table(table) => {
                    structure::table_markup(table, &names, budget.max_part_bytes)?
                }
            };
            // Bound the accumulated new markup before building any more.
            inserted = inserted.saturating_add(new.len() as u64);
            markup.push_str(&new);
            if inserted > budget.max_part_bytes {
                return Err(ExportError::Budget {
                    part: main_name.to_string(),
                    limit: budget.max_part_bytes,
                });
            }
        }
        patches.push((at..at, markup.into_bytes()));
    }
    for &(before, after) in &alignment.edited_tables {
        let element = xml.table(before)?;
        let markup = structure::table_markup(after, &names, budget.max_part_bytes)?;
        patches.push((element.span.clone(), markup.into_bytes()));
    }
    for table in &alignment.removed_tables {
        let element = xml.table(table)?;
        patches.push((element.span.clone(), Vec::new()));
    }
    // An insertion sorts before a removal or replacement starting at the
    // same byte, so neither is treated as overlapping.
    patches.sort_by_key(|(range, _)| (range.start, range.end));
    let mut output_len = main.data.len() as u64;
    let mut end = 0;
    for (range, bytes) in &patches {
        if range.start < end || range.end > main.data.len() {
            return Err(unsupported(None, "overlapping XML patches"));
        }
        end = range.end;
        output_len = output_len
            .checked_sub(range.len() as u64)
            .and_then(|n| n.checked_add(bytes.len() as u64))
            .ok_or_else(|| ExportError::Budget {
                part: main_name.to_string(),
                limit: budget.max_part_bytes,
            })?;
    }
    if output_len > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: main_name.to_string(),
            limit: budget.max_part_bytes,
        });
    }
    let total = package.content_types_xml().len() as u64
        + package
            .parts()
            .map(|part| part.data.len() as u64)
            .sum::<u64>();
    if total - main.data.len() as u64 + output_len > budget.max_total_bytes {
        return Err(ExportError::Budget {
            part: "package".into(),
            limit: budget.max_total_bytes,
        });
    }
    let mut main_bytes = Vec::new();
    let mut start = 0;
    for (range, bytes) in patches {
        main_bytes.extend_from_slice(&main.data[start..range.start]);
        main_bytes.extend_from_slice(&bytes);
        start = range.end;
    }
    main_bytes.extend_from_slice(&main.data[start..]);
    let mut items = vec![(
        "[Content_Types].xml",
        package.content_types_xml(),
        Method::Stored,
    )];
    for part in package.parts() {
        let data = if part.name == *main_name {
            main_bytes.as_slice()
        } else {
            &part.data
        };
        // Store the changed part: highly repetitive new text must not create
        // an archive that this very budget rejects for compression ratio.
        let method = if part.name == *main_name {
            Method::Stored
        } else {
            part.method
        };
        items.push((part.name.item_name(), data, method));
    }
    let bytes = write_archive(&items).map_err(|error| ExportError::Write(error.to_string()))?;
    let verified = import(&bytes, budget)?;
    if !verified.editability.is_unrestricted()
        || !compare::equivalent(
            current,
            &source.backing,
            &verified.document,
            &verified.backing,
        )
        || !compare::same_sections(current, &verified.document)
        || !compare::same_preserved(&source.backing, &verified.backing)
    {
        return Err(ExportError::RoundTripMismatch);
    }
    plan.mode = ExportMode::TextPatch;
    plan.warnings.push(ExportWarning::ZipMetadataRegenerated);
    plan.bytes = bytes.into();
    plan.changed.push(main_name.clone());
    plan.preserved.retain(|part| part != main_name);
    Ok(plan)
}

fn has_signatures(package: &joyoffice_opc::Package) -> bool {
    let signed_rel = |source| {
        package
            .relationships(source)
            .iter()
            .any(|rel| rel.kind.contains("digital-signature"))
    };
    signed_rel(None)
        || package.parts().any(|part| {
            part.name
                .as_str()
                .to_ascii_lowercase()
                .starts_with("/_xmlsignatures/")
                || part
                    .content_type
                    .as_deref()
                    .is_some_and(|ty| ty.contains("digital-signature"))
                || signed_rel(Some(&part.name))
        })
}

// Compare all non-text fields, including ones the human-readable semantic
// dump intentionally omits (custom styles, source handles, section anchors).
fn check_envelope(before: &Document, after: &Document) -> Result<(), ExportError> {
    let sections_equal = before.sections().len() == after.sections().len()
        && before
            .sections()
            .iter()
            .zip(after.sections())
            .all(|(a, b)| {
                a.id == b.id
                    && a.last_block == b.last_block
                    && a.page == b.page
                    && a.title_page == b.title_page
                    && a.headers == b.headers
                    && a.footers == b.footers
                    && a.preserved == b.preserved
            });
    let resources_equal = before.resources().len() == after.resources().len()
        && before
            .resources()
            .iter()
            .zip(after.resources().iter())
            .all(|(a, b)| {
                a.id() == b.id()
                    && a.media_type() == b.media_type()
                    && a.data() == b.data()
                    && a.source() == b.source()
            });
    if !resources_equal {
        return Err(unsupported(
            None,
            "image resources changed and media parts are not rewritten",
        ));
    }
    // Body blocks are aligned separately; their count may change.
    if before.body_id() != after.body_id()
        || !auxiliary_stories_match(before, after)
        || before.body().kind != after.body().kind
        || !sections_equal
        || before.settings() != after.settings()
        || before.numbering() != after.numbering()
        || before.styles().defaults != after.styles().defaults
        || before.styles().theme() != after.styles().theme()
        || !before.styles().iter().eq(after.styles().iter())
        || !before.annotations().eq(after.annotations())
    {
        return Err(unsupported(None, "non-text document content changed"));
    }
    Ok(())
}

fn auxiliary_stories_match(before: &Document, after: &Document) -> bool {
    let ids = |document: &Document| {
        let mut ids: Vec<_> = document
            .stories()
            .filter(|story| story.id != document.body_id())
            .map(|story| story.id)
            .collect();
        ids.sort();
        ids
    };
    let left = ids(before);
    if left != ids(after) {
        return false;
    }
    left.into_iter()
        .all(|id| match (before.story(id), after.story(id)) {
            (Some(left), Some(right)) => {
                left.kind == right.kind && story_signature(left) == story_signature(right)
            }
            _ => false,
        })
}

fn story_signature(story: &Story) -> String {
    story
        .blocks
        .iter()
        .map(|block| {
            block
                .as_paragraph()
                .map(|paragraph| {
                    paragraph.plain_text(|object| match &object.kind {
                        InlineKind::PageField(PageFieldKind::Page) => "field:Page".into(),
                        InlineKind::PageField(PageFieldKind::NumPages) => "field:NumPages".into(),
                        InlineKind::Break(BreakKind::Page) => "break:Page".into(),
                        InlineKind::Break(kind) => format!("break:{kind:?}"),
                        _ => "\u{FFFC}".into(),
                    })
                })
                .unwrap_or_else(|| format!("block:{}", block.id()))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(PartialEq)]
struct Group {
    style: Option<StyleId>,
    props: Arc<RunProps>,
    text: String,
}

fn groups(paragraph: &Paragraph) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    let text = paragraph.plain_text(|_| "\u{FFFC}".into());
    let mut chars = text.chars();
    for run in paragraph.runs() {
        let text: String = chars.by_ref().take(run.len).collect();
        if let Some(last) = groups.last_mut()
            && last.style == run.style
            && last.props == run.props
        {
            last.text.push_str(&text);
        } else {
            groups.push(Group {
                style: run.style.clone(),
                props: run.props.clone(),
                text,
            });
        }
    }
    groups
}

fn style_ids(paragraph: &Paragraph) -> Vec<Option<StyleId>> {
    let mut ids = vec![paragraph.style.clone()];
    ids.extend(paragraph.runs().iter().map(|run| run.style.clone()));
    ids
}

/// True when a paragraph or run style id was added, removed or replaced.
/// A text split that only changes how many unstyled runs exist is not a
/// style assignment.
fn style_assignment_differs(before: &Paragraph, after: &Paragraph) -> bool {
    let left = style_ids(before);
    let right = style_ids(after);
    left != right && (left.iter().any(Option::is_some) || right.iter().any(Option::is_some))
}

fn same_paragraph_metadata(a: &Paragraph, b: &Paragraph) -> bool {
    a.id() == b.id()
        && a.style == b.style
        && a.props == b.props
        && a.mark_props == b.mark_props
        && a.source == b.source
}

#[allow(clippy::too_many_arguments)]
fn patch_paragraph(
    styles: &StyleSheet,
    numbering: &NumberingStore,
    before: &Paragraph,
    after: &Paragraph,
    scope: &[&Element],
    element: &Element,
    patches: &mut Vec<(Range<usize>, Vec<u8>)>,
    limit: u64,
    source: &structure::Source<'_>,
) -> Result<(), ExportError> {
    let error = |reason| unsupported(Some(before.id()), reason);
    let numbering_changed = before.props.numbering != after.props.numbering;
    if style_assignment_differs(before, after) || numbering_changed {
        structure::style_assignment_ok(styles, before).map_err(error)?;
        structure::style_assignment_ok(styles, after).map_err(error)?;
        structure::numbering_assignment_ok(numbering, before).map_err(error)?;
        structure::numbering_assignment_ok(numbering, after).map_err(error)?;
        if before.source != after.source
            || !before.inlines().is_empty()
            || !after.inlines().is_empty()
        {
            return Err(error("style assignment cannot be exported"));
        }
        let mut scope = scope.to_vec();
        scope.push(element);
        let names = Names::in_scope(&scope);
        let bytes =
            structure::rewrite_styled_paragraph(source.bytes(), element, after, &names, limit)?;
        patches.push((element.span.clone(), bytes));
        return Ok(());
    }
    if !same_paragraph_metadata(before, after)
        || !before.inlines().is_empty()
        || !after.inlines().is_empty()
    {
        let before_plain = before.props.is_empty()
            && before.mark_props.is_empty()
            && before
                .runs()
                .iter()
                .all(|run| run.style.is_none() && run.props.is_empty());
        if before_plain
            && before.inlines().is_empty()
            && after.inlines().is_empty()
            && structure::direct_m1(before)
            && structure::direct_m1(after)
            && before.style == after.style
            && before.source == after.source
        {
            let mut scope = scope.to_vec();
            scope.push(element);
            let names = Names::in_scope(&scope);
            let bytes =
                structure::rewrite_direct_paragraph(source.bytes(), element, after, &names, limit)?;
            patches.push((element.span.clone(), bytes));
            return Ok(());
        }
        if before
            .inlines()
            .iter()
            .chain(after.inlines())
            .any(|inline| matches!(inline.kind, joyword_model::InlineKind::Image { .. }))
        {
            return Err(error("inline image paragraphs are not rewritten"));
        }
        return Err(error(
            "paragraph formatting, source or inline objects changed",
        ));
    }
    let before_groups = groups(before);
    let after_groups = groups(after);
    let same_bounds = before_groups.len() == after_groups.len()
        && before_groups
            .iter()
            .zip(&after_groups)
            .all(|(a, b)| a.style == b.style && a.props == b.props);
    // A split keeps a prefix of the source run groups on the head. Emptying
    // the runs that moved is the text-leaf patch; adding a group (a merge of
    // differently formatted paragraphs) has to copy source rPr bytes.
    let split_prefix = source.touched(before.id())
        && after_groups.len() < before_groups.len()
        && before_groups
            .iter()
            .zip(&after_groups)
            .all(|(a, b)| a.style == b.style && a.props == b.props);
    if !before.is_empty() && !after_groups.is_empty() && !same_bounds && !split_prefix {
        let before_plain = before.props.is_empty()
            && before.mark_props.is_empty()
            && before
                .runs()
                .iter()
                .all(|run| run.style.is_none() && run.props.is_empty());
        if before_plain
            && before.inlines().is_empty()
            && after.inlines().is_empty()
            && structure::direct_m1(before)
            && structure::direct_m1(after)
            && before.source == after.source
            && text_slots(element, source.bytes()).is_ok()
        {
            let mut scope = scope.to_vec();
            scope.push(element);
            let names = Names::in_scope(&scope);
            let bytes =
                structure::rewrite_direct_paragraph(source.bytes(), element, after, &names, limit)?;
            patches.push((element.span.clone(), bytes));
            return Ok(());
        }
        if source.touched(before.id()) {
            let mut scope = scope.to_vec();
            scope.push(element);
            let names = Names::in_scope(&scope);
            let bytes = source.rebuild(before, after, element, &names, limit)?;
            patches.push((element.span.clone(), bytes));
            return Ok(());
        }
        return Err(error("character formatting boundaries changed"));
    }
    let xml = source.bytes();
    let slots = text_slots(element, xml).map_err(error)?;
    if slots.len() != before.runs().len() {
        return Err(error("source text tokens do not match imported runs"));
    }
    if before.is_empty() && !after.is_empty() {
        if after_groups.len() != 1
            || after_groups[0].style.is_some()
            || !after_groups[0].props.is_empty()
        {
            return Err(error(
                "new text in an empty paragraph requires default character properties",
            ));
        }
        let mut scope = scope.to_vec();
        scope.push(element);
        let names = Names::in_scope(&scope);
        let text = bounded_text_xml(&names, &after_groups[0].text, limit)?;
        let run = format!("{}>{text}{}", names.start("r"), names.end("r"));
        let raw = &xml[element.span.clone()];
        if raw.ends_with(b"/>") {
            // Replace only '/>'; every original attribute/declaration stays.
            let start = element.span.end - 2;
            patches.push((
                start..element.span.end,
                format!(">{run}</{}>", element.qname).into_bytes(),
            ));
        } else {
            let relative = raw
                .iter()
                .rposition(|&b| b == b'<')
                .ok_or_else(|| error("missing paragraph end tag"))?;
            let start = element.span.start + relative;
            if !xml[start..element.span.end].starts_with(b"</") {
                return Err(error("missing paragraph end tag"));
            }
            patches.push((start..start, run.into_bytes()));
        }
        return Ok(());
    }
    let mut group_index = 0;
    let mut previous: Option<&joyword_model::Run> = None;
    for ((leaf, text), run) in slots.into_iter().zip(before.runs()) {
        if run.len != text.chars().count() {
            return Err(error("source text token length changed"));
        }
        let same = previous.is_some_and(|prev| prev.style == run.style && prev.props == run.props);
        if previous.is_some() && !same {
            group_index += 1;
        }
        let replacement = if !same {
            after_groups
                .get(group_index)
                .map_or("", |group| &group.text)
        } else {
            ""
        };
        // A replacement leaf may hold tabs as sibling elements, so it names
        // itself rather than relying on prefixes declared on the old leaf.
        patches.push((
            leaf.span.clone(),
            bounded_text_xml(&Names::Declared, replacement, limit)?.into_bytes(),
        ));
        previous = Some(run);
    }
    Ok(())
}

/// Source text leaves of a paragraph with their text, one per imported text
/// run. Only the plain subset is accepted; no unknown element is
/// reconstructed.
fn text_slots<'e>(
    element: &'e Element,
    xml: &[u8],
) -> Result<Vec<(&'e Element, String)>, &'static str> {
    // One source leaf corresponds to each import push_text call. Empty text
    // leaves do not create runs.
    let mut slots = Vec::new();
    for run in element.elements() {
        if run.is(W, "pPr") || run.is(W, "proofErr") {
            continue;
        }
        if !run.is(W, "r") {
            return Err("unsupported paragraph markup");
        }
        for leaf in run.elements() {
            if leaf.is(W, "rPr") || leaf.is(W, "lastRenderedPageBreak") {
                continue;
            }
            if !(leaf.is(W, "t") || leaf.is(W, "tab")) || leaf.elements().next().is_some() {
                return Err("unsupported run content");
            }
            if leaf.attributes.iter().any(|attr| {
                !(attr.namespace.as_deref() == Some(XML_NAMESPACE)
                    && attr.local.as_ref() == "space"
                    && matches!(attr.value.as_str(), "default" | "preserve"))
            }) {
                return Err("text leaf has unknown attributes");
            }
            if leaf
                .declarations
                .iter()
                .any(|(_, uri)| uri.as_ref() != W && uri.as_ref() != XML_NAMESPACE)
            {
                return Err("text leaf declares an unknown namespace");
            }
            let raw = &xml[leaf.span.clone()];
            if raw.windows(4).any(|w| w == b"<!--") || raw.windows(2).any(|w| w == b"<?") {
                return Err("text leaf contains comments or processing instructions");
            }
            let text = if leaf.is(W, "tab") {
                "\t".to_owned()
            } else {
                leaf.text()
            };
            if !text.is_empty() {
                slots.push((leaf, text));
            }
        }
    }
    Ok(slots)
}

/// How new WordprocessingML elements are named where they are written.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Names {
    /// A prefix already bound to WordprocessingML there, such as `w`.
    Bound(Box<str>),
    /// WordprocessingML is the default namespace there.
    Default,
    /// Nothing usable is in scope; every element declares a private prefix.
    Declared,
}

impl Names {
    /// Names for content of the innermost element of `scope` (outermost
    /// first). What Word itself writes is preferred: some readers match the
    /// `w:` prefix literally instead of resolving namespaces.
    fn in_scope(scope: &[&Element]) -> Self {
        let mut bound: Vec<(&str, &str)> = Vec::new();
        for element in scope {
            for (prefix, uri) in &element.declarations {
                bound.retain(|(declared, _)| *declared != &**prefix);
                bound.push((prefix, uri));
            }
        }
        let is_w = |prefix: &str| bound.contains(&(prefix, W));
        if is_w("w") {
            return Self::Bound("w".into());
        }
        if let Some((prefix, _)) = bound
            .iter()
            .rev()
            .find(|(prefix, uri)| !prefix.is_empty() && *uri == W)
        {
            return Self::Bound((*prefix).into());
        }
        if is_w("") {
            return Self::Default;
        }
        Self::Declared
    }

    /// Start tag up to, but excluding, the closing `>` or `/>`.
    fn start(&self, local: &str) -> String {
        match self {
            Self::Bound(prefix) => format!("<{prefix}:{local}"),
            Self::Default => format!("<{local}"),
            Self::Declared => format!(r#"<jwe:{local} xmlns:jwe="{W}""#),
        }
    }

    fn end(&self, local: &str) -> String {
        match self {
            Self::Bound(prefix) => format!("</{prefix}:{local}>"),
            Self::Default => format!("</{local}>"),
            Self::Declared => format!("</jwe:{local}>"),
        }
    }

    fn qname(&self, local: &str) -> String {
        match self {
            Self::Bound(prefix) => format!("{prefix}:{local}"),
            Self::Default => local.to_string(),
            Self::Declared => format!("jwe:{local}"),
        }
    }

    /// ` prefix:local="escaped"`, using the same prefix as [`Self::start`].
    fn attribute(&self, local: &str, value: &str) -> String {
        let mut escaped = String::new();
        for ch in value.chars() {
            match ch {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '"' => escaped.push_str("&quot;"),
                _ => escaped.push(ch),
            }
        }
        format!(" {}=\"{escaped}\"", self.qname(local))
    }
}

const PRESERVE: &str = r#" xml:space="preserve">"#;

fn bounded_text_xml(names: &Names, text: &str, limit: u64) -> Result<String, ExportError> {
    // XML 1.0 §2.2 Char; a tab becomes markup and a carriage return a
    // character reference. Nothing else can stand for the rest.
    if let Some(ch) = text.chars().find(|&ch| {
        !matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
    }) {
        return Err(unsupported(
            None,
            match ch {
                '\u{FFFE}' | '\u{FFFF}' => "text contains a noncharacter XML cannot represent",
                _ => "text contains a control character XML cannot represent",
            },
        ));
    }
    // Count the exact escaped output before allocating, including tab markup.
    let tab = names.start("tab").len() as u64 + 2;
    let leaf = (names.start("t").len() + PRESERVE.len() + names.end("t").len()) as u64;
    let mut size = 0u64;
    for (index, part) in text.split('\t').enumerate() {
        if index > 0 {
            size = size.saturating_add(tab);
        }
        if part.is_empty() {
            continue;
        }
        size = size.saturating_add(leaf);
        for ch in part.chars() {
            size = size.saturating_add(match ch {
                '&' => 5,
                '<' | '>' => 4,
                '\r' => 5,
                _ => ch.len_utf8() as u64,
            });
        }
        if size > limit {
            return Err(ExportError::Budget {
                part: "text leaf".into(),
                limit,
            });
        }
    }
    if size > limit {
        return Err(ExportError::Budget {
            part: "text leaf".into(),
            limit,
        });
    }
    Ok(text_xml(names, text))
}

fn text_xml(names: &Names, text: &str) -> String {
    let mut xml = String::new();
    for (index, part) in text.split('\t').enumerate() {
        if index > 0 {
            xml.push_str(&names.start("tab"));
            xml.push_str("/>");
        }
        if part.is_empty() {
            continue;
        }
        xml.push_str(&names.start("t"));
        xml.push_str(PRESERVE);
        for ch in part.chars() {
            match ch {
                '&' => xml.push_str("&amp;"),
                '<' => xml.push_str("&lt;"),
                '>' => xml.push_str("&gt;"),
                '\r' => xml.push_str("&#13;"),
                _ => xml.push(ch),
            }
        }
        xml.push_str(&names.end("t"));
    }
    xml
}

#[cfg(test)]
mod origin_binding {
    use std::path::PathBuf;
    use std::sync::Arc;

    use joyoffice_opc::Budget;

    use super::plan_export;
    use crate::import;

    fn fixture(id: u8) -> Vec<u8> {
        std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../tests/fixtures/docx/DOCX-MIN-{id:03}.docx")),
        )
        .unwrap()
    }

    #[test]
    fn origin_shares_the_backing_allocation_and_clone_keeps_it() {
        let bytes = fixture(1);
        let imported = import(&bytes, &Budget::default()).unwrap();
        let plan = plan_export(
            &imported.backing,
            &imported.document.snapshot(),
            &Budget::default(),
        )
        .unwrap();
        let origin = plan.origin.as_ref().expect("proven origin");
        assert!(Arc::ptr_eq(origin, &imported.backing.original_arc()));
        assert_eq!(&origin[..], bytes.as_slice());
        let cloned = plan.clone();
        assert!(Arc::ptr_eq(origin, cloned.origin.as_ref().unwrap()));
    }
}
