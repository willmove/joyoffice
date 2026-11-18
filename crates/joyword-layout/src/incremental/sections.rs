//! Reflow the complete group of continuous sections between two fresh page starts.

use super::*;
use crate::{Page, SectionStart, pass::index_lines};
use joyword_model::{InlineObject, PartKind};
use std::{collections::BTreeSet, ops::Range};

struct Window {
    blocks: Range<usize>,
    pages: Range<usize>,
    first_section: usize,
    sentinel: bool,
    initial_note: Option<(usize, u32)>,
}

struct Repaginated {
    pages: Vec<Page>,
    exit_number_delta: i64,
}

impl Window {
    fn find(doc: &Document, current: &DocLayout, block: usize, extend: usize) -> Option<Self> {
        let sections = doc.sections();
        let at = sections.iter().position(|(end, _)| *end >= block)?;
        let first_section = (0..=at)
            .rev()
            .find(|&i| i == 0 || sections[i].1.start != SectionStart::Continuous)?;
        let page_start = |i: usize| {
            matches!(
                sections[i].1.start,
                SectionStart::NextPage | SectionStart::OddPage | SectionStart::EvenPage
            )
        };
        let mut starts =
            (at + 1..sections.len()).filter(|&i| sections[i].1.start != SectionStart::Continuous);
        let next_section = if extend == 0 {
            starts.next()
        } else {
            starts.nth(extend - 1)?;
            starts.next()
        };
        if (first_section != 0 && !page_start(first_section))
            || (first_section + 1..next_section.unwrap_or(sections.len()))
                .any(|i| sections[i].1.start == SectionStart::NextColumn)
            || next_section.is_some_and(|i| !page_start(i))
        {
            return None;
        }
        let start = first_section
            .checked_sub(1)
            .map_or(0, |i| sections[i].0 + 1);
        let end = next_section.map_or(doc.body.len(), |i| sections[i - 1].0 + 1);
        let first = current.pages.partition_point(|p| p.section < first_section);
        let stop = next_section.map_or(current.pages.len(), |next| {
            current.pages.partition_point(|p| p.section < next)
        });
        if start >= end
            || end - start > 256
            || first >= stop
            || stop - first > 16
            || current.pages.get(first)?.first_block != start
        {
            return None;
        }
        if next_section.is_some() {
            // One real paragraph of the following section proves that no pending
            // footnote, extra page, or continuous-column state crosses the exit.
            let p = doc.body.get(end)?.as_para()?;
            let props = doc.styles.resolve_para(&p.props);
            if !plain_paragraph(doc, p)
                || p.text.is_empty()
                || props.keep_next
                || props.numbering.is_some()
                || p.text.contains(['\u{c}', '\u{e}'])
                || current.pages.get(stop)?.first_block != end
            {
                return None;
            }
        }
        Some(Self {
            blocks: start..end,
            pages: first..stop,
            first_section,
            sentinel: next_section.is_some(),
            initial_note: None,
        })
    }

    fn independent_notes(
        &mut self,
        doc: &Document,
        opts: &LayoutOptions,
        current: &DocLayout,
    ) -> bool {
        if doc.parts.values().any(|p| p.kind != PartKind::Footnote) {
            return false;
        }
        let mut ids = BTreeSet::new();
        let mut local_notes = BTreeSet::new();
        let mut first_note = None;
        let (mut prior, mut later) = (false, false);
        for (block, para) in doc
            .body
            .iter()
            .enumerate()
            .filter_map(|(b, p)| Some((b, p.as_para()?)))
        {
            if self.blocks.contains(&block)
                && doc.styles.resolve_para(&para.props).numbering.is_some()
            {
                return false;
            }
            for object in &para.objects {
                let InlineObject::NoteRef { id, .. } = object else {
                    return false;
                };
                // A duplicate anywhere in the document can change an earlier label.
                if !ids.insert(*id) {
                    return false;
                }
                prior |= block < self.blocks.start;
                later |= block >= self.blocks.end;
                if self.blocks.contains(&block) {
                    first_note.get_or_insert((block, *id));
                    let Some(note) = doc.parts.get(id) else {
                        return false;
                    };
                    if note.blocks.len() > 128
                        || note.blocks.iter().any(|b| {
                            b.as_para().is_none_or(|p| {
                                p.text.len() > 131_072
                                    || doc.styles.resolve_para(&p.props).numbering.is_some()
                            })
                        })
                    {
                        return false;
                    }
                    local_notes.insert(*id);
                }
            }
        }
        let sections = doc.sections();
        if later
            && sections
                .iter()
                .skip(self.first_section)
                .take_while(|(end, _)| *end < self.blocks.end)
                .any(|(_, sect)| {
                    doc.note_options(sect, false).num_restart
                        == joyword_model::section::NoteRestart::EachPage
                })
        {
            // Moving a per-page restart can renumber notes beyond the exit page.
            return false;
        }
        if prior && let Some((block, id)) = first_note {
            let Some((_, sect)) = sections.iter().find(|(end, _)| *end >= block) else {
                return false;
            };
            if doc.note_options(sect, false).num_restart
                == joyword_model::section::NoteRestart::Continuous
            {
                let placed = crate::notes::footnote_pages(doc, &current.pages);
                let numbers =
                    crate::notes::note_info(doc, opts.hides_deleted(StoryRef::Body), &placed);
                let Some(label) = numbers.get(&id) else {
                    return false;
                };
                self.initial_note = Some((block - self.blocks.start, label.n));
            }
        }
        current
            .index
            .iter()
            .all(|((story, _), locations)| match story {
                StoryRef::Part(id) if local_notes.contains(id) => {
                    locations.iter().all(|(page, _)| self.pages.contains(page))
                }
                _ => true,
            })
    }

