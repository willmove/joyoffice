//! Header and footer part writeback (ECMA-376 Part 1 §17.11.2 `hdr`,
//! §17.11.3 `ftr`, §17.11.6 `headerReference`, §17.11.5 `footerReference`).
//!
//! A changed story is written into the part its paragraphs were imported
//! from. Unchanged parts are left out of the replacement list, so the package
//! writer copies their original bytes. A new external hyperlink id is appended
//! to that part's existing relationships part. Markup this exporter cannot
//! represent, including a story with no part, is refused rather than invented.

use std::ops::Range;

use joyoffice_opc::{Budget, Package, PartName};
use joyword_model::{Document, Paragraph, Story, StoryKind};

use super::structure::{self, Source};
use super::{ExportError, Names, markers_of, patch_paragraph, splice, unsupported};
use crate::xml::{Element, XmlReader};
use crate::{ImportError, PackageBacking, W};

pub(super) struct PartWrite {
    pub name: PartName,
    pub bytes: Vec<u8>,
    /// `Some` only when the rewrite allocated a relationship id.
    pub rels: Option<(PartName, Vec<u8>)>,
}

/// Header or footer story content differs. Other stories are ignored here;
/// the envelope check still requires them to match.
pub(super) fn stories_differ(original: &Document, current: &Document) -> bool {
    let changed = |document: &Document, other: &Document| {
        document.stories().any(|story| {
            if !is_header_footer(story.kind) {
                return false;
            }
            match other.story(story.id) {
                Some(after) => story.kind != after.kind || !same_blocks(story, after),
                None => true,
            }
        })
    };
    changed(original, current) || changed(current, original)
}

pub(super) fn plan(
    package: &Package,
    backing: &PackageBacking,
    original: &Document,
    current: &Document,
    budget: &Budget,
) -> Result<Vec<PartWrite>, ExportError> {
    let mut writes = Vec::new();
    for story in original.stories() {
        if !is_header_footer(story.kind) {
            continue;
        }
        let Some(after) = current.story(story.id) else {
            return Err(unsupported(
                None,
                "header or footer story structure is not rewritten",
            ));
        };
        if story.kind != after.kind {
            return Err(unsupported(
                None,
                "header or footer story structure is not rewritten",
            ));
        }
        if same_blocks(story, after) {
            continue;
        }
        let write = write_story(package, backing, original, current, story, after, budget)?;
        if writes
            .iter()
            .any(|existing: &PartWrite| existing.name == write.name)
        {
            return Err(unsupported(
                None,
                "header or footer part is shared by stories",
            ));
        }
        writes.push(write);
    }
    Ok(writes)
}

/// Byte growth of replacement parts relative to the package. Negative when a
/// part shrinks.
pub(super) fn size_delta(package: &Package, writes: &[PartWrite]) -> i128 {
    let mut delta = 0i128;
    for write in writes {
        delta += growth(package, &write.name, write.bytes.len());
        if let Some((name, bytes)) = &write.rels {
            delta += growth(package, name, bytes.len());
        }
    }
    delta
}

fn growth(package: &Package, name: &PartName, new_len: usize) -> i128 {
    let old = package
        .part(name.as_str())
        .map(|part| part.data.len() as i128)
        .unwrap_or(0);
    new_len as i128 - old
}

fn is_header_footer(kind: StoryKind) -> bool {
    matches!(kind, StoryKind::Header | StoryKind::Footer)
}

