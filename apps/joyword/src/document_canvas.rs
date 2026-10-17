//! Editable in-memory engineering document: real engine snapshot, immutable layout and
//! CPU scene output. Loading, layout and raster work run on background jobs; a
//! layout is usable for geometry before its pages are rasterized, and pages are
//! rasterized one at a time around the viewport.
use gpui::RenderImage;
#[cfg(test)]
use joyword_engine::{EditPolicy, Session};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutKey, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document,
};
#[cfg(test)]
use joyword_model::{BlockSeq, Document, PageSetup, Paragraph};
#[cfg(test)]
use joyword_render::RasterPage;
use joyword_render::requests::{RasterRequestGate, RasterTicket};
use joyword_render::{
    Projection, RasterCancellation, RasterError, RasterLimit, RasterLimits, raster_size, rasterize,
};
use joyword_shell::input::InputView;
use std::collections::VecDeque;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
const SEED: &str = include_str!("../../../tests/fixtures/text/cjk-contract.txt");
/// Startup document package (DOCX-SAMPLE-001): the seed text as a real DOCX,
/// so the sample edits, journals and saves like any opened file.
const SAMPLE_DOCX: &[u8] = include_bytes!("../../../tests/fixtures/docx/DOCX-SAMPLE-001.docx");
/// Resident pixel ceiling of the page cache. It bounds memory, not the
/// document: the pages farthest from the viewport are evicted to stay within
/// it, and only a page too large for it on its own cannot be shown.
const MAX_CACHE_PIXELS: u64 = 16_777_216;
/// Pages kept rasterized after and before the visible ones.
const PREFETCH_PAGES: usize = 2;
/// Failed pages remembered so they are not requested again.
const MAX_FAILED_PAGES: usize = 64;

#[cfg(test)]
pub struct CanvasSource {
    pub session: Session,
    pub layout: LayoutSnapshot,
    pub font: LoadedFont,
    pub profile: SingleFontProfile,
}

/// The window's startup document: DOCX-SAMPLE-001 imported as a real package
/// session, so Save As, recovery journaling and undo behave exactly as they
/// do for a file the user opened.
pub struct AppSource {
    pub session: joyword_shell::docx::DocxSession,
    pub layout: LayoutSnapshot,
    pub font: LoadedFont,
    pub profile: SingleFontProfile,
}

fn load_font() -> Result<LoadedFont, String> {
    let requested = std::env::var("JOYWORD_FONT").ok();
    load_font_spec(requested.as_deref())
}
fn load_font_spec(requested: Option<&str>) -> Result<LoadedFont, String> {
    let candidates = if let Some(spec) = requested {
        let (path, index) = spec
            .rsplit_once(':')
            .ok_or("JOYWORD_FONT must be path:face")?;
        vec![(
            path.to_owned(),
            index.parse::<u32>().map_err(|_| "invalid font face")?,
        )]
    } else {
        vec![
            (
                "/System/Library/Fonts/Supplemental/Songti.ttc".to_owned(),
                6,
            ),
            (
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".to_owned(),
                2,
            ),
            ("C:\\Windows\\Fonts\\simsun.ttc".to_owned(), 0),
        ]
    };
    for (path, face) in candidates {
        if !Path::new(&path).is_file() {
            continue;
        }
        // Bound actual IO before allocating; metadata alone can race a writer.
        use std::io::Read as _;
        let mut data = Vec::new();
        std::fs::File::open(&path)
            .map_err(|e| e.to_string())?
            .take((128 << 20) + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > 128 << 20 {
            return Err("font exceeds 128 MiB".into());
        }
        return LoadedFont::from_bytes(data.into(), face)
            .map_err(|e| format!("font {path} face {face}: {e:?}"));
    }
    Err("CJK font unavailable; set JOYWORD_FONT=path:face".into())
}

fn sample_profile() -> SingleFontProfile {
    SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    }
}

fn lay_out_snapshot(
    snapshot: &joyword_model::DocumentSnapshot,
    font: &LoadedFont,
    profile: SingleFontProfile,
) -> Result<LayoutSnapshot, String> {
    let layout = layout_document(
        snapshot,
        FaceSet::uniform(font),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .map_err(|e| e.to_string())?;
    if !layout.diagnostics().is_empty() || layout.omitted_diagnostics() != 0 {
        return Err("selected font cannot display the complete sample".into());
    }
    Ok(layout)
}

pub fn load_app_source() -> Result<AppSource, String> {
    app_source_with_font(load_font()?)
}

pub(crate) fn app_source_with_font(font: LoadedFont) -> Result<AppSource, String> {
    let session =
        joyword_shell::docx::DocxSession::open_default(SAMPLE_DOCX).map_err(|e| e.to_string())?;
    let profile = sample_profile();
    let layout = lay_out_snapshot(&session.session().document().snapshot(), &font, profile)?;
    Ok(AppSource {
        session,
        layout,
        font,
        profile,
    })
}

#[cfg(test)]
pub(crate) fn source_with_text(font: LoadedFont, text: &str) -> Result<CanvasSource, String> {
    let mut document = Document::new(PageSetup::a4_zh_cn());
    let mut blocks = Vec::new();
    for text in text.lines() {
        let mut p = Paragraph::new(document.ids().node());
        p.push_text(text, None, Arc::default())
            .map_err(|e| e.to_string())?;
        blocks.push(p.into());
    }
    let story = document.body_id();
    document.story_mut(story).expect("body").blocks = BlockSeq::from_vec(blocks);
    let mut session = Session::new(document);
    session.set_policy(EditPolicy {
        read_only: false,
        agent_writes: false,
    });
    let profile = sample_profile();
    let layout = lay_out_snapshot(&session.document().snapshot(), &font, profile)?;
    Ok(CanvasSource {
        session,
        layout,
        font,
        profile,
    })
}

#[cfg(test)]
pub fn raster_pages(
    source: &CanvasSource,
    projection: Projection,
    cancellation: &RasterCancellation,
) -> Result<Vec<RasterPage>, String> {
    source
        .layout
        .ensure_current(
            &source.session.document().snapshot(),
            &source.font,
            source.profile,
        )
        .map_err(|e| e.to_string())?;
    source
        .layout
        .pages()
        .iter()
        .map(|scene| {
            rasterize(
                scene,
                std::slice::from_ref(&source.font),
                projection,
                page_limits(MAX_CACHE_PIXELS),
                cancellation,
            )
            .map_err(|e| e.to_string())
        })
        .collect()
}

/// Renderer limits for one page: no page may be larger than the whole cache.
fn page_limits(budget: u64) -> RasterLimits {
    RasterLimits {
        max_pixels: budget,
        ..RasterLimits::default()
    }
}

pub fn image_from_bgra(width: u32, height: u32, bgra: &[u8]) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(width, height, bgra.to_vec())?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

#[cfg(test)]
pub fn image(page: RasterPage) -> Arc<RenderImage> {
    let (w, h) = (page.width(), page.height());
    let buffer =
        image::RgbaImage::from_raw(w, h, page.into_bgra()).expect("validated raster dimensions");
    Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use joyword_engine::{Command, Transaction};
    use joyword_model::{Bias, Position, ScalarOffset};
    fn font() -> LoadedFont {
        LoadedFont::from_bytes(
            Arc::from(
                include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
            ),
            0,
        )
        .unwrap()
    }
    fn start(source: &CanvasSource) -> Position {
        Position {
            story: source.session.document().body_id(),
            paragraph: source.session.document().body().blocks.get(0).unwrap().id(),
            offset: ScalarOffset(0),
            bias: Bias::Before,
        }
    }
    #[test]
    fn real_engine_session_is_editable_and_layout_matches_source() {
        let mut source = source_with_text(font(), "AV\nfi").unwrap();
        source
            .layout
            .ensure_current(&source.session.snapshot(), &source.font, source.profile)
            .unwrap();
        assert_eq!(
            source.layout.pages()[0]
                .runs
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            ["AV", "fi"]
        );
        let at = start(&source);
        assert!(
            source
                .session
                .apply(Transaction::new(
                    source.session.version(),
                    vec![Command::InsertText {
                        at,
                        text: "A".into()
                    }]
                ))
                .is_ok()
        );
        assert!(!source.session.policy().agent_writes);
    }
    #[test]
    fn page_image_conversion_and_device_projection_need_no_window() {
        let source = source_with_text(font(), "AV\nfi").unwrap();
        let pages = raster_pages(
            &source,
            Projection::new(96.0, 0.8).unwrap(),
            &RasterCancellation::default(),
        )
        .unwrap();
        let pixels = pages[0].clone();
        let expected = pixels.clone().into_bgra();
        let image = image(pixels);
        assert_eq!(image.as_bytes(0).unwrap(), expected);
        assert_eq!(image.frame_count(), 1);
        let bigger = raster_pages(
            &source,
            Projection::new(192.0, 0.8).unwrap(),
            &RasterCancellation::default(),
        )
        .unwrap();
        assert!((i64::from(bigger[0].width()) - 2 * i64::from(pages[0].width())).abs() <= 1);
        assert!((i64::from(bigger[0].height()) - 2 * i64::from(pages[0].height())).abs() <= 1);
    }
    #[test]
    fn font_missing_invalid_spec_and_missing_sample_glyphs_are_errors() {
        assert!(load_font_spec(Some("invalid")).is_err());
        assert!(load_font_spec(Some("/no-such-joyword-test-font:0")).is_err());
        assert!(load_font_spec(Some("test:face")).is_err());
        assert!(source_with_text(font(), SEED).is_err());
    }
    #[test]
    fn cancelled_or_stale_layout_does_not_create_images() {
        let mut source = source_with_text(font(), "AV\nfi").unwrap();
        let cancel = RasterCancellation::default();
        cancel.cancel();
        assert!(raster_pages(&source, Projection::new(96.0, 1.0).unwrap(), &cancel).is_err());
        source.session.set_policy(EditPolicy::default());
        let at = start(&source);
        source
            .session
            .apply(Transaction::new(
                source.session.version(),
                vec![Command::InsertText {
                    at,
                    text: "A".into(),
                }],
            ))
            .unwrap();
        assert!(
            raster_pages(
                &source,
                Projection::new(96.0, 1.0).unwrap(),
                &RasterCancellation::default()
            )
            .is_err()
        );
    }
}

/// Identity of a layout request. Projection is not part of it: zoom and
/// device scale change pixels, never positions. Input revisions restart for
/// every opened document, so the document number keeps them apart.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutRequestKey {
    pub document: u64,
    pub revision: u64,
    pub font: joyword_layout::FontKey,
    pub profile: SingleFontProfile,
}
/// Identity of one page's pixels: the layout by its own key (version,
/// content, font and profile), the page and the projection. Pixels depend on
/// nothing else, so revisions reusing a layout keep them. The document number
/// keeps requests of another opened document apart.
#[derive(Clone, Debug, PartialEq)]
pub struct PageKey {
    pub document: u64,
    pub layout: LayoutKey,
    pub page: usize,
    pub projection: Projection,
}
impl PageKey {
    /// Whether both are pixels of the same layout at the same projection.
    fn same_raster(&self, other: &Self) -> bool {
        self.document == other.document
            && self.layout == other.layout
            && self.projection == other.projection
    }
}
/// A layout installed as soon as it lands. Geometry needs nothing more.
pub struct LayoutFrame {
    pub key: LayoutRequestKey,
    pub layout: Arc<LayoutSnapshot>,
}
pub struct LayoutJob {
    pub ticket: RasterTicket<LayoutRequestKey>,
    pub cancellation: LayoutCancellation,
}
/// One page of an installed layout to rasterize in the background.
pub struct RasterJob {
    pub ticket: RasterTicket<PageKey>,
    pub layout: Arc<LayoutSnapshot>,
    pub limits: RasterLimits,
    pub cancellation: RasterCancellation,
}
/// What one page's raster made: its pixels, and caret pixels copied from
/// them when the job carried the caret. The overlay is not cached; it is
/// shown over the page until the caret's page is rasterized again.
#[derive(Clone)]
pub struct RasterOutput {
    pub image: Arc<RenderImage>,
    /// Caret pixels copied from this page. `None` when no caret was supplied.
    pub caret_overlay: Option<CaretOverlay>,
}

