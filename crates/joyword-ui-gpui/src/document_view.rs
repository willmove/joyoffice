//! Desktop document pipeline adapted from JoyOffice's generation-gated canvas.
use super::*;
use crate::page_index::PageIndex;
use gpui::{RenderImage, Task, img, point};
use joyword_render::requests::RasterRequestGate;
use joyword_shell::reader::{MAX_CACHE_PIXELS, MAX_PAGE_PIXELS, RasterScene, Reader, WELCOME};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) enum OpenSource {
    File(PathBuf),
    LegacyRecovery(PathBuf),
    Welcome,
    Blank,
}
#[derive(Clone, PartialEq)]
struct RasterKey {
    epoch: u64,
    zoom: u16,
    dpi: u32,
    page: usize,
}
struct CachedPage {
    image: Arc<RenderImage>,
    pixels: u64,
    last_used: u64,
}

pub(super) struct DocumentCanvas {
    pub(super) reader: Option<Reader>,
    images: BTreeMap<(u16, u32, usize), CachedPage>,
    index: PageIndex,
    scene: Option<RasterScene>,
    active_raster: Option<(RasterKey, Arc<AtomicBool>)>,
    wanted: Vec<usize>,
    access: u64,
    pub(super) input_paint: Option<std::time::Instant>,
    open_paint: Option<std::time::Instant>,
    scroll_paint: Option<std::time::Instant>,
    pub(super) epoch: u64,
    pub(super) load_gate: RasterRequestGate<()>,
    raster_gate: RasterRequestGate<RasterKey>,
    pub(super) cancel: Arc<AtomicBool>,
    pub(super) load_task: Option<Task<()>>,
    pub(super) raster_task: Option<Task<()>>,
    pub(super) dialog_task: Option<Task<()>>,
    pub(super) loading: bool,
    pub(super) scale_key: Option<(u16, u32)>,
    pub(super) dpi: f32,
    pub(super) viewport: f32,
    pub(super) a11y_active: bool,
    pub(super) failed: BTreeMap<usize, String>,
}
impl Default for DocumentCanvas {
    fn default() -> Self {
        Self {
            reader: None,
            images: BTreeMap::new(),
            index: PageIndex::default(),
            scene: None,
            active_raster: None,
            wanted: Vec::new(),
            access: 0,
            input_paint: None,
            open_paint: None,
            scroll_paint: None,
            epoch: 0,
            load_gate: Default::default(),
            raster_gate: Default::default(),
            cancel: Arc::new(AtomicBool::new(false)),
            load_task: None,
            raster_task: None,
            dialog_task: None,
            loading: false,
            scale_key: None,
            dpi: 1.0,
            viewport: 600.0,
            a11y_active: false,
            failed: BTreeMap::new(),
        }
    }
}
impl DocumentCanvas {
    pub(super) fn factor(&self, zoom: u16) -> f32 {
        f32::from(zoom) / 100.0 * 96.0 / 72.0
    }
    pub(super) fn top(&self, page: usize, zoom: u16) -> f32 {
        self.index.top(page, self.factor(zoom))
    }
    fn range(&self, scroll: f32, zoom: u16) -> std::ops::Range<usize> {
        self.index.visible(self.factor(zoom), scroll, self.viewport)
    }
    fn discard_scene(&mut self) {
        if let Some((_, cancel)) = &self.active_raster {
            cancel.store(true, Ordering::Relaxed);
        }
        self.scene = None;
    }
    pub(super) fn invalidate_scene(&mut self) {
        self.discard_scene();
        self.index = self
            .reader
            .as_ref()
            .map_or_else(PageIndex::default, |r| PageIndex::new(&r.summary.pages));
    }
    pub(super) fn refresh_changed_page(&mut self, cx: &mut Context<JoyWord>) -> bool {
        let Some(page) = self.reader.as_ref().and_then(|reader| {
            self.scene
                .as_ref()
                .and_then(|scene| reader.changed_page_from(scene))
        }) else {
            return false;
        };
        self.discard_scene();
        self.images.retain(|&(_, _, cached_page), cached| {
            if cached_page == page {
                cx.drop_image(cached.image.clone(), None);
                false
            } else {
                true
            }
        });
        self.failed.remove(&page);
        true
    }
    fn has_image(&self, page: usize) -> bool {
        self.image(page).is_some()
    }
    fn image(&self, page: usize) -> Option<&CachedPage> {
        let (zoom, dpi) = self.scale_key?;
        self.images.get(&(zoom, dpi, page))
    }
    pub(super) fn measure_paint(
        &mut self,
        page: usize,
        caret_page: Option<usize>,
        scroll: f32,
        zoom: u16,
    ) {
        if !self.has_image(page) {
            return;
        }
        if Some(page) == caret_page {
            perf::sample("edit_to_paint", self.input_paint.take(), page);
        }
        if self.range(scroll, zoom).contains(&page) {
            perf::sample("open_to_paint", self.open_paint.take(), page);
            perf::sample("scroll_to_paint", self.scroll_paint.take(), page);
        }
    }
}

