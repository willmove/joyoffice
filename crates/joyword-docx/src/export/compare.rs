//! Verification of a written package against the document it should hold.
//!
//! Re-import changes identifiers, byte locators and text-token partitioning,
//! none of which is semantic. Preserved handles come from the same counter as
//! node identifiers, so a paragraph added or removed ahead of a preserved
//! fragment shifts its handle. Handles are therefore compared by what they
//! keep: the fragment's part, element, namespace context and exact bytes.
//! Which block ends a section is checked separately, because the semantic
//! dump does not name it.

use std::collections::HashMap;
use std::sync::Arc;

use joyword_model::{
    Block, BlockSeq, Document, Hyperlink, IdAllocator, InlineKind, InlineObject, NodeId,
    OpaqueBlock, ParaProps, Paragraph, PreservedId, RunProps, Segment, StyleId,
};

use crate::PackageBacking;

/// What a preserved handle refers to: part, element, in-scope namespace
/// declarations and exact bytes.
type Key<'a> = (&'a str, &'a str, &'a [(Arc<str>, Arc<str>)], &'a [u8]);

/// `actual` (re-imported with `actual_backing`) holds the same semantic
/// content as `expected` (whose handles refer to `expected_backing`).
pub(super) fn equivalent(
    expected: &Document,
    expected_backing: &PackageBacking,
    actual: &Document,
    actual_backing: &PackageBacking,
) -> bool {
    if expected.semantic_difference(actual).is_none() {
        return true;
    }
    let (Some(expected_keys), Some(actual_keys)) = (
        body_handles(expected, expected_backing),
        body_handles(actual, actual_backing),
    ) else {
        return false;
    };
    let mut keys: Vec<&Key<'_>> = expected_keys.values().chain(actual_keys.values()).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut ids = IdAllocator::default();
    let canonical: HashMap<&Key<'_>, PreservedId> =
        keys.into_iter().map(|key| (key, ids.preserved())).collect();
    let canonical = |handles: &HashMap<PreservedId, Key<'_>>, handle: PreservedId| {
        handles
            .get(&handle)
            .and_then(|key| canonical.get(key))
            .copied()
    };
    let expected = normalize(expected, &|handle| canonical(&expected_keys, handle));
    let actual = normalize(actual, &|handle| canonical(&actual_keys, handle));
    matches!((expected, actual), (Some(e), Some(a)) if e.semantic_difference(&a).is_none())
}

/// Every preserved handle the body refers to, with what it keeps.
fn body_handles<'a>(
    document: &Document,
    backing: &'a PackageBacking,
) -> Option<HashMap<PreservedId, Key<'a>>> {
    let mut handles = Vec::new();
    for block in document.body().blocks.iter() {
        match block {
            Block::Paragraph(paragraph) => {
                handles.extend(paragraph.props.preserved);
                handles.extend(paragraph.mark_props.preserved);
                handles.extend(
                    paragraph
                        .runs()
                        .iter()
                        .filter_map(|run| run.props.preserved),
                );
                handles.extend(paragraph.inlines().iter().filter_map(
                    |inline| match &inline.kind {
                        InlineKind::Preserved(preserved) => Some(preserved.handle),
                        _ => None,
                    },
                ));
            }
            Block::Opaque(opaque) => handles.push(opaque.preserved.handle),
            // The importer does not produce these yet; their handles stay
            // as they are and must then match exactly.
            Block::Table(_) | Block::Sdt(_) => {}
        }
    }
    handles
        .into_iter()
        .map(|handle| {
            let fragment = backing.fragment(handle)?;
            let key = (
                fragment.part.as_str(),
                &*fragment.element,
                fragment.namespaces.as_slice(),
                backing.fragment_bytes(handle)?,
            );
            Some((handle, key))
        })
        .collect()
}

fn remap(
    handle: Option<PreservedId>,
    canonical: &dyn Fn(PreservedId) -> Option<PreservedId>,
) -> Option<Option<PreservedId>> {
    match handle {
        None => Some(None),
        Some(handle) => canonical(handle).map(Some),
    }
}