#[derive(Clone)]
pub struct CaretOverlay {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}
/// What became of a background result.
pub enum Landed {
    /// Superseded, or made for another revision, layout or projection.
    Stale,
    /// Installed. Pixels it replaced or evicted are returned for the caller
    /// to release.
    Installed(Vec<Arc<RenderImage>>),
    /// The current request failed and nothing was installed. A failed page
    /// keeps its own error; it is not a canvas error.
    Failed(String),
}
/// What the box of one page shows.
#[derive(Clone, Debug)]
pub enum PageImage {
    /// Pixels of the installed layout at the current projection.
    Ready(Arc<RenderImage>),
    /// The last pixels of this page, from an earlier layout or projection,
    /// painted until its own land. Pixels never map positions.
    Stale(Arc<RenderImage>),
    /// Not rasterized yet: the blank page is its placeholder.
    Pending,
    /// This page alone exceeds the cache ceiling at the current projection.
    TooLarge,
    /// The raster of this page failed; other pages are unaffected.
    Failed(String),
}

struct CachedPage {
    key: PageKey,
    image: Arc<RenderImage>,
    pixels: u64,
}
/// Rasterized pages, oldest first. Its pixel budget bounds what stays
/// resident; the number of pages in the document never enters.
struct PageCache {
    budget: u64,
    entries: Vec<CachedPage>,
    failed: VecDeque<(PageKey, String)>,
}
impl PageCache {
    fn new(budget: u64) -> Self {
        Self {
            budget,
            entries: Vec::new(),
            failed: VecDeque::new(),
        }
    }
    fn get(&self, key: &PageKey) -> Option<&CachedPage> {
        self.entries.iter().find(|entry| &entry.key == key)
    }
    /// The newest pixels of `page`, whichever layout or projection they show.
    fn newest(&self, page: usize) -> Option<&CachedPage> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.key.page == page)
    }
    fn failure(&self, key: &PageKey) -> Option<&str> {
        self.failed
            .iter()
            .find(|(failed, _)| failed == key)
            .map(|(_, error)| error.as_str())
    }
    fn fail(&mut self, key: PageKey, error: String) {
        if self.failed.len() == MAX_FAILED_PAGES {
            self.failed.pop_front();
        }
        self.failed.push_back((key, error));
    }
    fn resident(&self) -> u64 {
        self.entries.iter().map(|entry| entry.pixels).sum()
    }
    /// Installs `image` for `key`, superseding older pixels of the same page.
    /// While over budget, pixels of other layouts or projections go first,
    /// then those farthest from `visible`; the new page always stays. Returns
    /// what left the cache.
    fn insert(
        &mut self,
        key: PageKey,
        image: Arc<RenderImage>,
        visible: &Range<usize>,
    ) -> Vec<Arc<RenderImage>> {
        let mut released = Vec::new();
        self.entries.retain(|entry| {
            let keep = entry.key.page != key.page;
            if !keep {
                released.push(entry.image.clone());
            }
            keep
        });
        let size = image.size(0);
        let pixels =
            u64::from(size.width.0.unsigned_abs()) * u64::from(size.height.0.unsigned_abs());
        self.entries.push(CachedPage { key, image, pixels });
        while self.resident() > self.budget {
            let Some((newest, older)) = self.entries.split_last() else {
                break;
            };
            let Some(victim) = older
                .iter()
                .enumerate()
                .max_by_key(|(_, entry)| {
                    (
                        !entry.key.same_raster(&newest.key),
                        distance(entry.key.page, visible),
                    )
                })
                .map(|(i, _)| i)
            else {
                break;
            };
            released.push(self.entries.remove(victim).image);
        }
        released
    }
    fn clear(&mut self) -> Vec<Arc<RenderImage>> {
        self.failed.clear();
        self.entries.drain(..).map(|entry| entry.image).collect()
    }
}
/// Pages between `page` and the nearest visible one.
fn distance(page: usize, visible: &Range<usize>) -> usize {
    if page < visible.start {
        visible.start - page
    } else {
        (page + 1).saturating_sub(visible.end)
    }
}
/// Pages to keep rasterized, most urgent first: the visible ones, then
/// alternately the next and the previous up to `PREFETCH_PAGES` away. The
/// list ends where its pages would no longer fit the cache together, so the
/// pixels it wants never evict each other. Pages refused on their own (too
/// large, invalid) are left out and never requested.
fn wanted_pages(
    layout: &LayoutSnapshot,
    projection: Projection,
    visible: Range<usize>,
    budget: u64,
) -> Vec<usize> {
    let pages = layout.pages();
    if pages.is_empty() {
        return Vec::new();
    }
    let first = visible.start.min(pages.len() - 1);
    let end = visible.end.clamp(first + 1, pages.len());
    let mut order: Vec<usize> = (first..end).collect();
    for step in 1..=PREFETCH_PAGES {
        order.extend(Some(end + step - 1).filter(|&page| page < pages.len()));
        order.extend(first.checked_sub(step));
    }
    let mut total = 0u64;
    let mut wanted = Vec::new();
    for page in order {
        let Ok((width, height)) = raster_size(&pages[page], projection, page_limits(budget)) else {
            continue;
        };
        total += u64::from(width) * u64::from(height);
        if total > budget {
            break;
        }
        wanted.push(page);
    }
    wanted
}

