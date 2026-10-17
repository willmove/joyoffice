//! Editable in-memory engineering document: real engine snapshot, immutable layout and
//! CPU scene output. Loading, layout and raster work run on background jobs; a
//! layout is usable for geometry before its pages are rasterized.
use gpui::RenderImage;
use joyword_engine::{EditPolicy, Session};
use joyword_foundation::Emu;
use joyword_layout::{
    FaceSet, FeatureSet, LayoutCancellation, LayoutLimits, LayoutSnapshot, LoadedFont,
    SingleFontProfile, layout_document,
};
use joyword_model::{BlockSeq, Document, PageSetup, Paragraph};
use joyword_render::requests::{RasterRequestGate, RasterTicket};
use joyword_render::{Projection, RasterCancellation, RasterPage};
use joyword_shell::input::InputView;
use std::path::Path;
use std::sync::Arc;

const SEED: &str = include_str!("../../../tests/fixtures/text/cjk-contract.txt");
/// Total UI cache ceiling across pages, in addition to each renderer budget.
#[cfg(test)]
const MAX_CACHE_PIXELS: u64 = 16_777_216;

pub struct CanvasSource {
    pub session: Session,
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

pub fn load_source() -> Result<CanvasSource, String> {
    source_with_text(load_font()?, SEED)
}
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
    let profile = SingleFontProfile {
        size: Emu::new(12 * Emu::PER_POINT),
        leading: Emu::new(18 * Emu::PER_POINT),
        baseline: Emu::new(14 * Emu::PER_POINT),
        features: FeatureSet::default(),
    };
    let layout = layout_document(
        &session.document().snapshot(),
        FaceSet::uniform(&font),
        profile,
        LayoutLimits::default(),
        &LayoutCancellation::default(),
    )
    .map_err(|e| e.to_string())?;
    if !layout.diagnostics().is_empty() || layout.omitted_diagnostics() != 0 {
        return Err("selected font cannot display the complete sample".into());
    }
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
    let scenes = source.layout.pages();
    let mut cache = joyword_render::NeighborhoodCache::new(scenes.len().max(1));
    let painted = cache
        .raster_neighborhood(joyword_render::NeighborhoodRequest {
            identity: 0,
            scenes,
            fonts: std::slice::from_ref(&source.font),
            projection,
            focus: 0,
            radius: scenes.len().saturating_sub(1),
            cancellation,
        })
        .map_err(|error| error.to_string())?;
    let mut pages = Vec::with_capacity(painted.len());
    for page in painted {
        let Some(image) = cache.cached(0, projection, page).cloned() else {
            return Err("neighborhood page missing".into());
        };
        pages.push(image);
    }
    Ok(pages)
}

pub fn image_from_bgra(width: u32, height: u32, bgra: &[u8]) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(width, height, bgra.to_vec())?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

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
/// Identity of a raster request: the installed layout by its generation,
/// which revisions reusing that layout keep, and the projection.
#[derive(Clone, Debug, PartialEq)]
pub struct RasterKey {
    pub layout: u64,
    pub projection: Projection,
}
/// A layout installed as soon as it lands. Geometry needs nothing more.
pub struct LayoutFrame {
    pub key: LayoutRequestKey,
    pub generation: u64,
    pub layout: Arc<LayoutSnapshot>,
}
/// Pixels of one installed layout. They stay painted under newer geometry
/// until the raster of a later layout or projection replaces them.
pub struct RasterFrame {
    pub key: RasterKey,
    /// Page index of `images[0]`. Later entries are the following painted pages.
    pub origin: usize,
    pub images: Vec<Arc<RenderImage>>,
    pub caret_overlay: Option<CaretOverlay>,
}

#[derive(Clone)]
pub struct RasterOutput {
    pub origin: usize,
    pub images: Vec<Arc<RenderImage>>,
    /// Caret pixels copied from the focused page. `None` when no caret was supplied.
    pub caret_overlay: Option<CaretOverlay>,
}

#[derive(Clone)]
pub struct CaretOverlay {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}
pub struct LayoutJob {
    pub ticket: RasterTicket<LayoutRequestKey>,
    pub cancellation: LayoutCancellation,
}
pub struct RasterJob {
    pub ticket: RasterTicket<RasterKey>,
    pub layout: Arc<LayoutSnapshot>,
    pub cancellation: RasterCancellation,
}
/// What became of a background result.
pub enum Landed {
    /// Superseded, or made for another revision, layout or projection.
    Stale,
    /// Installed. Pixels it replaced are returned for the caller to release.
    Installed(Vec<Arc<RenderImage>>),
    /// The current request failed and nothing was installed.
    Failed(String),
}

