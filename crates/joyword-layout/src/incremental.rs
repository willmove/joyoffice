//! Reuse unchanged page geometry, or repaginate a bounded chapter at stable page starts.

mod sections;

use std::{collections::BTreeMap, sync::Weak};

use joyword_model::{Settings, section::PageVAlign};

use super::{
    Arc, Blocks, DocLayout, Document, FieldCtx, LayoutCache, LayoutOptions, LineEnd, Paragraph,
    Path, Placed, SectionProps, StoryRef, TextDirection, ViewMode, cache::env_hash, layout, now_ms,
    para,
};

// Small documents already paginate quickly. Avoid holding an extra set of block references.
const MIN_PAGES: usize = 64;

pub(crate) struct Previous {
    body: Blocks,
    notes: BTreeMap<u32, joyword_model::Part>,
    section_context: bool,
    requires_section_flow: bool,
    media: BTreeMap<String, Arc<Vec<u8>>>,
    page_stories: bool,
    endnotes: Option<crate::notes::Endnotes>,
    /// First and last body anchor for a text story, including all its linked frames.
    boxes: BTreeMap<u32, (usize, usize)>,
    body_fields: bool,
    list_starts: Vec<crate::Counters>,
    /// Keep shaping work for the last verified edit and its bounded set of field sections.
    checked_section: Option<sections::SectionCache>,
    section: SectionProps,
    settings: Settings,
    options: LayoutOptions,
    env: u64,
    layout: Weak<DocLayout>,
}

enum Change {
    Body(usize),
    Part(u32),
}

/// Reuse stable page geometry or paginate the affected section group to a checked exit.
pub fn layout_shared(
    doc: &Document,
    cache: &mut LayoutCache,
    opts: &LayoutOptions,
) -> Arc<DocLayout> {
    let start = now_ms();
    if let Some(mut previous) = cache.previous.take()
        && let Some((change, mut next)) = previous.replace(doc, opts)
    {
        next.ms = now_ms() - start;
        let next = Arc::new(next);
        match change {
            Change::Body(block) => previous.body[block] = doc.body[block].clone(),
            Change::Part(id) => {
                previous.notes.insert(id, doc.parts[&id].clone());
            }
        }
        previous.options.typing_word = opts.typing_word;
        previous.layout = Arc::downgrade(&next);
        cache.previous = Some(previous);
        return next;
    }

    let endnotes = crate::notes::Endnotes::collect(doc, opts.hides_deleted(StoryRef::Body));
    // Bound the number of saved list instances across all section boundaries.
    // Each instance can have at most nine live levels.
    let capture_list_starts = !doc.numbering.nums.is_empty()
        && doc
            .sections()
            .len()
            .saturating_mul(doc.numbering.nums.len())
            <= 8192
        && eligible(doc, opts);
    let layout = Arc::new(crate::layout_at(
        doc,
        cache,
        opts,
        crate::PageOrigin {
            endnotes: endnotes.as_ref(),
            capture_list_starts,
            ..Default::default()
        },
    ));
    let mut requires_section_flow = endnotes.is_some() || uses_section_geometry(&doc.last_section);
    let mut body_fields = false;
    if layout.pages.len() >= MIN_PAGES
        && eligible(doc, opts)
        && doc.parts.values().all(|part| match part.kind {
            joyword_model::PartKind::Header | joyword_model::PartKind::Footer => part
                .blocks
                .iter()
                .all(|b| b.as_para().is_some_and(|p| running_paragraph(doc, p))),
            joyword_model::PartKind::TextBox => supported_box(doc, part),
            joyword_model::PartKind::Footnote | joyword_model::PartKind::Endnote => part
                .blocks
                .iter()
                .all(|b| b.as_para().is_some_and(|p| plain_paragraph(doc, p))),
            _ => false,
        })
        && doc.body.iter().all(|block| match block.as_ref() {
            joyword_model::Block::Para(p) => {
                let props = doc.styles.resolve_para(&p.props);
                let listed = props.numbering.is_some_and(|n| n.num != 0);
                requires_section_flow |= props.frame.is_some()
                    || props.drop_cap > 0
                    || listed
                    || p.section.as_deref().is_some_and(uses_section_geometry);
                (!listed || capture_list_starts)
                    && p.objects.iter().all(|object| {
                        body_fields |= matches!(object, joyword_model::InlineObject::Field { .. });
                        requires_section_flow |= object.is_drawing() || body_fields;
                        supported_body_object(object)
                    })
            }
            joyword_model::Block::Table(t) => table_flow(doc, t, None).is_some_and(|flow| {
                requires_section_flow |= flow != TableFlow::Inline;
                body_fields |= flow == TableFlow::PageFields;
                true
            }),
        })
        && let Some(boxes) = box_anchors(doc)
    {
        cache.previous = Some(Previous {
            boxes,
            endnotes,
            body: doc.body.clone(),
            notes: doc.parts.clone(),
            requires_section_flow,
            body_fields,
            list_starts: std::mem::take(&mut cache.list_starts),
            media: doc.media.clone(),
            page_stories: doc.parts.values().any(|p| {
                matches!(
                    p.kind,
                    joyword_model::PartKind::Header | joyword_model::PartKind::Footer
                )
            }),
            checked_section: None,
            section_context: requires_section_flow
                || doc.settings.even_odd_headers
                || doc.settings.mirror_margins
                || !doc.parts.is_empty()
                || doc.last_section.columns.count != 1
                || doc.body.iter().any(|b| {
                    b.as_para()
                        .is_some_and(|p| p.section.is_some() || !p.objects.is_empty())
                }),
            section: doc.last_section.clone(),
            settings: doc.settings.clone(),
            options: opts.clone(),
            env: cache.env,
            layout: Arc::downgrade(&layout),
        });
    }
    layout
}