/// Canvas state in two stages, each behind its own request gate. A layout is
/// installed as soon as it lands, so hit testing, IME geometry, reveal and
/// overlays are exact while its pages are still being rasterized. Pages are
/// rasterized one job at a time, nearest the viewport first, into a cache
/// whose pixel ceiling bounds memory rather than the document. Each page keeps
/// its last pixels painted until those of the installed layout land.
pub struct CanvasPipeline {
    document: u64,
    layout: Option<LayoutFrame>,
    pages: PageCache,
    /// Caret pixels of the page whose raster carried the caret, keyed by the
    /// pixels they were copied from.
    caret_overlay: Option<(PageKey, CaretOverlay)>,
    /// Pages the view showed at the last raster request.
    visible: Range<usize>,
    /// The page of the latest raster request, until it lands.
    running: Option<PageKey>,
    layout_gate: RasterRequestGate<LayoutRequestKey>,
    raster_gate: RasterRequestGate<PageKey>,
    layout_cancel: LayoutCancellation,
    raster_cancel: RasterCancellation,
}
impl Default for CanvasPipeline {
    fn default() -> Self {
        Self::with_budget(MAX_CACHE_PIXELS)
    }
}
impl CanvasPipeline {
    fn with_budget(budget: u64) -> Self {
        Self {
            document: 0,
            layout: None,
            pages: PageCache::new(budget),
            caret_overlay: None,
            visible: 0..0,
            running: None,
            layout_gate: RasterRequestGate::default(),
            raster_gate: RasterRequestGate::default(),
            layout_cancel: LayoutCancellation::default(),
            raster_cancel: RasterCancellation::default(),
        }
    }
    pub fn key(
        &self,
        view: &InputView,
        font: &LoadedFont,
        profile: SingleFontProfile,
    ) -> LayoutRequestKey {
        LayoutRequestKey {
            document: self.document,
            revision: view.revision(),
            font: font.key(),
            profile,
        }
    }
    /// The installed layout, current or not: page boxes and scroll extent.
    pub fn layout(&self) -> Option<&LayoutFrame> {
        self.layout.as_ref()
    }
    /// The installed layout only when it was made for `current`.
    pub fn ready_layout(&self, current: &LayoutRequestKey) -> Option<&LayoutFrame> {
        self.layout.as_ref().filter(|frame| &frame.key == current)
    }
    fn page_key(&self, layout: &LayoutSnapshot, page: usize, projection: Projection) -> PageKey {
        PageKey {
            document: self.document,
            layout: layout.key().clone(),
            page,
            projection,
        }
    }
    /// Pixels of `page` of the current layout at `projection`.
    #[cfg(test)]
    pub fn ready_page(
        &self,
        current: &LayoutRequestKey,
        page: usize,
        projection: Projection,
    ) -> Option<&Arc<RenderImage>> {
        let frame = self.ready_layout(current)?;
        let key = self.page_key(&frame.layout, page, projection);
        self.pages.get(&key).map(|entry| &entry.image)
    }
    /// What the box of `page` of the installed layout shows at `projection`.
    pub fn page_image(&self, page: usize, projection: Projection) -> PageImage {
        let Some(frame) = &self.layout else {
            return PageImage::Pending;
        };
        let Some(scene) = frame.layout.pages().get(page) else {
            return PageImage::Pending;
        };
        let key = self.page_key(&frame.layout, page, projection);
        if let Some(entry) = self.pages.get(&key) {
            return PageImage::Ready(entry.image.clone());
        }
        match raster_size(scene, projection, page_limits(self.pages.budget)) {
            Err(RasterError::LimitExceeded(RasterLimit::Pixels | RasterLimit::Dimensions)) => {
                return PageImage::TooLarge;
            }
            Err(error) => return PageImage::Failed(error.to_string()),
            Ok(_) => {}
        }
        if let Some(error) = self.pages.failure(&key) {
            return PageImage::Failed(error.to_owned());
        }
        self.pages.newest(page).map_or(PageImage::Pending, |entry| {
            PageImage::Stale(entry.image.clone())
        })
    }
    /// Caret pixels for `page` at `projection`, when the raster that made the
    /// page's current pixels carried the caret. Only that page ever has one.
    pub fn caret_overlay(&self, page: usize, projection: Projection) -> Option<&CaretOverlay> {
        let frame = self.layout.as_ref()?;
        let key = self.page_key(&frame.layout, page, projection);
        let (stored, overlay) = self.caret_overlay.as_ref()?;
        if *stored == key && self.pages.get(&key).is_some() {
            Some(overlay)
        } else {
            None
        }
    }
    /// Pixels resident in the page cache.
    #[cfg(test)]
    pub fn resident_pixels(&self) -> u64 {
        self.pages.resident()
    }
    /// A layout job for `current`, unless it is installed or already running.
    /// It cancels the older layout job. A raster job of the installed layout
    /// keeps running: its pixels are still the newest that can be shown.
    pub fn request_layout(&mut self, current: LayoutRequestKey) -> Option<LayoutJob> {
        if self.ready_layout(&current).is_some() {
            return None;
        }
        let ticket = self.layout_gate.request(current)?;
        self.layout_cancel.cancel();
        self.layout_cancel = LayoutCancellation::default();
        Some(LayoutJob {
            ticket,
            cancellation: self.layout_cancel.clone(),
        })
    }
    /// Only the latest request, still for `current`, installs.
    pub fn install_layout(
        &mut self,
        ticket: &RasterTicket<LayoutRequestKey>,
        current: &LayoutRequestKey,
        result: Result<LayoutSnapshot, String>,
    ) -> Landed {
        if !self.layout_gate.accepts(ticket, current) {
            return Landed::Stale;
        }
        match result {
            Ok(layout) => {
                self.layout = Some(LayoutFrame {
                    key: current.clone(),
                    layout: Arc::new(layout),
                });
                Landed::Installed(Vec::new())
            }
            Err(error) => Landed::Failed(error),
        }
    }
    /// A raster job for the most urgent page around `visible` of the current
    /// layout at `projection` that has no pixels yet. Resident pages, failed
    /// pages and the page being made are not requested again. A running job
    /// keeps going while its page is still wanted; otherwise the new job
    /// cancels it.
    pub fn request_raster(
        &mut self,
        current: &LayoutRequestKey,
        projection: Projection,
        visible: Range<usize>,
    ) -> Option<RasterJob> {
        self.visible = visible.clone();
        let layout = self.ready_layout(current)?.layout.clone();
        let wanted = wanted_pages(&layout, projection, visible, self.pages.budget)
            .into_iter()
            .map(|page| self.page_key(&layout, page, projection))
            .collect::<Vec<_>>();
        if self
            .running
            .as_ref()
            .is_some_and(|running| wanted.contains(running))
        {
            return None;
        }
        let key = wanted
            .into_iter()
            .find(|key| self.pages.get(key).is_none() && self.pages.failure(key).is_none())?;
        // Completion is tracked here, so a page whose earlier result was
        // dropped (landed stale, or evicted) gets a fresh ticket.
        let ticket = self.raster_gate.renew(key);
        self.raster_cancel.cancel();
        self.raster_cancel = RasterCancellation::default();
        self.running = Some(ticket.key().clone());
        Some(RasterJob {
            ticket,
            layout,
            limits: page_limits(self.pages.budget),
            cancellation: self.raster_cancel.clone(),
        })
    }
    /// Only the latest request, for the layout installed now and the current
    /// projection, installs. Pixels never map positions, so they may land
    /// while a newer layout is still pending. A failure stays with its page:
    /// that page shows it and is not requested again. A caret overlay rides
    /// the pixels it was copied from and stays keyed to them.
    pub fn install_raster(
        &mut self,
        ticket: &RasterTicket<PageKey>,
        projection: Projection,
        result: Result<RasterOutput, String>,
    ) -> Landed {
        if self.raster_gate.accepts(ticket, ticket.key()) {
            self.running = None;
        }
        let Some(frame) = &self.layout else {
            return Landed::Stale;
        };
        let current = self.page_key(&frame.layout, ticket.key().page, projection);
        if !self.raster_gate.accepts(ticket, &current) || self.raster_cancel.is_cancelled() {
            return Landed::Stale;
        }
        match result {
            Ok(output) => {
                if let Some(overlay) = output.caret_overlay {
                    self.caret_overlay = Some((current.clone(), overlay));
                }
                Landed::Installed(self.pages.insert(current, output.image, &self.visible))
            }
            Err(error) => {
                self.pages.fail(current, error.clone());
                Landed::Failed(error)
            }
        }
    }
    /// Keep the installed layout, and with it its pixels, for a revision whose
    /// snapshot it still matches: a commit equal to its preview, a selection
    /// change. Source IDs, version and content must all match.
    pub fn reuse(
        &mut self,
        view: &InputView,
        font: &LoadedFont,
        profile: SingleFontProfile,
    ) -> bool {
        let Some(frame) = &mut self.layout else {
            return false;
        };
        if frame
            .layout
            .ensure_current(view.snapshot(), font, profile)
            .is_err()
        {
            return false;
        }
        frame.key.revision = view.revision();
        true
    }
    /// Drop both stages for another document and cancel their jobs. Returns
    /// the pixels for the caller to release.
    pub fn clear(&mut self) -> Vec<Arc<RenderImage>> {
        self.cancel();
        self.document = self
            .document
            .checked_add(1)
            .expect("document number exhausted");
        self.layout = None;
        self.caret_overlay = None;
        self.running = None;
        self.visible = 0..0;
        self.pages.clear()
    }
    pub fn cancel(&self) {
        self.layout_cancel.cancel();
        self.raster_cancel.cancel();
    }
}

