use gpui_kit::base::Disableable;
use gpui_kit::component::{
    WindowExt,
    button::Button,
    input::{Input, InputState, Textarea, TextareaState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use joydeck_model::ShapeId;
use joydeck_shell::Document as Deck;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

actions!(
    viewer,
    [
        Open,
        Cancel,
        Previous,
        Next,
        First,
        Last,
        ZoomIn,
        ZoomOut,
        Fit,
        Report,
        Save,
        UndoEdit,
        RedoEdit,
        AddSlide,
        DuplicateObject,
        DuplicateSlides,
        DeleteSlides,
        MoveForward,
        MoveBackward,
        NewDocument,
        DeleteObject,
        CloseEditor,
        NextCell,
        PreviousCell,
        SaveAs,
        PresentStart,
        PresentCurrent
    ]
);
#[path = "basic_edit.rs"]
mod basic_edit;
use basic_edit::BasicEdit;
#[path = "chrome.rs"]
mod chrome;
#[path = "chrome_edit.rs"]
mod chrome_edit;
#[path = "chrome_nav.rs"]
mod chrome_nav;
#[path = "content_ui.rs"]
mod content_ui;
#[path = "delivery_ui.rs"]
mod delivery_ui;
#[path = "desktop_control.rs"]
pub(super) mod desktop_control;
#[path = "document_session.rs"]
mod document_session;
#[path = "document_ui.rs"]
mod document_ui;
#[path = "editor_actions.rs"]
mod editing;
#[path = "file_ui.rs"]
mod file_ui;
#[path = "format_ui.rs"]
mod format_ui;
#[path = "inline_text.rs"]
mod inline;
#[path = "organize_ui.rs"]
mod organize_ui;
#[path = "paragraph_ui.rs"]
mod paragraph_ui;
#[path = "review_ui.rs"]
mod review_ui;
#[path = "scene_benchmark.rs"]
mod scene_benchmark;
#[path = "scene_canvas.rs"]
mod scene_canvas;
#[path = "scene_view.rs"]
mod scene_view;
#[path = "selection_format.rs"]
mod selection_format;
#[path = "slideshow.rs"]
mod slideshow;
#[path = "table_ui.rs"]
mod table_ui;

// Document generation, page, raster width, font environment.
type ImageKey = (u64, usize, u32, u64);
pub struct Viewer {
    comparison: Option<joydeck_shell::comparison::Comparison>,
    compare_task: Option<Task<()>>,
    control: Option<desktop_control::Control>,
    control_task: Option<Task<()>>,
    recent_paths: Result<Vec<PathBuf>, String>,
    recent_task: Option<Task<()>>,
    font_picker: Entity<
        gpui_kit::component::select::SelectState<
            gpui_kit::component::select::SearchableVec<SharedString>,
        >,
    >,
    font_names: Vec<SharedString>,
    font_subscription: Option<Subscription>,
    font_task: Option<Task<()>>,
    delivery_task: Option<Task<()>>,
    delivery_cancel: joydeck_shell::Cancellation,
    delivery_busy: bool,
    benchmark: Option<Box<scene_benchmark::Benchmark>>,
    benchmark_task: Option<Task<()>>,
    retired_images: Vec<Arc<RenderImage>>,
    gpu_document: u64,
    ribbon: &'static str,
    backstage: bool,
    sorter: bool,
    slide_selection: Vec<joydeck_model::SlideId>,
    slide_anchor: Option<joydeck_model::SlideId>,
    notes_visible: bool,
    notes_syncing: bool,
    notes_loaded: Option<(u64, joydeck_model::SlideId, String)>,
    notes_subscription: Option<Subscription>,
    inspector: bool,
    content_task: Option<Task<()>>,
    cpu_view: bool,
    scene: Option<(ImageKey, Arc<joydeck_render::scene::Scene>)>,
    scene_cache: Arc<std::sync::Mutex<joydeck_render::scene::Cache>>,
    gpu_cache: scene_canvas::GpuCache,
    scene_pending: Option<ImageKey>,
    scene_task: Option<Task<()>>,
    scene_busy: bool,
    scene_cancel: Arc<AtomicBool>,
    inline: Option<Box<inline::Inline>>,
    cell: Option<(ShapeId, usize, usize)>,
    cell_range: Option<(ShapeId, joydeck_engine::CellRange)>,
    notes_input: Entity<TextareaState>,
    editor: Option<document_session::ViewEditor>,
    document_observer: Option<Subscription>,
    save_input: Entity<InputState>,
    text_input: Entity<InputState>,
    save_task: Option<Task<()>>,
    recovery_rows: Option<Vec<joydeck_shell::storage::Entry>>,
    refresh_pending: bool,
    pending_save_path: Option<String>,
    saving: bool,
    close_after_save: bool,
    close_ready: bool,
    pending_replace: Option<document_ui::ReplaceDocument>,
    replace_ready: bool,
    canvas_bounds: Bounds<Pixels>,
    zoom_bounds: Bounds<Pixels>,
    drag: Option<editing::Drag>,
    activation: Option<Subscription>,
    dark: bool,
    focus: FocusHandle,
    deck: Option<Arc<Deck>>,
    source: Option<PathBuf>,
    page: usize,
    zoom: Option<f32>,
    generation: u64,
    read_request: u64,
    render_generation: u64,
    image: Option<(ImageKey, Arc<RenderImage>)>,
    pending: Option<ImageKey>,
    thumbnails: BTreeMap<usize, Arc<RenderImage>>,
    thumbnail_errors: BTreeSet<usize>,
    thumb_busy: bool,
    scroll: UniformListScrollHandle,
    page_scroll: ScrollHandle,
    ribbon_scroll: ScrollHandle,
    read_cancel: Arc<AtomicBool>,
    paint_cancel: Arc<AtomicBool>,
    thumbs_cancel: Arc<AtomicBool>,
    read_task: Option<Task<()>>,
    paint_task: Option<Task<()>>,
    thumbs_task: Option<Task<()>>,
    dialog_task: Option<Task<()>>,
    loading: bool,
    message: String,
    rasters: u64,
    started: Instant,
}
impl Drop for Viewer {
    fn drop(&mut self) {
        self.delivery_cancel.cancel();
        self.scene_cancel.store(true, Ordering::Relaxed);
        for token in [&self.read_cancel, &self.paint_cancel, &self.thumbs_cancel] {
            token.store(true, Ordering::Relaxed);
        }
    }
}
fn image_of(r: Raster) -> std::result::Result<Arc<RenderImage>, String> {
    let pixels =
        image::RgbaImage::from_raw(r.width, r.height, r.bgra).ok_or("预览像素与画布尺寸不匹配")?;
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])))
}
impl Viewer {
    fn new(dark: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let font_picker = cx.new(|cx| {
            gpui_kit::component::select::SelectState::new(
                gpui_kit::component::select::SearchableVec::new(Vec::new()),
                None,
                window,
                cx,
            )
            .searchable(true)
        });
        Self {
            comparison: None,
            compare_task: None,
            control: None,
            control_task: None,
            recent_paths: Ok(vec![]),
            recent_task: None,
            font_picker,
            font_names: Vec::new(),
            font_subscription: None,
            font_task: None,
            delivery_task: None,
            delivery_cancel: Default::default(),
            delivery_busy: false,
            benchmark: None,
            benchmark_task: None,
            retired_images: vec![],
            gpu_document: 0,
            ribbon: "开始",
            backstage: false,
            sorter: false,
            slide_selection: vec![],
            slide_anchor: None,
            notes_visible: true,
            notes_syncing: false,
            notes_loaded: None,
            notes_subscription: None,
            inspector: false,
            content_task: None,
            cpu_view: false,
            scene: None,
            scene_cache: Arc::default(),
            gpu_cache: scene_canvas::GpuCache::default(),
            scene_pending: None,
            scene_task: None,
            scene_busy: false,
            scene_cancel: Arc::new(AtomicBool::new(false)),
            inline: None,
            cell: None,
            cell_range: None,
            notes_input: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("单击此处添加备注")
                    .auto_grow(1, 3)
            }),
            editor: None,
            document_observer: None,
            save_input: cx.new(|cx| {
                InputState::new(window, cx).placeholder("另存路径 .pptx / .joydeck（新文件）")
            }),
            text_input: cx.new(|cx| {
                InputState::new(window, cx).placeholder("选中文字对象后，在这里修改并应用")
            }),
            save_task: None,
            recovery_rows: None,
            refresh_pending: false,
            pending_save_path: None,
            saving: false,
            close_after_save: false,
            close_ready: false,
            pending_replace: None,
            replace_ready: false,
            canvas_bounds: Bounds::default(),
            zoom_bounds: Bounds::default(),
            drag: None,
            activation: None,
            dark,
            focus,
            deck: None,
            source: None,
            page: 0,
            zoom: None,
            generation: 0,
            read_request: 0,
            render_generation: 0,
            image: None,
            pending: None,
            thumbnails: BTreeMap::new(),
            thumbnail_errors: BTreeSet::new(),
            thumb_busy: false,
            scroll: UniformListScrollHandle::new(),
            page_scroll: ScrollHandle::new(),
            ribbon_scroll: ScrollHandle::new(),
            read_cancel: Arc::new(AtomicBool::new(false)),
            paint_cancel: Arc::new(AtomicBool::new(false)),
            thumbs_cancel: Arc::new(AtomicBool::new(false)),
            read_task: None,
            paint_task: None,
            thumbs_task: None,
            dialog_task: None,
            loading: false,
            message: "选择本地文稿开始编辑 · PPTX / JoyDeck v2 / DeckCraft 导入".into(),
            rasters: 0,
            started: Instant::now(),
        }
    }
    fn open(&mut self, _: &Open, w: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("打开文稿".into()),
        });
        let window = w.window_handle();
        let read_request = self.read_request;
        self.dialog_task = Some(cx.spawn(async move |view, cx| {
            let result = receiver.await;
            let _ = window.update(cx, |_, w, cx| {
                let _ = view.update(cx, |s, cx| {
                    if s.read_request != read_request {
                        return;
                    }
                    match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.into_iter().next() {
                                s.open_path(path, w, cx);
                            }
                        }
                        Ok(Ok(None)) => {}
                        _ => {
                            s.message = "无法打开文件选择器，请重新尝试".into();
                            cx.notify();
                        }
                    }
                });
            });
        }));
    }
    pub(super) fn open_path(&mut self, path: PathBuf, w: &mut Window, cx: &mut Context<Self>) {
        if !self.confirm_replace(document_ui::ReplaceDocument::Path(path.clone()), w, cx) {
            self.load(path, cx);
        }
    }
    pub(super) fn stop_read(&mut self) {
        self.read_cancel.store(true, Ordering::Relaxed);
        self.read_request += 1;
        self.read_task = None;
        self.loading = false;
    }
    pub(super) fn start_read(&mut self) -> (u64, joydeck_shell::Cancellation) {
        self.stop_read();
        self.read_cancel = Arc::new(AtomicBool::new(false));
        self.loading = true;
        (
            self.read_request,
            joydeck_shell::Cancellation::from_flag(self.read_cancel.clone()),
        )
    }
    fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.load_path_with_discard(path, false, cx);
    }
    pub(super) fn load_path_with_discard(
        &mut self,
        path: PathBuf,
        discard: bool,
        cx: &mut Context<Self>,
    ) {
        if !discard && self.block_replace(cx) {
            return;
        }
        if self.is_saving() || self.composing() {
            return;
        }
        self.end_inline(cx);
        let revision = self.editor.as_ref().map(|e| e.session.revision());
        let (request, cancel) = self.start_read();
        self.started = Instant::now();
        self.message = "正在后台读取、检查文稿… 可取消；当前文稿继续可浏览".into();
        eprintln!("R6/R7 open_started=true request={request}");
        self.read_task = Some(cx.spawn(async move |view, cx| {
            let source = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let root = joydeck_shell::editor::recovery_root();
                    let editor = if source == std::path::Path::new("@bundled") {
                        joydeck_shell::Document::from_bytes(
                            "r1-preview.deckcraft".into(),
                            include_bytes!("../../../tests/fixtures/joydeck/r1-preview.deckcraft")
                                .to_vec(),
                            &cancel,
                        )
                        .and_then(|doc| {
                            joydeck_shell::editor::Editor::from_document_cancellable(
                                doc, root, &cancel,
                            )
                        })
                    } else {
                        joydeck_shell::editor::Editor::prepare_open(&source, root, &cancel)
                    }?;
                    let identity = editor.source_identity();
                    let deck = editor.display()?;
                    cancel.check()?;
                    Ok::<_, String>((editor, deck, identity))
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.read_request != request {
                    eprintln!("R6/R7 stale_open_discarded=true");
                    return;
                }
                s.loading = false;
                match result {
                    Ok((editor, deck, identity)) => {
                        if revision != s.editor.as_ref().map(|e| e.session.revision())
                            || s.composing()
                            || s.is_saving()
                        {
                            s.message = "打开请求已过期 · 当前文稿保持不变".into();
                            cx.notify();
                            return;
                        }
                        let recent = editor
                            .source
                            .clone()
                            .map(|p| (editor.recovery.root.clone(), p));
                        let (document, deck) = if let Some(existing) = identity
                            .as_deref()
                            .and_then(|path| document_session::SharedDocument::find(path, cx))
                            .filter(|candidate| {
                                s.editor.as_ref().is_none_or(|current| {
                                    current.document != *candidate || current.has_other_views()
                                })
                            }) {
                            cx.background_executor()
                                .spawn(async move { drop(editor) })
                                .detach();
                            let display = existing.read(cx).display();
                            (existing, display)
                        } else {
                            (
                                document_session::SharedDocument::create(editor, identity, cx),
                                Ok(deck),
                            )
                        };
                        let deck = match deck {
                            Ok(deck) => deck,
                            Err(error) => {
                                s.message = error;
                                cx.notify();
                                return;
                            }
                        };
                        s.delivery_cancel.cancel();
                        s.generation += 1;
                        s.cell = None;
                        s.cell_range = None;
                        s.slide_selection.clear();
                        s.slide_anchor = None;
                        s.drag = None;
                        s.backstage = false;
                        s.attach_document(document, None, discard, cx);
                        cx.background_executor()
                            .spawn(async move {
                                if let Some((root, path)) = recent
                                    && let Err(e) =
                                        joydeck_shell::storage::remember_file(&root, &path)
                                {
                                    eprintln!("JoyDeck recent file update failed: {e}");
                                }
                            })
                            .detach();
                        s.paint_cancel.store(true, Ordering::Relaxed);
                        s.thumbs_cancel.store(true, Ordering::Relaxed);
                        s.render_generation += 1;
                        let pages = deck.info.slides.len();
                        s.deck = Some(Arc::new(deck));
                        s.page = 0;
                        s.zoom = None;
                        s.image = None;
                        s.pending = None;
                        s.thumbnails.clear();
                        s.thumbnail_errors.clear();
                        s.thumb_busy = false;
                        s.thumbs_task = None;
                        s.thumbs_cancel = Arc::new(AtomicBool::new(false));
                        s.scroll.scroll_to_item(0, ScrollStrategy::Top);
                        s.message = format!("已打开 · {} 张幻灯片", pages);
                        eprintln!(
                            "R6/R7 opened=true pages={} elapsed_ms={}",
                            pages,
                            s.started.elapsed().as_millis()
                        );
                    }
                    Err(error) => {
                        s.message = format!("打开失败：{error} · 当前文稿保持不变");
                        eprintln!("R6/R7 open_failed=true reason={error}");
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn cancel_read(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            self.stop_read();
            self.message = "已取消读取 · 当前文稿保持不变".into();
            eprintln!("R6/R7 open_cancelled=true");
            cx.notify();
        }
    }
    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            self.cancel_read(cx);
        } else if self.inline.is_some() {
            self.cancel_inline(cx);
        } else if self.drag.take().is_some() {
            self.message = "手势已取消，未修改文稿".into();
            cx.notify();
        }
    }
    fn select(&mut self, page: usize, cx: &mut Context<Self>) {
        self.end_inline(cx);
        self.cell = None;
        self.cell_range = None;
        if let Some(d) = &self.deck {
            let page = page.min(d.info.slides.len() - 1);
            if page != self.page {
                self.paint_cancel.store(true, Ordering::Relaxed);
                self.pending = None;
            }
            self.page = page;
            if let Some(editor) = &mut self.editor {
                let _ = editor.session.select(page, vec![]);
                if let Some(slide) = editor.session.model().slides.get(page) {
                    self.slide_selection = vec![slide.id];
                    self.slide_anchor = Some(slide.id);
                }
            }
            self.drag = None;
            self.scroll.scroll_to_item(self.page, ScrollStrategy::Top);
            cx.notify();
        }
    }
    fn previous(&mut self, _: &Previous, _: &mut Window, cx: &mut Context<Self>) {
        self.select(self.page.saturating_sub(1), cx);
    }
    fn next(&mut self, _: &Next, _: &mut Window, cx: &mut Context<Self>) {
        self.select(self.page + 1, cx);
    }
    fn first(&mut self, _: &First, _: &mut Window, cx: &mut Context<Self>) {
        self.select(0, cx);
    }
    fn last(&mut self, _: &Last, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = &self.deck {
            self.select(d.info.slides.len() - 1, cx);
        }
    }
    fn scale(&self, window: &Window) -> f32 {
        if let Some(zoom) = self.zoom {
            return zoom;
        }
        let Some(d) = &self.deck else {
            return 1.;
        };
        let sz = window.viewport_size();
        let inspector = if self.inspector { 264. } else { 0. };
        ((f32::from(sz.width) - self.rail_width(window) - 96. - inspector).max(64.)
            / (d.info.width_pt as f32))
            .min(
                (f32::from(sz.height) - if self.notes_visible { 292. } else { 230. }).max(64.)
                    / (d.info.height_pt as f32),
            )
            .clamp(0.01, 4.)
    }
    fn zoom_in(&mut self, _: &ZoomIn, w: &mut Window, cx: &mut Context<Self>) {
        self.zoom = Some((self.scale(w) + 0.25).clamp(0.25, 4.));
        cx.notify();
    }
    fn zoom_out(&mut self, _: &ZoomOut, w: &mut Window, cx: &mut Context<Self>) {
        self.zoom = Some((self.scale(w) - 0.25).clamp(0.25, 4.));
        cx.notify();
    }
    fn fit(&mut self, _: &Fit, _: &mut Window, cx: &mut Context<Self>) {
        self.zoom = None;
        cx.notify();
    }
    fn report(&mut self, _: &Report, window: &mut Window, cx: &mut Context<Self>) {
        let total = self.deck.as_ref().map_or(0, |d| d.info.warnings.len());
        let rows: Vec<String> = self
            .deck
            .as_ref()
            .map(|d| d.info.warnings.iter().take(200).cloned().collect())
            .unwrap_or_default();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("兼容报告 · 导入与输出限制")
                .child(format!("共 {total} 条提示；此面板最多显示前 200 条"))
                .child(
                    div()
                        .id("report-body")
                        .max_h(px(420.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .children(rows.iter().map(|s| div().text_sm().child(s.clone()))),
                )
        });
    }
    fn request_image(&mut self, width: u32, cx: &mut Context<Self>) {
        let Some(deck) = self.deck.clone() else {
            return;
        };
        let key = (
            self.render_generation,
            self.page,
            width,
            joydeck_fonts::FontDb::global().environment_revision(),
        );
        if self.image.as_ref().is_some_and(|(k, _)| *k == key) {
            self.pending = Some(key);
            return;
        }
        if self.pending == Some(key) {
            return;
        }
        self.paint_cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.paint_cancel = cancel.clone();
        self.pending = Some(key);
        self.paint_task = Some(cx.spawn(async move |view, cx| {
            let started = Instant::now();
            let result = cx
                .background_executor()
                .spawn(async move { render_doc(&deck, key.1, key.2, &cancel).and_then(image_of) })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.pending != Some(key)
                    || s.render_generation != key.0
                    || s.page != key.1
                    || joydeck_fonts::FontDb::global().environment_revision() != key.3
                {
                    return;
                }
                // Keep key until another request; render failure must not cause an infinite retry loop.
                match result {
                    Ok(r) => {
                        s.image = Some((key, r));
                        s.rasters += 1;
                        eprintln!(
                            "R6/R7 raster page={} width={} elapsed_ms={} count={}",
                            key.1 + 1,
                            key.2,
                            started.elapsed().as_millis(),
                            s.rasters
                        );
                    }
                    Err(e) => s.message = format!("预览失败：{e}"),
                }
                cx.notify();
            });
        }));
    }
    fn request_thumbnails(&mut self, range: std::ops::Range<usize>, cx: &mut Context<Self>) {
        if self.thumb_busy {
            return;
        }
        let Some(deck) = self.deck.clone() else {
            return;
        };
        let pages: Vec<_> = range
            .filter(|p| !self.thumbnails.contains_key(p) && !self.thumbnail_errors.contains(p))
            .take(12)
            .collect();
        if pages.is_empty() {
            return;
        }
        self.thumb_busy = true;
        let generation = self.render_generation;
        let font_revision = joydeck_fonts::FontDb::global().environment_revision();
        let cancel = self.thumbs_cancel.clone();
        self.thumbs_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut out = Vec::new();
                    for p in pages {
                        if cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        out.push((p, render_doc(&deck, p, 176, &cancel)));
                    }
                    out
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.render_generation != generation {
                    return;
                }
                s.thumb_busy = false;
                if joydeck_fonts::FontDb::global().environment_revision() != font_revision {
                    cx.notify();
                    return;
                }
                for (p, r) in result {
                    let Ok(image) = r.and_then(image_of) else {
                        s.thumbnail_errors.insert(p);
                        continue;
                    };
                    if s.thumbnails.len() >= 32
                        && let Some(key) =
                            s.thumbnails.keys().copied().max_by_key(|k| k.abs_diff(p))
                    {
                        s.thumbnails.remove(&key);
                    }
                    s.thumbnails.insert(p, image);
                }
                cx.notify();
            });
        }));
    }
}
impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // GPUI Kit refreshes every window when its application theme changes.
        self.dark = gpui_kit::component::Theme::global(cx).is_dark();
        if self.close_ready {
            self.close_ready = false;
            cx.defer_in(window, |s, w, _| {
                if !s.is_saving() && !s.requires_save_prompt() {
                    w.remove_window();
                }
            });
        }
        self.sync_font_picker(window, cx);
        for image in self.retired_images.drain(..) {
            let _ = window.drop_image(image);
        }
        if self.gpu_document != self.generation {
            self.gpu_document = self.generation;
            self.scene_cancel.store(true, Ordering::Relaxed);
            self.scene = None;
            self.scene_pending = None;
            self.scene_cache = Arc::default();
            self.gpu_cache.clear(window);
            self.benchmark = None;
        }
        let palette = chrome::palette(self.dark);
        if let Some(path) = self.pending_save_path.take() {
            self.save_input
                .update(cx, |s, cx| s.set_value(path, window, cx));
        }
        if self.refresh_pending {
            self.refresh_pending = false;
            self.sync_editor(window, cx);
        }
        if self.replace_ready {
            self.replace_ready = false;
            if let Some(action) = self.pending_replace.take() {
                self.continue_replace(action, window, cx);
            }
        }
        self.sync_notes(window, cx);
        self.show_recoveries(window, cx);
        let scale = self.scale(window);
        if let Some(e) = &self.editor {
            window.set_window_title(&format!(
                "{}{} - JoyDeck",
                self.document_name(),
                if e.session.dirty() { " *" } else { "" }
            ));
        }
        let deck = self.deck.clone();
        if let Some(d) = &deck {
            let width = ((d.info.width_pt as f32) * scale * window.scale_factor())
                .ceil()
                .clamp(1., 4096.) as u32;
            if self.benchmark.is_some() { /* Benchmark owns its immutable frame sequence. */
            } else if self.cpu_view || self.inline.is_some() {
                self.request_image(width, cx);
            } else {
                self.request_scene(width, cx);
            }
        }
        let count = deck.as_ref().map_or(0, |d| d.info.slides.len());
        let sz = window.viewport_size();
        let aw = (f32::from(sz.width)
            - self.rail_width(window)
            - if self.inspector { 264. } else { 0. })
        .max(64.);
        let ah = (f32::from(sz.height) - if self.notes_visible { 234. } else { 176. }).max(64.);
        let slide_canvas = if let Some(d) = &deck {
            let (w, h) = (
                (d.info.width_pt as f32) * scale,
                (d.info.height_pt as f32) * scale,
            );
            let current = self.image.as_ref().filter(|(key, _)| {
                (self.cpu_view || self.inline.is_some())
                    && (key.0 == self.render_generation || self.inline.is_some())
                    && key.1 == self.page
                    && key.3 == joydeck_fonts::FontDb::global().environment_revision()
            });
            div()
                .id("page-viewport")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|s, _, w, cx| s.focus.focus(w, cx)),
                )
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_scroll()
                .track_scroll(&self.page_scroll)
                .bg(rgb(palette.canvas))
                .child(
                    div()
                        .w(px((w + 64.).max(aw)))
                        .h(px((h + 64.).max(ah)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("editable-page")
                                .relative()
                                .w(px(w))
                                .h(px(h))
                                .bg(rgb(0xffffff))
                                .shadow_sm()
                                .overflow_hidden()
                                .children(current.map(|(_, i)| {
                                    img(i.clone())
                                        .w(px(w))
                                        .h(px(h))
                                        .object_fit(ObjectFit::Contain)
                                }))
                                .child(
                                    canvas(
                                        cx.processor(|s, bounds, _, _| {
                                            s.canvas_bounds = bounds;
                                        }),
                                        {
                                            let paint = cx.processor(move |s, bounds, w, cx| {
                                                s.paint_scene(bounds, scale, w);
                                                s.paint_benchmark(bounds, scale, w, cx);
                                                s.paint_inline(bounds, scale, w, cx);
                                                s.paint_selection_frame(bounds, scale, w);
                                            });
                                            move |bounds, _, w, cx| paint(bounds, w, cx)
                                        },
                                    )
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .size_full(),
                                )
                                .children(self.placeholder_overlays(scale, cx))
                                .children(self.selection_overlays(scale, 0x7a7a7a))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |s, event, w, cx| {
                                        s.pointer_down(event, scale, w, cx)
                                    }),
                                ),
                        ),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .bg(rgb(palette.canvas))
                .child(div().text_3xl().child("JoyDeck"))
                .child("打开文稿，编辑、导出或放映")
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(palette.muted))
                        .child(".deckcraft v1  /  PPTX · 本地读取 · 源文件不变"),
                )
                .child(
                    Button::new("empty-open")
                        .label("打开文稿  Ctrl+O")
                        .on_click(cx.listener(|s, _, w, cx| s.open(&Open, w, cx))),
                )
                .into_any_element()
        };
        self.register_basic_actions(div().id("viewer"), cx)
            .key_context("Viewer")
            .when(self.inline.is_some(), |d| {
                d.key_context("Viewer InlineText")
            })
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(palette.chrome))
            .text_color(rgb(palette.text))
            .font_family(if cfg!(target_os = "windows") {
                "Microsoft YaHei"
            } else {
                "sans-serif"
            })
            .when(self.save_unavailable().is_none(), |d| {
                d.on_action(cx.listener(Self::save))
                    .on_action(cx.listener(Self::save_as))
            })
            .on_action(cx.listener(|s, _: &NextCell, w, cx| s.next_cell(false, w, cx)))
            .on_action(cx.listener(|s, _: &PreviousCell, w, cx| s.next_cell(true, w, cx)))
            .on_key_down(cx.listener(Self::inline_key))
            .on_action(cx.listener(Self::new_document))
            .on_action(cx.listener(Self::request_close))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::first))
            .on_action(cx.listener(Self::last))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::fit))
            .on_action(cx.listener(Self::report))
            .on_action(cx.listener(|s, _: &PresentStart, _, cx| s.start_show(true, false, cx)))
            .on_action(cx.listener(|s, _: &PresentCurrent, _, cx| s.start_show(false, false, cx)))
            .child(
                canvas(|_, _, _| (), {
                    let capture = cx.processor(|s, (), w, cx| s.capture_gesture(w, cx));
                    move |_, (), w, cx| capture((), w, cx)
                })
                .absolute()
                .size_full(),
            )
            .child(self.title_chrome(cx))
            .child(self.editor_ribbon(window, cx))
            .child(if self.backstage {
                self.file_backstage(cx)
            } else if self.sorter {
                self.slide_sorter(cx)
            } else {
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .overflow_hidden()
                    .child(self.slide_rail(window, count, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(slide_canvas)
                            .when(self.notes_visible, |d| d.child(self.notes_pane())),
                    )
                    .children((self.inspector).then(|| self.content_inspector(cx)))
                    .into_any_element()
            })
            .child(self.status_chrome(scale, cx))
    }
}
pub fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", NextCell, Some("Viewer && InlineText && !Input")),
        KeyBinding::new(
            "shift-tab",
            PreviousCell,
            Some("Viewer && InlineText && !Input"),
        ),
        KeyBinding::new("ctrl-o", Open, Some("Viewer")),
        KeyBinding::new("ctrl-s", Save, Some("Viewer")),
        KeyBinding::new("ctrl-shift-s", SaveAs, Some("Viewer")),
        KeyBinding::new("cmd-shift-s", SaveAs, Some("Viewer")),
        KeyBinding::new("f5", PresentStart, Some("Viewer")),
        KeyBinding::new("shift-f5", PresentCurrent, Some("Viewer")),
        KeyBinding::new("cmd-s", Save, Some("Viewer")),
        KeyBinding::new("ctrl-z", UndoEdit, Some("Viewer && !Input")),
        KeyBinding::new("cmd-z", UndoEdit, Some("Viewer && !Input")),
        KeyBinding::new("cmd-shift-z", RedoEdit, Some("Viewer && !Input")),
        KeyBinding::new(
            "cmd-n",
            NewDocument,
            Some("Viewer && !Input && !InlineText"),
        ),
        KeyBinding::new("ctrl-y", RedoEdit, Some("Viewer && !Input")),
        KeyBinding::new("ctrl-shift-z", RedoEdit, Some("Viewer && !Input")),
        KeyBinding::new(
            "cmd-d",
            DuplicateObject,
            Some("Viewer && !Input && !InlineText"),
        ),
        KeyBinding::new(
            "ctrl-d",
            DuplicateObject,
            Some("Viewer && !Input && !InlineText"),
        ),
        KeyBinding::new("cmd-m", AddSlide, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("ctrl-m", AddSlide, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new(
            "delete",
            DeleteObject,
            Some("Viewer && !Input && !InlineText"),
        ),
        KeyBinding::new(
            "ctrl-n",
            NewDocument,
            Some("Viewer && !Input && !InlineText"),
        ),
        KeyBinding::new("ctrl-q", CloseEditor, Some("Viewer")),
        KeyBinding::new("cmd-q", CloseEditor, Some("Viewer")),
        KeyBinding::new("cmd-o", Open, Some("Viewer")),
        KeyBinding::new("escape", Cancel, Some("Viewer")),
        KeyBinding::new("pageup", Previous, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("pagedown", Next, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("left", Previous, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("right", Next, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("home", First, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("end", Last, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("ctrl-=", ZoomIn, Some("Viewer && !Input")),
        KeyBinding::new("ctrl--", ZoomOut, Some("Viewer && !Input")),
        KeyBinding::new("ctrl-0", Fit, Some("Viewer && !Input")),
    ]);
}
#[cfg(target_os = "macos")]
pub(super) fn bind_native_menus(cx: &mut App) {
    cx.set_menus([
        Menu::new("JoyDeck").items([MenuItem::action("退出 JoyDeck", super::Quit)]),
        Menu::new("文件").items([
            MenuItem::action("新建", NewDocument),
            MenuItem::action("打开…", Open),
            MenuItem::action("保存", Save),
            MenuItem::action("另存为…", SaveAs),
            MenuItem::separator(),
            MenuItem::action("关闭", CloseEditor),
        ]),
        Menu::new("编辑").items([
            MenuItem::action("撤销", UndoEdit),
            MenuItem::action("重做", RedoEdit),
            MenuItem::separator(),
            MenuItem::action("创建对象副本", DuplicateObject),
            MenuItem::action("删除对象", DeleteObject),
            MenuItem::action("上移一层", MoveForward),
            MenuItem::action("下移一层", MoveBackward),
        ]),
        Menu::new("幻灯片").items([
            MenuItem::action("新建幻灯片", AddSlide),
            MenuItem::action("复制幻灯片", DuplicateSlides),
            MenuItem::action("删除幻灯片", DeleteSlides),
        ]),
    ]);
}

pub fn open_window(
    dark: bool,
    dimensions: (f32, f32),
    cx: &mut App,
) -> Result<(AnyWindowHandle, Entity<Viewer>)> {
    let result = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(dimensions.0), px(dimensions.1)),
                cx,
            ))),
            window_min_size: Some(size(px(800.), px(600.))),
            ..Default::default()
        },
        cx,
        move |w, cx| {
            w.set_window_title("JoyDeck");
            let view = cx.new(|cx| Viewer::new(dark, w, cx));
            view.update(cx, |s, cx| {
                s.install_notes(w, cx);
                s.install_font_picker(w, cx);
                s.refresh_recent(cx);
                cx.defer_in(w, |s, w, cx| s.focus.focus(w, cx));
                s.activation = Some(cx.observe_window_activation(w, |s, w, cx| {
                    if w.is_window_active() {
                        cx.set_global(ActiveEditor {
                            window: w.window_handle(),
                            view: cx.entity().downgrade(),
                        });
                    } else {
                        s.commit_notes(w, cx);
                        s.end_inline(cx);
                        if s.drag.take().is_some() {
                            s.message = "失焦，手势已取消".into();
                            cx.notify();
                        }
                    }
                }));
            });
            let weak = view.downgrade();
            w.on_window_should_close(cx, move |w, cx| {
                weak.update(cx, |s, cx| {
                    if s.composing()
                        || s.notes_input
                            .update(cx, |i, cx| i.marked_text_range(w, cx).is_some())
                    {
                        s.message = "请先完成或取消组合输入".into();
                        cx.notify();
                        return false;
                    }
                    s.commit_notes(w, cx);
                    if s.requires_save_prompt() || s.is_saving() {
                        s.request_close(&CloseEditor, w, cx);
                        false
                    } else {
                        true
                    }
                })
                .unwrap_or(true)
            });
            view
        },
    );
    if let Ok((handle, view)) = &result {
        cx.set_global(ActiveEditor {
            window: *handle,
            view: view.downgrade(),
        });
    }
    result
}
#[derive(Clone)]
struct ActiveEditor {
    window: AnyWindowHandle,
    view: WeakEntity<Viewer>,
}
impl Global for ActiveEditor {}