impl JoyWord {
    pub(super) fn document_title(&self) -> String {
        let title = match &self.canvas.reader {
            Some(reader) if reader.summary.name != "Untitled" => {
                format!("{} — JoyWord", reader.summary.name)
            }
            _ => self.shell.title(),
        };
        if self.canvas.reader.as_ref().is_some_and(|r| r.is_dirty()) {
            format!("* {title}")
        } else {
            title
        }
    }
    pub(super) fn clear_images(&mut self, cx: &mut Context<Self>) {
        for (_, cached) in std::mem::take(&mut self.canvas.images) {
            cx.drop_image(cached.image, None);
        }
        self.canvas.failed.clear();
    }
    pub(super) fn begin_open(&mut self, source: OpenSource, cx: &mut Context<Self>) {
        if self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.is_dirty() || r.is_composing())
        {
            self.pending_open = Some(source);
            self.confirm_discard = true;
            cx.notify();
            return;
        }
        self.open_unchecked(source, cx);
    }
    pub(super) fn open_unchecked(&mut self, source: OpenSource, cx: &mut Context<Self>) {
        let started = perf::start();
        let Some(ticket) = self.canvas.load_gate.renew(()) else {
            self.shell.notice = Some("Open generation exhausted".into());
            return;
        };
        self.canvas.cancel.store(true, Ordering::Release);
        let cancel = Arc::new(AtomicBool::new(false));
        self.canvas.cancel = cancel.clone();
        self.canvas.loading = true;
        self.shell.notice = None;
        let from_legacy = matches!(&source, OpenSource::LegacyRecovery(_));
        let work = cx.background_executor().spawn(async move {
            // Platform executor threads can have a smaller stack than the nested DOCX
            // importer and layout require, particularly in debug builds.
            std::thread::Builder::new()
                .name("joyword-desktop-open".into())
                .stack_size(8 * 1024 * 1024)
                .spawn(move || match source {
                    OpenSource::File(path) => Reader::open(&path, &cancel),
                    OpenSource::LegacyRecovery(path) => Reader::recover_legacy(&path, &cancel),
                    OpenSource::Welcome => {
                        Reader::from_bytes("JoyWord 欢迎.docx", WELCOME, &cancel)
                    }
                    OpenSource::Blank => Reader::blank(&cancel),
                })
                .map_err(|error| format!("Cannot start desktop document open: {error}"))?
                .join()
                .map_err(|_| "Desktop document open worker failed".to_string())?
        });
        self.canvas.load_task = Some(cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |this, cx| {
                if !this.canvas.load_gate.accepts(&ticket, &()) {
                    return;
                }
                this.canvas.loading = false;
                perf::sample("open_worker", started, 0);
                match result {
                    Ok(mut reader) => {
                        if from_legacy {
                            this.legacy_recovery = None;
                        }
                        reader.set_review_author(this.shell.review_author());
                        if let Some(old) = this.canvas.reader.as_mut() {
                            old.finish_recovery(!old.is_dirty());
                        }
                        if let Err(e) = reader.enable_desktop_recovery() {
                            this.shell.notice = Some(e);
                        } else if let Some(notice) = reader.recovery_notice {
                            this.shell.notice = Some(notice.into());
                        }
                        if let Some(path) = reader.source_path()
                            && let Err(error) = this.shell.remember_document(path)
                            && this.shell.notice.is_none()
                        {
                            this.shell.notice = Some(error);
                        }
                        let Some(epoch) = this.canvas.epoch.checked_add(1) else {
                            this.shell.notice = Some("Document generation exhausted".into());
                            cx.notify();
                            return;
                        };
                        this.canvas.epoch = epoch;
                        this.clear_images(cx);
                        let document = reader.engine().document_id();
                        this.canvas.reader = Some(reader);
                        this.canvas.invalidate_scene();
                        this.canvas.open_paint = started;
                        this.canvas.input_paint = None;
                        this.install_input(document, cx);
                        this.canvas.scale_key = None;
                        this.file_page = false;
                        this.scroll.set_offset(point(px(0.0), px(0.0)));
                    }
                    Err(error) => this.shell.notice = Some(error),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn cancel_open(&mut self, cx: &mut Context<Self>) {
        self.canvas.cancel.store(true, Ordering::Release);
        let _ = self.canvas.load_gate.renew(());
        self.canvas.load_task = None;
        self.canvas.loading = false;
        cx.notify();
    }
    pub(super) fn pick_open(&mut self, cx: &mut Context<Self>) {
        // Same GPUI native dialog entry and receiver handling as the existing JoyWord.
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        self.canvas.dialog_task = Some(cx.spawn(async move |view, cx| match receiver.await {
            Ok(Ok(Some(mut paths))) => {
                if let Some(path) = paths.pop() {
                    let _ = view.update(cx, |this, cx| this.begin_open(OpenSource::File(path), cx));
                }
            }
            Ok(Ok(None)) => {}
            _ => {
                let _ = view.update(cx, |this, cx| {
                    this.shell.notice = Some(
                        "File dialog failed; a DOCX or .jwr path can also be passed on startup."
                            .into(),
                    );
                    cx.notify();
                });
            }
        }));
    }
    pub(super) fn page_step(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(reader) = &self.canvas.reader else {
            return;
        };
        let current = self
            .canvas
            .range(
                (-f32::from(self.scroll.offset().y)).max(0.0),
                self.shell.zoom,
            )
            .start;
        let next = current
            .saturating_add_signed(step)
            .min(reader.summary.pages.len().saturating_sub(1));
        self.scroll
            .set_offset(point(px(0.0), px(-self.canvas.top(next, self.shell.zoom))));
        cx.notify();
    }
    pub(super) fn prepare_canvas(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.canvas.dpi = window.scale_factor();
        self.canvas.a11y_active = window.is_a11y_active();
        let dpi = (self.canvas.dpi * 1000.0).round() as u32;
        let scale_key = (self.shell.zoom, dpi);
        if self.canvas.scale_key != Some(scale_key) {
            if let Some((old_zoom, _)) = self.canvas.scale_key
                && old_zoom != scale_key.0
            {
                let offset = self.scroll.offset();
                let scroll = self.canvas.index.zoomed_scroll(
                    self.canvas.factor(old_zoom),
                    self.canvas.factor(scale_key.0),
                    (-f32::from(offset.y)).max(0.0),
                    self.canvas.viewport,
                );
                self.scroll.set_offset(point(offset.x, px(-scroll)));
            }
            self.canvas.failed.clear();
            self.canvas.scale_key = Some(scale_key);
        }
        let Some(reader) = &self.canvas.reader else {
            return;
        };
        if self.canvas.index.is_empty() {
            self.canvas.index = PageIndex::new(&reader.summary.pages);
        }
        let visible = self.canvas.range(
            (-f32::from(self.scroll.offset().y)).max(0.0),
            self.shell.zoom,
        );
        let first = visible.start.saturating_sub(2);
        let end = visible.end.saturating_add(2);
        self.page_bounds
            .retain(|page, _| (first..end).contains(page));
        let mut candidates: Vec<_> = visible.clone().collect();
        for delta in 1..=2 {
            if let Some(page) = visible.start.checked_sub(delta) {
                candidates.push(page)
            }
            let page = visible.end + delta - 1;
            if page < reader.summary.pages.len() {
                candidates.push(page)
            }
        }
        let scale = self.canvas.factor(self.shell.zoom) * self.canvas.dpi;
        let mut used = 0_u64;
        let mut wanted = Vec::new();
        for page in candidates {
            let size = &reader.summary.pages[page];
            let pixels =
                ((size.width * size.height * scale * scale).ceil() as u64).min(MAX_PAGE_PIXELS);
            if used + pixels > MAX_CACHE_PIXELS {
                continue;
            }
            used += pixels;
            wanted.push(page);
        }
        self.canvas.access = self.canvas.access.saturating_add(1);
        for page in &wanted {
            if let Some(entry) = self.canvas.images.get_mut(&(scale_key.0, dpi, *page)) {
                entry.last_used = self.canvas.access;
            }
        }
        self.canvas.wanted = wanted;
        if let Some((key, cancel)) = &self.canvas.active_raster {
            if key.epoch != self.canvas.epoch
                || (key.zoom, key.dpi) != scale_key
                || !self.canvas.wanted.contains(&key.page)
                || (!visible.contains(&key.page)
                    && visible
                        .clone()
                        .any(|p| !self.canvas.has_image(p) && !self.canvas.failed.contains_key(&p)))
            {
                cancel.store(true, Ordering::Relaxed);
            }
            return;
        }
        let Some(page) =
            self.canvas.wanted.iter().copied().find(|page| {
                !self.canvas.has_image(*page) && !self.canvas.failed.contains_key(page)
            })
        else {
            return;
        };
        let key = RasterKey {
            epoch: self.canvas.epoch,
            zoom: self.shell.zoom,
            dpi,
            page,
        };
        let Some(ticket) = self.canvas.raster_gate.renew(key.clone()) else {
            return;
        };
        let scene = self
            .canvas
            .scene
            .get_or_insert_with(|| reader.scene())
            .clone();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.canvas.active_raster = Some((key, cancelled.clone()));
        let worker_cancel = cancelled.clone();
        let work = cx.background_executor().spawn(async move {
            let started = perf::start();
            let mut raster = scene.raster_cancellable(page, scale, &worker_cancel)?;
            perf::sample("raster_worker", started, page);
            if worker_cancel.load(Ordering::Relaxed) {
                return Err("Page rendering cancelled".into());
            }
            // GPUI's image path consumes premultiplied BGRA; layout stays in document points.
            for pixel in raster.pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2)
            }
            let buffer = image::RgbaImage::from_raw(raster.width, raster.height, raster.pixels)
                .ok_or("Invalid raster buffer")?;
            Ok::<_, String>((
                Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])),
                u64::from(raster.width) * u64::from(raster.height),
            ))
        });
        self.canvas.raster_task = Some(cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |this, cx| {
                this.canvas.raster_task = None;
                this.canvas.active_raster = None;
                let current = RasterKey {
                    epoch: this.canvas.epoch,
                    zoom: this.shell.zoom,
                    dpi: (this.canvas.dpi * 1000.0).round() as u32,
                    page,
                };
                if !cancelled.load(Ordering::Relaxed)
                    && this.canvas.wanted.contains(&page)
                    && this.canvas.raster_gate.accepts(&ticket, &current)
                {
                    match result {
                        Ok((image, pixels)) => {
                            let mut resident: u64 =
                                this.canvas.images.values().map(|entry| entry.pixels).sum();
                            while resident + pixels > MAX_CACHE_PIXELS {
                                let oldest = this
                                    .canvas
                                    .images
                                    .iter()
                                    .filter(|((zoom, dpi, page), _)| {
                                        (*zoom, *dpi) != (current.zoom, current.dpi)
                                            || !this.canvas.wanted.contains(page)
                                    })
                                    .min_by_key(|(_, entry)| entry.last_used)
                                    .map(|(&page, _)| page);
                                let Some(old) =
                                    oldest.and_then(|page| this.canvas.images.remove(&page))
                                else {
                                    break;
                                };
                                resident -= old.pixels;
                                cx.drop_image(old.image, None);
                            }
                            if resident + pixels <= MAX_CACHE_PIXELS {
                                perf::sample("raster_installed", perf::start(), page);
                                this.canvas.images.insert(
                                    (current.zoom, current.dpi, page),
                                    CachedPage {
                                        image,
                                        pixels,
                                        last_used: this.canvas.access,
                                    },
                                );
                            } else {
                                this.canvas
                                    .failed
                                    .insert(page, "Page cache budget reached".into());
                            }
                        }
                        Err(error) => {
                            this.canvas.failed.insert(page, error);
                        }
                    }
                }
                cx.notify();
            });
        }));
    }
    pub(super) fn document_status(&self) -> String {
        if self.canvas.loading {
            return self.shell.tr("正在打开文档…", "Opening document…").into();
        }
        match &self.canvas.reader {
            Some(reader) => {
                let current = self
                    .canvas
                    .range(
                        (-f32::from(self.scroll.offset().y)).max(0.0),
                        self.shell.zoom,
                    )
                    .start
                    + 1;
                format!(
                    "{} {current} / {} · {} · {}",
                    self.shell.tr("页", "Page"),
                    reader.summary.pages.len(),
                    if reader.is_composing() {
                        self.shell.tr("输入法组合中", "Composing")
                    } else {
                        self.shell.tr("可编辑", "Editable")
                    },
                    reader.summary.name
                )
            }
            None => self
                .shell
                .tr(
                    "请选择文档（DOCX、RTF、ODT、HTML 等）",
                    "Choose a document (DOCX, RTF, ODT, HTML…)",
                )
                .into(),
        }
    }
    pub(super) fn document_workspace(&self, cx: &mut Context<Self>) -> gpui::Div {
        let p = self.palette();
        let mut area = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(rgb(p.canvas))
            .child(
                div()
                    .flex()
                    .h(px(34.0))
                    .flex_shrink_0()
                    .px_4()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_color(rgb(p.muted))
                            .text_size(px(12.0))
                            .child(self.document_status()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .when(self.canvas.loading, |row| {
                                row.child(self.button(
                                    "cancel-open",
                                    self.shell.tr("取消打开", "Cancel open"),
                                    "file.cancel_open",
                                    cx,
                                ))
                            })
                            .child(self.button(
                                "previous-page",
                                self.shell.tr("上一页", "Previous"),
                                "view.previous_page",
                                cx,
                            ))
                            .child(self.button(
                                "next-page",
                                self.shell.tr("下一页", "Next"),
                                "view.next_page",
                                cx,
                            )),
                    ),
            );
        let Some(reader) = &self.canvas.reader else {
            return area.child(div().p_8().text_color(rgb(p.muted)).child(self.shell.tr(
                "使用「打开」选择真实 DOCX 文档。",
                "Use Open to choose a DOCX document.",
            )));
        };
        let range = self.canvas.range(
            (-f32::from(self.scroll.offset().y)).max(0.0),
            self.shell.zoom,
        );
        let shown = range.start.saturating_sub(2)..(range.end + 2).min(reader.summary.pages.len());
        let scale = self.canvas.factor(self.shell.zoom);
        let total_height = self.canvas.top(reader.summary.pages.len(), self.shell.zoom);
        let before = self.canvas.top(shown.start, self.shell.zoom);
        let after = (total_height - self.canvas.top(shown.end, self.shell.zoom)).max(0.0);
        let width = self.canvas.index.width(scale);
        let pages = div()
            .flex()
            .flex_col()
            .items_center()
            .min_w(px(width + 64.0))
            .child(div().h(px(before)).flex_shrink_0())
            .children(shown.map(|index| {
                let size = &reader.summary.pages[index];
                let content = match self.canvas.image(index) {
                    Some(page) => img(page.image.clone()).size_full().into_any_element(),
                    None => div()
                        .p_6()
                        .text_color(rgb(p.muted))
                        .child(self.canvas.failed.get(&index).cloned().unwrap_or_else(|| {
                            self.shell.tr("页面载入中…", "Loading page…").into()
                        }))
                        .into_any_element(),
                };
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .flex_shrink_0()
                    .pb_6()
                    .child(
                        div()
                            .h(px(18.0))
                            .text_size(px(11.0))
                            .text_color(rgb(p.muted))
                            .child(format!("{} {}", self.shell.tr("第", "Page"), index + 1)),
                    )
                    .child(
                        div()
                            .w(px(size.width * scale))
                            .h(px(size.height * scale))
                            .flex_shrink_0()
                            .bg(rgb(0xffffff))
                            .shadow_md()
                            .relative()
                            .child(content)
                            .child(self.page_input(index, cx)),
                    )
            }))
            .child(div().h(px(after + 24.0)).flex_shrink_0());
        area = area.child(
            self.accessible_text(
                div()
                    .id("document-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll(cx.listener(|v, _, _, cx| {
                        v.canvas.scroll_paint = perf::start();
                        cx.notify();
                    }))
                    .child(pages)
                    .relative()
                    .child(self.input_registration(cx)),
                cx,
            ),
        );
        area
    }
}
