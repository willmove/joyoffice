//! Reflow the complete group of continuous sections between two fresh page starts.

use super::*;
use crate::{Page, SectionStart, pass::index_lines};
use joyword_model::{InlineObject, PartKind};
use std::{collections::BTreeSet, ops::Range};

pub(super) struct SectionCache {
    blocks: Range<usize>,
    layout: Box<LayoutCache>,
    /// Only sections replayed into the immediately preceding layout can skip old-page verification.
    field_layouts: BTreeMap<(usize, usize), LayoutCache>,
}

struct Window {
    blocks: Range<usize>,
    pages: Range<usize>,
    first_section: usize,
    next_section: Option<usize>,
    initial_note: Option<(usize, u32)>,
}

struct Repaginated {
    pages: Vec<Page>,
    exit_number_delta: i64,
}

/// Work left for one edit, shared by the primary and all dependent sections.
struct SectionReflow<'a> {
    doc: &'a Document,
    opts: &'a LayoutOptions,
    current: &'a Arc<DocLayout>,
    remaining_pages: usize,
    remaining_blocks: usize,
}

impl SectionReflow<'_> {
    fn consume(&mut self, pages: usize, blocks: usize) -> Option<()> {
        let pages = self.remaining_pages.checked_sub(pages)?;
        let blocks = self.remaining_blocks.checked_sub(blocks)?;
        self.remaining_pages = pages;
        self.remaining_blocks = blocks;
        Some(())
    }
}

struct FieldReplay {
    window: Window,
    replacement: Vec<Page>,
    shifted_suffix: Option<Vec<Page>>,
    cache: LayoutCache,
}