/// The body with canonical handles and adjacent equally formatted text
/// joined into one run.
fn normalize(
    document: &Document,
    canonical: &dyn Fn(PreservedId) -> Option<PreservedId>,
) -> Option<Document> {
    let blocks = document
        .body()
        .blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph(paragraph) => rebuild(paragraph, canonical).map(Block::from),
            Block::Opaque(opaque) => {
                let mut opaque = OpaqueBlock::clone(opaque);
                opaque.preserved.handle = canonical(opaque.preserved.handle)?;
                Some(Block::from(opaque))
            }
            other => Some(other.clone()),
        })
        .collect::<Option<Vec<_>>>()?;
    let mut normalized = document.clone();
    let body = normalized.body_id();
    normalized.story_mut(body).expect("body").blocks = BlockSeq::from_vec(blocks);
    Some(normalized)
}

enum Position<'a> {
    Char(char),
    Inline(&'a InlineObject),
}

/// Text collected for one run of equal formatting and the same hyperlink.
type Pending = (
    Option<StyleId>,
    Arc<RunProps>,
    Option<Arc<Hyperlink>>,
    String,
);

fn flush(rebuilt: &mut Paragraph, pending: &mut Option<Pending>) -> Option<()> {
    match pending.take() {
        Some((style, props, link, text)) => {
            rebuilt.push_text(&text, style, props).ok()?;
            rebuilt.set_last_run_link(link);
            Some(())
        }
        None => Some(()),
    }
}

fn rebuild(
    paragraph: &Paragraph,
    canonical: &dyn Fn(PreservedId) -> Option<PreservedId>,
) -> Option<Paragraph> {
    let mut rebuilt = Paragraph::new(paragraph.id());
    rebuilt.style = paragraph.style.clone();
    rebuilt.props = Arc::new(ParaProps {
        preserved: remap(paragraph.props.preserved, canonical)?,
        ..ParaProps::clone(&paragraph.props)
    });
    rebuilt.mark_props = Arc::new(RunProps {
        preserved: remap(paragraph.mark_props.preserved, canonical)?,
        ..RunProps::clone(&paragraph.mark_props)
    });
    rebuilt.source = paragraph.source.clone();
    let segments = paragraph.segments();
    let mut positions = segments.iter().flat_map(|segment| match segment {
        Segment::Text(text) => text.chars().map(Position::Char).collect::<Vec<_>>(),
        Segment::Inline(inline) => vec![Position::Inline(inline)],
    });
    let mut pending: Option<Pending> = None;
    for run in paragraph.runs() {
        let props = Arc::new(RunProps {
            preserved: remap(run.props.preserved, canonical)?,
            ..RunProps::clone(&run.props)
        });
        for _ in 0..run.len {
            match positions.next()? {
                Position::Char(ch) => match &mut pending {
                    Some((style, same, link, text))
                        if *style == run.style && *same == props && *link == run.link =>
                    {
                        text.push(ch);
                    }
                    _ => {
                        flush(&mut rebuilt, &mut pending)?;
                        pending = Some((
                            run.style.clone(),
                            props.clone(),
                            run.link.clone(),
                            ch.into(),
                        ));
                    }
                },
                Position::Inline(inline) => {
                    flush(&mut rebuilt, &mut pending)?;
                    let mut inline = inline.clone();
                    if let InlineKind::Preserved(preserved) = &mut inline.kind {
                        preserved.handle = canonical(preserved.handle)?;
                    }
                    rebuilt.push_inline(inline, run.style.clone(), props.clone());
                    rebuilt.set_last_run_link(run.link.clone());
                }
            }
        }
    }
    if positions.next().is_some() {
        return None;
    }
    flush(&mut rebuilt, &mut pending)?;
    Some(rebuilt)
}

/// Section ends of `actual` fall after the same body block positions as in
/// `expected`.
pub(super) fn same_sections(expected: &Document, actual: &Document) -> bool {
    fn boundaries(document: &Document) -> Option<Vec<Option<usize>>> {
        let index: HashMap<NodeId, usize> = document
            .body()
            .blocks
            .iter()
            .enumerate()
            .map(|(i, block)| (block.id(), i))
            .collect();
        document
            .sections()
            .iter()
            .map(|section| match section.last_block {
                None => Some(None),
                Some(id) => index.get(&id).map(|&i| Some(i)),
            })
            .collect()
    }
    let expected = boundaries(expected);
    expected.is_some() && expected == boundaries(actual)
}