/// Canvas state in two stages, each behind its own request gate. A layout is
/// installed as soon as it lands, so hit testing, IME geometry, reveal and
/// overlays are exact while its pages are still being rasterized; the last
/// pixels stay painted until those of the installed layout land.
#[derive(Default)]
pub struct CanvasPipeline {
    document: u64,
    generation: u64,
    layout: Option<LayoutFrame>,
    raster: Option<RasterFrame>,
    layout_gate: RasterRequestGate<LayoutRequestKey>,
    raster_gate: RasterRequestGate<RasterKey>,
    layout_cancel: LayoutCancellation,
    raster_cancel: RasterCancellation,
}
impl CanvasPipeline {
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
    /// The painted pixels, whichever layout or projection they came from.
    pub fn raster(&self) -> Option<&RasterFrame> {
        self.raster.as_ref()
    }
    /// The installed layout only when it was made for `current`.
    pub fn ready_layout(&self, current: &LayoutRequestKey) -> Option<&LayoutFrame> {
        self.layout.as_ref().filter(|frame| &frame.key == current)
    }
    /// Pixels of the current layout at `projection`.
    pub fn ready_raster(
        &self,
        current: &LayoutRequestKey,
        projection: Projection,
    ) -> Option<&RasterFrame> {
        let key = RasterKey {
            layout: self.ready_layout(current)?.generation,
            projection,
        };
        self.raster.as_ref().filter(|frame| frame.key == key)
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
                self.generation = self
                    .generation
                    .checked_add(1)
                    .expect("layout generation exhausted");
                self.layout = Some(LayoutFrame {
                    key: current.clone(),
                    generation: self.generation,
                    layout: Arc::new(layout),
                });
                Landed::Installed(Vec::new())
            }
            Err(error) => Landed::Failed(error),
        }
    }
    /// A raster job for the current layout at `projection`, unless its pixels
    /// are installed or already being made. It cancels the older raster job.
    pub fn request_raster(
        &mut self,
        current: &LayoutRequestKey,
        projection: Projection,
    ) -> Option<RasterJob> {
        if self.ready_raster(current, projection).is_some() {
            return None;
        }
        let frame = self.ready_layout(current)?;
        let layout = frame.layout.clone();
        let ticket = self.raster_gate.request(RasterKey {
            layout: frame.generation,
            projection,
        })?;
        self.raster_cancel.cancel();
        self.raster_cancel = RasterCancellation::default();
        Some(RasterJob {
            ticket,
            layout,
            cancellation: self.raster_cancel.clone(),
        })
    }
    /// Only the latest request, for the layout installed now and the current
    /// projection, installs. Pixels never map positions, so they may land
    /// while a newer layout is still pending.
    pub fn install_raster(
        &mut self,
        ticket: &RasterTicket<RasterKey>,
        projection: Projection,
        result: Result<RasterOutput, String>,
    ) -> Landed {
        let Some(frame) = &self.layout else {
            return Landed::Stale;
        };
        let current = RasterKey {
            layout: frame.generation,
            projection,
        };
        if !self.raster_gate.accepts(ticket, &current) {
            return Landed::Stale;
        }
        match result {
            Ok(output) => {
                let replaced = self.raster.replace(RasterFrame {
                    key: current,
                    origin: output.origin,
                    images: output.images,
                    caret_overlay: output.caret_overlay,
                });
                Landed::Installed(replaced.map(|frame| frame.images).unwrap_or_default())
            }
            Err(error) => Landed::Failed(error),
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
        self.raster
            .take()
            .map(|frame| frame.images)
            .unwrap_or_default()
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
/// Background raster stage for one installed layout.
///
/// Pages outside `focus ± radius` are not painted. The app passes the
/// viewport page and a radius of one; tests that need every page pass a
/// radius that covers the document.
pub fn raster_layout(
    layout: &LayoutSnapshot,
    font: &LoadedFont,
    projection: Projection,
    cancellation: &RasterCancellation,
    focus: usize,
    radius: usize,
    caret: Option<(
        joyword_foundation::Emu,
        joyword_foundation::Emu,
        joyword_foundation::Emu,
    )>,
) -> Result<RasterOutput, String> {
    if layout.pages().is_empty() {
        return Ok(RasterOutput {
            origin: 0,
            images: Vec::new(),
            caret_overlay: None,
        });
    }
    let mut cache = joyword_render::NeighborhoodCache::new(radius.saturating_mul(2).max(1) + 1);
    let identity = layout.key().version.get();
    let painted = cache
        .raster_neighborhood(joyword_render::NeighborhoodRequest {
            identity,
            scenes: layout.pages(),
            fonts: std::slice::from_ref(font),
            projection,
            focus,
            radius,
            cancellation,
        })
        .map_err(|error| error.to_string())?;
    let origin = painted.first().copied().unwrap_or(0);
    let mut images = Vec::with_capacity(painted.len());
    for page in painted {
        let Some(raster) = cache.cached(identity, projection, page).cloned() else {
            return Err("neighborhood page missing".into());
        };
        images.push(image(raster));
    }
    let caret_overlay = caret.and_then(|(x, top, height)| {
        let page = images.first()?;
        let bytes = page.as_bytes(0)?;
        let scene = layout.pages().get(origin)?;
        let width = projection.pixels(scene.width).ceil() as u32;
        let height_px = projection.pixels(scene.height).ceil() as u32;
        Some(CaretOverlay {
            width,
            height: height_px,
            bgra: joyword_render::paint_caret_overlay(
                bytes, width, height_px, projection, x, top, height,
            ),
        })
    });
    Ok(RasterOutput {
        origin,
        images,
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
    /// The layout stage, run here as the app runs it on the background executor.
    fn lay_out(pipeline: &mut CanvasPipeline, view: &InputView, source: &CanvasSource) -> Landed {
        let key = pipeline.key(view, &source.font, source.profile);
        let job = pipeline.request_layout(key.clone()).expect("layout job");
        let result = layout_view(view, &source.font, source.profile, &job.cancellation);
        pipeline.install_layout(&job.ticket, &key, result)
    }
    fn paint(
        pipeline: &mut CanvasPipeline,
        view: &InputView,
        source: &CanvasSource,
        projection: Projection,
    ) -> Landed {
        let key = pipeline.key(view, &source.font, source.profile);
        let job = pipeline
            .request_raster(&key, projection)
            .expect("raster job");
        let result = raster_layout(
            &job.layout,
            &source.font,
            projection,
            &job.cancellation,
            0,
            job.layout.pages().len().saturating_sub(1),
            None,
        );
        pipeline.install_raster(&job.ticket, projection, result)
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
        assert!(matches!(
            paint(pipeline, view, source, projection),
            Landed::Installed(_)
        ));
    }
    fn pixels(pipeline: &CanvasPipeline) -> Vec<u8> {
        pipeline.raster().unwrap().images[0]
            .as_bytes(0)
            .unwrap()
            .to_vec()
    }
    fn same(a: &[Arc<RenderImage>], b: &[Arc<RenderImage>]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(a, b)| Arc::ptr_eq(a, b))
    }
    #[test]
    fn geometry_installs_after_layout_while_the_last_pixels_stay_painted() {
        let source = source();
        let mut input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let at_80 = zoom(0.8);
        settle(&mut pipeline, input.view(), &source, at_80);
        let painted = pipeline.raster().unwrap().images.clone();
        input.mark(None, "fi", None).unwrap();
        let key = pipeline.key(input.view(), &source.font, source.profile);
        assert!(pipeline.ready_layout(&key).is_none());
        let job = pipeline.request_layout(key.clone()).unwrap();
        // One job per request; no raster before its layout.
        assert!(pipeline.request_layout(key.clone()).is_none());
        assert!(pipeline.request_raster(&key, at_80).is_none());
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
        assert!(pipeline.ready_raster(&key, at_80).is_none());
        // The last pixels stay painted until those of this layout land.
        let shown = pipeline.raster().unwrap();
        assert!(same(&shown.images, &painted));
        assert_ne!(shown.key.layout, ready.generation);
        let Landed::Installed(replaced) = paint(&mut pipeline, input.view(), &source, at_80) else {
            panic!("raster of the current layout must install");
        };
        assert!(same(&replaced, &painted));
        assert!(pipeline.ready_raster(&key, at_80).is_some());
        // Zoom changes pixels only: geometry stays ready, no layout job.
        let at_100 = zoom(1.0);
        assert!(pipeline.ready_raster(&key, at_100).is_none());
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
        let running = pipeline.request_raster(&base, at_100).unwrap();
        input.mark(None, "AV", None).unwrap();
        let pending = pipeline.key(input.view(), &source.font, source.profile);
        let layout_job = pipeline.request_layout(pending.clone()).unwrap();
        assert!(!running.cancellation.is_cancelled());
        let result = raster_layout(
            &running.layout,
            &source.font,
            at_100,
            &running.cancellation,
            0,
            running.layout.pages().len().saturating_sub(1),
            None,
        );
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
            pipeline.request_raster(&current, at_120).unwrap()
        };
        let late_pixels = raster_layout(
            &late.layout,
            &source.font,
            at_120,
            &Default::default(),
            0,
            late.layout.pages().len().saturating_sub(1),
            None,
        );
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
        let first = pipeline.request_raster(&mark_key, at_100).unwrap();
        assert!(late.cancellation.is_cancelled());
        let first_pixels = raster_layout(
            &first.layout,
            &source.font,
            at_100,
            &Default::default(),
            0,
            first.layout.pages().len().saturating_sub(1),
            None,
        );
        let second = pipeline.request_raster(&mark_key, at_80).unwrap();
        assert!(first.cancellation.is_cancelled());
        assert!(
            raster_layout(
                &first.layout,
                &source.font,
                at_100,
                &first.cancellation,
                0,
                first.layout.pages().len().saturating_sub(1),
                None,
            )
            .is_err()
        );
        for projection in [at_80, at_100] {
            assert!(matches!(
                pipeline.install_raster(&first.ticket, projection, first_pixels.clone()),
                Landed::Stale
            ));
        }
        let second_pixels = raster_layout(
            &second.layout,
            &source.font,
            at_80,
            &second.cancellation,
            0,
            second.layout.pages().len().saturating_sub(1),
            None,
        );
        assert!(matches!(
            pipeline.install_raster(&second.ticket, at_100, second_pixels.clone()),
            Landed::Stale
        ));
        assert!(matches!(
            pipeline.install_raster(&second.ticket, at_80, second_pixels),
            Landed::Installed(_)
        ));
        assert!(pipeline.ready_raster(&mark_key, at_80).is_some());
    }
    #[test]
    fn commit_equal_to_its_preview_reuses_layout_and_pixels_without_jobs() {
        let source = source();
        let mut input = input(&source);
        let mut pipeline = CanvasPipeline::default();
        let at_80 = zoom(0.8);
        settle(&mut pipeline, input.view(), &source, at_80);
        let original = pixels(&pipeline);
        let original_key = pipeline.layout().unwrap().layout.key().clone();
        // A selection change keeps layout and pixels.
        input.select(1..4, false).unwrap();
        assert!(pipeline.reuse(input.view(), &source.font, source.profile));
        let selected = pipeline.key(input.view(), &source.font, source.profile);
        assert!(pipeline.ready_raster(&selected, at_80).is_some());
        input.mark(None, "fi\nAV", None).unwrap();
        assert!(!input.backend().can_undo());
        assert_eq!(input.backend().version(), source.session.version());
        assert!(!pipeline.reuse(input.view(), &source.font, source.profile));
        settle(&mut pipeline, input.view(), &source, at_80);
        let preview = pipeline.layout().unwrap().layout.clone();
        let preview_pixels = pipeline.raster().unwrap().images.clone();
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
        assert!(pipeline.request_raster(&committed, at_80).is_none());
        let reused = pipeline.ready_raster(&committed, at_80).unwrap();
        assert!(same(&reused.images, &preview_pixels));
        // Painting the commit from scratch gives the same layout and pixels.
        let mut fresh = CanvasPipeline::default();
        settle(&mut fresh, input.view(), &source, at_80);
        assert_eq!(fresh.layout().unwrap().layout.key(), preview.key());
        assert_eq!(pixels(&fresh), pixels(&pipeline));
        input.undo().unwrap();
        assert!(!pipeline.reuse(input.view(), &source.font, source.profile));
        settle(&mut pipeline, input.view(), &source, at_80);
        assert_eq!(pixels(&pipeline), original);
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
        let painted = pipeline.raster().unwrap().images.clone();
        let x_key = pipeline.key(x.view(), &source.font, source.profile);
        let x_raster = pipeline.request_raster(&x_key, zoom(1.0)).unwrap();
        assert!(same(&pipeline.clear(), &painted));
        assert!(x_raster.cancellation.is_cancelled());
        assert!(pipeline.layout().is_none() && pipeline.raster().is_none());
        assert!(matches!(
            pipeline.install_raster(
                &x_raster.ticket,
                zoom(1.0),
                Ok(RasterOutput {
                    origin: 0,
                    images: Vec::new(),
                    caret_overlay: None,
                }),
            ),
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
        assert!(pipeline.ready_raster(&y_key, at_80).is_some());
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
    fn viewport_raster_paints_the_neighborhood_and_the_caret_overlay_is_separate()
    -> Result<(), String> {
        let source = source_with_text(font(), &vec!["AV fi"; 80].join("\n"))?;
        assert!(source.layout.pages().len() > 1);
        let projection = zoom(2.0);
        let line = &source.layout.lines()[0];
        let output = raster_layout(
            &source.layout,
            &source.font,
            projection,
            &Default::default(),
            0,
            0,
            Some((line.carets[0].x, line.top, line.height)),
        )
        .unwrap();
        assert_eq!(output.origin, 0);
        assert_eq!(output.images.len(), 1);
        let page = &output.images[0];
        let before = page
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
        assert_eq!(page.as_bytes(0), Some(before.as_slice()));
        Ok(())
    }
}