    fn fragment(&self, doc: &Document, current: &DocLayout, body: &Blocks) -> Option<Document> {
        let mut fragment = doc.clone();
        let end = self.blocks.end + usize::from(self.sentinel);
        fragment.body = body.get(self.blocks.start..end)?.to_vec();
        if self.sentinel {
            fragment.last_section = doc
                .sections()
                .into_iter()
                .find(|(i, _)| *i >= self.blocks.end)?
                .1
                .clone();
        }
        // Seed the displayed page number so odd/even-dependent text and section
        // restarts see the same entering page as the full paginator.
        if self.first_section != 0 {
            section_at(&mut fragment, 0)?.page_num_start =
                Some(current.pages.get(self.pages.start)?.number);
        }
        if let Some((block, number)) = self.initial_note {
            section_at(&mut fragment, block)?.footnote_pr.num_start = Some(number);
        }
        Some(fragment)
    }

    fn pages(
        &self,
        doc: &Document,
        opts: &LayoutOptions,
        cache: &mut LayoutCache,
        current: &DocLayout,
    ) -> Option<Repaginated> {
        let mut pages = crate::layout_at(doc, cache, opts, self.pages.start).pages;
        let count = pages.len().checked_sub(usize::from(self.sentinel))?;
        if !(1..=16).contains(&count) {
            return None;
        }
        for page in &mut pages {
            page.section += self.first_section;
            page.first_block += self.blocks.start;
            let shift = |path: &mut Path| -> Option<()> {
                let top = path.0.first_mut()?;
                *top = top.checked_add(u32::try_from(self.blocks.start).ok()?)?;
                Some(())
            };
            for path in &mut page.float_tables {
                shift(path)?;
            }
            for item in &mut page.items {
                let path = match item {
                    Placed::Lines {
                        story: StoryRef::Body,
                        path,
                        ..
                    }
                    | Placed::Cell {
                        story: StoryRef::Body,
                        table: path,
                        ..
                    } => path,
                    Placed::Lines {
                        story: StoryRef::Part(_),
                        ..
                    }
                    | Placed::Fill { .. }
                    | Placed::Rule { .. } => continue,
                    _ => return None,
                };
                shift(path)?;
            }
        }
        let mut exit_number_delta = 0;
        if self.sentinel {
            let exit = pages.pop()?;
            let mut expected = current.pages.get(self.pages.end)?.clone();
            expected.items.truncate(1);
            exit_number_delta = i64::from(exit.number) - i64::from(expected.number);
            expected.number = exit.number;
            if exit.items.len() != 1 || !equivalent_page(&exit, &expected) {
                return None;
            }
        }
        Some(Repaginated {
            pages,
            exit_number_delta,
        })
    }
}

// A changed physical page count can change which side of the sheet an odd
// or even section begins on. Only reuse a suffix independent of that choice.
// Number restarts end propagation of a displayed-number delta. The physical
// offset persists to later facing-page restarts.
fn append_shifted_suffix(
    doc: &Document,
    suffix: &[Page],
    number_delta: i64,
    physical_delta: i64,
    output: &mut Vec<Page>,
) -> Option<()> {
    let Some(first) = suffix.first() else {
        return Some(());
    };
    let sections = doc.sections();
    let mut restart = sections.len();
    let same_sheet_side = physical_delta % 2 == 0;
    let same_number_side = number_delta % 2 == 0;
    if !same_number_side && (doc.settings.mirror_margins || doc.settings.even_odd_headers) {
        return None;
    }
    for (i, (_, section)) in sections.iter().enumerate().skip(first.section) {
        let reset = section.page_num_start.is_some();
        match section.start {
            SectionStart::NextPage
                if !reset || !doc.settings.even_odd_headers || same_sheet_side => {}
            SectionStart::OddPage | SectionStart::EvenPage if same_number_side || i >= restart => {}
            SectionStart::Continuous if !reset => continue,
            _ => return None,
        }
        if reset && i > first.section {
            restart = restart.min(i);
        }
    }
    for old in suffix {
        let mut page = old.clone();
        if old.section < restart && number_delta != 0 {
            // Saturated numbering has lost the original distance to the restart.
            if old.number == u32::MAX {
                return None;
            }
            page.number = u32::try_from(i64::from(old.number) + number_delta).ok()?;
        }
        output.push(page);
    }
    Some(())
}

