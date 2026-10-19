//! Paragraph layout cache and incremental pagination (T208, LAYOUT-010).
//!
//! A fitted paragraph is reused when its content, resolved style, available
//! width, shaping face and profile inputs match. Page number, footnote number
//! and wrap identity join that key only when the paragraph actually depends
//! on them, so a paragraph that does not can move between pages and still hit.
//! Pagination restarts at the earliest invalidated paragraph (walking back a
//! keep-next chain) and copies the previous snapshot once a later page
//! boundary matches. Feedback that repeats a state or exhausts its pass
//! budget returns [`LayoutError::Unstable`] and does not publish the
//! temporary pages, unless every body page field has a cached result.
//! That case draws the cache once and notes `PageFieldCached`.

use std::collections::{BTreeSet, HashMap};

use joyword_foundation::Emu;
use joyword_model::{
    Anchor, AnnotationKind, Block, Document, DocumentSnapshot, DocumentVersion, LineSpacing,
    NodeId, NumberLabel, NumberSuffix, NumberingProblem, Paragraph, ScalarOffset, StyleId,
    StyleKind, StyleSheet,
};
use sha2::{Digest, Sha256};

use crate::document::{
    FittedParagraph, LayoutSnapshot, PageGeometry, ParagraphAnchor, PlaceStop, WorkSpent,
    apply_section, assemble, dominant_slot, fit_paragraph, open_snapshot, preflight, record_fit,
};
use crate::style_paint;
use crate::view::ViewOptions;
use crate::{
    FaceSet, FontKey, FontSlot, LayoutCancellation, LayoutError, LayoutLimits, SingleFontProfile,
};

/// Passes spent resolving page-number / footnote / wrap feedback.
pub const MAX_LAYOUT_PASSES: usize = 8;

/// Default resident cap for fitted paragraphs. One entry larger than the cap
/// is not stored; eviction drops the least recently used entries.
pub const DEFAULT_CACHE_BUDGET: usize = 32 << 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DependencyFlags {
    pub page_number: bool,
    pub footnote_number: bool,
    pub wrap: bool,
}

impl DependencyFlags {
    pub fn any(self) -> bool {
        self.page_number || self.footnote_number || self.wrap
    }
}

/// Values supplied by pagination. Ignored fields are the ones the paragraph
/// does not depend on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExternalContext {
    pub page_number: u32,
    pub footnote_number: u32,
    pub wrap: u64,
    /// `PAGE` after `w:pgNumType/@w:start`. Zero is the unset seed.
    pub displayed_page: i64,
    pub num_pages: u32,
    pub section_pages: u32,
    /// Draw the stored field result instead of a live number.
    pub force_cache: bool,
}

/// What a transaction asked layout to drop. The app maps an engine ChangeSet
/// into this set; layout does not depend on the engine crate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayoutInvalidation {
    pub paragraphs: BTreeSet<NodeId>,
    pub inserted: BTreeSet<NodeId>,
    pub removed: BTreeSet<NodeId>,
    pub moved: BTreeSet<NodeId>,
    pub styles: BTreeSet<StyleId>,
    /// `w:docDefaults` changed. Every paragraph's resolved style is stale.
    pub defaults_changed: bool,
}

/// External values that actually participate in a cache key. `None` means
/// the paragraph does not depend on that input, so two pages still collide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExternalKey {
    page_number: Option<u32>,
    footnote_number: Option<u32>,
    wrap: Option<u64>,
    displayed_page: Option<i64>,
    num_pages: Option<u32>,
    section_pages: Option<u32>,
    force_cache: Option<bool>,
}

pub fn external_key(flags: DependencyFlags, context: ExternalContext) -> ExternalKey {
    ExternalKey {
        page_number: flags.page_number.then_some(context.page_number),
        footnote_number: flags.footnote_number.then_some(context.footnote_number),
        wrap: flags.wrap.then_some(context.wrap),
        displayed_page: flags.page_number.then_some(context.displayed_page),
        num_pages: flags.page_number.then_some(context.num_pages),
        section_pages: flags.page_number.then_some(context.section_pages),
        force_cache: flags.page_number.then_some(context.force_cache),
    }
}

/// Styles whose resolved properties change when `changed` changes: the
/// styles themselves, anything that `basedOn`s them, and styles linked to them.
pub fn affected_style_closure(
    sheet: &StyleSheet,
    changed: &BTreeSet<StyleId>,
) -> BTreeSet<StyleId> {
    let mut affected = changed.clone();
    let mut grew = true;
    while grew {
        grew = false;
        let snapshot: Vec<StyleId> = affected.iter().cloned().collect();
        for style in sheet.iter() {
            if affected.contains(&style.id) {
                continue;
            }
            let chain = sheet.chain(&style.id);
            let hit = chain.styles.iter().any(|item| {
                snapshot.iter().any(|id| id == &item.id)
                    || item
                        .link
                        .as_ref()
                        .is_some_and(|link| snapshot.iter().any(|id| id == link))
            });
            if hit {
                affected.insert(style.id.clone());
                grew = true;
            }
        }
    }
    affected
}