/// Background layout stage.
pub fn layout_view(
    view: &InputView,
    font: &LoadedFont,
    profile: SingleFontProfile,
    cancellation: &LayoutCancellation,
) -> Result<LayoutSnapshot, String> {
    layout_document(
        view.snapshot(),
        FaceSet::uniform(font),
        profile,
        LayoutLimits::default(),
        cancellation,
    )
    .map_err(|e| e.to_string())
}
/// Background raster stage: one page of an installed layout. When `caret`
/// (x, top and height) is on this page, its output also carries caret pixels
/// copied from the fresh page; the page image itself keeps no caret.
pub fn raster_page(
    layout: &LayoutSnapshot,
    page: usize,
    font: &LoadedFont,
    projection: Projection,
    limits: RasterLimits,
    caret: Option<(Emu, Emu, Emu)>,
    cancellation: &RasterCancellation,
) -> Result<RasterOutput, String> {
    let scene = layout
        .pages()
        .get(page)
        .ok_or_else(|| RasterError::InvalidPage.to_string())?;
    let raster = rasterize(
        scene,
        std::slice::from_ref(font),
        projection,
        limits,
        cancellation,
    )
    .map_err(|e| e.to_string())?;
    let (width, height) = (raster.width(), raster.height());
    let bgra = raster.into_bgra();
    let caret_overlay = caret.map(|(x, top, height_emu)| CaretOverlay {
        width,
        height,
        bgra: joyword_render::paint_caret_overlay(
            &bgra, width, height, projection, x, top, height_emu,
        ),
    });
    let Some(image) = image_from_bgra(width, height, &bgra) else {
        return Err("raster image".into());
    };
    Ok(RasterOutput {
        image,
        caret_overlay,
    })
}