#[cfg(target_os = "macos")]
pub fn open_system_document(path: PathBuf, dark: bool, dimensions: (f32, f32), cx: &mut App) {
    if let Some(active) = cx.try_global::<ActiveEditor>().cloned()
        && matches!(
            active.window.update(cx, |_, w, cx| {
                w.activate_window();
                cx.activate(true);
                active.view.update(cx, |s, cx| {
                    if s.loading || s.source.as_deref() != Some(path.as_path()) {
                        s.open_path(path.clone(), w, cx);
                    }
                })
            }),
            Ok(Ok(()))
        )
    {
        return;
    }
    match open_window(dark, dimensions, cx) {
        Ok((_, view)) => {
            load_path(&view, path, cx);
            cx.activate(true);
        }
        Err(error) => eprintln!("JoyDeck could not open a window for the document: {error}"),
    }
}

pub fn quit(cx: &mut App) {
    if let Some(active) = cx.try_global::<ActiveEditor>().cloned() {
        let _ = active.window.update(cx, |_, w, cx| {
            active
                .view
                .update(cx, |s, cx| s.request_close(&CloseEditor, w, cx))
        });
    } else {
        cx.quit();
    }
}
pub fn new_document(view: &Entity<Viewer>, handle: AnyWindowHandle, cx: &mut App) {
    let _ = handle.update(cx, |_, w, cx| {
        view.update(cx, |s, cx| s.new_document(&NewDocument, w, cx))
    });
}
pub fn load_path(view: &Entity<Viewer>, path: PathBuf, cx: &mut App) {
    view.update(cx, |s, cx| s.load(path, cx));
}

struct Raster {
    width: u32,
    height: u32,
    bgra: Vec<u8>,
}
fn render_doc(
    deck: &Deck,
    page: usize,
    width: u32,
    cancel: &Arc<AtomicBool>,
) -> std::result::Result<Raster, String> {
    let scale = (f64::from(width) / deck.info.width_pt)
        .min(8.)
        .min(8192. / deck.info.height_pt)
        .min((16_000_000. / (deck.info.width_pt * deck.info.height_pt)).sqrt())
        .max(0.05);
    let mut r = deck.render(
        page,
        scale,
        &joydeck_shell::Cancellation::from_flag(cancel.clone()),
    )?;
    for pixel in r.pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    Ok(Raster {
        width: r.width,
        height: r.height,
        bgra: r.pixels,
    })
}

pub(super) fn deck_palette(dark: bool) -> joyoffice_ui::Palette {
    chrome::palette(dark)
}

impl Viewer {
    pub(super) fn document_name(&self) -> &str {
        let name = self
            .deck
            .as_ref()
            .map(|d| d.info.name.as_str())
            .unwrap_or("JoyDeck");
        if self.source.is_none() && name == "Untitled.joydeck" {
            "演示文稿1"
        } else {
            name
        }
    }
}