fn supported_body_object(object: &joyword_model::InlineObject) -> bool {
    use joyword_model::InlineObject::*;
    if let Field { instr, .. } = object {
        return matches!(
            crate::fields::field_name(instr).as_str(),
            "PAGE" | "NUMPAGES" | "SECTION" | "SECTIONPAGES"
        );
    }
    matches!(
        object,
        NoteRef { .. }
            | BookmarkStart { .. }
            | BookmarkEnd { .. }
            | Image { .. }
            | Graphic { .. }
            | Shape { .. }
            | Group { .. }
    )
}

fn running_paragraph(doc: &Document, p: &Paragraph) -> bool {
    let props = doc.styles.resolve_para(&p.props);
    p.section.is_none()
        && props.frame.is_none()
        && props.drop_cap == 0
        && props.numbering.is_none()
        && p.objects
            .iter()
            .all(|o| matches!(o, joyword_model::InlineObject::Field { .. }))
}

fn plain_paragraph(doc: &Document, p: &Paragraph) -> bool {
    let r = doc.styles.resolve_para(&p.props);
    p.section.is_none() && p.objects.is_empty() && r.frame.is_none() && r.drop_cap == 0
}

#[derive(PartialEq, Eq)]
enum TableFlow {
    Inline,
    Section,
    PageFields,
}

/// Tables with nested cells or floating anchors require the section's complete
/// flow state. Bound the total visited cells and blocks across every nested table.
fn table_flow(
    doc: &Document,
    table: &joyword_model::Table,
    typing: Option<crate::TypingWord>,
) -> Option<TableFlow> {
    let mut owns_typing = typing.is_none();
    let mut pending = vec![(table, 0)];
    let mut result = TableFlow::Inline;
    let (mut cells, mut blocks) = (0usize, 0usize);
    while let Some((table, depth)) = pending.pop() {
        if table.rows.len() > 128 || depth > joyword_model::table::MAX_TABLE_DEPTH {
            return None;
        }
        if result == TableFlow::Inline && (depth > 0 || table.props.float.is_some()) {
            result = TableFlow::Section;
        }
        for row in &table.rows {
            cells = cells.checked_add(row.cells.len())?;
            if cells > 512 {
                return None;
            }
            for cell in &row.cells {
                blocks = blocks.checked_add(cell.blocks.len())?;
                if blocks > 512 {
                    return None;
                }
                for block in &cell.blocks {
                    match block.as_ref() {
                        joyword_model::Block::Table(t) => pending.push((t, depth + 1)),
                        joyword_model::Block::Para(p) => {
                            let props = doc.styles.resolve_para(&p.props);
                            if p.section.is_some()
                                || props.frame.is_some()
                                || props.drop_cap != 0
                                || props.numbering.is_some()
                            {
                                return None;
                            }
                            if !p.objects.is_empty() {
                                if !p.objects.iter().all(|o| {
                                    matches!(o, joyword_model::InlineObject::Field { .. })
                                        && supported_body_object(o)
                                }) {
                                    return None;
                                }
                                result = TableFlow::PageFields;
                            }
                            owns_typing |= typing.is_some_and(|word| p.rev == word.rev);
                        }
                    }
                }
            }
        }
    }
    owns_typing.then_some(result)
}

fn supported_box(doc: &Document, part: &joyword_model::Part) -> bool {
    part.blocks.iter().all(|b| match b.as_ref() {
        joyword_model::Block::Para(p) => {
            plain_paragraph(doc, p) && doc.styles.resolve_para(&p.props).numbering.is_none()
        }
        joyword_model::Block::Table(t) => matches!(
            table_flow(doc, t, None),
            Some(TableFlow::Inline | TableFlow::Section)
        ),
    })
}

fn box_anchors(doc: &Document) -> Option<BTreeMap<u32, (usize, usize)>> {
    let mut anchors = BTreeMap::new();
    if !doc
        .parts
        .values()
        .any(|p| p.kind == joyword_model::PartKind::TextBox)
    {
        return Some(anchors);
    }
    // Tables admitted by table_flow have no drawing objects. All other
    // eligible frames are in body paragraphs, including the children of groups.
    for (block, p) in doc
        .body
        .iter()
        .enumerate()
        .filter_map(|(i, b)| Some((i, b.as_para()?)))
    {
        let mut pending: Vec<_> = p.objects.iter().filter(|o| o.is_drawing()).collect();
        while let Some(o) = pending.pop() {
            match o {
                joyword_model::InlineObject::Group { children, .. } => {
                    pending.extend(children.iter().map(|child| &child.obj))
                }
                joyword_model::InlineObject::Shape {
                    story: Some(id), ..
                } => {
                    if doc.parts.get(id)?.kind != joyword_model::PartKind::TextBox {
                        return None;
                    }
                    anchors.entry(*id).or_insert((block, block)).1 = block;
                }
                _ => {}
            }
        }
    }
    for chain in doc.text_box_chains() {
        let mut bounds = (usize::MAX, 0);
        for id in &chain {
            let (first, last) = *anchors.get(id)?;
            bounds = (bounds.0.min(first), bounds.1.max(last));
        }
        for id in chain {
            anchors.insert(id, bounds);
        }
    }
    Some(anchors)
}

