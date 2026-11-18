//! Reuse unchanged page geometry, or repaginate a bounded chapter at stable page starts.

use std::sync::Weak;

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
    section: SectionProps,
    settings: Settings,
    options: LayoutOptions,
    env: u64,
    layout: Weak<DocLayout>,
}

/// Return the editor's layout. A paragraph wholly on one page can be replaced in situ if
/// its vertical geometry is unchanged and the document has no dependent objects or stories.
pub fn layout_shared(
    doc: &Document,
    cache: &mut LayoutCache,
    opts: &LayoutOptions,
) -> Arc<DocLayout> {
    let start = now_ms();
    if let Some(mut previous) = cache.previous.take()
        && let Some((block, mut next)) = previous.replace(doc, opts)
    {
        next.ms = now_ms() - start;
        let next = Arc::new(next);
        previous.body[block] = doc.body[block].clone();
        previous.options.typing_word = opts.typing_word;
        previous.layout = Arc::downgrade(&next);
        cache.previous = Some(previous);
        return next;
    }

    let layout = Arc::new(layout(doc, cache, opts));
    if layout.pages.len() >= MIN_PAGES
        && eligible(doc, opts)
        && doc.body.iter().all(|block| match block.as_ref() {
            joyword_model::Block::Para(p) => plain_paragraph(doc, p),
            joyword_model::Block::Table(t) => plain_table(doc, t),
        })
    {
        cache.previous = Some(Previous {
            body: doc.body.clone(),
            section: doc.last_section.clone(),
            settings: doc.settings.clone(),
            options: opts.clone(),
            env: cache.env,
            layout: Arc::downgrade(&layout),
        });
    }
    layout
}

fn plain_paragraph(doc: &Document, p: &Paragraph) -> bool {
    let r = doc.styles.resolve_para(&p.props);
    p.section.is_none() && p.objects.is_empty() && r.frame.is_none() && r.drop_cap == 0
}

fn plain_table(doc: &Document, table: &joyword_model::Table) -> bool {
    if table.props.float.is_some() || table.rows.len() > 128 {
        return false;
    }
    let mut cells = 0usize;
    let mut paragraphs = 0usize;
    for row in &table.rows {
        cells = cells.saturating_add(row.cells.len());
        if cells > 512 {
            return false;
        }
        for cell in &row.cells {
            paragraphs = paragraphs.saturating_add(cell.blocks.len());
            if paragraphs > 512
                || !cell.blocks.iter().all(|block| {
                    block.as_para().is_some_and(|p| {
                        plain_paragraph(doc, p)
                            && doc.styles.resolve_para(&p.props).numbering.is_none()
                    })
                })
            {
                return false;
            }
        }
    }
    true
}

fn eligible(doc: &Document, opts: &LayoutOptions) -> bool {
    let section = &doc.last_section;
    opts.view == ViewMode::Print
        && !opts.comment_balloons
        && doc.parts.is_empty()
        && doc.comments.is_empty()
        && doc.revisions.is_empty()
        && doc.media.is_empty()
        && doc.footnote_separators.is_empty()
        && !doc.settings.mirror_margins
        && section.columns.count == 1
        && section.columns.widths.is_empty()
        && section.text_direction == TextDirection::Horizontal
        && section.valign == PageVAlign::Top
        && section.line_numbers.is_none()
}