fn same_blocks(before: &Story, after: &Story) -> bool {
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

fn write_story(
    package: &Package,
    backing: &PackageBacking,
    original: &Document,
    current: &Document,
    before: &Story,
    after: &Story,
    budget: &Budget,
) -> Result<PartWrite, ExportError> {
    let part_str = story_part(before)?;
    let part_name = PartName::new(part_str)
        .map_err(|_| unsupported(None, "header or footer part name is invalid"))?;
    let part = package
        .part(part_name.as_str())
        .ok_or_else(|| unsupported(None, "header or footer part is missing"))?;
    let root = XmlReader::new(part_name.as_str(), &part.data, budget)
        .and_then(XmlReader::whole)
        .map_err(|error| ImportError::Xml {
            part: part_name.to_string(),
            error,
        })?;
    let expected = match before.kind {
        StoryKind::Header => "hdr",
        StoryKind::Footer => "ftr",
        _ => {
            return Err(unsupported(
                None,
                "header or footer part is not a story part",
            ));
        }
    };
    if !root.is(W, expected) {
        return Err(unsupported(
            None,
            "header or footer part is not a story part",
        ));
    }
    if let Some(bytes) = empty_root(before, after, &part.data, &root, budget.max_part_bytes)? {
        return Ok(PartWrite {
            name: part_name,
            bytes,
            rels: None,
        });
    }
    let xml = Source::new(
        part_name.as_str(),
        &part.data,
        &root,
        backing,
        original,
        &[&root],
        current.styles().theme().colors.clone(),
    );
    let edited = paragraph_edits(before, after)?;
    let mut patches = Vec::new();
    for (was, now) in edited {
        let element = xml.paragraph(was)?;
        patch_paragraph(
            current.styles(),
            current.numbering(),
            was,
            now,
            &markers_of(original, was.id()),
            &markers_of(current, now.id()),
            &[&root],
            element,
            &mut patches,
            budget.max_part_bytes,
            &xml,
        )?;
    }
    let bytes = apply_patches(&part.data, patches, budget.max_part_bytes, &part_name)?;
    let rels = fresh_relationships(package, &part_name, &xml, budget)?;
    Ok(PartWrite {
        name: part_name,
        bytes,
        rels,
    })
}

fn story_part(story: &Story) -> Result<&str, ExportError> {
    let mut found = None;
    for block in story.blocks.iter() {
        let paragraph = block.as_paragraph().ok_or_else(|| {
            unsupported(
                Some(block.id()),
                "header or footer block is not a paragraph",
            )
        })?;
        let source = paragraph.source.as_deref().ok_or_else(|| {
            unsupported(
                Some(paragraph.id()),
                "header or footer paragraph has no part",
            )
        })?;
        match found {
            None => found = Some(source.part.as_ref()),
            Some(part) if part == source.part.as_ref() => {}
            Some(_) => {
                return Err(unsupported(
                    Some(paragraph.id()),
                    "header or footer story spans parts",
                ));
            }
        }
    }
    found.ok_or_else(|| unsupported(None, "header or footer paragraph has no part"))
}

/// A part with no `w:p` is imported as one empty paragraph whose source is
/// the root element. Typing into it writes one plain paragraph. Any other
/// markup stays refused.
fn empty_root(
    before: &Story,
    after: &Story,
    xml: &[u8],
    root: &Element,
    limit: u64,
) -> Result<Option<Vec<u8>>, ExportError> {
    let Some(was) = single_paragraph(before) else {
        return Ok(None);
    };
    let Some(span) = was
        .source
        .as_deref()
        .and_then(|source| locator_span(&source.locator))
    else {
        return Ok(None);
    };
    if span != (root.span.start, root.span.end) {
        return Ok(None);
    }
    if root.elements().next().is_some() {
        return Err(unsupported(
            Some(was.id()),
            "header or footer markup is not rewritten",
        ));
    }
    let now = single_paragraph(after).ok_or_else(|| {
        unsupported(
            Some(was.id()),
            "header or footer story structure is not rewritten",
        )
    })?;
    if was.id() != now.id() || was.source != now.source {
        return Err(unsupported(
            Some(now.id()),
            "header or footer story structure is not rewritten",
        ));
    }
    if !plain_paragraph(now) {
        return Err(unsupported(
            Some(now.id()),
            "empty header or footer accepts only plain text",
        ));
    }
    let names = Names::in_scope(&[root]);
    let markup = plain_paragraph_markup(&names, &now.plain_text(|_| String::new()), limit)?;
    let bytes = splice_into_root(xml, root, &markup)?;
    if bytes.len() as u64 > limit {
        return Err(ExportError::Budget {
            part: was
                .source
                .as_deref()
                .map(|source| source.part.to_string())
                .unwrap_or_else(|| "header".into()),
            limit,
        });
    }
    Ok(Some(bytes))
}

fn single_paragraph(story: &Story) -> Option<&Paragraph> {
    if story.blocks.len() != 1 {
        return None;
    }
    story.blocks.get(0)?.as_paragraph()
}

fn plain_paragraph(paragraph: &Paragraph) -> bool {
    paragraph.style.is_none()
        && paragraph.props.is_empty()
        && paragraph.mark_props.is_empty()
        && paragraph.inlines().is_empty()
        && paragraph.runs().iter().all(|run| {
            run.style.is_none()
                && run.props.is_empty()
                && run.link.is_none()
                && run.revision.is_none()
        })
}

fn plain_paragraph_markup(names: &Names, text: &str, limit: u64) -> Result<String, ExportError> {
    if text.is_empty() {
        return Ok(format!("{}/>", names.start("p")));
    }
    let leaf = super::bounded_text_xml(names, text, limit)?;
    Ok(format!(
        "{}>{}>{leaf}{}{}",
        names.start("p"),
        names.start("r"),
        names.end("r"),
        names.end("p")
    ))
}

fn splice_into_root(xml: &[u8], root: &Element, markup: &str) -> Result<Vec<u8>, ExportError> {
    let raw = &xml[root.span.clone()];
    if raw.ends_with(b"/>") {
        let mut replacement = Vec::new();
        replacement.push(b'>');
        replacement.extend_from_slice(markup.as_bytes());
        replacement.extend_from_slice(format!("</{}>", root.qname).as_bytes());
        return Ok(splice(
            xml,
            (root.span.end - 2)..root.span.end,
            &replacement,
        ));
    }
    let relative = raw
        .iter()
        .rposition(|&byte| byte == b'<')
        .ok_or_else(|| unsupported(None, "header or footer element has no end tag"))?;
    let at = root.span.start + relative;
    if !xml[at..root.span.end].starts_with(b"</") {
        return Err(unsupported(None, "header or footer element has no end tag"));
    }
    Ok(splice(xml, at..at, markup.as_bytes()))
}

fn paragraph_edits<'a>(
    before: &'a Story,
    after: &'a Story,
) -> Result<Vec<(&'a Paragraph, &'a Paragraph)>, ExportError> {
    if before.blocks.len() != after.blocks.len() {
        return Err(unsupported(
            None,
            "header or footer story structure is not rewritten",
        ));
    }
    let mut edited = Vec::new();
    for (left, right) in before.blocks.iter().zip(after.blocks.iter()) {
        let Some(was) = left.as_paragraph() else {
            return Err(unsupported(
                Some(left.id()),
                "header or footer block is not a paragraph",
            ));
        };
        let Some(now) = right.as_paragraph() else {
            return Err(unsupported(
                Some(right.id()),
                "header or footer block is not a paragraph",
            ));
        };
        if was.id() != now.id() || was.source != now.source {
            return Err(unsupported(
                Some(now.id()),
                "header or footer story structure is not rewritten",
            ));
        }
        if was != now {
            edited.push((was, now));
        }
    }
    Ok(edited)
}