#[cfg(test)]
mod input_pipeline_tests {
    use super::*;
    use joyword_model::Bias;
    use joyword_shell::input::{InputLimits, TextInput};
    fn font() -> LoadedFont {
        LoadedFont::from_bytes(
            Arc::from(
                include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
            ),
            0,
        )
        .unwrap()
    }
    fn source() -> CanvasSource {
        source_with_text(font(), "AV\nfi").unwrap()
    }
    fn pages_source(lines: usize) -> CanvasSource {
        source_with_text(font(), &vec!["AV fi"; lines].join("\n")).unwrap()
    }
    fn input(source: &CanvasSource) -> TextInput {
        TextInput::new(
            Session::new(source.session.document().clone()),
            InputLimits::default(),
        )
        .unwrap()
    }
    fn zoom(zoom: f64) -> Projection {
        Projection::new(96.0, zoom).unwrap()
    }
    /// Pixels `rasterize` gives one page of `source` at `projection`.
    fn page_pixels(source: &CanvasSource, projection: Projection) -> u64 {
        let (w, h) = raster_size(
            &source.layout.pages()[0],
            projection,
            RasterLimits::default(),
        )
        .unwrap();
        u64::from(w) * u64::from(h)
    }
    /// The layout stage, run here as the app runs it on the background executor.
    fn lay_out(pipeline: &mut CanvasPipeline, view: &InputView, source: &CanvasSource) -> Landed {
        let key = pipeline.key(view, &source.font, source.profile);
        let job = pipeline.request_layout(key.clone()).expect("layout job");
        let result = layout_view(view, &source.font, source.profile, &job.cancellation);
        pipeline.install_layout(&job.ticket, &key, result)
    }
    /// The raster stage of `job`, observing `cancellation`.
    fn run(
        job: &RasterJob,
        source: &CanvasSource,
        cancellation: &RasterCancellation,
    ) -> Result<RasterOutput, String> {
        let key = job.ticket.key();
        raster_page(
            &job.layout,
            key.page,
            &source.font,
            key.projection,
            job.limits,
            None,
            cancellation,
        )
    }
    /// One raster job for the first page.
    fn paint(
        pipeline: &mut CanvasPipeline,
        view: &InputView,
        source: &CanvasSource,
        projection: Projection,
    ) -> Landed {
        let key = pipeline.key(view, &source.font, source.profile);
        let job = pipeline
            .request_raster(&key, projection, 0..1)
            .expect("raster job");
        let result = run(&job, source, &job.cancellation);
        pipeline.install_raster(&job.ticket, projection, result)
    }
    /// Raster jobs as the app runs them until none is left for `visible`.
    /// Returns the pages painted in order and the pixels released meanwhile.
    fn drain(
        pipeline: &mut CanvasPipeline,
        view: &InputView,
        source: &CanvasSource,
        projection: Projection,
        visible: Range<usize>,
    ) -> (Vec<usize>, Vec<Arc<RenderImage>>) {
        let key = pipeline.key(view, &source.font, source.profile);
        let (mut painted, mut released) = (Vec::new(), Vec::new());
        while let Some(job) = pipeline.request_raster(&key, projection, visible.clone()) {
            painted.push(job.ticket.key().page);
            assert!(painted.len() <= 16, "raster requests must settle");
            let result = run(&job, source, &job.cancellation);
            match pipeline.install_raster(&job.ticket, projection, result) {
                Landed::Installed(gone) => released.extend(gone),
                Landed::Failed(error) => panic!("raster failed: {error}"),
                Landed::Stale => panic!("raster of the current layout must install"),
            }
            assert!(pipeline.resident_pixels() <= pipeline.pages.budget);
        }
        (painted, released)
    }
    fn settle(
        pipeline: &mut CanvasPipeline,
        view: &InputView,
        source: &CanvasSource,
        projection: Projection,
    ) {
        assert!(matches!(
            lay_out(pipeline, view, source),
            Landed::Installed(_)
        ));
        assert!(!drain(pipeline, view, source, projection, 0..1).0.is_empty());
    }
    /// Pixels shown for `visible`, driving raster jobs as the app does.
    fn show(
        pipeline: &mut CanvasPipeline,
        view: &InputView,
        source: &CanvasSource,
        projection: Projection,
        visible: std::ops::Range<usize>,
    ) -> Vec<Arc<RenderImage>> {
        let key = pipeline.key(view, &source.font, source.profile);
        if pipeline.ready_layout(&key).is_none() {
            assert!(matches!(
                lay_out(pipeline, view, source),
                Landed::Installed(_)
            ));
        }
        drain(pipeline, view, source, projection, visible.clone());
        visible
            .map(|page| {
                let image = pipeline.ready_page(&key, page, projection);
                image.expect("visible page has pixels").clone()
            })
            .collect()
    }
    fn pixels(
        pipeline: &CanvasPipeline,
        key: &LayoutRequestKey,
        projection: Projection,
    ) -> Vec<u8> {
        let image = pipeline.ready_page(key, 0, projection).unwrap();
        image.as_bytes(0).unwrap().to_vec()
    }
    fn same(a: &[Arc<RenderImage>], b: &[Arc<RenderImage>]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| Arc::ptr_eq(a, b))
    }
    /// Pages whose pixels of the installed layout at `projection` are resident.
    fn ready(pipeline: &CanvasPipeline, projection: Projection) -> Vec<usize> {
        let pages = pipeline.layout().unwrap().layout.pages().len();
        (0..pages)
            .filter(|&page| matches!(pipeline.page_image(page, projection), PageImage::Ready(_)))
            .collect()
    }
    #[test]
    fn geometry_installs_after_layout_while_the_last_pixels_stay_painted() {
        let source = source();
        let mut input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let at_80 = zoom(0.8);
        settle(&mut pipeline, input.view(), &source, at_80);
        let base = pipeline.key(input.view(), &source.font, source.profile);
        let painted = pipeline.ready_page(&base, 0, at_80).unwrap().clone();
        input.mark(None, "fi", None).unwrap();
        let key = pipeline.key(input.view(), &source.font, source.profile);
        assert!(pipeline.ready_layout(&key).is_none());
        let job = pipeline.request_layout(key.clone()).unwrap();
        // One job per request; no raster before its layout.
        assert!(pipeline.request_layout(key.clone()).is_none());
        assert!(pipeline.request_raster(&key, at_80, 0..1).is_none());
        let result = layout_view(
            input.view(),
            &source.font,
            source.profile,
            &job.cancellation,
        );
        assert!(matches!(
            pipeline.install_layout(&job.ticket, &key, result),
            Landed::Installed(_)
        ));
        // Positions map through the new layout before any of its pixels exist.
        let ready = pipeline.ready_layout(&key).unwrap();
        ready
            .layout
            .ensure_current(input.view().snapshot(), &source.font, source.profile)
            .unwrap();
        let end = input.view().index().position(2, Bias::After).unwrap();
        let caret = ready.layout.caret(end).unwrap();
        assert_eq!(
            ready
                .layout
                .hit_test(caret.page, caret.x, caret.top)
                .unwrap()
                .offset,
            end.offset
        );
        assert!(pipeline.ready_page(&key, 0, at_80).is_none());
        // The last pixels of the page stay painted until those of this layout land.
        assert!(matches!(
            pipeline.page_image(0, at_80),
            PageImage::Stale(shown) if Arc::ptr_eq(&shown, &painted)
        ));
        let Landed::Installed(replaced) = paint(&mut pipeline, input.view(), &source, at_80) else {
            panic!("raster of the current layout must install");
        };
        assert!(same(&replaced, std::slice::from_ref(&painted)));
        let landed = pipeline.ready_page(&key, 0, at_80).unwrap().clone();
        assert!(matches!(
            pipeline.page_image(0, at_80),
            PageImage::Ready(shown) if Arc::ptr_eq(&shown, &landed)
        ));
        // Zoom changes pixels only: geometry stays ready, no layout job, and
        // the page keeps its pixels until those at the new zoom land.
        let at_100 = zoom(1.0);
        assert!(pipeline.ready_page(&key, 0, at_100).is_none());
        assert!(matches!(
            pipeline.page_image(0, at_100),
            PageImage::Stale(shown) if Arc::ptr_eq(&shown, &landed)
        ));
        assert!(pipeline.ready_layout(&key).is_some());
        assert!(pipeline.request_layout(key.clone()).is_none());
        assert!(matches!(
            paint(&mut pipeline, input.view(), &source, at_100),
            Landed::Installed(_)
        ));
    }
    #[test]
    fn stale_layouts_lose_to_newer_requests_including_a_to_b_to_a() {
        let source = source();
        let mut input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        settle(&mut pipeline, input.view(), &source, zoom(0.8));
        input.mark(None, "AV", None).unwrap();
        let a_view = input.view().clone();
        let a_key = pipeline.key(&a_view, &source.font, source.profile);
        let a = pipeline.request_layout(a_key.clone()).unwrap();
        // Finished before it saw the cancellation.
        let a_result = || layout_view(&a_view, &source.font, source.profile, &Default::default());
        input.mark(None, "fi", None).unwrap();
        let b_view = input.view().clone();
        let b_key = pipeline.key(&b_view, &source.font, source.profile);
        let b = pipeline.request_layout(b_key.clone()).unwrap();
        assert!(a.cancellation.is_cancelled());
        assert!(!b.cancellation.is_cancelled());
        let b_result = || layout_view(&b_view, &source.font, source.profile, &b.cancellation);
        // Previews keep the document version; only their content differs.
        let (la, lb) = (a_result().unwrap(), b_result().unwrap());
        assert_eq!(la.key().version, lb.key().version);
        assert_ne!(la.key().content_hash(), lb.key().content_hash());
        assert_ne!(a_key, b_key);
        for current in [&a_key, &b_key] {
            assert!(matches!(
                pipeline.install_layout(&a.ticket, current, a_result()),
                Landed::Stale
            ));
        }
        // A → B → A: the same content again, but the old A ticket stays out.
        input.cancel_composition().unwrap();
        input.mark(None, "AV", None).unwrap();
        let again_key = pipeline.key(input.view(), &source.font, source.profile);
        let again = pipeline.request_layout(again_key.clone()).unwrap();
        assert!(b.cancellation.is_cancelled());
        assert!(matches!(
            pipeline.install_layout(&a.ticket, &again_key, a_result()),
            Landed::Stale
        ));
        assert!(matches!(
            pipeline.install_layout(&b.ticket, &again_key, Ok(lb)),
            Landed::Stale
        ));
        assert!(pipeline.ready_layout(&again_key).is_none());
        // A cancelled job cannot produce a layout at all.
        assert!(layout_view(input.view(), &source.font, source.profile, &a.cancellation).is_err());
        let result = layout_view(
            input.view(),
            &source.font,
            source.profile,
            &again.cancellation,
        );
        assert!(matches!(
            pipeline.install_layout(&again.ticket, &again_key, result),
            Landed::Installed(_)
        ));
        assert_eq!(
            pipeline.ready_layout(&again_key).unwrap().layout.key(),
            la.key()
        );
    }
    #[test]
    fn stale_rasters_cannot_install_over_another_layout_or_projection() {
        let source = source();
        let mut input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let (at_80, at_100, at_120) = (zoom(0.8), zoom(1.0), zoom(1.2));
        settle(&mut pipeline, input.view(), &source, at_80);
        let base = pipeline.key(input.view(), &source.font, source.profile);
        // A raster of the installed layout may land while a newer layout is
        // pending; it only shows pixels and positions still wait.
        let running = pipeline.request_raster(&base, at_100, 0..1).unwrap();
        input.mark(None, "AV", None).unwrap();
        let pending = pipeline.key(input.view(), &source.font, source.profile);
        let layout_job = pipeline.request_layout(pending.clone()).unwrap();
        assert!(!running.cancellation.is_cancelled());
        let result = run(&running, &source, &running.cancellation);
        assert!(matches!(
            pipeline.install_raster(&running.ticket, at_100, result),
            Landed::Installed(_)
        ));
        assert!(pipeline.ready_layout(&pending).is_none());
        // Once the new layout installs, an older layout's raster is stale even
        // without a newer raster request.
        let late = {
            input.cancel_composition().unwrap();
            assert!(pipeline.reuse(input.view(), &source.font, source.profile));
            let current = pipeline.key(input.view(), &source.font, source.profile);
            pipeline.request_raster(&current, at_120, 0..1).unwrap()
        };
        let late_pixels = run(&late, &source, &Default::default());
        input.mark(None, "AV", None).unwrap();
        let mark_key = pipeline.key(input.view(), &source.font, source.profile);
        assert!(matches!(
            pipeline.install_layout(
                &layout_job.ticket,
                &mark_key,
                layout_view(
                    input.view(),
                    &source.font,
                    source.profile,
                    &Default::default()
                )
            ),
            Landed::Stale
        ));
        assert!(matches!(
            lay_out(&mut pipeline, input.view(), &source),
            Landed::Installed(_)
        ));
        assert!(!late.cancellation.is_cancelled());
        assert!(matches!(
            pipeline.install_raster(&late.ticket, at_120, late_pixels),
            Landed::Stale
        ));
        // Pixels for another projection, or from a superseded ticket, are stale.
        let first = pipeline.request_raster(&mark_key, at_100, 0..1).unwrap();
        assert!(late.cancellation.is_cancelled());
        let first_pixels = run(&first, &source, &Default::default());
        let second = pipeline.request_raster(&mark_key, at_80, 0..1).unwrap();
        assert!(first.cancellation.is_cancelled());
        assert!(run(&first, &source, &first.cancellation).is_err());
        for projection in [at_80, at_100] {
            assert!(matches!(
                pipeline.install_raster(&first.ticket, projection, first_pixels.clone()),
                Landed::Stale
            ));
        }
        let second_pixels = run(&second, &source, &second.cancellation);
        assert!(matches!(
            pipeline.install_raster(&second.ticket, at_100, second_pixels.clone()),
            Landed::Stale
        ));
        assert!(matches!(
            pipeline.install_raster(&second.ticket, at_80, second_pixels),
            Landed::Installed(_)
        ));
        assert!(pipeline.ready_page(&mark_key, 0, at_80).is_some());
    }
    #[test]
    fn commit_equal_to_its_preview_reuses_layout_and_pixels_without_jobs() {
        let source = source();
        let mut input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let at_80 = zoom(0.8);
        settle(&mut pipeline, input.view(), &source, at_80);
        let base = pipeline.key(input.view(), &source.font, source.profile);
        let original = pixels(&pipeline, &base, at_80);
        let original_key = pipeline.layout().unwrap().layout.key().clone();
        // A selection change keeps layout and pixels.
        input.select(1..4, false).unwrap();
        assert!(pipeline.reuse(input.view(), &source.font, source.profile));
        let selected = pipeline.key(input.view(), &source.font, source.profile);
        assert!(pipeline.ready_page(&selected, 0, at_80).is_some());
        input.mark(None, "fi\nAV", None).unwrap();
        assert!(!input.backend().can_undo());
        assert_eq!(input.backend().version(), source.session.version());
        assert!(!pipeline.reuse(input.view(), &source.font, source.profile));
        settle(&mut pipeline, input.view(), &source, at_80);
        let preview = pipeline.layout().unwrap().layout.clone();
        let marked = pipeline.key(input.view(), &source.font, source.profile);
        let preview_pixels = pipeline.ready_page(&marked, 0, at_80).unwrap().clone();
        let (range, _) = input.view().selected_range();
        let at = input
            .view()
            .index()
            .position(range.start, Bias::After)
            .unwrap();
        let caret = preview.caret(at).unwrap();
        assert_eq!(
            preview
                .hit_test(caret.page, caret.x, caret.top)
                .unwrap()
                .offset,
            at.offset
        );
        input.unmark().unwrap();
        // The commit publishes the previewed snapshot: no layout or raster job.
        assert!(pipeline.reuse(input.view(), &source.font, source.profile));
        let committed = pipeline.key(input.view(), &source.font, source.profile);
        assert!(pipeline.request_layout(committed.clone()).is_none());
        assert!(pipeline.request_raster(&committed, at_80, 0..1).is_none());
        let reused = pipeline.ready_page(&committed, 0, at_80).unwrap();
        assert!(Arc::ptr_eq(reused, &preview_pixels));
        // Painting the commit from scratch gives the same layout and pixels.
        let mut fresh = CanvasPipeline::default();
        settle(&mut fresh, input.view(), &source, at_80);
        let fresh_key = fresh.key(input.view(), &source.font, source.profile);
        assert_eq!(fresh.layout().unwrap().layout.key(), preview.key());
        assert_eq!(
            pixels(&fresh, &fresh_key, at_80),
            pixels(&pipeline, &committed, at_80)
        );
        input.undo().unwrap();
        assert!(!pipeline.reuse(input.view(), &source.font, source.profile));
        settle(&mut pipeline, input.view(), &source, at_80);
        let undone = pipeline.key(input.view(), &source.font, source.profile);
        assert_eq!(pixels(&pipeline, &undone, at_80), original);
        assert!(!input.backend().can_undo());
        assert_ne!(pipeline.layout().unwrap().layout.key(), &original_key);
    }
    #[test]
    fn another_document_cancels_jobs_and_never_matches_old_revisions() {
        let source = source();
        let x = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let at_80 = zoom(0.8);
        settle(&mut pipeline, x.view(), &source, at_80);
        let x_key = pipeline.key(x.view(), &source.font, source.profile);
        let painted = pipeline.ready_page(&x_key, 0, at_80).unwrap().clone();
        let x_raster = pipeline.request_raster(&x_key, zoom(1.0), 0..1).unwrap();
        let x_pixels = run(&x_raster, &source, &Default::default());
        assert!(same(&pipeline.clear(), &[painted]));
        assert!(x_raster.cancellation.is_cancelled());
        assert!(pipeline.layout().is_none());
        assert_eq!(pipeline.resident_pixels(), 0);
        assert!(matches!(pipeline.page_image(0, at_80), PageImage::Pending));
        assert!(matches!(
            pipeline.install_raster(&x_raster.ticket, zoom(1.0), x_pixels),
            Landed::Stale
        ));
        // A layout still running when the next document opens at the same input
        // revision, font and profile: neither matches nor blocks the new request.
        let x_pending = pipeline.key(x.view(), &source.font, source.profile);
        let x_layout = pipeline.request_layout(x_pending.clone()).unwrap();
        pipeline.clear();
        assert!(x_layout.cancellation.is_cancelled());
        let y_source = source_with_text(font(), "fi").unwrap();
        let y = input(&y_source);
        let y_key = pipeline.key(y.view(), &y_source.font, y_source.profile);
        assert_eq!(
            (y_key.revision, y_key.font, y_key.profile),
            (x_pending.revision, x_pending.font, x_pending.profile)
        );
        assert_ne!(y_key, x_pending);
        let x_result = layout_view(x.view(), &source.font, source.profile, &Default::default());
        assert!(matches!(
            pipeline.install_layout(&x_layout.ticket, &y_key, x_result),
            Landed::Stale
        ));
        settle(&mut pipeline, y.view(), &y_source, at_80);
        assert!(pipeline.ready_page(&y_key, 0, at_80).is_some());
    }
    #[test]
    fn a_reopened_identical_document_is_painted_again() {
        let source = source();
        let mut pipeline = CanvasPipeline::default();
        let at_80 = zoom(0.8);
        settle(&mut pipeline, input(&source).view(), &source, at_80);
        // Same content, version and projection as the pixels that last
        // landed: the new document still gets its own requests.
        pipeline.clear();
        let again = input(&source);
        settle(&mut pipeline, again.view(), &source, at_80);
        let key = pipeline.key(again.view(), &source.font, source.profile);
        assert!(pipeline.ready_page(&key, 0, at_80).is_some());
    }
    #[test]
    fn zooming_back_while_a_page_is_in_flight_still_paints_it_later() {
        let source = source();
        let input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let (at_80, at_100) = (zoom(0.8), zoom(1.0));
        settle(&mut pipeline, input.view(), &source, at_80);
        let key = pipeline.key(input.view(), &source.font, source.profile);
        // Zoomed in, then back before its page lands: 80% is resident, so
        // nothing new starts, and the 100% result lands stale.
        let job = pipeline.request_raster(&key, at_100, 0..1).unwrap();
        assert!(pipeline.request_raster(&key, at_80, 0..1).is_none());
        let late = run(&job, &source, &job.cancellation);
        assert!(matches!(
            pipeline.install_raster(&job.ticket, at_80, late.clone()),
            Landed::Stale
        ));
        // Zoomed in again: the dropped page is requested afresh, and the old
        // ticket stays out even for the projection it was made for.
        let again = pipeline
            .request_raster(&key, at_100, 0..1)
            .expect("a page whose result was dropped is requested again");
        assert!(matches!(
            pipeline.install_raster(&job.ticket, at_100, late),
            Landed::Stale
        ));
        let result = run(&again, &source, &again.cancellation);
        assert!(matches!(
            pipeline.install_raster(&again.ticket, at_100, result),
            Landed::Installed(_)
        ));
        assert!(pipeline.ready_page(&key, 0, at_100).is_some());
    }
    #[test]
    fn long_document_on_a_2x_display_paints_the_visible_pages() {
        let source = source_with_text(font(), &vec!["AV fi"; 1000].join("\n")).unwrap();
        let pages = source.layout.pages().len();
        assert!(pages >= 20, "{pages} pages");
        let input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        // 100% on a 2x display: about 3.57 M px per A4 page, so a raster of
        // the whole document exceeded the shared budget at its fifth page.
        let retina = Projection::new(192.0, 1.0).unwrap();
        let a4 = &source.layout.pages()[0];
        let page = retina.pixels(a4.width).ceil() * retina.pixels(a4.height).ceil();
        assert!(5.0 * page > MAX_CACHE_PIXELS as f64);
        for visible in [0..2, 18..20, pages - 1..pages] {
            let images = show(
                &mut pipeline,
                input.view(),
                &source,
                retina,
                visible.clone(),
            );
            assert_eq!(images.len(), visible.len());
            for image in images {
                let size = image.size(0);
                assert_eq!(
                    (size.width.0 as f64, size.height.0 as f64),
                    (
                        retina.pixels(a4.width).ceil(),
                        retina.pixels(a4.height).ceil()
                    )
                );
            }
        }
    }
    #[test]
    fn resident_pixels_stay_within_the_cache_at_any_page_count() {
        let source = pages_source(400);
        let pages = source.layout.pages().len();
        assert!(pages >= 8, "{pages} pages");
        let input = input(&source);
        // 150% on a 2x display: two pages fit the cache together, three do not.
        let retina = Projection::new(192.0, 1.5).unwrap();
        let page = page_pixels(&source, retina);
        assert!(2 * page <= MAX_CACHE_PIXELS && 3 * page > MAX_CACHE_PIXELS);
        let mut pipeline = CanvasPipeline::default();
        for visible in [0..1, 1..2, pages - 2..pages, 3..4, 0..2] {
            show(
                &mut pipeline,
                input.view(),
                &source,
                retina,
                visible.clone(),
            );
            assert!(pipeline.resident_pixels() <= MAX_CACHE_PIXELS);
            // The visible pages and the next one that fits stay resident.
            let wanted = visible.start..(visible.start + 2).min(pages);
            assert_eq!(ready(&pipeline, retina), wanted.collect::<Vec<_>>());
        }
    }
    #[test]
    fn eviction_drops_the_farthest_pages_and_other_projections_first() {
        let source = pages_source(400);
        let pages = source.layout.pages().len();
        assert!(pages >= 8, "{pages} pages");
        let input = input(&source);
        let (at_100, at_110) = (zoom(1.0), zoom(1.1));
        let (p100, p110) = (page_pixels(&source, at_100), page_pixels(&source, at_110));
        let mut pipeline = CanvasPipeline::with_budget(5 * p100);
        show(&mut pipeline, input.view(), &source, at_100, 0..1);
        assert_eq!(ready(&pipeline, at_100), [0, 1, 2]);
        let images = |pipeline: &CanvasPipeline, pages: &[usize]| {
            let key = pipeline.key(input.view(), &source.font, source.profile);
            pages
                .iter()
                .map(|&page| pipeline.ready_page(&key, page, at_100).unwrap().clone())
                .collect::<Vec<_>>()
        };
        let first = images(&pipeline, &[0, 1]);
        // Scrolled to 4..5: 3, 6 and 2 are wanted around it; 0 and 1 are the
        // farthest and leave, farthest first.
        let (painted, released) = drain(&mut pipeline, input.view(), &source, at_100, 4..5);
        assert_eq!(painted, [4, 5, 3, 6]);
        assert!(same(&released, &first));
        assert_eq!(ready(&pipeline, at_100), [2, 3, 4, 5, 6]);
        // Back to the top: 6, then 5 are now the farthest.
        let far = images(&pipeline, &[6, 5]);
        let (painted, released) = drain(&mut pipeline, input.view(), &source, at_100, 0..1);
        assert_eq!(painted, [0, 1]);
        assert!(same(&released, &far));
        assert_eq!(ready(&pipeline, at_100), [0, 1, 2, 3, 4]);
        // Zooming in around 3..4: four pages fit at 110%. Each page's new
        // pixels supersede its old ones. Page 5 then needs room: page 1 at
        // 100% goes, not page 2 at 110% although it is farther from view.
        assert!(4 * p110 <= 5 * p100 && 5 * p110 > 5 * p100);
        assert!(p100 + 4 * p110 > 5 * p100);
        let (painted, _) = drain(&mut pipeline, input.view(), &source, at_110, 3..4);
        assert_eq!(painted, [3, 4, 2, 5]);
        assert_eq!(ready(&pipeline, at_110), [2, 3, 4, 5]);
        for page in [0, 1] {
            assert!(matches!(
                pipeline.page_image(page, at_110),
                PageImage::Pending
            ));
        }
    }
    #[test]
    fn scrolling_and_zoom_request_only_missing_pages_near_the_viewport() {
        let source = pages_source(600);
        let pages = source.layout.pages().len();
        assert!(pages >= 12, "{pages} pages");
        let input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let at_80 = zoom(0.8);
        assert!(matches!(
            lay_out(&mut pipeline, input.view(), &source),
            Landed::Installed(_)
        ));
        let key = pipeline.key(input.view(), &source.font, source.profile);
        let mut paint = |visible: Range<usize>, projection| {
            drain(&mut pipeline, input.view(), &source, projection, visible).0
        };
        // Visible pages first, then the next and previous ones nearest first.
        assert_eq!(paint(0..2, at_80), [0, 1, 2, 3]);
        assert_eq!(paint(1..3, at_80), [4]);
        assert!(paint(1..3, at_80).is_empty());
        let end = pages - 2..pages;
        let (last, before) = (pages - 1, pages - 3);
        assert_eq!(
            paint(end.clone(), at_80),
            [before + 1, last, before, before - 1]
        );
        // Zoom needs no layout; only pages near the viewport are painted again.
        let at_100 = zoom(1.0);
        assert_eq!(
            paint(end.clone(), at_100),
            [before + 1, last, before, before - 1]
        );
        assert!(pipeline.request_layout(key.clone()).is_none());
        assert!(matches!(
            pipeline.page_image(0, at_100),
            PageImage::Stale(_)
        ));
        // A running job keeps going while its page is still wanted; once the
        // view leaves it, the next request cancels it and its pixels are stale.
        let job = pipeline.request_raster(&key, at_80, 5..6).unwrap();
        assert_eq!(job.ticket.key().page, 5);
        assert!(pipeline.request_raster(&key, at_80, 4..6).is_none());
        let next = pipeline.request_raster(&key, at_100, 0..1).unwrap();
        assert_eq!(next.ticket.key().page, 0);
        assert!(job.cancellation.is_cancelled());
        let late = run(&job, &source, &Default::default());
        assert!(matches!(
            pipeline.install_raster(&job.ticket, at_80, late),
            Landed::Stale
        ));
    }
    #[test]
    fn a_page_too_large_for_the_cache_shows_its_own_notice() {
        let source = pages_source(100);
        let input = input(&source);
        let at_100 = zoom(1.0);
        let mut pipeline = CanvasPipeline::with_budget(page_pixels(&source, at_100) - 1);
        assert!(matches!(
            lay_out(&mut pipeline, input.view(), &source),
            Landed::Installed(_)
        ));
        let key = pipeline.key(input.view(), &source.font, source.profile);
        // No job is started; every page says so, and geometry is unaffected.
        assert!(pipeline.request_raster(&key, at_100, 0..2).is_none());
        for page in 0..source.layout.pages().len() {
            assert!(matches!(
                pipeline.page_image(page, at_100),
                PageImage::TooLarge
            ));
        }
        assert!(pipeline.ready_layout(&key).is_some());
        // Zoomed out, the pages fit again.
        let at_50 = zoom(0.5);
        assert!(
            !drain(&mut pipeline, input.view(), &source, at_50, 0..1)
                .0
                .is_empty()
        );
        assert!(matches!(pipeline.page_image(0, at_50), PageImage::Ready(_)));
        // Back at 100%, the notice wins over stretching smaller pixels.
        assert!(matches!(
            pipeline.page_image(0, at_100),
            PageImage::TooLarge
        ));
        // The renderer's own dimension ceiling is reported the same way.
        let mut pipeline = CanvasPipeline::default();
        assert!(matches!(
            lay_out(&mut pipeline, input.view(), &source),
            Landed::Installed(_)
        ));
        let key = pipeline.key(input.view(), &source.font, source.profile);
        let huge = Projection::new(1200.0, 1.0).unwrap();
        assert!(pipeline.request_raster(&key, huge, 0..1).is_none());
        assert!(matches!(pipeline.page_image(0, huge), PageImage::TooLarge));
    }
    #[test]
    fn a_failed_page_keeps_its_error_and_does_not_stop_the_others() {
        let source = pages_source(100);
        let input = input(&source);
        let at_80 = zoom(0.8);
        let mut pipeline = CanvasPipeline::default();
        assert!(matches!(
            lay_out(&mut pipeline, input.view(), &source),
            Landed::Installed(_)
        ));
        let key = pipeline.key(input.view(), &source.font, source.profile);
        let job = pipeline.request_raster(&key, at_80, 0..1).unwrap();
        assert_eq!(job.ticket.key().page, 0);
        assert!(matches!(
            pipeline.install_raster(&job.ticket, at_80, Err("injected".into())),
            Landed::Failed(_)
        ));
        assert!(matches!(
            pipeline.page_image(0, at_80),
            PageImage::Failed(error) if error == "injected"
        ));
        // Not requested again, so no loop; its neighbours still are.
        let (painted, _) = drain(&mut pipeline, input.view(), &source, at_80, 0..1);
        assert_eq!(painted, [1, 2]);
        assert!(pipeline.ready_layout(&key).is_some());
        // Another projection is another request.
        let at_100 = zoom(1.0);
        let (painted, _) = drain(&mut pipeline, input.view(), &source, at_100, 0..1);
        assert_eq!(painted, [0, 1, 2]);
    }
    #[test]
    fn missing_font_glyph_keeps_typed_unicode_and_emits_diagnostics() {
        let source = source();
        let mut input = input(&source);
        input.replace(None, "🙂").unwrap();
        let rendered = layout_view(
            input.view(),
            &source.font,
            source.profile,
            &Default::default(),
        )
        .unwrap();
        assert!(input.view().index().text().starts_with("🙂"));
        assert!(!rendered.diagnostics().is_empty());
        assert!(
            rendered.pages()[0]
                .runs
                .iter()
                .any(|run| run.text.contains("🙂"))
        );
        input.undo().unwrap();
        assert_eq!(input.view().index().text(), "AV\nfi");
    }
    #[test]
    fn the_caret_overlay_rides_the_raster_of_its_own_page() -> Result<(), String> {
        let source = source_with_text(font(), &vec!["AV fi"; 80].join("\n"))?;
        assert!(source.layout.pages().len() > 1);
        let projection = zoom(2.0);
        let line = &source.layout.lines()[0];
        let caret = Some((line.carets[0].x, line.top, line.height));
        // The caret page's raster carries an overlay apart from its pixels.
        let output = raster_page(
            &source.layout,
            0,
            &source.font,
            projection,
            RasterLimits::default(),
            caret,
            &Default::default(),
        )?;
        let before = output
            .image
            .as_bytes(0)
            .ok_or_else(|| "page bytes".to_string())?
            .to_vec();
        let overlay = output
            .caret_overlay
            .as_ref()
            .map(|overlay| overlay.bgra.clone())
            .unwrap_or_default();
        assert!(!overlay.is_empty());
        assert_ne!(overlay, before);
        assert_eq!(output.image.as_bytes(0), Some(before.as_slice()));
        // Another page, or a job without the caret, has no overlay.
        let other = raster_page(
            &source.layout,
            1,
            &source.font,
            projection,
            RasterLimits::default(),
            None,
            &Default::default(),
        )?;
        assert!(other.caret_overlay.is_none());
        Ok(())
    }
    #[test]
    fn one_raster_stays_within_the_ui_cache_budget() {
        let source = source_with_text(font(), &vec!["AV fi"; 100].join("\n")).unwrap();
        let pages = source.layout.pages();
        assert!(pages.len() >= 3);
        // 150% on a 2x display: each page fits alone, three do not share one budget.
        let retina = Projection::new(192.0, 1.5).unwrap();
        let page = retina.pixels(pages[0].width).ceil() * retina.pixels(pages[0].height).ceil();
        assert!(page < MAX_CACHE_PIXELS as f64 && 3.0 * page > MAX_CACHE_PIXELS as f64);
        let mut cache = joyword_render::NeighborhoodCache::new(1);
        let painted = cache
            .raster_neighborhood(joyword_render::NeighborhoodRequest {
                identity: 1,
                scenes: pages,
                fonts: std::slice::from_ref(&source.font),
                projection: retina,
                focus: 0,
                radius: 0,
                cancellation: &Default::default(),
            })
            .unwrap();
        assert_eq!(painted, vec![0]);
        assert!(cache.cached(1, retina, 1).is_none());
        let image = cache.cached(1, retina, 0).unwrap();
        let pixels = u64::from(image.width()) * u64::from(image.height());
        assert!(pixels <= MAX_CACHE_PIXELS);
    }
}