/// Paragraphs whose fitted lines must not be treated as unchanged.
pub fn invalidated_paragraphs(
    document: &Document,
    invalidation: &LayoutInvalidation,
) -> BTreeSet<NodeId> {
    let mut ids = BTreeSet::new();
    ids.extend(invalidation.paragraphs.iter().copied());
    ids.extend(invalidation.inserted.iter().copied());
    ids.extend(invalidation.moved.iter().copied());
    if invalidation.defaults_changed {
        for block in document.body().blocks.iter() {
            ids.insert(block.id());
        }
        return ids;
    }
    if invalidation.styles.is_empty() {
        return ids;
    }
    let affected = affected_style_closure(document.styles(), &invalidation.styles);
    for block in document.body().blocks.iter() {
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        if paragraph_uses(document.styles(), paragraph, &affected) {
            ids.insert(paragraph.id());
        }
    }
    ids
}

fn paragraph_uses(sheet: &StyleSheet, paragraph: &Paragraph, affected: &BTreeSet<StyleId>) -> bool {
    if let Some(id) = &paragraph.style {
        if chain_hits(sheet, id, affected) {
            return true;
        }
    } else if sheet
        .default_of(StyleKind::Paragraph)
        .is_some_and(|style| affected.contains(&style.id) || chain_hits(sheet, &style.id, affected))
    {
        return true;
    }
    for run in paragraph.runs() {
        if let Some(id) = &run.style {
            if chain_hits(sheet, id, affected) {
                return true;
            }
        } else if sheet.default_of(StyleKind::Character).is_some_and(|style| {
            affected.contains(&style.id) || chain_hits(sheet, &style.id, affected)
        }) {
            return true;
        }
    }
    false
}

fn chain_hits(sheet: &StyleSheet, id: &StyleId, affected: &BTreeSet<StyleId>) -> bool {
    sheet
        .chain(id)
        .styles
        .iter()
        .any(|style| affected.contains(&style.id))
}

/// Field instructions that name PAGE / NUMPAGES or a footnote reference.
/// Layout still rejects annotations in preflight; the flag exists so the key
/// cannot forget the dependency once those paragraphs are accepted.
pub fn paragraph_dependencies(document: &Document, paragraph: &Paragraph) -> DependencyFlags {
    let mut flags = DependencyFlags::default();
    for annotation in document.annotations() {
        let AnnotationKind::Field { separator } = &annotation.kind else {
            continue;
        };
        let Anchor::Range { start, end } = &annotation.anchor else {
            continue;
        };
        if start.paragraph != paragraph.id() {
            continue;
        }
        let instruction = instruction_text(paragraph, start.offset, separator.as_ref(), end);
        let upper = instruction.to_ascii_uppercase();
        if has_token(&upper, "PAGE")
            || has_token(&upper, "NUMPAGES")
            || has_token(&upper, "SECTIONPAGES")
        {
            flags.page_number = true;
        }
        if has_token(&upper, "FOOTNOTE") || has_token(&upper, "NOTEREF") {
            flags.footnote_number = true;
        }
    }
    if paragraph
        .inlines()
        .iter()
        .any(|inline| matches!(inline.kind, joyword_model::InlineKind::PageField(_)))
    {
        flags.page_number = true;
    }
    flags
}

fn instruction_text(
    paragraph: &Paragraph,
    start: ScalarOffset,
    separator: Option<&joyword_model::Position>,
    end: &joyword_model::Position,
) -> String {
    let chars: Vec<char> = paragraph.plain_text(|_| String::new()).chars().collect();
    let from = start.0.min(chars.len());
    let to = separator
        .filter(|position| position.paragraph == paragraph.id())
        .map(|position| position.offset.0)
        .or_else(|| (end.paragraph == paragraph.id()).then_some(end.offset.0))
        .unwrap_or(chars.len())
        .min(chars.len());
    if from >= to {
        String::new()
    } else {
        chars[from..to].iter().collect()
    }
}