fn section_at(doc: &mut Document, block: usize) -> Option<&mut SectionProps> {
    if let Some(i) = doc
        .body
        .iter()
        .enumerate()
        .skip(block)
        .find_map(|(i, b)| b.as_para()?.section.as_ref().map(|_| i))
    {
        doc.para_mut(StoryRef::Body, &Path::top(i))
            .ok()?
            .section
            .as_deref_mut()
    } else {
        Some(&mut doc.last_section)
    }
}

fn equivalent_page(page: &Page, original: &Page) -> bool {
    page.section == original.section
        && page.number == original.number
        && page.header_story == original.header_story
        && page.footer_story == original.footer_story
        && page.hf_slot == original.hf_slot
        && same_page_entry(page, original, 0)
}

impl Previous {
    pub(super) fn reflow_sections(
        &mut self,
        doc: &Document,
        opts: &LayoutOptions,
        block: usize,
        current: &Arc<DocLayout>,
    ) -> Option<DocLayout> {
        let facing = doc.settings.even_odd_headers
            || doc.settings.mirror_margins
            || doc
                .sections()
                .iter()
                .any(|(_, s)| matches!(s.start, SectionStart::OddPage | SectionStart::EvenPage));
        // An added page may be absorbed by a later odd/even break. Carry layout
        // through successive fresh starts until the exit and suffix agree.
        for extend in 0..=if facing { 3 } else { 0 } {
            let window = Window::find(doc, current, block, extend)?;
            if let Some(next) = self.reflow_window(doc, opts, window, current) {
                return Some(next);
            }
        }
        None
    }

    fn reflow_window(
        &mut self,
        doc: &Document,
        opts: &LayoutOptions,
        mut window: Window,
        current: &Arc<DocLayout>,
    ) -> Option<DocLayout> {
        if !window.independent_notes(doc, opts, current) {
            return None;
        }
        // On first entry replay the old section against the real document.
        // A successfully produced replacement is already a verified base for
        // the next edit of that same section. Reuse only its private shaping cache.
        let mut cache = match self.checked_section.take() {
            Some((blocks, cache)) if blocks == window.blocks => cache,
            _ => {
                let mut cache = Box::new(LayoutCache::new());
                let mut before = window.fragment(doc, current, &self.body)?;
                before.parts.clone_from(&self.notes);
                let old = window.pages(&before, &self.options, &mut cache, current)?;
                if old.pages.len() != window.pages.len()
                    || old.exit_number_delta != 0
                    || !old
                        .pages
                        .iter()
                        .zip(&current.pages[window.pages.clone()])
                        .all(|(a, b)| equivalent_page(a, b))
                {
                    return None;
                }
                cache
            }
        };
        let after = window.fragment(doc, current, &doc.body)?;
        let replacement = window.pages(&after, opts, &mut cache, current)?;
        let moved = replacement.pages.len() != window.pages.len();
        let count = current.pages.len() - window.pages.len() + replacement.pages.len();
        let mut pages = Vec::with_capacity(count);
        pages.extend_from_slice(&current.pages[..window.pages.start]);
        pages.extend(replacement.pages);
        let suffix = &current.pages[window.pages.end..];
        let dirty = if moved {
            append_shifted_suffix(
                doc,
                suffix,
                replacement.exit_number_delta,
                count as i64 - current.pages.len() as i64,
                &mut pages,
            )?;
            // Bitmap caches are indexed by physical page. Also invalidate any
            // former tail pages, so a later undo cannot reuse a stale bitmap.
            window.pages.start..count.max(current.pages.len())
        } else {
            if replacement.exit_number_delta != 0 {
                return None;
            }
            pages.extend_from_slice(suffix);
            window.pages
        };
        self.checked_section = Some((window.blocks, cache));
        Some(DocLayout {
            index: index_lines(&pages),
            pages,
            ms: 0.0,
            text_boxes: current.text_boxes,
            page_patch: Some((Arc::downgrade(current), dirty)),
        })
    }
}
