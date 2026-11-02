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
        NewDocument,
        DeleteObject,
        CloseEditor,
        NextCell,
        PreviousCell
    ]
);
#[path = "editor_actions.rs"]
mod editing;
#[path = "inline_text.rs"]
mod inline;

type ImageKey = (u64, usize, u32);
pub struct Viewer {
    inline: Option<Box<inline::Inline>>,
    cell: Option<(ShapeId, usize, usize)>,
    notes_input: Entity<TextareaState>,
    editor: Option<joydeck_shell::editor::Editor>,
    save_input: Entity<InputState>,
    text_input: Entity<InputState>,
    save_task: Option<Task<()>>,
    checkpoint_task: Option<Task<()>>,
    recovery_rows: Option<Vec<joydeck_shell::storage::Entry>>,
    refresh_pending: bool,
    pending_save_path: Option<String>,
    saving: bool,
    close_after_save: bool,
    canvas_bounds: Bounds<Pixels>,
    drag: Option<editing::Drag>,
    activation: Option<Subscription>,
    dark: bool,
    input: Entity<InputState>,
    focus: FocusHandle,
    deck: Option<Arc<Deck>>,
    source: Option<PathBuf>,
    page: usize,
    zoom: Option<f32>,
    generation: u64,
    render_generation: u64,
    image: Option<(ImageKey, Arc<RenderImage>)>,
    pending: Option<ImageKey>,
    thumbnails: BTreeMap<usize, Arc<RenderImage>>,
    thumbnail_errors: BTreeSet<usize>,
    thumb_busy: bool,
    scroll: UniformListScrollHandle,
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
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("输入 .pptx / .joydeck / .deckcraft 路径")
        });
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            inline: None,
            cell: None,
            notes_input: cx.new(|cx| TextareaState::new(window, cx).auto_grow(3, 8)),
            editor: None,
            save_input: cx.new(|cx| {
                InputState::new(window, cx).placeholder("另存路径 .pptx / .joydeck（新文件）")
            }),
            text_input: cx.new(|cx| {
                InputState::new(window, cx).placeholder("选中文字对象后，在这里修改并应用")
            }),
            save_task: None,
            checkpoint_task: None,
            recovery_rows: None,
            refresh_pending: false,
            pending_save_path: None,
            saving: false,
            close_after_save: false,
            canvas_bounds: Bounds::default(),
            drag: None,
            activation: None,
            dark,
            input,
            focus,
            deck: None,
            source: None,
            page: 0,
            zoom: None,
            generation: 0,
            render_generation: 0,
            image: None,
            pending: None,
            thumbnails: BTreeMap::new(),
            thumbnail_errors: BTreeSet::new(),
            thumb_busy: false,
            scroll: UniformListScrollHandle::new(),
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
    fn open(&mut self, _: &Open, _: &mut Window, cx: &mut Context<Self>) {
        if self.block_replace(cx) {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("打开文稿".into()),
        });
        self.dialog_task = Some(cx.spawn(async move |view, cx| {
            let result = receiver.await;
            let _ = view.update(cx, |s, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(p) = paths.into_iter().next() {
                        s.load(p, cx);
                    }
                }
                Ok(Ok(None)) => {}
                _ => {
                    s.message = "文件选择器不可用；可在路径框输入文件路径".into();
                    cx.notify();
                }
            });
        }));
    }
    fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.block_replace(cx) {
            return;
        }
        self.end_inline(cx);
        self.cell = None;

        self.read_cancel.store(true, Ordering::Relaxed);
        let cancel = Arc::new(AtomicBool::new(false));
        self.read_cancel = cancel.clone();
        self.generation += 1;
        let generation = self.generation;
        self.loading = true;
        self.started = Instant::now();
        self.message = "正在后台读取、检查文稿… 可取消；当前文稿继续可浏览".into();
        self.read_task = Some(cx.spawn(async move |view, cx| {
            let source = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    if source == std::path::Path::new("@bundled") {
                        joydeck_shell::Document::from_bytes(
                            "r1-preview.deckcraft".into(),
                            include_bytes!("../../../tests/fixtures/joydeck/r1-preview.deckcraft")
                                .to_vec(),
                            &joydeck_shell::Cancellation::from_flag(cancel),
                        )
                        .and_then(|doc| {
                            joydeck_shell::editor::Editor::from_document(
                                doc,
                                joydeck_shell::editor::recovery_root(),
                            )
                        })
                    } else {
                        if cancel.load(Ordering::Relaxed) {
                            return Err("operation cancelled".into());
                        }
                        joydeck_shell::editor::Editor::open(
                            &source,
                            joydeck_shell::editor::recovery_root(),
                        )
                    }
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.generation != generation {
                    eprintln!("R3 stale_open_discarded=true");
                    return;
                }
                s.loading = false;
                match result {
                    Ok(editor) => {
                        let deck = match editor.display() {
                            Ok(d) => d,
                            Err(e) => {
                                s.message = e;
                                cx.notify();
                                return;
                            }
                        };
                        s.editor = Some(editor);
                        s.paint_cancel.store(true, Ordering::Relaxed);
                        s.thumbs_cancel.store(true, Ordering::Relaxed);
                        s.render_generation += 1;
                        let pages = deck.info.slides.len();
                        s.deck = Some(Arc::new(deck));
                        s.source = if path == std::path::Path::new("@bundled") {
                            None
                        } else {
                            Some(path.clone())
                        };
                        let default = if path
                            .extension()
                            .is_some_and(|e| e == "pptx" || e == "joydeck")
                        {
                            path.to_string_lossy().into_owned()
                        } else {
                            "".to_string()
                        };
                        s.pending_save_path = Some(default);
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
                        s.message = format!(
                            "读取完成 · {} 页 · {} ms · R3 编辑",
                            pages,
                            s.started.elapsed().as_millis()
                        );
                        eprintln!(
                            "R3 opened=true pages={} elapsed_ms={}",
                            pages,
                            s.started.elapsed().as_millis()
                        );
                    }
                    Err(error) => {
                        s.message = format!("打开失败：{error} · 当前文稿保持不变");
                        eprintln!("R3 open_failed=true reason={error}");
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.inline.is_some() {
            self.cancel_inline(cx);
            return;
        }
        if self.drag.take().is_some() {
            self.message = "手势已取消，未修改文稿".into();
            cx.notify();
            return;
        }
        if self.loading {
            self.read_cancel.store(true, Ordering::Relaxed);
            self.generation += 1;
            self.loading = false;
            self.read_task = None;
            self.message = "已取消读取 · 当前文稿保持不变".into();
            cx.notify();
        }
    }
    fn select(&mut self, page: usize, cx: &mut Context<Self>) {
        self.end_inline(cx);
        self.cell = None;
        if let Some(d) = &self.deck {
            let page = page.min(d.info.slides.len() - 1);
            if page != self.page {
                self.paint_cancel.store(true, Ordering::Relaxed);
                self.pending = None;
            }
            self.page = page;
            if let Some(editor) = &mut self.editor {
                let _ = editor.session.select(page, vec![]);
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
        ((f32::from(sz.width) - 288.).max(64.) / (d.info.width_pt as f32))
            .min((f32::from(sz.height) - 376.).max(64.) / (d.info.height_pt as f32))
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
                .title("兼容报告 · 只读预览")
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
        let key = (self.render_generation, self.page, width);
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
                if s.pending != Some(key) || s.render_generation != key.0 || s.page != key.1 {
                    return;
                }
                // Keep key until another request; render failure must not cause an infinite retry loop.
                match result {
                    Ok(r) => {
                        s.image = Some((key, r));
                        s.rasters += 1;
                        eprintln!(
                            "R3 raster page={} width={} elapsed_ms={} count={}",
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
                s.thumb_busy = false;
                cx.notify();
            });
        }));
    }
}
impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = if self.dark {
            joyoffice_ui::Palette::dark(joyoffice_ui::Brand::Deck)
        } else {
            joyoffice_ui::Palette::light(joyoffice_ui::Brand::Deck)
        };
        if let Some(path) = self.pending_save_path.take() {
            self.save_input
                .update(cx, |s, cx| s.set_value(path, window, cx));
        }
        if self.refresh_pending {
            self.refresh_pending = false;
            self.sync_editor(window, cx);
        }
        self.show_recoveries(window, cx);
        let scale = self.scale(window);
        if let Some(e) = &self.editor {
            window.set_window_title(&format!(
                "JoyDeck · R3 · {}{}",
                self.deck
                    .as_ref()
                    .map(|d| d.info.name.as_str())
                    .unwrap_or("Untitled"),
                if e.session.dirty() { " *" } else { "" }
            ));
        }
        let deck = self.deck.clone();
        if let Some(d) = &deck {
            self.request_image(
                ((d.info.width_pt as f32) * scale * window.scale_factor())
                    .ceil()
                    .clamp(1., 2048.) as u32,
                cx,
            );
        }
        let count = deck.as_ref().map_or(0, |d| d.info.slides.len());
        let warning_count = deck.as_ref().map_or(0, |d| d.info.warnings.len());
        let source = deck
            .as_ref()
            .map(|d| d.info.name.clone())
            .unwrap_or_else(|| "未打开文稿".into());
        let sz = window.viewport_size();
        let aw = (f32::from(sz.width) - 224.).max(64.);
        let ah = (f32::from(sz.height) - 312.).max(64.);
        let canvas = if let Some(d) = &deck {
            let (w, h) = (
                (d.info.width_pt as f32) * scale,
                (d.info.height_pt as f32) * scale,
            );
            let current = self.image.as_ref().filter(|(key, _)| {
                (key.0 == self.render_generation || self.inline.is_some()) && key.1 == self.page
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
                                .shadow_lg()
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
                                                s.paint_inline(bounds, scale, w, cx)
                                            });
                                            move |bounds, _, w, cx| paint(bounds, w, cx)
                                        },
                                    )
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .size_full(),
                                )
                                .children(self.selection_overlays(scale, palette.accent))
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
                .child("打开一份文稿，开始只读浏览")
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
        div()
            .id("viewer")
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
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(|s, _: &NextCell, w, cx| s.next_cell(false, w, cx)))
            .on_action(cx.listener(|s, _: &PreviousCell, w, cx| s.next_cell(true, w, cx)))
            .on_key_down(cx.listener(Self::inline_key))
            .on_action(cx.listener(Self::undo_edit))
            .on_action(cx.listener(Self::redo_edit))
            .on_action(cx.listener(Self::new_document))
            .on_action(cx.listener(Self::delete_object))
            .on_action(cx.listener(Self::request_close))
            .on_mouse_move(cx.listener(move |s, event, _, cx| s.pointer_move(event, scale, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |s, event, w, cx| s.pointer_up(event, scale, w, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |s, event, w, cx| s.pointer_up(event, scale, w, cx)),
            )
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
            .child(
                div()
                    .h(px(56.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(palette.border))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .text_color(rgb(palette.accent))
                                    .child("JoyDeck · R3"),
                            )
                            .child(div().text_sm().text_ellipsis().child(source))
                            .child(div().text_xs().text_color(rgb(palette.accent)).child(
                                if self.editor.as_ref().is_some_and(|e| e.session.dirty()) {
                                    "未保存"
                                } else {
                                    "无修改"
                                },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("open")
                                    .label("打开文稿")
                                    .on_click(cx.listener(|s, _, w, cx| s.open(&Open, w, cx))),
                            )
                            .child(
                                Button::new("theme")
                                    .label(if self.dark { "浅色" } else { "深色" })
                                    .on_click(cx.listener(|s, _, w, cx| {
                                        s.dark = !s.dark;
                                        crate::apply_theme(s.dark, Some(w), cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("speaker-notes")
                                    .label("备注")
                                    .disabled(count == 0)
                                    .on_click(cx.listener(|s, _, w, cx| s.edit_notes(w, cx))),
                            )
                            .child(
                                Button::new("compatibility")
                                    .label(format!("兼容报告 · {warning_count}"))
                                    .disabled(count == 0)
                                    .on_click(cx.listener(|s, _, w, cx| s.report(&Report, w, cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(52.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .child(div().flex_1().child(Input::new(&self.input)))
                    .child(
                        Button::new("open-path")
                            .label("读取路径")
                            .on_click(cx.listener(|s, _, w, cx| {
                                let path = s.input.read(cx).value().to_string();
                                if !path.trim().is_empty() {
                                    s.load(PathBuf::from(path.trim().trim_matches('"')), cx);
                                    s.focus.focus(w, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("cancel-read")
                            .label("取消读取")
                            .disabled(!self.loading)
                            .on_click(cx.listener(|s, _, w, cx| s.cancel(&Cancel, w, cx))),
                    ),
            )
            .child(self.edit_toolbar(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(224.))
                            .h_full()
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(rgb(palette.border))
                            .child(
                                uniform_list(
                                    "slide-list",
                                    count,
                                    cx.processor(move |s, range: std::ops::Range<usize>, _, cx| {
                                        s.request_thumbnails(range.clone(), cx);
                                        range
                                            .map(|p| {
                                                let title = s
                                                    .deck
                                                    .as_ref()
                                                    .map(|d| {
                                                        d.info.slides[p]
                                                            .title
                                                            .chars()
                                                            .take(80)
                                                            .collect::<String>()
                                                    })
                                                    .unwrap_or_default();
                                                div()
                                                    .id(p)
                                                    .h(px(144.))
                                                    .p_3()
                                                    .cursor_pointer()
                                                    .bg(rgb(if s.page == p {
                                                        palette.selected
                                                    } else {
                                                        palette.chrome
                                                    }))
                                                    .on_click(cx.listener(move |s, _, w, cx| {
                                                        s.select(p, cx);
                                                        s.focus.focus(w, cx);
                                                    }))
                                                    .child(
                                                        div()
                                                            .w(px(192.))
                                                            .h(px(108.))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .bg(rgb(palette.canvas))
                                                            .overflow_hidden()
                                                            .children(s.thumbnails.get(&p).map(
                                                                |i| {
                                                                    img(i.clone())
                                                                        .max_w_full()
                                                                        .max_h_full()
                                                                        .object_fit(
                                                                            ObjectFit::Contain,
                                                                        )
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        div().text_xs().text_ellipsis().child(
                                                            format!("{:02}  {}", p + 1, title),
                                                        ),
                                                    )
                                            })
                                            .collect::<Vec<_>>()
                                    }),
                                )
                                .track_scroll(&self.scroll)
                                .h_full(),
                            ),
                    )
                    .child(canvas),
            )
            .child(
                div()
                    .h(px(44.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .border_t_1()
                    .border_color(rgb(palette.border))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .overflow_hidden()
                            .child(self.message.clone()),
                    )
                    .child(
                        Button::new("previous")
                            .label("上一页")
                            .disabled(count == 0 || self.page == 0)
                            .on_click(cx.listener(|s, _, w, cx| s.previous(&Previous, w, cx))),
                    )
                    .child(format!(
                        "{} / {count}",
                        if count == 0 { 0 } else { self.page + 1 }
                    ))
                    .child(
                        Button::new("next")
                            .label("下一页")
                            .disabled(count == 0 || self.page + 1 >= count)
                            .on_click(cx.listener(|s, _, w, cx| s.next(&Next, w, cx))),
                    )
                    .child(
                        Button::new("zoom-out")
                            .label("−")
                            .disabled(count == 0)
                            .on_click(cx.listener(|s, _, w, cx| s.zoom_out(&ZoomOut, w, cx))),
                    )
                    .child(format!("{:.0}%", scale * 100.))
                    .child(
                        Button::new("zoom-in")
                            .label("+")
                            .disabled(count == 0)
                            .on_click(cx.listener(|s, _, w, cx| s.zoom_in(&ZoomIn, w, cx))),
                    )
                    .child(
                        Button::new("fit")
                            .label("适应窗口")
                            .disabled(count == 0)
                            .on_click(cx.listener(|s, _, w, cx| s.fit(&Fit, w, cx))),
                    ),
            )
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
        KeyBinding::new("cmd-s", Save, Some("Viewer")),
        KeyBinding::new("ctrl-z", UndoEdit, Some("Viewer && !Input")),
        KeyBinding::new("ctrl-y", RedoEdit, Some("Viewer && !Input")),
        KeyBinding::new("ctrl-shift-z", RedoEdit, Some("Viewer && !Input")),
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
        KeyBinding::new("ctrl-=", ZoomIn, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("ctrl--", ZoomOut, Some("Viewer && !Input && !InlineText")),
        KeyBinding::new("ctrl-0", Fit, Some("Viewer && !Input && !InlineText")),
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
            w.set_window_title("JoyDeck · R3 编辑器");
            let view = cx.new(|cx| Viewer::new(dark, w, cx));
            view.update(cx, |s, cx| {
                s.activation = Some(cx.observe_window_activation(w, |s, w, cx| {
                    if !w.is_window_active() {
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
                    if s.editor.as_ref().is_some_and(|e| e.session.dirty()) || s.saving {
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