fn has_token(upper: &str, token: &str) -> bool {
    upper
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|word| word == token)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ProfileFitKey {
    size: i64,
    kerning: bool,
    liga: bool,
    calt: bool,
    leading: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MixedFaces {
    ascii: FontKey,
    h_ansi: FontKey,
    east_asia: FontKey,
    complex: FontKey,
    fallback: FontKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FontEnvKey {
    shaping: FontKey,
    mixed: Option<MixedFaces>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ParagraphCacheKey {
    content: [u8; 32],
    resolved_style: [u8; 32],
    available_width: i64,
    font: FontEnvKey,
    profile: ProfileFitKey,
    external: ExternalKey,
    /// Hidden text and field codes change the shaped string. Formatting marks
    /// do not, so they stay off this key and are drawn when the line is placed.
    show_hidden: bool,
    field_codes: bool,
}

struct CacheEntry {
    fit: FittedParagraph,
    bytes: usize,
    stamp: u64,
}

struct Flow {
    version: DocumentVersion,
    content: [u8; 32],
    font: FontKey,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    ids: Vec<NodeId>,
    keys: Vec<ParagraphCacheKey>,
    starts: Vec<ParagraphAnchor>,
}

/// Resident fitted paragraphs plus the flow of the last successful snapshot.
pub struct ParagraphLayoutCache {
    budget: usize,
    used: usize,
    clock: u64,
    map: HashMap<ParagraphCacheKey, CacheEntry>,
    flow: Option<Flow>,
}

impl Default for ParagraphLayoutCache {
    fn default() -> Self {
        Self::with_budget(DEFAULT_CACHE_BUDGET)
    }
}

impl ParagraphLayoutCache {
    pub fn with_budget(budget: usize) -> Self {
        Self {
            budget,
            used: 0,
            clock: 0,
            map: HashMap::new(),
            flow: None,
        }
    }

    pub fn resident_bytes(&self) -> usize {
        self.used
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.used = 0;
        self.flow = None;
    }

    fn touch(&mut self, key: &ParagraphCacheKey) {
        self.clock = self.clock.saturating_add(1);
        if let Some(entry) = self.map.get_mut(key) {
            entry.stamp = self.clock;
        }
    }

    fn insert(&mut self, key: ParagraphCacheKey, fit: FittedParagraph) {
        let bytes = fit_bytes(&fit);
        if bytes > self.budget {
            return;
        }
        if let Some(old) = self.map.remove(&key) {
            self.used = self.used.saturating_sub(old.bytes);
        }
        self.evict_for(bytes);
        if self.used.saturating_add(bytes) > self.budget {
            return;
        }
        self.clock = self.clock.saturating_add(1);
        self.used = self.used.saturating_add(bytes);
        self.map.insert(
            key,
            CacheEntry {
                fit,
                bytes,
                stamp: self.clock,
            },
        );
    }

    fn evict_for(&mut self, extra: usize) {
        while self.used.saturating_add(extra) > self.budget && !self.map.is_empty() {
            let oldest = self
                .map
                .iter()
                .min_by_key(|(_, entry)| entry.stamp)
                .map(|(key, _)| *key);
            let Some(key) = oldest else {
                break;
            };
            if let Some(entry) = self.map.remove(&key) {
                self.used = self.used.saturating_sub(entry.bytes);
            }
        }
    }

    fn cloned(&mut self, key: &ParagraphCacheKey) -> Option<FittedParagraph> {
        if !self.map.contains_key(key) {
            return None;
        }
        self.touch(key);
        self.map.get(key).map(|entry| entry.fit.clone())
    }
}

fn fit_bytes(fit: &FittedParagraph) -> usize {
    let mut bytes = std::mem::size_of::<FittedParagraph>();
    for line in &fit.lines {
        bytes = bytes.saturating_add(line.run.text.len());
        bytes = bytes.saturating_add(
            line.run
                .glyphs
                .len()
                .saturating_mul(std::mem::size_of::<crate::ShapedGlyph>()),
        );
        bytes = bytes.saturating_add(line.clusters.len().saturating_mul(48));
        bytes = bytes.saturating_add(line.run.missing.len().saturating_mul(8));
    }
    bytes
        .saturating_add(fit.diagnostics.len().saturating_mul(64))
        .max(1)
}

/// One step of the feedback loop. The snapshot from a step that does not
/// match `assumed` is dropped; only a stable step is returned.
pub fn run_layout_passes<T, S, F>(seed: T, mut step: F) -> Result<S, LayoutError>
where
    T: Clone + Eq,
    F: FnMut(&T) -> Result<(S, T), LayoutError>,
{
    let mut assumed = seed;
    let mut seen = Vec::new();
    for _ in 0..MAX_LAYOUT_PASSES {
        if seen.iter().any(|item| item == &assumed) {
            return Err(LayoutError::Unstable);
        }
        seen.push(assumed.clone());
        let (snapshot, produced) = step(&assumed)?;
        if produced == assumed {
            return Ok(snapshot);
        }
        assumed = produced;
    }
    Err(LayoutError::Unstable)
}

/// Cache, previous snapshot and invalidation for one incremental layout.
pub struct LayoutReuse<'a> {
    pub cache: &'a mut ParagraphLayoutCache,
    pub previous: Option<&'a LayoutSnapshot>,
    pub invalidation: &'a LayoutInvalidation,
}

/// Lay out `document`, reusing [`LayoutReuse`]. The geometry matches
/// [`crate::layout_document`] for the same snapshot;
/// [`crate::LayoutStats::shape_calls`] counts only shaping done on this call.
pub fn layout_incremental(
    document: &DocumentSnapshot,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    cancellation: &LayoutCancellation,
    reuse: LayoutReuse<'_>,
) -> Result<LayoutSnapshot, LayoutError> {
    layout_incremental_with_view(
        document,
        faces,
        profile,
        limits,
        cancellation,
        reuse,
        ViewOptions::default(),
    )
}

/// [`layout_incremental`] with the view options stored on the snapshot key.
pub fn layout_incremental_with_view(
    document: &DocumentSnapshot,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    cancellation: &LayoutCancellation,
    reuse: LayoutReuse<'_>,
    view: ViewOptions,
) -> Result<LayoutSnapshot, LayoutError> {
    // A table row is not a body paragraph, and a row may also split across
    // pages. Suffix copy assumes body paragraphs line up, so a document with
    // a table is laid out in full. Opaque blocks are skipped by that full
    // layout and by the paragraph path below.
    if document
        .body()
        .blocks
        .iter()
        .any(|block| matches!(block, Block::Table(_)))
    {
        return crate::layout_with_view(document, faces, profile, limits, cancellation, view);
    }
    let LayoutReuse {
        cache,
        previous,
        invalidation,
    } = reuse;
    cancellation.check()?;
    profile.validate()?;
    let limits = limits.capped();
    let geometry = preflight(document, profile, limits, cancellation, view)?;
    let flow_ok = match (&cache.flow, previous) {
        (Some(flow), Some(previous)) => flow_matches(flow, previous, faces, profile, limits, view),
        _ => false,
    };
    let seed = seed_page_contexts(document);
    let settled = run_layout_passes(seed, |assumed| {
        layout_pass(
            cache,
            PassInput {
                document,
                faces,
                profile,
                limits,
                geometry,
                cancellation,
                previous: previous.filter(|_| flow_ok),
                invalidation,
                assumed,
                view,
            },
        )
    });
    let snapshot = match settled {
        Ok(snapshot) => snapshot,
        Err(LayoutError::Unstable) if body_fields_have_cache(document) => {
            let forced = forced_cache_contexts(document);
            let (snapshot, _) = layout_pass(
                cache,
                PassInput {
                    document,
                    faces,
                    profile,
                    limits,
                    geometry,
                    cancellation,
                    previous: previous.filter(|_| flow_ok),
                    invalidation,
                    assumed: &forced,
                    view,
                },
            )?;
            snapshot
        }
        Err(error) => return Err(error),
    };
    remember_flow(cache, &snapshot, document, faces, profile, limits);
    Ok(snapshot)
}

fn flow_matches(
    flow: &Flow,
    previous: &LayoutSnapshot,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    view: ViewOptions,
) -> bool {
    previous.key().version == flow.version
        && previous.key().content_hash() == flow.content
        && previous.key().font == flow.font
        && previous.key().profile == flow.profile
        && previous.key().view == view
        && faces.ascii.key() == flow.font
        && limits == flow.limits
        && profile == flow.profile
}

fn remember_flow(
    cache: &mut ParagraphLayoutCache,
    snapshot: &LayoutSnapshot,
    document: &Document,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
) {
    let mut ids = Vec::new();
    let mut keys = Vec::new();
    let mut starts = Vec::new();
    let marks = joyword_model::list_marks(document);
    let plans = crate::pages::section_plans(document).unwrap_or_default();
    for (index, block) in document.body().blocks.iter().enumerate() {
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        let deps = paragraph_dependencies(document, paragraph);
        let context = context_of(snapshot, paragraph.id(), deps);
        let width = crate::pages::plan_for(&plans, index)
            .map(|plan| plan.fit_width.raw())
            .unwrap_or_else(|| snapshot.geometry().content_width().raw());
        let key = paragraph_key(
            document,
            paragraph,
            faces,
            profile,
            width,
            context,
            marks.get(&paragraph.id()),
            snapshot.key().view,
        );
        let anchor = snapshot
            .lines()
            .iter()
            .position(|line| line.paragraph == paragraph.id())
            .map(|line| ParagraphAnchor {
                line,
                page: snapshot.lines()[line].page,
                top: snapshot.lines()[line].top.raw()
                    - style_paint::flow(document, paragraph)
                        .map(|flow| flow.before)
                        .unwrap_or(Emu::new(0))
                        .raw(),
            });
        let Some(anchor) = anchor else {
            continue;
        };
        ids.push(paragraph.id());
        keys.push(key);
        starts.push(anchor);
    }
    cache.flow = Some(Flow {
        version: snapshot.key().version,
        content: snapshot.key().content_hash(),
        font: faces.ascii.key(),
        profile,
        limits,
        ids,
        keys,
        starts,
    });
}

fn context_of(snapshot: &LayoutSnapshot, id: NodeId, deps: DependencyFlags) -> ExternalContext {
    if !deps.any() {
        return ExternalContext::default();
    }
    let line = snapshot.lines().iter().find(|line| line.paragraph == id);
    let page = line
        .map(|line| {
            u32::try_from(line.page)
                .unwrap_or(u32::MAX)
                .saturating_add(1)
        })
        .unwrap_or(1);
    let section = line
        .and_then(|line| snapshot.page_box(line.page).map(|(_, section)| section))
        .unwrap_or(0);
    let mut context = ExternalContext::default();
    if deps.page_number {
        context.page_number = page;
        context.displayed_page = line
            .map(|line| snapshot.displayed_page(line.page))
            .unwrap_or(0);
        context.num_pages = u32::try_from(snapshot.page_count()).unwrap_or(u32::MAX);
        context.section_pages = snapshot.section_page_count(section);
    }
    if deps.footnote_number {
        context.footnote_number = 1;
    }
    context
}

pub(crate) fn seed_page_contexts(document: &Document) -> Vec<ExternalContext> {
    document
        .body()
        .blocks
        .iter()
        .map(|block| page_seed(document, block, false))
        .collect()
}

pub(crate) fn forced_cache_contexts(document: &Document) -> Vec<ExternalContext> {
    document
        .body()
        .blocks
        .iter()
        .map(|block| page_seed(document, block, true))
        .collect()
}

fn page_seed(document: &Document, block: &Block, force_cache: bool) -> ExternalContext {
    let Some(paragraph) = block.as_paragraph() else {
        return ExternalContext::default();
    };
    if !paragraph_dependencies(document, paragraph).page_number {
        return ExternalContext::default();
    }
    ExternalContext {
        page_number: 1,
        displayed_page: 1,
        num_pages: 1,
        section_pages: 1,
        force_cache,
        ..ExternalContext::default()
    }
}

pub(crate) fn body_fields_have_cache(document: &Document) -> bool {
    let mut any = false;
    for block in document.body().blocks.iter() {
        let Some(paragraph) = block.as_paragraph() else {
            continue;
        };
        for inline in paragraph.inlines() {
            let joyword_model::InlineKind::PageField(field) = &inline.kind else {
                continue;
            };
            any = true;
            if field.cached.is_none() {
                return false;
            }
        }
    }
    any
}

pub(crate) fn contexts_after(
    document: &Document,
    snapshot: &LayoutSnapshot,
) -> Vec<ExternalContext> {
    document
        .body()
        .blocks
        .iter()
        .map(|block| {
            let Some(paragraph) = block.as_paragraph() else {
                return ExternalContext::default();
            };
            context_of(
                snapshot,
                paragraph.id(),
                paragraph_dependencies(document, paragraph),
            )
        })
        .collect()
}

struct PassInput<'a> {
    document: &'a DocumentSnapshot,
    faces: FaceSet<'a>,
    profile: SingleFontProfile,
    limits: LayoutLimits,
    geometry: PageGeometry,
    cancellation: &'a LayoutCancellation,
    previous: Option<&'a LayoutSnapshot>,
    invalidation: &'a LayoutInvalidation,
    assumed: &'a [ExternalContext],
    view: ViewOptions,
}

fn layout_pass(
    cache: &mut ParagraphLayoutCache,
    input: PassInput<'_>,
) -> Result<(LayoutSnapshot, Vec<ExternalContext>), LayoutError> {
    let PassInput {
        document,
        faces,
        profile,
        limits,
        geometry,
        cancellation,
        previous,
        invalidation,
        assumed,
        view,
    } = input;
    let invalid = invalidated_paragraphs(document, invalidation);
    // Clone the previous flow before the cache is mutated. A hit or a miss
    // both borrow `cache` exclusively.
    let stored = cache
        .flow
        .as_ref()
        .filter(|_| previous.is_some())
        .map(|flow| (flow.ids.clone(), flow.keys.clone(), flow.starts.clone()));
    let plans = crate::pages::section_plans(document)?;
    let mut result = open_snapshot(document, faces, profile, limits, geometry, view)?;
    let mut prepared = Vec::new();
    let mut deps = Vec::new();
    let mut same = Vec::new();
    let marks = joyword_model::list_marks(document);
    let mut total_lines = 0usize;
    let mut charged_shapes = 0usize;
    let mut charged_bytes = 0usize;
    let mut charged_glyphs = 0usize;
    let mut any_deps = false;
    let mut rank = 0usize;
    for (index, block) in document.body().blocks.iter().enumerate() {
        cancellation.check()?;
        let Some(paragraph) = block.as_paragraph() else {
            if matches!(block, Block::Opaque(_)) {
                result.omit_opaque_block();
                deps.push(DependencyFlags::default());
                continue;
            }
            return Err(LayoutError::Unsupported {
                node: Some(block.id()),
                feature: "non-paragraph block",
            });
        };
        let flags = paragraph_dependencies(document, paragraph);
        any_deps |= flags.any();
        let context = if flags.any() {
            assumed.get(index).copied().unwrap_or_default()
        } else {
            ExternalContext::default()
        };
        let plan = crate::pages::plan_for(&plans, index).ok_or(LayoutError::InvalidDocument)?;
        let key = paragraph_key(
            document,
            paragraph,
            faces,
            profile,
            plan.fit_width.raw(),
            context,
            marks.get(&paragraph.id()),
            view,
        );
        // Flow vectors are paragraph-rank. `index` is the body block index
        // and is only used for section geometry.
        let unchanged = stored.as_ref().is_some_and(|(ids, keys, _)| {
            ids.get(rank) == Some(&paragraph.id())
                && keys.get(rank) == Some(&key)
                && !invalid.contains(&paragraph.id())
        });
        let remaining_shapes = limits.max_shape_calls.saturating_sub(charged_shapes);
        if remaining_shapes == 0 {
            return Err(LayoutError::LimitExceeded(crate::LayoutLimit::ShapeCalls));
        }
        let numbering = document
            .sections()
            .get(plan.section)
            .map(|section| section.numbering.clone())
            .unwrap_or_default();
        let fitted = if let Some(hit) = cache.cloned(&key) {
            if hit.shape_calls > remaining_shapes
                || charged_bytes.saturating_add(hit.shaped_bytes) > limits.max_shaped_bytes
                || total_lines.saturating_add(hit.lines.len()) > limits.max_lines
                || charged_glyphs.saturating_add(hit.glyphs) > limits.max_glyphs
            {
                return Err(limit_for_hit(
                    hit.shape_calls > remaining_shapes,
                    charged_bytes.saturating_add(hit.shaped_bytes) > limits.max_shaped_bytes,
                    total_lines.saturating_add(hit.lines.len()) > limits.max_lines,
                ));
            }
            result.stats_mut().cache_hits += 1;
            adopt(hit, document, paragraph)?
        } else {
            let fitted = fit_paragraph(
                document,
                paragraph,
                faces,
                profile,
                plan.geometry,
                plan.fit_width,
                limits,
                WorkSpent {
                    shapes: charged_shapes,
                    bytes: charged_bytes,
                    glyphs: charged_glyphs,
                    lines: total_lines,
                },
                cancellation,
                marks.get(&paragraph.id()),
                view,
                None,
                context,
                &numbering,
            )?;
            cache.insert(key, fitted.clone());
            result.stats_mut().shape_calls += fitted.shape_calls;
            result.stats_mut().shaped_bytes += fitted.shaped_bytes;
            fitted
        };
        let fitted = {
            let mut fitted = fitted;
            apply_section(&mut fitted, document, &plans, index);
            if fitted.used_field_cache {
                result.note_once(crate::document::LayoutNote::PageFieldCached {
                    paragraph: fitted.id,
                });
            }
            fitted
        };
        record_fit(&mut result, &fitted);
        charged_shapes += fitted.shape_calls;
        charged_bytes += fitted.shaped_bytes;
        charged_glyphs += fitted.glyphs;
        total_lines += fitted.lines.len();
        result.stats_mut().glyphs += fitted.glyphs;
        deps.push(flags);
        same.push(unchanged);
        prepared.push(fitted);
        rank += 1;
    }
    walk_back_keep_next(document, &mut same);
    let stop = match (previous, stored.as_ref()) {
        (Some(previous), Some((_, _, starts))) if starts.len() == prepared.len() && !any_deps => {
            Some(PlaceStop {
                previous,
                starts,
                same: &same,
                place_from: 0,
            })
        }
        _ => None,
    };
    let mut snapshot = assemble(result, prepared, cancellation, stop)?;
    crate::pages::paint_stories(&mut snapshot, document, faces, profile, cancellation)?;
    let produced = produced_contexts(document, &snapshot, &deps);
    Ok((snapshot, produced))
}

fn adopt(
    mut hit: FittedParagraph,
    document: &Document,
    paragraph: &Paragraph,
) -> Result<FittedParagraph, LayoutError> {
    let flow = style_paint::flow(document, paragraph)?;
    hit.id = paragraph.id();
    hit.len = paragraph.len();
    hit.before = flow.before;
    hit.after = flow.after;
    hit.group_height = None;
    hit.keep_next = flow.keep_next;
    hit.keep_lines = flow.keep_lines;
    hit.page_break_before = flow.page_break_before;
    hit.justification = flow.justification;
    Ok(hit)
}

fn limit_for_hit(shapes: bool, bytes: bool, lines: bool) -> LayoutError {
    if shapes {
        LayoutError::LimitExceeded(crate::LayoutLimit::ShapeCalls)
    } else if bytes {
        LayoutError::LimitExceeded(crate::LayoutLimit::ShapedBytes)
    } else if lines {
        LayoutError::LimitExceeded(crate::LayoutLimit::Lines)
    } else {
        LayoutError::LimitExceeded(crate::LayoutLimit::Glyphs)
    }
}

fn produced_contexts(
    document: &Document,
    snapshot: &LayoutSnapshot,
    deps: &[DependencyFlags],
) -> Vec<ExternalContext> {
    document
        .body()
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let flags = deps.get(index).copied().unwrap_or_default();
            if !flags.any() {
                return ExternalContext::default();
            }
            let Some(paragraph) = block.as_paragraph() else {
                return ExternalContext::default();
            };
            context_of(snapshot, paragraph.id(), flags)
        })
        .collect()
}