impl Previous {
    fn replace(&self, doc: &Document, opts: &LayoutOptions) -> Option<(usize, DocLayout)> {
        if self.body.len() != doc.body.len()
            || self.section != doc.last_section
            || self.settings != doc.settings
            || !eligible(doc, opts)
            || self.options
                != (LayoutOptions {
                    typing_word: self.options.typing_word,
                    ..opts.clone()
                })
            || self.env != env_hash(doc, opts)
        {
            return None;
        }
        let mut changes = self
            .body
            .iter()
            .zip(&doc.body)
            .enumerate()
            .filter(|(_, (before, after))| !Arc::ptr_eq(before, after));
        let (block, (before, after)) = changes.next()?;
        if changes.next().is_some() {
            return None;
        }
        if let (Some(_), Some(table)) = (before.as_table(), after.as_table()) {
            if self.options.typing_word.is_some()
                || opts.typing_word.is_some()
                || !plain_table(doc, table)
            {
                return None;
            }
            let current = self.layout.upgrade()?;
            return self
                .reflow(doc, opts, block, &current)
                .map(|layout| (block, layout));
        }
        let (before, after) = (before.as_para()?, after.as_para()?);
        if !ordinary_edit(before, after)
            || self
                .options
                .typing_word
                .is_some_and(|t| t.rev != before.rev)
            || opts.typing_word.is_some_and(|t| t.rev != after.rev)
        {
            return None;
        }

        let current = self.layout.upgrade()?;
        let path = Path::top(block);
        let [(page, item)] = current.index.get(&(StoryRef::Body, path))?.as_slice() else {
            return self
                .reflow(doc, opts, block, &current)
                .map(|layout| (block, layout));
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
        let new = para::layout_para(
            after,
            &para::ParaEnv {
                doc,
                width: self.section.text_width().max(18.0),
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
                grid: para::Grid::of(doc, &self.section),
                fit_objects: false,
                fitted: &[],
            },
        );
        if !same_flow(old, &new) {
            return self
                .reflow(doc, opts, block, &current)
                .map(|layout| (block, layout));
        }

        let mut next = (*current).clone();
        let Placed::Lines { para, .. } = next.pages.get_mut(*page)?.items.get_mut(*item)? else {
            return None;
        };
        *para = Arc::new(new);
        next.page_patch = Some((Arc::downgrade(&current), *page..*page + 1));
        Some((block, next))
    }

    /// Reflow a plain chapter bracketed by explicit page starts. First replay its old
    /// geometry and match every line against the original pages. A different page count,
    /// unstable entry state or a paragraph that depends on surrounding stories uses the
    /// complete paginator. The UI can keep pages on both sides of a successful replacement.
    fn reflow(
        &self,
        doc: &Document,
        opts: &LayoutOptions,
        block: usize,
        current: &Arc<DocLayout>,
    ) -> Option<DocLayout> {
        const MAX_BLOCKS: usize = 128;
        const MAX_PAGES: usize = 16;
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
                    return b.as_table().is_none_or(|t| !plain_table(doc, t));
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
        || piece.first_block + start != original.first_block
        || piece.items.len() != original.items.len()
        || !original.header.is_empty()
        || !original.footer.is_empty()
        || !original.float_tables.is_empty()
    {
        return false;
    }
    piece
        .items
        .iter()
        .zip(&original.items)
        .all(|(a, b)| match (a, b) {
            (
                Placed::Lines {
                    story: StoryRef::Body,
                    path: ap,
                    para: a,
                    l0: al,
                    l1: ae,
                    x: ax,
                    y: ay,
                    turn: TextDirection::Horizontal,
                },
                Placed::Lines {
                    story: StoryRef::Body,
                    path: bp,
                    para: b,
                    l0: bl,
                    l1: be,
                    x: bx,
                    y: by,
                    turn: TextDirection::Horizontal,
                },
            ) => {
                same_body_path(ap, bp, start)
                    && (al, ae, ax, ay) == (bl, be, bx, by)
                    && same_flow(a, b)
                    && a.lines
                        .iter()
                        .zip(&b.lines)
                        .all(|(a, b)| (a.c0, a.c1) == (b.c0, b.c1))
            }
            (
                Placed::Cell {
                    rect: a,
                    covered: ac,
                    table: at,
                    row: ar,
                    cell: ai,
                    story: StoryRef::Body,
                    clip: ap,
                },
                Placed::Cell {
                    rect: b,
                    covered: bc,
                    table: bt,
                    row: br,
                    cell: bi,
                    story: StoryRef::Body,
                    clip: bp,
                },
            ) => same_body_path(at, bt, start) && (a, ac, ar, ai, ap) == (b, bc, br, bi, bp),
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
            _ => false,
        })
}

fn ordinary_edit(before: &Paragraph, after: &Paragraph) -> bool {
    before.props == after.props
        && before.mark == after.mark
        && after.section.is_none()
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