impl Window {
    // A continuation can land before or after the first independent page start.
    // Prefer narrow windows and exhaust the same three boundary extensions at either end.
    fn candidates<'a>(
        doc: &'a Document,
        current: &'a DocLayout,
        block: usize,
    ) -> impl Iterator<Item = Self> + 'a {
        (0..=3).flat_map(move |breadth| {
            (0..=breadth)
                .filter_map(move |before| Self::find(doc, current, block, before, breadth - before))
        })
    }

    fn locate_in(&self, original: &DocLayout, pages: &[Page]) -> Option<Range<usize>> {
        // Earlier reflows may have shifted both boundaries. Section and block
        // identity still locate the window in the current candidate.
        let start = pages.partition_point(|page| page.section < self.first_section);
        let end = original
            .pages
            .get(self.pages.end)
            .map_or(pages.len(), |next| {
                pages.partition_point(|page| page.section < next.section)
            });
        (start < end
            && pages.get(start)?.first_block == self.blocks.start
            && (self.next_section.is_none() || pages.get(end)?.first_block == self.blocks.end))
            .then_some(start..end)
    }

    fn find(
        doc: &Document,
        current: &DocLayout,
        block: usize,
        before: usize,
        extend: usize,
    ) -> Option<Self> {
        let sections = doc.sections();
        let at = sections.iter().position(|(end, _)| *end >= block)?;
        let first_section = (0..=at)
            .rev()
            .filter(|&i| i == 0 || sections[i].1.start != SectionStart::Continuous)
            .nth(before)?;
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
            next_section,
            initial_note: None,
        })
    }

    fn independent_notes(
        &mut self,
        doc: &Document,
        opts: &LayoutOptions,
        current: &DocLayout,
    ) -> bool {
        if doc.parts.values().any(|p| {
            !matches!(
                p.kind,
                PartKind::Header
                    | PartKind::Footer
                    | PartKind::Footnote
                    | PartKind::Endnote
                    | PartKind::TextBox
            )
        }) {
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
            for object in &para.objects {
                let InlineObject::NoteRef { id, .. } = object else {
                    if !supported_body_object(object) {
                        return false;
                    }
                    continue;
                };
                // A duplicate anywhere in the document can change an earlier label.
                if !ids.insert(*id) {
                    return false;
                }
                // Endnote labels and their actual destination come from the whole document.
                if doc
                    .parts
                    .get(id)
                    .is_some_and(|p| p.kind == PartKind::Endnote)
                {
                    continue;
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

    fn fragment(&self, doc: &Document, current: &[Page], body: &Blocks) -> Option<Document> {
        let mut fragment = doc.clone();
        let end = self.blocks.end + usize::from(self.next_section.is_some());
        fragment.body = body.get(self.blocks.start..end)?.to_vec();
        if self.next_section.is_some() {
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
            let first = section_at(&mut fragment, 0)?;
            first.page_num_start = Some(current.get(self.pages.start)?.number);
            let links = crate::headers::HfLinks::new(doc.sections());
            first.headers = links.heads.set(self.first_section);
            first.footers = links.feet.set(self.first_section);
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
        current: &[Page],
        endnotes: Option<&crate::notes::Endnotes>,
        list_starts: &[crate::Counters],
    ) -> Option<Repaginated> {
        let outside = current
            .len()
            .checked_sub(self.pages.len() + usize::from(self.next_section.is_some()))?;
        let origin = crate::PageOrigin {
            endnotes,
            list_seed: list_starts.get(self.first_section),
            capture_list_starts: !list_starts.is_empty() && self.next_section.is_some(),
            trailing_sentinel: self.next_section.is_some(),
            preceding_pages: self.pages.start,
            preceding_lines: current.get(self.pages.start)?.numbered_lines_before,
            preceding_sections: u32::try_from(self.first_section).ok()?,
            outside_pages: u32::try_from(outside).ok()?,
        };
        let mut pages = crate::layout_at(doc, cache, opts, origin).pages;
        if origin.capture_list_starts
            && let Some(next) = self.next_section
            && cache.list_starts.last()? != list_starts.get(next)?
        {
            // Reusing the suffix would change a later list label, possibly in a different list level.
            return None;
        }
        let count = pages
            .len()
            .checked_sub(usize::from(self.next_section.is_some()))?;
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
            relocate_items(&mut page.items, u32::try_from(self.blocks.start).ok()?)?;
        }
        let mut exit_number_delta = 0;
        if self.next_section.is_some() {
            let exit = pages.pop()?;
            let mut expected = current.get(self.pages.end)?.clone();
            // A numbered paragraph can add several synthetic margin labels.
            // Only that real paragraph and its labels are allowed at the exit:
            // an arriving footnote or other body flow still invalidates it.
            let first = Path::top(self.blocks.end);
            let mut body_lines = 0;
            for item in &exit.items {
                match item {
                    Placed::Lines {
                        story: StoryRef::Body,
                        path,
                        ..
                    } if *path == first => body_lines += 1,
                    Placed::Lines { story, path, .. }
                        if *story == crate::DECOR_STORY && path.0.is_empty() => {}
                    _ => return None,
                }
            }
            expected.items.truncate(exit.items.len());
            exit_number_delta = i64::from(exit.number) - i64::from(expected.number);
            expected.number = exit.number;
            if body_lines != 1 || !equivalent_page(&exit, &expected) {
                return None;
            }
        }
        Some(Repaginated {
            pages,
            exit_number_delta,
        })
    }
}

/// Body paths in a clipped canvas are relative to the fragment as well.
fn relocate_items(items: &mut [Placed], start: u32) -> Option<()> {
    for item in items {
        let (story, path) = match item {
            Placed::Lines { story, path, .. }
            | Placed::Image { story, path, .. }
            | Placed::Object { story, path, .. }
            | Placed::Graphic { story, path, .. }
            | Placed::Cell {
                story, table: path, ..
            } => (*story, path),
            Placed::Shape {
                object: Some(pos), ..
            } => (pos.story, &mut pos.path),
            Placed::Clip { items, .. } => {
                relocate_items(items, start)?;
                continue;
            }
            Placed::Shape { object: None, .. } | Placed::Fill { .. } | Placed::Rule { .. } => {
                continue;
            }
        };
        if story == StoryRef::Body {
            let block = path.0.first_mut()?;
            *block = block.checked_add(start)?;
        }
    }
    Some(())
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

/// Section fragments relocate all body and drawing paths before comparison.
pub(super) fn same_drawing(a: &Placed, b: &Placed) -> bool {
    match (a, b) {
        (
            Placed::Image {
                rect,
                media,
                crop,
                story,
                path,
                off,
                spin,
            },
            Placed::Image {
                rect: r,
                media: m,
                crop: c,
                story: s,
                path: p,
                off: o,
                spin: t,
            },
        ) => (rect, media, crop, story, path, off, spin) == (r, m, c, s, p, o, t),
        (
            Placed::Graphic {
                rect,
                graphic,
                story,
                path,
                off,
                spin,
            },
            Placed::Graphic {
                rect: r,
                graphic: g,
                story: s,
                path: p,
                off: o,
                spin: t,
            },
        ) => (rect, graphic, story, path, off, spin) == (r, g, s, p, o, t),
        (
            Placed::Object {
                rect,
                spin,
                story,
                path,
                off,
                text_box,
                wrap,
                origin,
            },
            Placed::Object {
                rect: r,
                spin: t,
                story: s,
                path: p,
                off: o,
                text_box: b,
                wrap: w,
                origin: z,
            },
        ) => (rect, spin, story, path, off, text_box, wrap, origin) == (r, t, s, p, o, b, w, z),
        (
            Placed::Shape {
                rect,
                kind,
                fill,
                stroke,
                stroke_width,
                freeform,
                effects,
                spin,
                extra,
                object,
                alt,
            },
            Placed::Shape {
                rect: r,
                kind: k,
                fill: f,
                stroke: s,
                stroke_width: w,
                freeform: g,
                effects: e,
                spin: t,
                extra: x,
                object: o,
                alt: l,
            },
        ) => {
            (
                rect,
                kind,
                fill,
                stroke,
                stroke_width,
                freeform,
                effects,
                spin,
                extra,
                object,
                alt,
            ) == (r, k, f, s, w, g, e, t, x, o, l)
        }
        _ => false,
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

/// Find body fields in reused pages whose displayed value changed when a section was replayed.
fn collect_changed_fields(
    doc: &Document,
    previous: &[Page],
    pages: &[Page],
    changed: &Range<usize>,
    replacement_end: usize,
    blocks: &mut BTreeSet<usize>,
) -> Option<()> {
    let sections = doc.sections();
    let old_totals = crate::cache::Totals::of(previous, &sections);
    let new_totals = crate::cache::Totals::of(pages, &sections);
    let reused = previous[..changed.start]
        .iter()
        .zip(&pages[..changed.start])
        .chain(
            previous[changed.end..]
                .iter()
                .zip(&pages[replacement_end..]),
        );
    for (old, new) in reused {
        let mut pending = vec![new.items.as_slice()];
        while let Some(items) = pending.pop() {
            for item in items {
                if let Placed::Clip { items, .. } = item {
                    pending.push(items);
                    continue;
                }
                let Placed::Lines {
                    story: StoryRef::Body,
                    path,
                    para,
                    ..
                } = item
                else {
                    continue;
                };
                if !para.has_page_fields {
                    continue;
                }
                let block = *path.0.first()? as usize;
                let si = sections.partition_point(|(last, _)| *last < block);
                if doc
                    .para(StoryRef::Body, path)?
                    .objects
                    .iter()
                    .any(|object| {
                        let InlineObject::Field {
                            instr,
                            locked: false,
                            ..
                        } = object
                        else {
                            return false;
                        };
                        match crate::fields::field_name(instr).as_str() {
                            "PAGE" => old.number != new.number,
                            "NUMPAGES" => old_totals.pages != new_totals.pages,
                            "SECTIONPAGES" => old_totals.section(si) != new_totals.section(si),
                            _ => false,
                        }
                    })
                {
                    blocks.insert(block);
                    if blocks.len() > 256 {
                        return None;
                    }
                }
            }
        }
    }
    Some(())
}

impl Previous {
    pub(super) fn reflow_sections(
        &mut self,
        doc: &Document,
        opts: &LayoutOptions,
        block: usize,
        current: &Arc<DocLayout>,
    ) -> Option<DocLayout> {
        // A continued footnote can enter or leave the first window. Include
        // its anchor or continuation before accepting the checked exit. The
        // same expansion absorbs changes at a later odd/even section break.
        let mut pass = SectionReflow {
            doc,
            opts,
            current,
            remaining_pages: 16,
            remaining_blocks: 256,
        };
        let checked = self
            .checked_section
            .as_ref()
            .map(|s| s.blocks.clone())
            .filter(|r| r.contains(&block));
        if let Some(range) = &checked
            && let Some(window) =
                Window::candidates(doc, current, block).find(|w| w.blocks == *range)
            && let Some(next) = self.reflow_window(&mut pass, window)
        {
            return Some(next);
        }
        Window::candidates(doc, current, block)
            .filter(|w| checked.as_ref() != Some(&w.blocks))
            .find_map(|window| self.reflow_window(&mut pass, window))
    }

    fn independent_window(
        &self,
        window: &mut Window,
        doc: &Document,
        opts: &LayoutOptions,
        current: &DocLayout,
    ) -> bool {
        !self.boxes.values().any(|&(first, last)| {
            first < window.blocks.end
                && last >= window.blocks.start
                && !(window.blocks.contains(&first) && window.blocks.contains(&last))
        }) && window.independent_notes(doc, opts, current)
    }

    fn check_previous_window(
        &self,
        window: &Window,
        doc: &Document,
        cache: &mut LayoutCache,
        current: &DocLayout,
    ) -> Option<()> {
        let mut before = window.fragment(doc, &current.pages, &self.body)?;
        before.parts.clone_from(&self.notes);
        let old = window.pages(
            &before,
            &self.options,
            cache,
            &current.pages,
            self.endnotes.as_ref(),
            &self.list_starts,
        )?;
        (old.pages.len() == window.pages.len()
            && old.exit_number_delta == 0
            && old
                .pages
                .iter()
                .zip(&current.pages[window.pages.clone()])
                .all(|(a, b)| equivalent_page(a, b)))
        .then_some(())
    }

    fn replay_field(
        &self,
        mut window: Window,
        pass: &mut SectionReflow<'_>,
        pages: &[Page],
        verified: Option<LayoutCache>,
    ) -> Option<FieldReplay> {
        let (doc, opts, current) = (pass.doc, pass.opts, pass.current);
        if !self.independent_window(&mut window, doc, opts, current) {
            return None;
        }
        let located = window.locate_in(current, pages)?;
        let reserved = window.pages.len().max(located.len());
        pass.consume(reserved, window.blocks.len())?;
        let mut cache = match verified {
            Some(cache) => cache,
            None => {
                let mut cache = LayoutCache::new();
                self.check_previous_window(&window, doc, &mut cache, current)?;
                cache
            }
        };
        window.pages = located;
        let fragment = window.fragment(doc, pages, &doc.body)?;
        let next = window.pages(
            &fragment,
            opts,
            &mut cache,
            pages,
            self.endnotes.as_ref(),
            &self.list_starts,
        )?;
        pass.consume(next.pages.len().saturating_sub(reserved), 0)?;
        let delta = next.pages.len() as i64 - window.pages.len() as i64;
        let shifted_suffix = if delta == 0 && next.exit_number_delta == 0 {
            None
        } else {
            let suffix = &pages[window.pages.end..];
            let mut shifted = Vec::with_capacity(suffix.len());
            append_shifted_suffix(doc, suffix, next.exit_number_delta, delta, &mut shifted)?;
            Some(shifted)
        };
        Some(FieldReplay {
            window,
            replacement: next.pages,
            shifted_suffix,
            cache,
        })
    }

    /// Repeat affected sections until the page counts and displayed numbers settle.
    /// Every pass, including a repeat visit, consumes the same document-edit budget.
    fn refresh_remote_fields(
        &self,
        pass: &mut SectionReflow<'_>,
        edited: &Window,
        pages: &mut Vec<Page>,
        field_layouts: &mut BTreeMap<(usize, usize), LayoutCache>,
    ) -> Option<usize> {
        let (doc, current) = (pass.doc, pass.current);
        let replacement_end = pages.len() - (current.pages.len() - edited.pages.end);
        let mut blocks = BTreeSet::new();
        collect_changed_fields(
            doc,
            &current.pages,
            pages,
            &edited.pages,
            replacement_end,
            &mut blocks,
        )?;
        let mut previous_fields = std::mem::take(field_layouts);
        let mut first_dirty = edited.pages.start;
        while let Some(block) = blocks.pop_first() {
            let replay = Window::candidates(doc, current, block).find_map(|remote| {
                let key = (remote.blocks.start, remote.blocks.end);
                let verified = field_layouts
                    .remove(&key)
                    .or_else(|| previous_fields.remove(&key));
                self.replay_field(remote, pass, pages, verified)
            })?;
            let remote = replay.window;
            blocks.retain(|b| !remote.blocks.contains(b));
            first_dirty = first_dirty.min(remote.pages.start);
            field_layouts.insert((remote.blocks.start, remote.blocks.end), replay.cache);
            if let Some(suffix) = replay.shifted_suffix {
                let replacement_end = remote.pages.start + replay.replacement.len();
                let mut updated = Vec::with_capacity(replacement_end + suffix.len());
                updated.extend_from_slice(&pages[..remote.pages.start]);
                updated.extend(replay.replacement);
                updated.extend(suffix);
                collect_changed_fields(
                    doc,
                    pages,
                    &updated,
                    &remote.pages,
                    replacement_end,
                    &mut blocks,
                )?;
                *pages = updated;
            } else {
                for (page, updated) in pages[remote.pages].iter_mut().zip(replay.replacement) {
                    *page = updated;
                }
            }
        }
        Some(first_dirty)
    }

    fn reflow_window(
        &mut self,
        pass: &mut SectionReflow<'_>,
        mut window: Window,
    ) -> Option<DocLayout> {
        let (doc, opts, current) = (pass.doc, pass.opts, pass.current);
        if !self.independent_window(&mut window, doc, opts, current) {
            return None;
        }
        pass.consume(window.pages.len(), window.blocks.len())?;
        // On first entry replay the old section against the real document.
        // A successfully produced replacement is already a verified base for
        // the next edit of that same section. Reuse only its private shaping cache.
        let (mut cache, mut field_layouts) = match self.checked_section.take() {
            Some(cache) if cache.blocks == window.blocks => (cache.layout, cache.field_layouts),
            _ => {
                let mut cache = Box::new(LayoutCache::new());
                self.check_previous_window(&window, doc, &mut cache, current)?;
                (cache, BTreeMap::new())
            }
        };
        let after = window.fragment(doc, &current.pages, &doc.body)?;
        let replacement = window.pages(
            &after,
            opts,
            &mut cache,
            &current.pages,
            self.endnotes.as_ref(),
            &self.list_starts,
        )?;
        pass.consume(
            replacement.pages.len().saturating_sub(window.pages.len()),
            0,
        )?;
        let moved = replacement.pages.len() != window.pages.len();
        let count = current.pages.len() - window.pages.len() + replacement.pages.len();
        let mut pages = Vec::with_capacity(count);
        pages.extend_from_slice(&current.pages[..window.pages.start]);
        pages.extend(replacement.pages);
        let suffix = &current.pages[window.pages.end..];
        let mut dirty = if moved {
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
            window.pages.clone()
        };
        if self.body_fields && moved {
            dirty.start = dirty.start.min(self.refresh_remote_fields(
                pass,
                &window,
                &mut pages,
                &mut field_layouts,
            )?);
            dirty.end = dirty.end.max(pages.len());
        } else {
            field_layouts.clear();
        }
        if self.page_stories {
            dirty = update_page_stories(doc, opts, &mut cache, &mut pages, current, dirty);
        }
        self.checked_section = Some(SectionCache {
            blocks: window.blocks,
            layout: cache,
            field_layouts,
        });
        Some(DocLayout {
            index: index_lines(&pages),
            pages,
            ms: 0.0,
            text_boxes: current.text_boxes,
            page_patch: Some((Arc::downgrade(current), dirty)),
        })
    }
}

/// Re-evaluate the full-document fields and expand the repaint range only
/// where the running story actually changed, including pages before the edit.
pub(super) fn update_page_stories(
    doc: &Document,
    opts: &LayoutOptions,
    cache: &mut LayoutCache,
    pages: &mut [Page],
    current: &DocLayout,
    mut dirty: Range<usize>,
) -> Range<usize> {
    crate::pass::refresh_page_stories(doc, opts, cache, pages);
    let mut changed = !dirty.is_empty();
    for (i, (page, old)) in pages.iter().zip(&current.pages).enumerate() {
        if !dirty.contains(&i)
            && (page.header_story != old.header_story
                || page.footer_story != old.footer_story
                || page.hf_slot != old.hf_slot
                || !same_items(&page.header, &old.header, 0)
                || !same_items(&page.footer, &old.footer, 0))
        {
            dirty = if changed {
                dirty.start.min(i)..dirty.end.max(i + 1)
            } else {
                i..i + 1
            };
            changed = true;
        }
    }
    if changed { dirty } else { 0..0 }
}
