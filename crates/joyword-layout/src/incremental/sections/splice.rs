//! Repaginate edits that replace several body paragraphs within a verified
//! section window. Reused pages still need paths in the new document.
use super::*;

impl Previous {
    pub(in crate::incremental) fn reflow_splice(
        &mut self,
        doc: &Document,
        opts: &LayoutOptions,
        old: Range<usize>,
        new: Range<usize>,
    ) -> Option<DocLayout> {
        if old.len().max(new.len()) > 256 {
            return None;
        }
        // Only body paragraphs without independent list, frame or note state may
        // change. Unchanged tables and other paragraphs between two anchors stay shared.
        let suitable = |block: &Arc<joyword_model::Block>| {
            block.as_para().is_some_and(|p| {
                let props = doc.styles.resolve_para(&p.props);
                props.numbering.is_none()
                    && props.frame.is_none()
                    && props.drop_cap == 0
                    && p.objects
                        .iter()
                        .all(|o| o.is_drawing() && supported_body_object(o))
            })
        };
        let (before_span, after_span) = (self.body.get(old.clone())?, doc.body.get(new.clone())?);
        // Joining paragraphs may copy the following section mark while checking
        // inherited settings. Keep those blocks in the replay and compare their content.
        let equal_tail = before_span
            .iter()
            .rev()
            .zip(after_span.iter().rev())
            .take_while(|(old, new)| old == new)
            .count();
        let before_edits = &before_span[..before_span.len() - equal_tail];
        let after_edits = &after_span[..after_span.len() - equal_tail];
        if old.len() == new.len() {
            if before_edits
                .iter()
                .zip(after_edits)
                .any(|(a, b)| !Arc::ptr_eq(a, b) && !(suitable(a) && suitable(b)))
            {
                return None;
            }
        } else if !before_edits.iter().chain(after_edits).all(suitable) {
            return None;
        }
        let drawings = |span: &[Arc<joyword_model::Block>]| {
            span.iter()
                .filter_map(|b| b.as_para())
                .flat_map(|p| &p.objects)
                .filter(|o| o.is_drawing())
                .count()
        };
        if drawings(before_edits) != drawings(after_edits) {
            return None;
        }
        let sections = |span: &[Arc<joyword_model::Block>]| {
            span.iter()
                .filter_map(|b| b.as_para()?.section.clone())
                .collect::<Vec<_>>()
        };
        let (old_marks, new_marks) = (sections(before_edits), sections(after_edits));
        let section_change = old_marks != new_marks;
        let section_delta = isize::try_from(new_marks.len())
            .ok()?
            .checked_sub(isize::try_from(old_marks.len()).ok()?)?;
        if section_change
            && (!self.list_starts.is_empty()
                || self.endnotes.is_some()
                || self.body_fields
                || self.page_stories)
        {
            // Their entering counters, fields or linked stories depend on the old
            // section ordinal. Full pagination rebuilds those global summaries.
            return None;
        }
        for (typing, span) in [
            (self.options.typing_word, before_span),
            (opts.typing_word, after_span),
        ] {
            if typing.is_some_and(|word| {
                !span
                    .iter()
                    .any(|b| b.as_para().is_some_and(|p| p.rev == word.rev))
            }) {
                return None;
            }
        }
        let current = self.layout.upgrade()?;
        let mut before = doc.clone();
        before.body.clone_from(&self.body);
        let anchor = old.start.min(before.body.len().checked_sub(1)?);
        let delta = isize::try_from(new.len())
            .ok()?
            .checked_sub(isize::try_from(old.len()).ok()?)?;
        let mut window = Window::candidates(&before, &current, anchor).find_map(|mut w| {
            if old.start < w.blocks.start || old.end > w.blocks.end {
                return None;
            }
            if section_change {
                // The leading checkpoint precedes every changed start type. The unchanged
                // successor must start on the same body block in the edited document.
                let old_sections = before.sections();
                let new_sections = doc.sections();
                if old_sections.get(w.first_section)?.1 != new_sections.get(w.first_section)?.1 {
                    return None;
                }
                if let Some(next) = w.next_section {
                    let new_next = next.checked_add_signed(section_delta)?;
                    let last = new_next.checked_sub(1)?;
                    if new_sections.get(last)?.0.checked_add(1)?
                        != w.blocks.end.checked_add_signed(delta)?
                        || old_sections.get(next)?.1 != new_sections.get(new_next)?.1
                    {
                        return None;
                    }
                }
            }
            self.independent_window(&mut w, &before, &self.options, &current)
                .then_some(w)
        })?;
        let (cache, prior_fields) = match self.checked_section.take() {
            Some(previous) if previous.blocks == window.blocks => {
                (previous.layout, previous.field_layouts)
            }
            _ => {
                let mut cache = Box::new(LayoutCache::new());
                self.check_previous_window(&window, &before, &mut cache, &current)?;
                (cache, BTreeMap::new())
            }
        };
        window.blocks.end = window.blocks.end.checked_add_signed(delta)?;
        if let Some(next) = &mut window.next_section {
            *next = next.checked_add_signed(section_delta)?;
        }
        if window.blocks.len() > 256 {
            return None;
        }

        // The checked exit uses the next real body paragraph. Rebase its
        // absolute path, all later hit targets, and floating-table anchors.
        let aligned = if delta == 0 && section_delta == 0 {
            Arc::clone(&current)
        } else {
            let mut pages = current.pages.clone();
            let from = u32::try_from(old.end).ok()?;
            for page in &mut pages[window.pages.end..] {
                page.section = page.section.checked_add_signed(section_delta)?;
            }
            for page in &mut pages[window.pages.start..] {
                if page.first_block >= old.end {
                    page.first_block = page.first_block.checked_add_signed(delta)?;
                }
                for path in &mut page.float_tables {
                    let block = path.0.first_mut()?;
                    if *block >= from {
                        *block = u32::try_from(i64::from(*block) + delta as i64).ok()?;
                    }
                }
                relocate_items(&mut page.items, from, delta as i64)?;
            }
            Arc::new(DocLayout {
                // The private snapshot only consults note locations to check
                // that a continuation stays inside its window. Body paths
                // are rebuilt once, on the published layout in reflow_window.
                index: current
                    .index
                    .iter()
                    .filter(|((story, _), _)| matches!(story, StoryRef::Part(_)))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                pages,
                text_boxes: current.text_boxes,
                ..Default::default()
            })
        };
        // Subsequent field checks use the new coordinates. Their unchanged
        // body blocks are shared with the original; the edit itself was replayed above.
        self.body.clone_from(&doc.body);
        self.section_context |= section_change;
        self.requires_section_flow |= section_change;
        if !self.boxes.is_empty() {
            self.boxes = box_anchors(doc)?;
        }
        let mut field_layouts = BTreeMap::new();
        for ((start, end), layout) in prior_fields {
            if end <= old.start {
                field_layouts.insert((start, end), layout);
            } else if start >= old.end {
                field_layouts.insert(
                    (
                        start.checked_add_signed(delta)?,
                        end.checked_add_signed(delta)?,
                    ),
                    layout,
                );
            }
        }
        self.checked_section = Some(SectionCache {
            blocks: window.blocks.clone(),
            layout: cache,
            field_layouts,
        });
        let mut pass = SectionReflow {
            doc,
            opts,
            current: &aligned,
            remaining_pages: 16,
            remaining_blocks: 256,
        };
        let mut next = self.reflow_window(&mut pass, window)?;
        let (_, dirty) = next.page_patch.as_mut()?;
        if delta != 0 || section_delta != 0 {
            dirty.end = next.pages.len().max(current.pages.len());
        }
        next.page_patch = Some((Arc::downgrade(&current), dirty.clone()));
        Some(next)
    }
}