fn eligible(doc: &Document, opts: &LayoutOptions) -> bool {
    opts.view == ViewMode::Print
        && !opts.comment_balloons
        && doc.comments.is_empty()
        && doc.revisions.is_empty()
        && doc.footnote_separators.is_empty()
        && doc.endnote_separators.is_empty()
}

// A single paragraph cannot determine which column width to use after a break,
// the turned page coordinates, or the vertical offset of a whole page.
fn uses_section_geometry(section: &SectionProps) -> bool {
    !section.columns.widths.is_empty()
        || section.text_direction.is_turned()
        || section.valign != PageVAlign::Top
        || section.line_numbers.is_some()
}

impl Previous {
    fn replace(&mut self, doc: &Document, opts: &LayoutOptions) -> Option<(Change, DocLayout)> {
        if self.body.len() != doc.body.len()
            || self.section != doc.last_section
            || self.settings != doc.settings
            || !eligible(doc, opts)
            || self.notes.len() != doc.parts.len()
            || self.media.len() != doc.media.len()
            || self
                .media
                .iter()
                .any(|(key, old)| !doc.media.get(key).is_some_and(|new| Arc::ptr_eq(old, new)))
            || self.options
                != (LayoutOptions {
                    typing_word: self.options.typing_word,
                    ..opts.clone()
                })
            || self.env != env_hash(doc, opts)
            || (self.endnotes.is_some()
                && self.endnotes
                    != crate::notes::Endnotes::collect(doc, opts.hides_deleted(StoryRef::Body)))
        {
            return None;
        }
        let mut edited_part = None;
        for (id, old) in &self.notes {
            let new = doc.parts.get(id)?;
            if old.kind != new.kind || old.body != new.body {
                return None;
            }
            if old.blocks.len() != new.blocks.len()
                || old
                    .blocks
                    .iter()
                    .zip(&new.blocks)
                    .any(|(a, b)| !Arc::ptr_eq(a, b))
            {
                if edited_part.is_some()
                    || !matches!(
                        new.kind,
                        joyword_model::PartKind::Footnote
                            | joyword_model::PartKind::Endnote
                            | joyword_model::PartKind::TextBox
                            | joyword_model::PartKind::Header
                            | joyword_model::PartKind::Footer
                    )
                    || !match new.kind {
                        joyword_model::PartKind::TextBox => supported_box(doc, new),
                        joyword_model::PartKind::Footnote | joyword_model::PartKind::Endnote => new
                            .blocks
                            .iter()
                            .all(|b| b.as_para().is_some_and(|p| plain_paragraph(doc, p))),
                        _ => new.blocks.iter().all(|b| {
                            b.as_para().is_some_and(|p| {
                                plain_paragraph(doc, p) && running_paragraph(doc, p)
                            })
                        }),
                    }
                    || [(self.options.typing_word, old), (opts.typing_word, new)]
                        .into_iter()
                        .any(|(word, part)| {
                            word.is_some_and(|w| {
                                !part
                                    .blocks
                                    .iter()
                                    .any(|b| b.as_para().is_some_and(|p| p.rev == w.rev))
                            })
                        })
                {
                    return None;
                }
                edited_part = Some(*id);
            }
        }
        let mut changes = self
            .body
            .iter()
            .zip(&doc.body)
            .enumerate()
            .filter(|(_, (before, after))| !Arc::ptr_eq(before, after));
        if let Some(id) = edited_part {
            if changes.next().is_some() {
                return None;
            }
            if matches!(
                self.notes.get(&id)?.kind,
                joyword_model::PartKind::Header | joyword_model::PartKind::Footer
            ) {
                // A plain running story can be edited without paginating the body
                // if every linked section still reserves the same space for it.
                if !self
                    .notes
                    .get(&id)?
                    .blocks
                    .iter()
                    .all(|b| b.as_para().is_some_and(|p| plain_paragraph(doc, p)))
                {
                    return None;
                }
                let current = self.layout.upgrade()?;
                let mut before = doc.clone();
                before.parts = self.notes.clone();
                let mut cache = LayoutCache::new();
                if !crate::pass::same_story_limits(&before, doc, opts, &mut cache, &current.pages) {
                    return None;
                }
                let mut next = (*current).clone();
                let initially_clean = next.pages.len()..next.pages.len();
                let dirty = sections::update_page_stories(
                    doc,
                    opts,
                    &mut cache,
                    &mut next.pages,
                    &current,
                    initially_clean,
                );
                next.page_patch = Some((Arc::downgrade(&current), dirty));
                self.checked_section = None;
                return Some((Change::Part(id), next));
            }
            let anchor = match doc.parts.get(&id)?.kind {
                joyword_model::PartKind::TextBox => self.boxes.get(&id)?.0,
                joyword_model::PartKind::Endnote => {
                    let plan = self.endnotes.as_ref()?;
                    if let Some(section) = plan.sections.iter().position(|ids| ids.contains(&id)) {
                        doc.sections().get(section)?.0
                    } else if plan.document.contains(&id) {
                        doc.body.len().checked_sub(1)?
                    } else {
                        return None;
                    }
                }
                _ => doc.body.iter().position(|b| b.as_para().is_some_and(|p| p.objects.iter().any(|o|
                    matches!(o, joyword_model::InlineObject::NoteRef { id: target, .. } if *target == id))))?,
            };
            return self
                .reflow_sections(doc, opts, anchor, &self.layout.upgrade()?)
                .map(|layout| (Change::Part(id), layout));
        }
        let (block, (before, after)) = changes.next()?;
        if changes.next().is_some() {
            return None;
        }
        if let (Some(old), Some(table)) = (before.as_table(), after.as_table()) {
            if table_flow(doc, old, self.options.typing_word)?
                != table_flow(doc, table, opts.typing_word)?
            {
                return None;
            }
            let current = self.layout.upgrade()?;
            return self
                .reflow(doc, opts, block, &current)
                .map(|layout| (Change::Body(block), layout));
        }
        let (before, after) = (before.as_para()?, after.as_para()?);
        if self
            .options
            .typing_word
            .is_some_and(|t| t.rev != before.rev)
            || opts.typing_word.is_some_and(|t| t.rev != after.rev)
        {
            return None;
        }

        let current = self.layout.upgrade()?;
        if self.requires_section_flow || self.page_stories || !ordinary_edit(before, after) {
            if before.props != after.props
                || before.section != after.section
                || before.mark != after.mark
                || before.objects != after.objects
                || !self.section_context
            {
                return None;
            }
            return self
                .reflow_sections(doc, opts, block, &current)
                .map(|layout| (Change::Body(block), layout));
        }
        let path = Path::top(block);
        let [(page, item)] = current.index.get(&(StoryRef::Body, path))?.as_slice() else {
            return self
                .reflow(doc, opts, block, &current)
                .map(|layout| (Change::Body(block), layout));
        };
        let page_ref = current.pages.get(*page)?;
        let Placed::Lines {
            para: old,
            l0: 0,
            l1,
            turn: TextDirection::Horizontal,
            ..
        } = page_ref.items.get(*item)?
        else {
            return None;
        };
        if *l1 != old.lines.len()
            || old.rp.numbering.is_some()
            || old.label.is_some()
            || old.rp.mirror_indents
        {
            return None;
        }
        let section = if self.section_context {
            doc.sections().into_iter().find(|(end, _)| *end >= block)?.1
        } else {
            &self.section
        };
        let width = if section.columns.count == 1 {
            section.text_width().max(18.)
        } else {
            section.column_boxes().first()?.1
        };
        let new = para::layout_para(
            after,
            &para::ParaEnv {
                doc,
                width,
                label: None,
                fields: &FieldCtx::default(),
                show_hidden: opts.show_hidden,
                field_codes: opts.field_codes,
                hide_deleted: opts.hides_deleted(StoryRef::Body),
                table: None,
                proofing: opts.proofing.then_some(opts.proof),
                typing_word: opts.typing_word.map(|t| (t.start, t.end)),
                numeral: opts.numeral,
                exclusions: &[],
                eq_number: 0,
                read_line_spacing: None,
                grid: para::Grid::of(doc, section),
                fit_objects: false,
                fitted: &[],
            },
        );
        if !same_flow(old, &new) {
            return self
                .reflow(doc, opts, block, &current)
                .map(|layout| (Change::Body(block), layout));
        }

        let mut next = (*current).clone();
        let Placed::Lines { para, .. } = next.pages.get_mut(*page)?.items.get_mut(*item)? else {
            return None;
        };
        *para = Arc::new(new);
        next.page_patch = Some((Arc::downgrade(&current), *page..*page + 1));
        self.checked_section = None;
        Some((Change::Body(block), next))
    }