/// Both packages keep the same preserved fragments, byte for byte; only
/// their positions may differ.
pub(super) fn same_preserved(expected: &PackageBacking, actual: &PackageBacking) -> bool {
    fn fragments(backing: &PackageBacking) -> Option<Vec<Key<'_>>> {
        let mut all = backing
            .fragments()
            .map(|(id, fragment)| {
                Some((
                    fragment.part.as_str(),
                    &*fragment.element,
                    fragment.namespaces.as_slice(),
                    backing.fragment_bytes(id)?,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        all.sort_unstable();
        Some(all)
    }
    let expected = fragments(expected);
    expected.is_some() && expected == fragments(actual)
}

#[cfg(test)]
mod tests {
    use joyoffice_opc::{Budget, Method, write_archive};

    use super::*;
    use crate::{ImportResult, W, import};

    fn imported(body: &str) -> ImportResult {
        let document = format!(r#"<w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#);
        let items: [(&str, &[u8], Method); 3] = [
            (
                "[Content_Types].xml",
                br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
                Method::Stored,
            ),
            (
                "_rels/.rels",
                br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
                Method::Stored,
            ),
            ("word/document.xml", document.as_bytes(), Method::Stored),
        ];
        import(&write_archive(&items).unwrap(), &Budget::default()).unwrap()
    }

    const BODY: &str = r#"<w:p><w:r><w:rPr><w:highlight w:val="yellow"/></w:rPr><w:t>A</w:t></w:r></w:p><w:p><w:r><w:rPr><w:highlight w:val="green"/></w:rPr><w:t>B</w:t></w:r></w:p>"#;

    fn replace(document: &mut Document, index: usize, paragraph: Paragraph) {
        let body = document.body_id();
        document
            .story_mut(body)
            .unwrap()
            .blocks
            .update(index, |block| *block = paragraph.into());
    }

    fn paragraph(document: &Document, index: usize) -> &Paragraph {
        document
            .body()
            .blocks
            .get(index)
            .unwrap()
            .as_paragraph()
            .unwrap()
    }

    #[test]
    fn shifted_handles_compare_by_kept_bytes() {
        let source = imported(BODY);
        let shifted = imported(&format!("<w:p><w:r><w:t>X</w:t></w:r></w:p>{BODY}"));
        let mut expected = source.document.clone();
        let mut lead = Paragraph::new(expected.ids().node());
        lead.push_text("X", None, Arc::default()).unwrap();
        let body = expected.body_id();
        expected
            .story_mut(body)
            .unwrap()
            .blocks
            .insert(0, lead.into());
        assert!(expected.semantic_difference(&shifted.document).is_some());
        assert!(equivalent(
            &expected,
            &source.backing,
            &shifted.document,
            &shifted.backing
        ));
        assert!(same_preserved(&source.backing, &shifted.backing));
    }

    #[test]
    fn different_kept_bytes_or_text_still_differ() {
        let source = imported(BODY);
        let document = &source.document;
        assert!(equivalent(
            document,
            &source.backing,
            document,
            &source.backing
        ));
        // Swap the two highlight fragments between the paragraphs.
        let mut swapped = document.clone();
        for (index, other) in [(0, 1), (1, 0)] {
            let original = paragraph(document, index);
            let run = &original.runs()[0];
            let mut props = RunProps::clone(&run.props);
            props.preserved = paragraph(document, other).runs()[0].props.preserved;
            let mut rebuilt = Paragraph::new(original.id());
            rebuilt.source = original.source.clone();
            rebuilt
                .push_text(
                    &original.plain_text(|_| String::new()),
                    None,
                    Arc::new(props),
                )
                .unwrap();
            replace(&mut swapped, index, rebuilt);
        }
        assert!(!equivalent(
            &swapped,
            &source.backing,
            document,
            &source.backing
        ));
        // Changed text with identical handles.
        let mut edited = document.clone();
        let mut changed = paragraph(document, 0).clone();
        changed
            .insert_text(joyword_model::ScalarOffset(1), "!")
            .unwrap();
        replace(&mut edited, 0, changed);
        assert!(!equivalent(
            &edited,
            &source.backing,
            document,
            &source.backing
        ));
    }

    #[test]
    fn section_boundaries_compare_by_block_position() {
        let source = imported(r#"<w:p><w:pPr><w:sectPr/></w:pPr></w:p><w:p/>"#);
        let document = &source.document;
        assert!(same_sections(document, document));
        let mut moved = document.clone();
        let second = moved.body().blocks.get(1).unwrap().id();
        moved.sections_mut()[0].last_block = Some(second);
        assert!(!same_sections(document, &moved));
        let mut dangling = document.clone();
        let removed = dangling.ids().node();
        dangling.sections_mut()[0].last_block = Some(removed);
        assert!(!same_sections(&dangling, document));
    }
}