fn fresh_relationships(
    package: &Package,
    part_name: &PartName,
    xml: &Source<'_>,
    budget: &Budget,
) -> Result<Option<(PartName, Vec<u8>)>, ExportError> {
    let fresh = xml.fresh_hyperlinks();
    if fresh.is_empty() {
        return Ok(None);
    }
    let rels_name = part_name.relationships_part();
    let rels = package
        .part(rels_name.as_str())
        .ok_or_else(|| unsupported(None, "header or footer relationships part is missing"))?;
    let appended = structure::append_hyperlink_relationships(&rels.data, &fresh)?;
    if appended.len() as u64 > budget.max_part_bytes {
        return Err(ExportError::Budget {
            part: rels_name.to_string(),
            limit: budget.max_part_bytes,
        });
    }
    Ok(Some((rels_name, appended)))
}

pub(super) fn apply_patches(
    xml: &[u8],
    mut patches: Vec<(Range<usize>, Vec<u8>)>,
    limit: u64,
    part: &PartName,
) -> Result<Vec<u8>, ExportError> {
    patches.sort_by_key(|(range, _)| (range.start, range.end));
    let mut output_len = xml.len() as u64;
    let mut end = 0;
    for (range, bytes) in &patches {
        if range.start < end || range.end > xml.len() {
            return Err(unsupported(None, "overlapping XML patches"));
        }
        end = range.end;
        output_len = output_len
            .checked_sub(range.len() as u64)
            .and_then(|n| n.checked_add(bytes.len() as u64))
            .ok_or_else(|| ExportError::Budget {
                part: part.to_string(),
                limit,
            })?;
    }
    if output_len > limit {
        return Err(ExportError::Budget {
            part: part.to_string(),
            limit,
        });
    }
    let mut out = Vec::new();
    let mut start = 0;
    for (range, bytes) in patches {
        out.extend_from_slice(&xml[start..range.start]);
        out.extend_from_slice(&bytes);
        start = range.end;
    }
    out.extend_from_slice(&xml[start..]);
    Ok(out)
}

fn locator_span(locator: &str) -> Option<(usize, usize)> {
    let (start, end) = locator.strip_prefix('@')?.split_once("..")?;
    Some((start.parse().ok()?, end.parse().ok()?))
}