fn paragraph_has_page_break(paragraph: &Paragraph) -> bool {
    paragraph.inlines().iter().any(|inline| {
        matches!(
            inline.kind,
            joyword_model::InlineKind::Break(joyword_model::BreakKind::Page)
        )
    })
}

fn walk_back_keep_next(document: &Document, same: &mut [bool]) {
    let paragraphs: Vec<(usize, &Paragraph)> = document
        .body()
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(block_index, block)| {
            block
                .as_paragraph()
                .map(|paragraph| (block_index, paragraph))
        })
        .collect();
    let plans = crate::pages::section_plans(document).unwrap_or_default();
    let mut index = same.iter().position(|flag| !flag).unwrap_or(same.len());
    while index > 0 && index <= paragraphs.len() {
        let (_, previous) = paragraphs[index - 1];
        let here_breaks = paragraphs
            .get(index)
            .is_some_and(|(block_index, paragraph)| {
                style_paint::flow(document, paragraph).is_ok_and(|flow| flow.page_break_before)
                    || paragraph_has_page_break(paragraph)
                    || crate::pages::section_break_before(&plans, *block_index)
            })
            || paragraph_has_page_break(previous);
        let previous_keeps = style_paint::flow(document, previous).is_ok_and(|flow| flow.keep_next);
        if previous_keeps && !here_breaks {
            same[index - 1] = false;
            index -= 1;
        } else {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paragraph_key(
    document: &Document,
    paragraph: &Paragraph,
    faces: FaceSet<'_>,
    profile: SingleFontProfile,
    available_width: i64,
    context: ExternalContext,
    mark: Option<&Result<NumberLabel, NumberingProblem>>,
    view: ViewOptions,
) -> ParagraphCacheKey {
    let marked = crate::pages::marked_text(document, paragraph);
    let text = paragraph.plain_text(|_| String::new());
    let slot = dominant_slot(&text);
    let mixed = slot != FontSlot::Ascii;
    let flags = paragraph_dependencies(document, paragraph);
    ParagraphCacheKey {
        content: digest(marked.as_bytes()),
        resolved_style: resolved_style(document, paragraph, mark),
        available_width,
        font: FontEnvKey {
            shaping: faces.face(slot).key(),
            mixed: mixed.then_some(MixedFaces {
                ascii: faces.ascii.key(),
                h_ansi: faces.h_ansi.key(),
                east_asia: faces.east_asia.key(),
                complex: faces.complex.key(),
                fallback: faces.fallback.key(),
            }),
        },
        profile: ProfileFitKey {
            size: profile.size.raw(),
            kerning: profile.features.kerning,
            liga: profile.features.standard_ligatures,
            calt: profile.features.contextual_alternates,
            leading: uses_profile_leading(document, paragraph).then_some(profile.leading.raw()),
        },
        external: external_key(flags, context),
        show_hidden: view.show_hidden,
        field_codes: view.field_codes,
    }
}

fn uses_profile_leading(document: &Document, paragraph: &Paragraph) -> bool {
    match style_paint::flow(document, paragraph) {
        Ok(flow) => !matches!(flow.line, Some(LineSpacing::Auto(240))),
        Err(_) => true,
    }
}

fn resolved_style(
    document: &Document,
    paragraph: &Paragraph,
    mark: Option<&Result<NumberLabel, NumberingProblem>>,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    let styles = document.styles();
    hash.update(format!("{:?}", paragraph.props).as_bytes());
    let interval = match document.settings().default_tab_stop {
        None => 720 * Emu::PER_TWIP,
        Some(measure) => measure.absolute().map(|length| length.raw()).unwrap_or(0),
    };
    hash.update(interval.to_le_bytes());
    match mark {
        None => hash.update([0]),
        Some(Ok(label)) => {
            hash.update([1]);
            hash.update(label.text.as_bytes());
            hash.update([match label.suffix {
                NumberSuffix::Tab => 0,
                NumberSuffix::Space => 1,
                NumberSuffix::Nothing => 2,
            }]);
            hash.update(
                label
                    .size
                    .map(|size| size.to_string())
                    .unwrap_or_default()
                    .as_bytes(),
            );
            hash.update([u8::from(label.bold)]);
        }
        Some(Err(problem)) => {
            hash.update([2]);
            hash.update(problem.to_string().as_bytes());
        }
    }
    hash.update(format!("{:?}", paragraph.mark_props).as_bytes());
    hash_used(
        &mut hash,
        styles,
        paragraph.style.as_ref(),
        StyleKind::Paragraph,
    );
    hash.update(format!("{:?}", styles.defaults.para).as_bytes());
    hash.update(format!("{:?}", styles.defaults.run).as_bytes());
    hash.update(format!("{:?}", styles.theme()).as_bytes());
    for run in paragraph.runs() {
        hash.update(format!("{:?}", run.props).as_bytes());
        hash_used(&mut hash, styles, run.style.as_ref(), StyleKind::Character);
    }
    hash.finalize().into()
}

fn hash_used(hash: &mut Sha256, sheet: &StyleSheet, id: Option<&StyleId>, kind: StyleKind) {
    if let Some(id) = id {
        hash_chain(hash, sheet, id);
    } else if let Some(style) = sheet.default_of(kind) {
        hash_chain(hash, sheet, &style.id);
    }
}

fn hash_chain(hash: &mut Sha256, sheet: &StyleSheet, id: &StyleId) {
    let chain = sheet.chain(id);
    for style in &chain.styles {
        hash.update(style.id.as_str().as_bytes());
        hash.update(format!("{:?}", style.para).as_bytes());
        hash.update(format!("{:?}", style.run).as_bytes());
        if let Some(link) = &style.link {
            hash.update(link.as_str().as_bytes());
        }
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontKey;
    use crate::document::FittedParagraph;
    use joyword_foundation::Emu;
    use joyword_model::{BlockSeq, Document, PageSetup, Paragraph, Style, StyleKind};
    use std::sync::Arc;

    fn blank_fit(document: &mut Document) -> FittedParagraph {
        FittedParagraph {
            id: document.ids().node(),
            len: 0,
            lines: Vec::new(),
            diagnostics: Vec::new(),
            omitted: 0,
            shape_calls: 1,
            shaped_bytes: 4,
            glyphs: 1,
            before: Emu::new(0),
            after: Emu::new(0),
            line_box: Emu::new(1),
            height: Emu::new(1),
            group_height: None,
            keep_next: false,
            keep_lines: false,
            page_break_before: false,
            justification: None,
            baseline: Emu::new(0),
            geometry: PageGeometry {
                width: Emu::new(11_906 * Emu::PER_TWIP),
                height: Emu::new(16_838 * Emu::PER_TWIP),
                left: Emu::new(1_800 * Emu::PER_TWIP),
                top: Emu::new(1_440 * Emu::PER_TWIP),
                right: Emu::new(1_800 * Emu::PER_TWIP),
                bottom: Emu::new(1_440 * Emu::PER_TWIP),
                header: Emu::new(851 * Emu::PER_TWIP),
                footer: Emu::new(992 * Emu::PER_TWIP),
            },
            section_index: 0,
            section_break_before: false,
            section_edge: crate::pages::SectionEdge::Same,
            column_count: 1,
            column_gap: 0,
            column_width: 1,
            column_breaks: false,
            section_id: document.ids().node(),
            page_start: None,
            used_field_cache: false,
            piece_breaks: Vec::new(),
            piece_lines: Vec::new(),
            trailing_breaks: 0,
            frame_x: Emu::new(0),
            frame_width: Emu::new(1),
            pinned: false,
        }
    }

    fn key_for(external: ExternalKey) -> ParagraphCacheKey {
        ParagraphCacheKey {
            content: [1; 32],
            resolved_style: [2; 32],
            available_width: 100,
            font: FontEnvKey {
                shaping: FontKey::from_raw_parts([0; 32], 0),
                mixed: None,
            },
            profile: ProfileFitKey {
                size: 12,
                kerning: false,
                liga: false,
                calt: false,
                leading: None,
            },
            external,
            show_hidden: false,
            field_codes: false,
        }
    }

    #[test]
    fn omitting_a_live_page_dependency_reuses_another_pages_fit() {
        let mut document = Document::new(PageSetup::a4_zh_cn());
        let page1 = ExternalContext {
            page_number: 1,
            ..ExternalContext::default()
        };
        let page2 = ExternalContext {
            page_number: 2,
            ..ExternalContext::default()
        };
        let live = DependencyFlags {
            page_number: true,
            ..DependencyFlags::default()
        };
        let on_page_1 = key_for(external_key(live, page1));
        let on_page_2 = key_for(external_key(live, page2));
        assert_ne!(on_page_1, on_page_2);
        let mut cache = ParagraphLayoutCache::default();
        cache.insert(on_page_1, blank_fit(&mut document));
        assert!(cache.cloned(&on_page_2).is_none());
        // Dropping the flag makes both pages one key, so page 2 reads page 1.
        let forgotten = DependencyFlags::default();
        let stale_page_1 = key_for(external_key(forgotten, page1));
        let stale_page_2 = key_for(external_key(forgotten, page2));
        assert_eq!(stale_page_1, stale_page_2);
        let mut stale = ParagraphLayoutCache::default();
        let stored = blank_fit(&mut document);
        let stored_id = stored.id;
        stale.insert(stale_page_1, stored);
        let hit = stale.cloned(&stale_page_2);
        assert_eq!(hit.as_ref().map(|fitted| fitted.id), Some(stored_id));
    }

    #[test]
    fn cache_evicts_to_stay_inside_its_budget() {
        let mut document = Document::new(PageSetup::a4_zh_cn());
        let first = blank_fit(&mut document);
        let bytes = fit_bytes(&first);
        let mut cache = ParagraphLayoutCache::with_budget(bytes);
        let first_key = key_for(ExternalKey::default());
        cache.insert(first_key, first);
        assert_eq!(cache.resident_bytes(), bytes);
        let mut second_key = key_for(ExternalKey::default());
        second_key.content = [9; 32];
        cache.insert(second_key, blank_fit(&mut document));
        assert!(cache.resident_bytes() <= bytes);
        assert!(cache.cloned(&first_key).is_none());
        assert!(cache.cloned(&second_key).is_some());
        let mut huge = ParagraphLayoutCache::with_budget(bytes.saturating_sub(1));
        huge.insert(first_key, blank_fit(&mut document));
        assert_eq!(huge.resident_bytes(), 0);
    }

    #[test]
    fn repeated_feedback_is_unstable_and_unpublished() {
        let result = run_layout_passes(0u8, |assumed| {
            let produced = if *assumed == 0 { 1 } else { 0 };
            Ok((*assumed, produced))
        });
        assert!(matches!(result, Err(LayoutError::Unstable)));
    }

    #[test]
    fn style_closure_reaches_dependents_and_not_the_rest() {
        let mut document = Document::new(PageSetup::a4_zh_cn());
        let mut parent = Style::new("Parent", StyleKind::Paragraph, Some("Parent"));
        parent.is_default = true;
        let mut child = Style::new("Child", StyleKind::Paragraph, Some("Child"));
        child.based_on = Some(parent.id.clone());
        let mut linked = Style::new("Linked", StyleKind::Character, Some("Linked"));
        linked.link = Some(parent.id.clone());
        let other = Style::new("Other", StyleKind::Paragraph, Some("Other"));
        assert!(document.styles_mut().insert(parent.clone()).is_ok());
        assert!(document.styles_mut().insert(child.clone()).is_ok());
        assert!(document.styles_mut().insert(linked.clone()).is_ok());
        assert!(document.styles_mut().insert(other.clone()).is_ok());
        let mut changed = BTreeSet::new();
        changed.insert(parent.id.clone());
        let affected = affected_style_closure(document.styles(), &changed);
        assert!(affected.contains(&parent.id));
        assert!(affected.contains(&child.id));
        assert!(affected.contains(&linked.id));
        assert!(!affected.contains(&other.id));

        let story = document.body_id();
        let mut using_child = Paragraph::new(document.ids().node());
        using_child.style = Some(child.id.clone());
        assert!(
            using_child
                .push_text("子样式", None, Arc::default())
                .is_ok()
        );
        let mut using_other = Paragraph::new(document.ids().node());
        using_other.style = Some(other.id.clone());
        assert!(using_other.push_text("其他", None, Arc::default()).is_ok());
        let mut bare = Paragraph::new(document.ids().node());
        assert!(bare.push_text("默认", None, Arc::default()).is_ok());
        let installed = document.story_mut(story).map(|body| {
            body.blocks = BlockSeq::from_vec(vec![
                using_child.clone().into(),
                using_other.clone().into(),
                bare.clone().into(),
            ]);
            true
        });
        assert_eq!(installed, Some(true));
        let invalidation = LayoutInvalidation {
            styles: changed,
            ..LayoutInvalidation::default()
        };
        let ids = invalidated_paragraphs(&document, &invalidation);
        assert!(ids.contains(&using_child.id()));
        assert!(ids.contains(&bare.id()));
        assert!(!ids.contains(&using_other.id()));
    }

    #[test]
    fn defaults_change_invalidates_every_paragraph() {
        let mut document = Document::new(PageSetup::a4_zh_cn());
        let story = document.body_id();
        let mut first = Paragraph::new(document.ids().node());
        assert!(first.push_text("甲", None, Arc::default()).is_ok());
        let mut second = Paragraph::new(document.ids().node());
        assert!(second.push_text("乙", None, Arc::default()).is_ok());
        let first_id = first.id();
        let second_id = second.id();
        let installed = document.story_mut(story).map(|body| {
            body.blocks = BlockSeq::from_vec(vec![first.into(), second.into()]);
            true
        });
        assert_eq!(installed, Some(true));
        let ids = invalidated_paragraphs(
            &document,
            &LayoutInvalidation {
                defaults_changed: true,
                ..LayoutInvalidation::default()
            },
        );
        assert!(ids.contains(&first_id) && ids.contains(&second_id));
    }
}