#[cfg(test)]
mod startup_sample_tests {
    use super::*;
    use crate::input_ui::DocEditor;
    use joyword_shell::input::{InputLimits, TextInput};

    fn paragraphs(session: &joyword_shell::docx::DocxSession) -> Vec<String> {
        session
            .session()
            .document()
            .body()
            .blocks
            .iter()
            .filter_map(|block| block.as_paragraph())
            .map(|paragraph| paragraph.plain_text(|_| String::new()))
            .collect()
    }

    /// The window's startup document is DOCX-SAMPLE-001: seed text, clean
    /// state, and a real package backing so Save As works before any open.
    /// Layout is not exercised here: the bundled test font does not cover the
    /// full seed, which the runtime font check rejects by design.
    #[test]
    fn startup_sample_matches_seed_and_saves_a_copy() {
        let session = joyword_shell::docx::DocxSession::open_default(SAMPLE_DOCX).unwrap();
        assert_eq!(
            paragraphs(&session),
            SEED.lines().map(str::to_owned).collect::<Vec<_>>()
        );
        let mut input = TextInput::new(session, InputLimits::default()).unwrap();
        assert!(DocEditor::can_save_copy(&input));
        assert!(!DocEditor::has_unsaved_changes(&input));
        assert!(DocEditor::replace(&mut input, None, "校验").unwrap());

        let dir = std::env::temp_dir().join(format!("joyword-sample-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let destination = dir.join("sample-copy.docx");
        let receipt = DocEditor::save_copy(&input, &destination).unwrap();
        let reopened =
            joyword_shell::docx::DocxSession::open_default(&std::fs::read(receipt.path()).unwrap())
                .unwrap();
        let text = paragraphs(&reopened).join("\n");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(text.contains("校验"), "{text}");
        for line in SEED.lines() {
            assert!(text.contains(line), "{text}");
        }
    }

    /// A bare in-memory session still cannot save: no package backing, no copy.
    #[test]
    fn in_memory_session_cannot_save_a_copy() {
        let font = LoadedFont::from_bytes(
            Arc::from(
                include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
            ),
            0,
        )
        .unwrap();
        let source = source_with_text(font, "AV").unwrap();
        let input = TextInput::new(source.session, InputLimits::default()).unwrap();
        assert!(!DocEditor::can_save_copy(&input));
    }
}