    /// Reflow a plain chapter bracketed by explicit page starts. First replay its old
    /// geometry and match every line against the original pages. A different page count,
    /// unstable entry state or a paragraph that depends on surrounding stories uses the
    /// complete paginator. The UI can keep pages on both sides of a successful replacement.
    fn reflow(
        &mut self,
        doc: &Document,
        opts: &LayoutOptions,
        block: usize,
        current: &Arc<DocLayout>,
    ) -> Option<DocLayout> {
        const MAX_BLOCKS: usize = 128;
        const MAX_PAGES: usize = 16;
        if self.section_context {
            return self.reflow_sections(doc, opts, block, current);
        }
        let is_start = |i: usize| -> bool {
            doc.body
                .get(i)
                .and_then(|b| b.as_para())
                .is_some_and(|p| doc.styles.resolve_para(&p.props).page_break_before)
        };
        let page_at = |i| {
            current
                .index
                .get(&(StoryRef::Body, Path::top(i)))
                .or_else(|| {
                    current
                        .index
                        .get(&(StoryRef::Body, Path(vec![i as u32, 0, 0, 0])))
                })?
                .first()
                .map(|&(page, _)| page)
        };
        let chapter = (block.saturating_sub(MAX_BLOCKS)..=block)
            .rev()
            .find(|&i| i == 0 || is_start(i))
            .and_then(|start| {
                let end = (block + 1..=doc.body.len().min(start + MAX_BLOCKS))
                    .find(|&i| i == doc.body.len() || is_start(i))?;
                let stop = if end == doc.body.len() {
                    current.pages.len()
                } else {
                    page_at(end)?
                };
                Some((start, end, page_at(start)?, stop))
            });
        let natural = chapter.is_none();
        let (start, end, first, stop) = match chapter {
            Some(bounds) => bounds,
            None => {
                // Start before the edit, on a whole unchanged paragraph. The preceding page
                // has already rejected this same paragraph. Leave one unchanged page as an
                // exit checkpoint so downstream positions can be reused.
                let edited_page = page_at(block)?;
                let page_head = |page: usize| match current.pages.get(page)?.items.first()? {
                    Placed::Lines {
                        story: StoryRef::Body,
                        path,
                        l0: 0,
                        ..
                    } if path.depth() == 0 => Some(path.last()),
                    _ => None,
                };
                let (first, start) = (edited_page.saturating_sub(3)..=edited_page)
                    .rev()
                    .find_map(|p| page_head(p).filter(|&i| i < block).map(|i| (p, i)))?;
                let (stop, end) = (edited_page + 2..current.pages.len().min(edited_page + 6))
                    .find_map(|p| page_head(p).map(|i| (p, i)))?;
                if end.saturating_sub(start) > MAX_BLOCKS {
                    return None;
                }
                (start, end, first, stop)
            }
        };
        let original = current.pages.get(first..stop)?;
        if original.is_empty()
            || original.len() > MAX_PAGES
            || doc.last_section.title_page
            || doc.body[start..end].iter().any(|b| {
                let Some(p) = b.as_para() else {
                    return b
                        .as_table()
                        .is_none_or(|t| table_flow(doc, t, None) != Some(TableFlow::Inline));
                };
                let r = doc.styles.resolve_para(&p.props);
                (natural
                    && p.text.contains([
                        joyword_model::para::PAGE_BREAK,
                        joyword_model::para::COLUMN_BREAK,
                    ]))
                    || r.numbering.is_some()
                    || r.keep_next
                    || r.mirror_indents
                    || r.contextual_spacing
                    || r.borders.is_some()
                    || r.shading.is_some()
                    || !p.objects.is_empty()
                    || p.section.is_some()
            })
        {
            return None;
        }
        // A break-before at the top and a break taken after a paragraph can treat space before
        // differently. Admit a new entry state only when this spacing is zero in both cases.
        let heading = doc.styles.resolve_para(&doc.body[start].as_para()?.props);
        if heading.space_before != 0. || heading.before_auto {
            return None;
        }
        let mut fragment = doc.clone();
        fragment.body = self.body[start..end].to_vec();
        let mut scratch = LayoutCache::new();
        let old = layout(&fragment, &mut scratch, &self.options);
        if old.pages.len() != original.len()
            || !old
                .pages
                .iter()
                .zip(original)
                .all(|(piece, original)| same_page_entry(piece, original, start))
        {
            return None;
        }
        fragment.body[block - start] = doc.body[block].clone();
        let replacement = layout(&fragment, &mut scratch, opts);
        if replacement.pages.len() != original.len()
            || (natural && !same_page_entry(replacement.pages.last()?, original.last()?, start))
        {
            return None;
        }
        let mut next = current.as_ref().clone();
        next.index.retain(|(story, path), _| {
            *story != StoryRef::Body
                || path
                    .0
                    .first()
                    .is_none_or(|&b| !(start..end).contains(&(b as usize)))
        });
        for (local, mut page) in replacement.pages.into_iter().enumerate() {
            let absolute = first + local;
            let target = next.pages.get_mut(absolute)?;
            for (index, item) in page.items.iter_mut().enumerate() {
                let (path, indexed) = match item {
                    Placed::Lines {
                        story: StoryRef::Body,
                        path,
                        ..
                    } => (path, true),
                    Placed::Cell {
                        story: StoryRef::Body,
                        table,
                        ..
                    } => (table, false),
                    Placed::Fill { .. } | Placed::Rule { .. } => continue,
                    _ => return None,
                };
                let top = path.0.first_mut()?;
                *top = top.checked_add(u32::try_from(start).ok()?)?;
                if !indexed {
                    continue;
                }
                next.index
                    .entry((StoryRef::Body, path.clone()))
                    .or_default()
                    .push((absolute, index));
            }
            target.items = page.items;
            target.first_block = start + page.first_block;
        }
        next.page_patch = Some((Arc::downgrade(current), first..stop));
        Some(next)
    }
}

