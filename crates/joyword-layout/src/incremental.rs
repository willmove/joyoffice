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
        && doc.body.iter().all(|block| {
            block.as_para().is_some_and(|p| {
                if p.section.is_some() || !p.objects.is_empty() {
                    return false;
                }
                let resolved = doc.styles.resolve_para(&p.props);
                resolved.frame.is_none() && resolved.drop_cap == 0
            })
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
        let start = (block.saturating_sub(MAX_BLOCKS)..=block)
            .rev()
            .find(|&i| i == 0 || is_start(i))?;
        let end = (block + 1..=doc.body.len().min(start.saturating_add(MAX_BLOCKS)))
            .find(|&i| i == doc.body.len() || is_start(i))?;
        let page_at = |i| {
            current
                .index
                .get(&(StoryRef::Body, Path::top(i)))?
                .first()
                .map(|&(p, _)| p)
        };
        let first = page_at(start)?;
        let stop = if end == doc.body.len() {
            current.pages.len()
        } else {
            page_at(end)?
        };
        let original = current.pages.get(first..stop)?;
        if original.is_empty()
            || original.len() > MAX_PAGES
            || doc.last_section.title_page
            || doc.body[start..end].iter().any(|b| {
                let Some(p) = b.as_para() else { return true };
                let r = doc.styles.resolve_para(&p.props);
                r.numbering.is_some()
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
        if replacement.pages.len() != original.len() {
            return None;
        }
        let mut next = current.as_ref().clone();
        for i in start..end {
            next.index.remove(&(StoryRef::Body, Path::top(i)));
        }
        for (local, mut page) in replacement.pages.into_iter().enumerate() {
            let absolute = first + local;
            let target = next.pages.get_mut(absolute)?;
            for (index, item) in page.items.iter_mut().enumerate() {
                let Placed::Lines {
                    story: StoryRef::Body,
                    path,
                    ..
                } = item
                else {
                    return None;
                };
                if path.depth() != 0 {
                    return None;
                }
                *path = Path::top(start + path.last());
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
                ap.depth() == 0
                    && bp.depth() == 0
                    && ap.last() + start == bp.last()
                    && (al, ae, ax, ay) == (bl, be, bx, by)
                    && same_flow(a, b)
                    && a.lines
                        .iter()
                        .zip(&b.lines)
                        .all(|(a, b)| (a.c0, a.c1) == (b.c0, b.c1))
            }
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
