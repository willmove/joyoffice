use gpui_kit::base::Disableable;
use gpui_kit::component::{
    WindowExt,
    button::Button,
    input::{Input, InputState},
};
use gpui_kit::*;
use joydeck::{
    document::{self, Deck},
    preview,
};
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
        Open, Cancel, Previous, Next, First, Last, ZoomIn, ZoomOut, Fit, Report
    ]
);
type ImageKey = (u64, usize, u32);
pub struct Viewer {
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
fn image_of(r: preview::Raster) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_raw(r.width, r.height, r.bgra).expect("validated raster dimensions"),
    )]))
}
impl Viewer {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("输入 .joydeck / .pptx 路径"));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
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
            message: "选择本地文稿开始浏览 · 支持 .joydeck v1 / PPTX 预览子集".into(),
            rasters: 0,
            started: Instant::now(),
        }
    }
    fn open(&mut self, _: &Open, _: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("打开只读文稿".into()),
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
                .spawn(async move { document::open(&source, &cancel) })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.generation != generation {
                    eprintln!("M1 stale_open_discarded=true");
                    return;
                }
                s.loading = false;
                match result {
                    Ok(mut deck) => {
                        deck.warn(
                            "font-layout",
                            None,
                            "",
                            "使用系统字体和简化文字排版；未保证文稿指定字体、自动换行与富文本保真",
                        );
                        s.paint_cancel.store(true, Ordering::Relaxed);
                        s.thumbs_cancel.store(true, Ordering::Relaxed);
                        s.render_generation += 1;
                        s.deck = Some(Arc::new(deck));
                        s.source = Some(path);
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
                            "读取完成 · {} 页 · {} ms · 只读",
                            s.deck.as_ref().unwrap().slides.len(),
                            s.started.elapsed().as_millis()
                        );
                        eprintln!(
                            "M1 opened=true pages={} elapsed_ms={}",
                            s.deck.as_ref().unwrap().slides.len(),
                            s.started.elapsed().as_millis()
                        );
                    }
                    Err(error) => {
                        s.message = format!("打开失败：{error} · 当前文稿保持不变");
                        eprintln!("M1 open_failed=true reason={error}");
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
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
        if let Some(d) = &self.deck {
            let page = page.min(d.slides.len() - 1);
            if page != self.page {
                self.paint_cancel.store(true, Ordering::Relaxed);
                self.pending = None;
            }
            self.page = page;
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
            self.select(d.slides.len() - 1, cx);
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
        ((f32::from(sz.width) - 288.).max(64.) / d.width)
            .min((f32::from(sz.height) - 216.).max(64.) / d.height)
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
        let total = self.deck.as_ref().map_or(0, |d| d.warnings.len());
        let rows: Vec<String> = self
            .deck
            .as_ref()
            .map(|d| {
                d.warnings
                    .iter()
                    .take(200)
                    .map(|w| {
                        format!(
                            "[{}] {} · {}\n{}",
                            w.code,
                            w.page
                                .map(|p| format!("第 {} 页", p + 1))
                                .unwrap_or_else(|| "整份文稿".into()),
                            w.part,
                            w.message
                        )
                    })
                    .collect()
            })
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
                .spawn(async move { preview::render(&deck, key.1, key.2, &cancel) })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.pending != Some(key) || s.render_generation != key.0 || s.page != key.1 {
                    return;
                }
                // Keep key until another request; render failure must not cause an infinite retry loop.
                match result {
                    Ok(r) => {
                        s.image = Some((key, image_of(r)));
                        s.rasters += 1;
                        eprintln!(
                            "M1 raster page={} width={} elapsed_ms={} count={}",
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
                        out.push((p, preview::render(&deck, p, 176, &cancel)));
                    }
                    out
                })
                .await;
            let _ = view.update(cx, |s, cx| {
                if s.render_generation != generation {
                    return;
                }
                for (p, r) in result {
                    let Ok(r) = r else {
                        s.thumbnail_errors.insert(p);
                        continue;
                    };
                    if s.thumbnails.len() >= 32
                        && let Some(key) =
                            s.thumbnails.keys().copied().max_by_key(|k| k.abs_diff(p))
                    {
                        s.thumbnails.remove(&key);
                    }
                    s.thumbnails.insert(p, image_of(r));
                }
                s.thumb_busy = false;
                cx.notify();
            });
        }));
    }
}
impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = self.scale(window);
        let deck = self.deck.clone();
        if let Some(d) = &deck {
            self.request_image(
                (d.width * scale * window.scale_factor())
                    .ceil()
                    .clamp(1., 2048.) as u32,
                cx,
            );
        }
        let count = deck.as_ref().map_or(0, |d| d.slides.len());
        let warning_count = deck.as_ref().map_or(0, |d| d.warnings.len());
        let source = self
            .source
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "未打开文稿".into());
        let sz = window.viewport_size();
        let aw = (f32::from(sz.width) - 224.).max(64.);
        let ah = (f32::from(sz.height) - 152.).max(64.);
        let canvas = if let Some(d) = &deck {
            let (w, h) = (d.width * scale, d.height * scale);
            let current = self
                .image
                .as_ref()
                .filter(|(key, _)| key.0 == self.render_generation && key.1 == self.page);
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
                .bg(rgb(0x101923))
                .child(
                    div()
                        .w(px((w + 64.).max(aw)))
                        .h(px((h + 64.).max(ah)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .w(px(w))
                                .h(px(h))
                                .bg(rgb(0xffffff))
                                .shadow_lg()
                                .overflow_hidden()
                                .children(current.map(|(_, i)| img(i.clone()).w(px(w)).h(px(h)))),
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
                .bg(rgb(0x101923))
                .child(div().text_3xl().child("JoyDeck"))
                .child("打开一份文稿，开始只读浏览")
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x93a4b7))
                        .child(".joydeck v1  /  PPTX · 本地读取 · 源文件不变"),
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
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(0x182330))
            .text_color(rgb(0xe7edf5))
            .font_family(if cfg!(target_os = "windows") {
                "Microsoft YaHei"
            } else {
                "sans-serif"
            })
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
                    .border_color(rgb(0x304154))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .items_center()
                            .child(div().text_lg().child("JoyDeck"))
                            .child(div().text_sm().child(source))
                            .child(div().text_xs().text_color(rgb(0x54d8bd)).child("只读预览")),
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
                            .border_color(rgb(0x304154))
                            .child(
                                uniform_list(
                                    "slide-list",
                                    count,
                                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                                        s.request_thumbnails(range.clone(), cx);
                                        range
                                            .map(|p| {
                                                let title = s
                                                    .deck
                                                    .as_ref()
                                                    .map(|d| {
                                                        d.slides[p]
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
                                                        0x274455
                                                    } else {
                                                        0x182330
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
                                                            .bg(rgb(0x293a4c))
                                                            .overflow_hidden()
                                                            .children(s.thumbnails.get(&p).map(
                                                                |i| {
                                                                    img(i.clone())
                                                                        .max_w_full()
                                                                        .max_h_full()
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        div().text_xs().overflow_hidden().child(
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
                    .border_color(rgb(0x304154))
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
        KeyBinding::new("ctrl-o", Open, Some("Viewer")),
        KeyBinding::new("escape", Cancel, Some("Viewer")),
        KeyBinding::new("pageup", Previous, Some("Viewer && !Input")),
        KeyBinding::new("pagedown", Next, Some("Viewer && !Input")),
        KeyBinding::new("left", Previous, Some("Viewer && !Input")),
        KeyBinding::new("right", Next, Some("Viewer && !Input")),
        KeyBinding::new("home", First, Some("Viewer && !Input")),
        KeyBinding::new("end", Last, Some("Viewer && !Input")),
        KeyBinding::new("ctrl-=", ZoomIn, Some("Viewer && !Input")),
        KeyBinding::new("ctrl--", ZoomOut, Some("Viewer && !Input")),
        KeyBinding::new("ctrl-0", Fit, Some("Viewer && !Input")),
    ]);
}
pub fn open_window(cx: &mut App) -> Result<(AnyWindowHandle, Entity<Viewer>)> {
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1180.), px(800.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(520.))),
            ..Default::default()
        },
        cx,
        |w, cx| {
            w.set_window_title("JoyDeck — M1 Read-only Viewer");
            cx.new(|cx| Viewer::new(w, cx))
        },
    )
}
pub fn load_path(view: &Entity<Viewer>, path: PathBuf, cx: &mut App) {
    view.update(cx, |s, cx| s.load(path, cx));
}

/// Drives production actions on a real native window; does not simulate physical keys.
pub fn smoke(handle: AnyWindowHandle, view: &Entity<Viewer>, cx: &mut App) {
    let fixtures = std::env::var_os("JOYDECK_FIXTURES")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("fixtures")))
                .filter(|p| p.is_dir())
        })
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/joydeck")
        });
    load_path(view, fixtures.join("native-20.joydeck"), cx);
    let view = view.downgrade();
    cx.spawn(async move |cx| {
        let start = Instant::now();
        let mut phase = 0;
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(60))
                .await;
            if start.elapsed().as_secs() > 30 {
                eprintln!("M1_SMOKE failed=true timeout_phase={phase}");
                break;
            }
            let result = handle.update(cx, |_, w, cx| {
                view.update(cx, |s, cx| {
                    let ready = s.image.as_ref().is_some_and(|(k, _)| {
                        k.0 == s.render_generation && k.1 == s.page && s.pending == Some(*k)
                    });
                    match phase {
                        0 if !s.loading && ready => {
                            assert_eq!(s.deck.as_ref().unwrap().slides.len(), 20);
                            eprintln!("M1_SMOKE native_open=true");
                            w.dispatch_action(Box::new(Next), cx);
                            phase = 1;
                        }
                        1 if s.page == 1 && ready => {
                            eprintln!("M1_SMOKE native_next=true");
                            w.dispatch_action(Box::new(Last), cx);
                            phase = 2;
                        }
                        2 if s.page == 19 && ready => {
                            eprintln!("M1_SMOKE last_page=true");
                            w.dispatch_action(Box::new(First), cx);
                            phase = 3;
                        }
                        3 if s.page == 0 && ready => {
                            s.select(2, cx);
                            s.request_image(640, cx);
                            s.select(0, cx);
                            phase = 4;
                        }
                        4 if ready => {
                            s.select(2, cx);
                            phase = 5;
                        }
                        5 if s.page == 2 && ready => {
                            eprintln!("M1_SMOKE rapid_navigation_no_stale=true");
                            for _ in 0..20 {
                                s.zoom_in(&ZoomIn, w, cx);
                            }
                            phase = 6;
                        }
                        6 if ready
                            && s.zoom == Some(4.)
                            && s.image.as_ref().unwrap().0.2 == 2048 =>
                        {
                            eprintln!("M1_SMOKE zoom_max=true raster_bounded=true");
                            for _ in 0..20 {
                                s.zoom_out(&ZoomOut, w, cx);
                            }
                            phase = 7;
                        }
                        7 if ready
                            && s.zoom == Some(0.25)
                            && s.image.as_ref().unwrap().0.2 < 2048 =>
                        {
                            eprintln!("M1_SMOKE zoom_min=true");
                            s.fit(&Fit, w, cx);
                            w.resize(size(px(940.), px(680.)));
                            phase = 8;
                        }
                        8 if ready && s.zoom.is_none() && s.scale(w) < 1. => {
                            eprintln!("M1_SMOKE fit_resize=true");
                            let old = s.deck.clone().unwrap();
                            s.load(fixtures.join("public-format-20.pptx"), cx);
                            s.cancel(&Cancel, w, cx);
                            assert!(Arc::ptr_eq(&old, s.deck.as_ref().unwrap()));
                            assert!(!s.loading);
                            eprintln!("M1_SMOKE cancel_preserves_document=true");
                            s.load(fixtures.join("native-20.joydeck"), cx);
                            s.load(fixtures.join("public-format-20.pptx"), cx);
                            phase = 9;
                        }
                        9 if !s.loading
                            && ready
                            && s.source
                                .as_ref()
                                .is_some_and(|p| p.extension().is_some_and(|e| e == "pptx")) =>
                        {
                            let d = s.deck.as_ref().unwrap();
                            assert_eq!(d.slides.len(), 20);
                            assert!(d.slides[0].title.contains("20"));
                            eprintln!("M1_SMOKE pptx_open_order=true");
                            w.dispatch_action(Box::new(Next), cx);
                            phase = 10;
                        }
                        10 if s.page == 1 && ready => {
                            assert!(
                                s.deck
                                    .as_ref()
                                    .unwrap()
                                    .warnings
                                    .iter()
                                    .any(|w| w.page == Some(1))
                            );
                            s.report(&Report, w, cx);
                            assert!(w.has_active_dialog(cx));
                            w.close_dialog(cx);
                            eprintln!("M1_SMOKE pptx_next_report=true");
                            s.load(fixtures.join("broken.pptx"), cx);
                            phase = 11;
                        }
                        11 if !s.loading && s.message.starts_with("打开失败") => {
                            assert_eq!(s.page, 1);
                            assert_eq!(s.deck.as_ref().unwrap().slides.len(), 20);
                            assert!(
                                s.source
                                    .as_ref()
                                    .unwrap()
                                    .ends_with("public-format-20.pptx")
                            );
                            eprintln!("M1_SMOKE failure_preserves_document=true");
                            assert!(s.thumbnails.len() <= 32);
                            eprintln!(
                                "M1_SMOKE passed=true native_pages=20 pptx_pages=20 elapsed_ms={}",
                                start.elapsed().as_millis()
                            );
                            phase = 12;
                        }
                        _ => {}
                    }
                    phase == 12
                })
            });
            match result {
                Ok(Ok(true)) => break,
                Ok(Ok(false)) => {}
                _ => {
                    eprintln!("M1_SMOKE failed=true window_closed");
                    break;
                }
            }
        }
    })
    .detach();
}