fn same_body_path(local: &Path, absolute: &Path, offset: usize) -> bool {
    local
        .0
        .split_first()
        .zip(absolute.0.split_first())
        .is_some_and(|((&a, tail_a), (&b, tail_b))| {
            (a as usize).checked_add(offset) == Some(b as usize) && tail_a == tail_b
        })
}

fn same_page_entry(piece: &super::Page, original: &super::Page, start: usize) -> bool {
    if piece.w != original.w
        || piece.h != original.h
        || piece.body != original.body
        || piece.numbered_lines_before != original.numbered_lines_before
        || piece.first_block + start != original.first_block
        || piece.items.len() != original.items.len()
        || piece.float_tables.len() != original.float_tables.len()
        || !piece
            .float_tables
            .iter()
            .zip(&original.float_tables)
            .all(|(a, b)| same_body_path(a, b, start))
    {
        return false;
    }
    same_items(&piece.items, &original.items, start)
}

fn same_items(left: &[Placed], right: &[Placed], start: usize) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| match (a, b) {
            (
                Placed::Lines {
                    story: left_story,
                    path: ap,
                    para: a,
                    l0: al,
                    l1: ae,
                    x: ax,
                    y: ay,
                    turn: at,
                },
                Placed::Lines {
                    story: right_story,
                    path: bp,
                    para: b,
                    l0: bl,
                    l1: be,
                    x: bx,
                    y: by,
                    turn: bt,
                },
            ) => {
                let path_matches = left_story == right_story
                    && match left_story {
                        StoryRef::Body => same_body_path(ap, bp, start),
                        StoryRef::Part(_) => ap == bp,
                    };
                path_matches
                    && a.shown == b.shown
                    && (al, ae, ax, ay, at) == (bl, be, bx, by, bt)
                    && a.rp == b.rp
                    && a.styles == b.styles
                    && a.label == b.label
                    && a.drop_cap == b.drop_cap
                    && a.drop_cap_w == b.drop_cap_w
                    && a.issues == b.issues
                    && a.height == b.height
                    && a.notes == b.notes
                    && a.lines == b.lines
                    && a.glyphs == b.glyphs
                    && a.clusters == b.clusters
            }
            (
                Placed::Cell {
                    rect: a,
                    covered: ac,
                    table: at,
                    row: ar,
                    cell: ai,
                    story: as_,
                    clip: ap,
                },
                Placed::Cell {
                    rect: b,
                    covered: bc,
                    table: bt,
                    row: br,
                    cell: bi,
                    story: bs,
                    clip: bp,
                },
            ) => {
                as_ == bs
                    && (if *as_ == StoryRef::Body {
                        same_body_path(at, bt, start)
                    } else {
                        at == bt
                    })
                    && (a, ac, ar, ai, ap) == (b, bc, br, bi, bp)
            }
            (
                Placed::Clip { rect, spin, items },
                Placed::Clip {
                    rect: r,
                    spin: s,
                    items: i,
                },
            ) => rect == r && spin == s && same_items(items, i, start),
            (Placed::Fill { rect: a, color: ac }, Placed::Fill { rect: b, color: bc }) => {
                (a, ac) == (b, bc)
            }
            (
                Placed::Rule {
                    x0: ax,
                    y0: ay,
                    x1: bx,
                    y1: by,
                    border: ab,
                },
                Placed::Rule {
                    x0: cx,
                    y0: cy,
                    x1: dx,
                    y1: dy,
                    border: bb,
                },
            ) => (ax, ay, bx, by, ab) == (cx, cy, dx, dy, bb),
            _ => start == 0 && sections::same_drawing(a, b),
        })
}

fn ordinary_edit(before: &Paragraph, after: &Paragraph) -> bool {
    before.props == after.props
        && before.mark == after.mark
        && before.section.is_none()
        && after.section.is_none()
        && before.objects.is_empty()
        && after.objects.is_empty()
        && [&before.text, &after.text].into_iter().all(|text| {
            !text.is_empty()
                && !text.contains([
                    joyword_model::para::PAGE_BREAK,
                    joyword_model::para::COLUMN_BREAK,
                    joyword_model::para::OBJ,
                ])
        })
}

fn same_flow(old: &para::ParaLayout, new: &para::ParaLayout) -> bool {
    old.rp == new.rp
        && old.rp.borders.is_none()
        && old.rp.shading.is_none()
        && old.height == new.height
        && old.left_out.is_empty()
        && new.left_out.is_empty()
        && old.lines.len() == new.lines.len()
        && old
            .lines
            .last()
            .is_some_and(|line| line.end == LineEnd::Para)
        && old.lines.iter().zip(&new.lines).all(|(before, after)| {
            // Page fitting can use the text bottom instead of the line height.
            before.top == after.top
                && before.height == after.height
                && before.left == after.left
                && before.right == after.right
                && before.baseline == after.baseline
                && before.text_desc == after.text_desc
                && before.end == after.end
                && !before.beside
                && !after.beside
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(lay: &DocLayout, block: usize) -> Option<&Arc<para::ParaLayout>> {
        let &(page, item) = lay
            .index
            .get(&(StoryRef::Body, Path::top(block)))?
            .first()?;
        match lay.pages.get(page)?.items.get(item)? {
            Placed::Lines { para, .. } => Some(para),
            _ => None,
        }
    }

    #[test]
    fn reuses_page_flow_for_single_line_edits_and_falls_back_on_wrapping()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = Document::new();
        doc.body.clear();
        for _ in 0..MIN_PAGES {
            doc.body
                .push(joyword_model::para_block(Paragraph::with_text(
                    "中 A paragraph for typing",
                    Default::default(),
                )));
            doc.body
                .push(joyword_model::para_block(Paragraph::with_text(
                    "\u{000c}",
                    Default::default(),
                )));
        }
        let mut cache = LayoutCache::new();
        let mut opts = LayoutOptions::default();
        let mut previous = layout_shared(&doc, &mut cache, &opts);
        assert!(previous.pages.len() >= MIN_PAGES);
        for block in [0, (MIN_PAGES - 1) * 2] {
            let changed = doc.para_mut(StoryRef::Body, &Path::top(block))?;
            changed.insert_text(0, "你 ", &Default::default())?;
            // A move from the first paragraph ends its typing state before this edit.
            opts.typing_word = Some(crate::TypingWord {
                rev: changed.rev,
                start: 0,
                end: "你".len(),
            });
            let next = layout_shared(&doc, &mut cache, &opts);
            let full = layout(&doc, &mut LayoutCache::new(), &opts);
            assert_eq!(next.changed_page_from(&previous), Some(block / 2));
            assert_eq!(next.index, full.index);
            assert_eq!(format!("{:?}", next.pages), format!("{:?}", full.pages));
            assert!(Arc::ptr_eq(
                paragraph(&previous, 5).ok_or("missing original paragraph")?,
                paragraph(&next, 5).ok_or("missing retained paragraph")?,
            ));
            opts.typing_word = None;
            previous = layout_shared(&doc, &mut cache, &opts);
        }
        doc.para_mut(StoryRef::Body, &Path::top(0))?.insert_text(
            0,
            &"words ".repeat(150),
            &Default::default(),
        )?;
        let wrapped = layout_shared(&doc, &mut cache, &opts);
        assert_eq!(wrapped.changed_page_from(&previous), None);
        let full = layout(&doc, &mut LayoutCache::new(), &opts);
        assert_eq!(wrapped.index, full.index);
        assert_eq!(format!("{:?}", wrapped.pages), format!("{:?}", full.pages));
        Ok(())
    }

    #[test]
    fn chapter_reflow_matches_full_pagination_after_inserting_and_deleting_lines()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = Document::new();
        doc.body.clear();
        doc.styles.default_chr.font = Some("Source Sans 3".into());
        doc.last_section.page_h = 240.;
        doc.last_section.margin_top = 24.;
        doc.last_section.margin_bottom = 24.;
        let text = "Alpha beta gamma. ".repeat(9);
        for _ in 0..64 {
            for i in 0..12 {
                let mut p = Paragraph::with_text(&text, Default::default());
                p.props.page_break_before = Some(i == 0);
                p.props.space_before = Some(0.);
                doc.body.push(joyword_model::para_block(p));
            }
        }
        let opts = LayoutOptions::default();
        let mut cache = LayoutCache::new();
        let mut previous = layout_shared(&doc, &mut cache, &opts);
        let block = 32 * 12 + 3;
        let change = "Extra words that wrap. ".repeat(4);
        for insert in [true, false] {
            let para = doc.para_mut(StoryRef::Body, &Path::top(block))?;
            if insert {
                para.insert_text(0, &change, &Default::default())?;
            } else {
                para.delete(0, change.len())?;
            }
            let next = layout_shared(&doc, &mut cache, &opts);
            let pages = next
                .changed_pages_from(&previous)
                .ok_or("expected local chapter reflow")?;
            assert!(pages.len() >= 2 && pages.len() < 16);
            let full = layout(&doc, &mut LayoutCache::new(), &opts);
            assert_eq!(next.index, full.index);
            for (a, b) in next.pages.iter().zip(&full.pages) {
                assert_eq!(format!("{a:?}"), format!("{b:?}"));
            }
            previous = next;
        }
        Ok(())
    }

    #[test]
    fn natural_page_checkpoint_matches_full_layout_and_propagation_falls_back()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = Document::new();
        doc.body.clear();
        doc.styles.default_chr.font = Some("Source Sans 3".into());
        doc.last_section.page_h = 240.;
        doc.last_section.margin_top = 24.;
        doc.last_section.margin_bottom = 24.;
        // Four-line paragraphs moving whole leave room for the short paragraphs between them.
        for i in 0..800 {
            let words = if i % 2 == 0 { 16 } else { 3 };
            let mut p =
                Paragraph::with_text(&"alpha beta gamma ".repeat(words), Default::default());
            p.props.keep_lines = Some(true);
            p.props.space_before = Some(0.);
            doc.body.push(joyword_model::para_block(p));
        }
        let opts = LayoutOptions::default();
        let mut cache = LayoutCache::new();
        let old = layout_shared(&doc, &mut cache, &opts);
        assert!(old.pages.len() >= MIN_PAGES);
        let phrase = " words".repeat(15);
        let block = 401;
        doc.para_mut(StoryRef::Body, &Path::top(block))?
            .insert_text(0, &phrase, &Default::default())?;
        let next = layout_shared(&doc, &mut cache, &opts);
        assert!(
            paragraph(&old, block).ok_or("old")?.lines.len()
                < paragraph(&next, block).ok_or("new")?.lines.len()
        );
        assert!(
            next.changed_pages_from(&old)
                .is_some_and(|r| (2..=8).contains(&r.len()))
        );
        let full = layout(&doc, &mut LayoutCache::new(), &opts);
        assert_eq!(full.index, next.index);
        assert_eq!(format!("{:?}", full.pages), format!("{:?}", next.pages));
        // A long insertion can't converge by the saved page.
        doc.para_mut(StoryRef::Body, &Path::top(block))?
            .insert_text(0, &phrase.repeat(180), &Default::default())?;
        let changed = layout_shared(&doc, &mut cache, &opts);
        assert!(changed.changed_pages_from(&next).is_none());
        assert_eq!(
            changed.index,
            layout(&doc, &mut LayoutCache::new(), &opts).index
        );
        Ok(())
    }

    #[test]
    fn reflows_an_inline_table_and_updates_nested_hit_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = Document::new();
        doc.body.clear();
        doc.styles.default_chr.font = Some("Source Sans 3".into());
        for i in 0..96 {
            let mut p = Paragraph::with_text(&format!("Chapter {i}"), Default::default());
            p.props.page_break_before = Some(true);
            p.props.space_before = Some(0.);
            doc.body.push(joyword_model::para_block(p));
            if i == 48 {
                let mut table = joyword_model::Table::new(2, 2, 220.);
                table.rows[0].cells[0] = joyword_model::Cell::with_text("Original cell");
                doc.body.push(Arc::new(joyword_model::Block::Table(table)));
            }
        }
        let opts = LayoutOptions::default();
        let mut cache = LayoutCache::new();
        let old = layout_shared(&doc, &mut cache, &opts);
        let path = Path(vec![49, 0, 0, 0]);
        doc.para_mut(StoryRef::Body, &path)?.insert_text(
            0,
            &"A longer cell value ".repeat(8),
            &Default::default(),
        )?;
        let next = layout_shared(&doc, &mut cache, &opts);
        assert_eq!(next.changed_pages_from(&old), Some(48..49));
        let full = layout(&doc, &mut LayoutCache::new(), &opts);
        assert_eq!(next.index, full.index);
        assert_eq!(format!("{:?}", next.pages), format!("{:?}", full.pages));
        let bounds = next
            .index
            .get(&(StoryRef::Body, path))
            .ok_or("new cell index")?;
        assert!(bounds.iter().all(|(page, _)| *page == 48));
        Ok(())
    }

    #[test]
    fn keeps_section_columns_and_footnotes_when_editing_their_page()
    -> Result<(), Box<dyn std::error::Error>> {
        use joyword_model::{InlineObject, PartKind, Pos, para::NoteKind};
        let mut doc = Document::new();
        doc.body.clear();
        doc.styles.default_chr.font = Some("Source Sans 3".into());
        for page in 0..80 {
            let mut text = Paragraph::with_text("First body line", Default::default());
            text.props.page_break_before = Some(page != 0);
            doc.body.push(joyword_model::para_block(text));
            doc.body
                .push(joyword_model::para_block(Paragraph::with_text(
                    "Second body line",
                    Default::default(),
                )));
        }
        let mut first_section = doc.last_section.clone();
        first_section.columns.count = 2;
        doc.para_mut(StoryRef::Body, &Path::top(119))?.section = Some(Box::new(first_section));
        let note = doc.add_part(
            PartKind::Footnote,
            vec![joyword_model::para_block(Paragraph::with_text(
                "Note on the edited page",
                Default::default(),
            ))],
        );
        doc.insert_object(
            &Pos::body(80, 5),
            InlineObject::NoteRef {
                kind: NoteKind::Footnote,
                id: note,
                custom: String::new(),
            },
            &Default::default(),
        )?;
        let mut cache = LayoutCache::new();
        let opts = LayoutOptions::default();
        let old = layout_shared(&doc, &mut cache, &opts);
        let target = Path::top(81);
        let &(page, _) = old
            .index
            .get(&(StoryRef::Body, target.clone()))
            .and_then(|v| v.first())
            .ok_or("body index")?;
        let note_path = (StoryRef::Part(note), Path::top(0));
        assert!(
            old.index
                .get(&note_path)
                .is_some_and(|v| v.iter().any(|(p, _)| *p == page))
        );
        doc.para_mut(StoryRef::Body, &target)?
            .insert_text(0, "Text ", &Default::default())?;
        let next = layout_shared(&doc, &mut cache, &opts);
        assert_eq!(next.changed_pages_from(&old), Some(page..page + 1));
        let full = layout(&doc, &mut LayoutCache::new(), &opts);
        assert_eq!(full.index, next.index);
        assert_eq!(format!("{:?}", full.pages), format!("{:?}", next.pages));
        // An independent note edit can change the space reserved below the columns.
        doc.para_mut(StoryRef::Part(note), &Path::top(0))?
            .insert_text(0, &"Long note ".repeat(90), &Default::default())?;
        doc.para_mut(StoryRef::Body, &target)?
            .insert_text(0, "New ", &Default::default())?;
        let changed = layout_shared(&doc, &mut cache, &opts);
        assert!(changed.changed_pages_from(&next).is_none());
        assert_eq!(
            changed.index,
            layout(&doc, &mut LayoutCache::new(), &opts).index
        );
        Ok(())
    }

    #[test]
    fn reuses_wrapped_paragraph_when_every_line_keeps_its_geometry()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = Document::new();
        doc.body.clear();
        let text =
            "中 This paragraph contains enough words to wrap without a manual break. ".repeat(3);
        for _ in 0..MIN_PAGES {
            doc.body
                .push(joyword_model::para_block(Paragraph::with_text(
                    &text,
                    Default::default(),
                )));
            doc.body
                .push(joyword_model::para_block(Paragraph::with_text(
                    "\u{000c}",
                    Default::default(),
                )));
        }
        let mut cache = LayoutCache::new();
        let opts = LayoutOptions::default();
        let old = layout_shared(&doc, &mut cache, &opts);
        let block = (MIN_PAGES - 1) * 2;
        assert!(paragraph(&old, block).is_some_and(|p| p.lines.len() > 1));
        let para = doc.para_mut(StoryRef::Body, &Path::top(block))?;
        para.insert_text(para.text.len(), "x", &Default::default())?;
        let next = layout_shared(&doc, &mut cache, &opts);
        let full = layout(&doc, &mut LayoutCache::new(), &opts);
        assert_eq!(next.changed_page_from(&old), Some(MIN_PAGES - 1));
        assert_eq!(next.index, full.index);
        assert_eq!(format!("{:?}", next.pages), format!("{:?}", full.pages));
        Ok(())
    }
}
