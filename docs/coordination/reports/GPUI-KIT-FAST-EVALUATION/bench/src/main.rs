//! Frame-cost benchmark for a JoyWord-like window, compiled against several
//! GPUI sources from one file. Scratch experiment only; not product code.
//!
//! Scenarios (one per process):
//!   blink   - only a caret entity toggles each frame
//!   type    - one document line changes each frame (reshape + repaint)
//!   scroll  - document scroll offset moves each frame
//!   refresh - window.refresh() each frame with no state change
//!
//! Output: one `RESULT {json}` line on stdout.

#[cfg(feature = "kit")]
extern crate gpui_kit as gpui;

use gpui::*;
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static GLYPHS_PAINTED: AtomicUsize = AtomicUsize::new(0);
static PAGE_PAINTS: AtomicUsize = AtomicUsize::new(0);
use std::time::{Duration, Instant};

#[cfg(all(feature = "split", not(feature = "kit")))]
fn application() -> Application {
    gpui_platform::application()
}
#[cfg(feature = "kit")]
fn application() -> Application {
    gpui_kit::application()
}
#[cfg(not(feature = "split"))]
fn application() -> Application {
    Application::new()
}

const VARIANT: &str = if cfg!(all(feature = "kit", feature = "fast")) {
    "kit+fast"
} else if cfg!(feature = "kit") {
    "kit"
} else if cfg!(feature = "fast") {
    "gpui-fast-0.1.8"
} else if cfg!(feature = "split") {
    "gpui-pre-0.3.9"
} else {
    "gpui-0.2.2"
};

fn pages() -> usize {
    static PAGES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *PAGES.get_or_init(|| {
        std::env::var("BENCH_PAGES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8)
    })
}
const LINES_PER_PAGE: usize = 40;
const PAGE_W: f32 = 635.0;
const PAGE_H: f32 = 898.0;
const MARGIN: f32 = 60.0;
const FONT_SIZE: f32 = 13.0;
const LINE_H: f32 = 19.5;
const FAMILY: &str = "PingFang SC";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Scenario {
    Blink,
    Type,
    Scroll,
    Refresh,
}

#[derive(Clone)]
struct Args {
    scenario: Scenario,
    warmup: usize,
    frames: usize,
    cached: bool,
    ime: bool,
    vsync: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        scenario: Scenario::Blink,
        warmup: 120,
        frames: 600,
        cached: false,
        ime: false,
        vsync: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--scenario" => {
                args.scenario = match it.next().as_deref() {
                    Some("blink") => Scenario::Blink,
                    Some("type") => Scenario::Type,
                    Some("scroll") => Scenario::Scroll,
                    Some("refresh") => Scenario::Refresh,
                    other => panic!("unknown scenario {other:?}"),
                }
            }
            "--frames" => args.frames = it.next().unwrap().parse().unwrap(),
            "--warmup" => args.warmup = it.next().unwrap().parse().unwrap(),
            "--cached" => args.cached = true,
            "--ime" => args.ime = true,
            "--vsync" => args.vsync = true,
            other => panic!("unknown arg {other}"),
        }
    }
    args
}

fn line_text(page: usize, line: usize) -> String {
    format!(
        "第{}页第{:02}行：JoyWord 帧开销基准，中英文混排 mixed CJK/Latin run {} 用于测量绘制。",
        page + 1,
        line + 1,
        page * LINES_PER_PAGE + line
    )
}

// ---------------------------------------------------------------- toolbar

struct Toolbar;

const TOOL_LABELS: [&str; 24] = [
    "粘贴", "剪切", "复制", "格式刷", "加粗", "斜体", "下划线", "删除线", "上标", "下标", "字体颜色",
    "高亮", "左对齐", "居中", "右对齐", "两端", "行距", "项目符号", "编号", "缩进+", "缩进-", "样式",
    "查找", "替换",
];
const TABS: [&str; 8] = ["文件", "开始", "插入", "设计", "布局", "引用", "审阅", "视图"];

impl Render for Toolbar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = div()
            .flex()
            .flex_row()
            .gap_2()
            .px_2()
            .children(TABS.iter().enumerate().map(|(i, t)| {
                div()
                    .id(("tab", i))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .hover(|s| s.bg(rgb(0xe6e6e6)))
                    .child(*t)
            }));
        let rows = (0..2).map(|row| {
            div()
                .flex()
                .flex_row()
                .gap_1()
                .px_2()
                .children(TOOL_LABELS.iter().enumerate().map(move |(i, label)| tool_button(row * 100 + i, label)))
        });
        div()
            .flex()
            .flex_col()
            .gap_1()
            .py_1()
            .bg(rgb(0xf7f7f7))
            .border_b_1()
            .border_color(rgb(0xdddddd))
            .child(tabs)
            .children(rows)
    }
}

#[cfg(not(feature = "kit"))]
fn tool_button(id: usize, label: &'static str) -> AnyElement {
    div()
        .id(("tool", id))
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(0xd0d0d0))
        .bg(rgb(0xffffff))
        .hover(|s| s.bg(rgb(0xe8f0fe)))
        .text_size(px(12.0))
        .child(label)
        .into_any_element()
}

#[cfg(feature = "kit")]
fn tool_button(id: usize, label: &'static str) -> AnyElement {
    use gpui_kit::component::Sizable;
    use gpui_kit::component::button::{Button, ButtonVariants};
    Button::new(("tool", id))
        .outline()
        .small()
        .label(label)
        .into_any_element()
}

// ---------------------------------------------------------------- sidebar

struct Sidebar;

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("outline")
            .w(px(220.0))
            .h_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .bg(rgb(0xfafafa))
            .border_r_1()
            .border_color(rgb(0xdddddd))
            .children((0..80usize).map(|i| {
                div()
                    .id(("outline-item", i))
                    .px_3()
                    .py_1()
                    .text_size(px(12.0))
                    .hover(|s| s.bg(rgb(0xeeeeee)))
                    .child(format!("{}. 标题 Heading {}", i + 1, i + 1))
            }))
    }
}

// ---------------------------------------------------------------- caret

struct Caret {
    visible: bool,
}

impl Render for Caret {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(MARGIN + 2.0))
            .left(px(MARGIN + 120.0))
            .w(px(2.0))
            .h(px(LINE_H))
            .bg(if self.visible {
                rgb(0x111111)
            } else {
                rgb(0xffffff)
            })
    }
}

// ---------------------------------------------------------------- document

#[derive(Clone)]
struct ShapedPageLine {
    glyphs: Vec<(Point<Pixels>, FontId, GlyphId)>,
}

struct Document {
    pages: Vec<Rc<Vec<ShapedPageLine>>>,
    focus: FocusHandle,
    ime: bool,
    caret: Entity<Caret>,
    scroll: ScrollHandle,
    typed: String,
    retype: bool,
}

fn shape(window: &Window, text: String) -> ShapedPageLine {
    let run = TextRun {
        len: text.len(),
        font: font(FAMILY),
        color: black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = window
        .text_system()
        .shape_line(text.into(), px(FONT_SIZE), &[run], None);
    let mut glyphs = Vec::new();
    for r in shaped.runs.iter() {
        for g in r.glyphs.iter() {
            glyphs.push((g.position, r.font_id, g.id));
        }
    }
    ShapedPageLine { glyphs }
}

impl Document {
    fn new(ime: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pages = (0..pages())
            .map(|p| {
                Rc::new(
                    (0..LINES_PER_PAGE)
                        .map(|l| shape(window, line_text(p, l)))
                        .collect(),
                )
            })
            .collect();
        let focus = cx.focus_handle();
        if ime {
            #[cfg(feature = "split")]
            window.focus(&focus, cx);
            #[cfg(not(feature = "split"))]
            window.focus(&focus);
        }
        Self {
            pages,
            focus,
            ime,
            caret: cx.new(|_| Caret { visible: true }),
            scroll: ScrollHandle::new(),
            typed: String::new(),
            retype: false,
        }
    }
}

fn page_canvas(
    lines: Rc<Vec<ShapedPageLine>>,
    input: Option<(FocusHandle, Entity<Document>)>,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            // Like JoyWord's canvas: the editing surface registers the IME
            // handler from inside the scrolled content.
            if let Some((focus, entity)) = input {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
            }
            let mask = window.content_mask().bounds;
            let baseline = px(FONT_SIZE);
            PAGE_PAINTS.fetch_add(1, Ordering::Relaxed);
            for (i, line) in lines.iter().enumerate() {
                let y = bounds.origin.y + px(MARGIN + i as f32 * LINE_H);
                // Paint only lines intersecting the visible mask.
                if y + px(LINE_H) < mask.origin.y || y > mask.origin.y + mask.size.height {
                    continue;
                }
                let origin_x = bounds.origin.x + px(MARGIN);
                for (pos, font_id, glyph_id) in line.glyphs.iter() {
                    let origin = point(origin_x + pos.x, y + baseline + pos.y);
                    let _ = window.paint_glyph(origin, *font_id, *glyph_id, px(FONT_SIZE), black());
                    GLYPHS_PAINTED.fetch_add(1, Ordering::Relaxed);
                }
            }
        },
    )
    .size_full()
}

impl Render for Document {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.retype {
            self.retype = false;
            let text = format!("{}{}", line_text(0, 0), self.typed);
            Rc::make_mut(&mut self.pages[0])[0] = shape(window, text);
        }
        let caret = self.caret.clone();
        let input = self
            .ime
            .then(|| (self.focus.clone(), cx.entity()));
        div()
            .id("document")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .bg(rgb(0xe9e9ef))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .py_4()
                    .children(self.pages.iter().enumerate().map(|(i, lines)| {
                        let page = div()
                            .relative()
                            .w(px(PAGE_W))
                            .h(px(PAGE_H))
                            .flex_none()
                            .bg(rgb(0xffffff))
                            .shadow_sm()
                            .child(page_canvas(
                                lines.clone(),
                                if i == 0 { input.clone() } else { None },
                            ));
                        if i == 0 {
                            page.child(caret.clone()).into_any_element()
                        } else {
                            page.into_any_element()
                        }
                    })),
            )
    }
}

// Minimal handler: the benchmark only needs it registered, not driven.
impl EntityInputHandler for Document {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        Some(self.typed.clone())
    }
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }
    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        None
    }
    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        _text: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        _new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }
    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }
    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(0)
    }
}

// ---------------------------------------------------------------- shell

struct Shell {
    args: Args,
    toolbar: Entity<Toolbar>,
    sidebar: Entity<Sidebar>,
    document: Entity<Document>,
    started: Instant,
    frame: usize,
    first_frame_ms: Option<f64>,
    last_wall: Option<Instant>,
    last_thread: Option<cpu_time::ThreadTime>,
    t0_wall: Option<Instant>,
    t0_proc: Option<cpu_time::ProcessTime>,
    intervals_ms: Vec<f64>,
    thread_cpu_ms: Vec<f64>,
}

impl Shell {
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::env::var_os("BENCH_DEBUG").is_some() && self.frame % 30 == 0 {
            eprintln!("tick {}", self.frame);
        }
        let now = Instant::now();
        let thread_now = cpu_time::ThreadTime::now();
        if self.first_frame_ms.is_none() {
            self.first_frame_ms = Some(self.started.elapsed().as_secs_f64() * 1e3);
        }
        let warmup = self.args.warmup;
        if self.frame == warmup {
            self.t0_wall = Some(now);
            self.t0_proc = Some(cpu_time::ProcessTime::now());
        } else if self.frame > warmup {
            let last_wall = self.last_wall.unwrap();
            let last_thread = self.last_thread.unwrap();
            self.intervals_ms
                .push((now - last_wall).as_secs_f64() * 1e3);
            self.thread_cpu_ms.push(
                thread_now
                    .as_duration()
                    .saturating_sub(last_thread.as_duration())
                    .as_secs_f64()
                    * 1e3,
            );
        }
        self.last_wall = Some(now);
        self.last_thread = Some(thread_now);

        if self.frame >= warmup + self.args.frames {
            self.finish(cx);
            return;
        }
        self.frame += 1;
        self.mutate(window, cx);
        cx.on_next_frame(window, |this, window, cx| this.tick(window, cx));
    }

    fn mutate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.args.scenario {
            Scenario::Blink => self.document.update(cx, |doc, cx| {
                doc.caret.update(cx, |c, cx| {
                    c.visible = !c.visible;
                    cx.notify();
                })
            }),
            Scenario::Type => self.document.update(cx, |doc, cx| {
                if doc.typed.chars().count() > 12 {
                    doc.typed.clear();
                }
                doc.typed.push('字');
                doc.retype = true;
                cx.notify();
            }),
            Scenario::Scroll => self.document.update(cx, |doc, cx| {
                let max = px((PAGE_H + 16.0) * (pages() as f32 - 1.5));
                let mut y = -doc.scroll.offset().y + px(7.0);
                if y > max {
                    y = px(0.0);
                }
                doc.scroll.set_offset(point(px(0.0), -y));
                cx.notify();
            }),
            Scenario::Refresh => window.refresh(),
        }
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let wall = self.t0_wall.unwrap().elapsed();
        let proc_cpu = self.t0_proc.unwrap().elapsed();
        let n = self.thread_cpu_ms.len().max(1) as f64;
        let mut sorted = self.thread_cpu_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pct = |p: f64| sorted[((sorted.len() as f64 - 1.0) * p).round() as usize];
        let mean_cpu = self.thread_cpu_ms.iter().sum::<f64>() / n;
        let mean_interval = self.intervals_ms.iter().sum::<f64>() / n;
        let mut iv = self.intervals_ms.clone();
        iv.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let iv_p95 = iv[((iv.len() as f64 - 1.0) * 0.95).round() as usize];
        println!(
            "RESULT {{\"variant\":\"{}\",\"scenario\":\"{:?}\",\"cached\":{},\"retention_env\":\"{}\",\"layers_env\":\"{}\",\"ime\":{},\"frames\":{},\"first_frame_ms\":{:.1},\"wall_s\":{:.3},\"mean_interval_ms\":{:.3},\"p95_interval_ms\":{:.3},\"main_cpu_mean_ms\":{:.4},\"main_cpu_p50_ms\":{:.4},\"main_cpu_p95_ms\":{:.4},\"main_cpu_max_ms\":{:.4},\"proc_cpu_per_frame_ms\":{:.4}}}",
            VARIANT,
            self.args.scenario,
            self.args.cached,
            std::env::var("GPUI_VIEW_RETENTION").unwrap_or_default(),
            std::env::var("GPUI_SCROLL_LAYERS").unwrap_or_default(),
            self.args.ime,
            self.thread_cpu_ms.len(),
            self.first_frame_ms.unwrap_or(0.0),
            wall.as_secs_f64(),
            mean_interval,
            iv_p95,
            mean_cpu,
            pct(0.5),
            pct(0.95),
            sorted.last().copied().unwrap_or(0.0),
            proc_cpu.as_secs_f64() * 1e3 / n,
        );
        cx.quit();
    }
}

fn wrap(view: AnyView, cached: bool, style: StyleRefinement) -> AnyElement {
    if cached {
        view.cached(style).into_any_element()
    } else {
        view.into_any_element()
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if std::env::var_os("BENCH_DEBUG").is_some() && self.frame < 3 {
            eprintln!("shell render frame={}", self.frame);
        }
        let cached = self.args.cached;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x222222))
            .font_family(FAMILY)
            .text_size(px(13.0))
            .child(wrap(
                self.toolbar.clone().into(),
                cached,
                StyleRefinement::default().w_full(),
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(wrap(
                        self.sidebar.clone().into(),
                        cached,
                        StyleRefinement::default().h_full(),
                    ))
                    .child(wrap(
                        self.document.clone().into(),
                        cached,
                        StyleRefinement::default().flex_1().h_full(),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .px_3()
                    .py_1()
                    .bg(rgb(0xf0f0f0))
                    .text_size(px(11.0))
                    .child(format!("第 1 页，共 {} 页", pages()))
                    .child("字数 12345")
                    .child("中文(中国)")
                    .child("100%")
                    .child(VARIANT),
            )
    }
}

fn clear_arena(arena: ArenaClearNeeded, _cx: &mut App) {
    #[cfg(feature = "split")]
    arena.clear(_cx);
    #[cfg(not(feature = "split"))]
    arena.clear();
}

/// Calls the public `Window::draw` once per frame after a separate update that
/// applies the scenario's change, timing only the draw on the main thread.
/// Excludes GPU encoding/present; independent of display vsync or screen lock.
fn drive_direct(window: AnyWindowHandle, shell: Entity<Shell>, args: Args, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let total = args.warmup + args.frames;
        let mut cpu_ms = Vec::with_capacity(args.frames);
        let mut wall_ms = Vec::with_capacity(args.frames);
        let mut proc0 = None;
        let mut wall0 = None;
        for i in 0..total {
            if i == args.warmup {
                proc0 = Some(cpu_time::ProcessTime::now());
                wall0 = Some(Instant::now());
                GLYPHS_PAINTED.store(0, Ordering::Relaxed);
                PAGE_PAINTS.store(0, Ordering::Relaxed);
            }
            let _ = window.update(cx, |_, window, cx| {
                shell.update(cx, |s, cx| s.mutate(window, cx))
            });
            let w = Instant::now();
            let c = cpu_time::ThreadTime::now();
            let _ = window.update(cx, |_, window, cx| {
                let arena = window.draw(cx);
                clear_arena(arena, cx);
            });
            let cpu = c.elapsed().as_secs_f64() * 1e3;
            let wall = w.elapsed().as_secs_f64() * 1e3;
            if i >= args.warmup {
                cpu_ms.push(cpu);
                wall_ms.push(wall);
            }
        }
        let proc_ms = proc0.unwrap().elapsed().as_secs_f64() * 1e3;
        let glyphs_per_frame = GLYPHS_PAINTED.load(Ordering::Relaxed) as f64 / args.frames as f64;
        let page_paints_per_frame = PAGE_PAINTS.load(Ordering::Relaxed) as f64 / args.frames as f64;
        let loop_s = wall0.unwrap().elapsed().as_secs_f64();
        let n = cpu_ms.len() as f64;
        let stats = |v: &Vec<f64>| {
            let mut s = v.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p = |q: f64| s[((s.len() as f64 - 1.0) * q).round() as usize];
            (v.iter().sum::<f64>() / n, p(0.5), p(0.95), *s.last().unwrap())
        };
        let (cm, c50, c95, cmax) = stats(&cpu_ms);
        let (wm, w50, w95, _) = stats(&wall_ms);
        println!(
            "RESULT {{\"mode\":\"direct\",\"variant\":\"{}\",\"scenario\":\"{:?}\",\"cached\":{},\"ime\":{},\"retention_env\":\"{}\",\"layers_env\":\"{}\",\"pages\":{},\"frames\":{},\"draw_cpu_mean_ms\":{:.4},\"draw_cpu_p50_ms\":{:.4},\"draw_cpu_p95_ms\":{:.4},\"draw_cpu_max_ms\":{:.4},\"draw_wall_mean_ms\":{:.4},\"draw_wall_p50_ms\":{:.4},\"draw_wall_p95_ms\":{:.4},\"proc_cpu_per_frame_ms\":{:.4},\"loop_s\":{:.3},\"glyphs_painted_per_frame\":{:.1},\"page_paints_per_frame\":{:.2}}}",
            VARIANT,
            args.scenario,
            args.cached,
            args.ime,
            std::env::var("GPUI_VIEW_RETENTION").unwrap_or_default(),
            std::env::var("GPUI_SCROLL_LAYERS").unwrap_or_default(),
            pages(),
            cpu_ms.len(),
            cm, c50, c95, cmax, wm, w50, w95,
            proc_ms / n,
            loop_s,
            glyphs_per_frame,
            page_paints_per_frame,
        );
        let _ = cx.update(|cx| cx.quit());
    })
    .detach();
}

fn main() {
    let started = Instant::now();
    let args = parse_args();
    application().run(move |cx: &mut App| {
        #[cfg(feature = "kit")]
        gpui_kit::init(cx);
        let bounds = Bounds::new(point(px(40.0), px(60.0)), size(px(1100.0), px(760.0)));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: std::env::var("BENCH_FOCUS").is_ok_and(|v| v == "1"),
            show: true,
            kind: match std::env::var("BENCH_KIND").as_deref() {
                Ok("normal") => WindowKind::Normal,
                Ok("floating") => WindowKind::Floating,
                _ => WindowKind::PopUp,
            },
            #[cfg(feature = "split")]
            inactive_frame_interval: None,
            ..Default::default()
        };
        let args2 = args.clone();
        let build = move |window: &mut Window, cx: &mut App| {
            cx.new(|cx: &mut Context<Shell>| {
                let ime = args2.ime;
                let document = cx.new(|cx| Document::new(ime, window, cx));
                let shell = Shell {
                    args: args2,
                    toolbar: cx.new(|_| Toolbar),
                    sidebar: cx.new(|_| Sidebar),
                    document,
                    started,
                    frame: 0,
                    first_frame_ms: None,
                    last_wall: None,
                    last_thread: None,
                    t0_wall: None,
                    t0_proc: None,
                    intervals_ms: Vec::new(),
                    thread_cpu_ms: Vec::new(),
                };
                if shell.args.vsync {
                    cx.on_next_frame(window, |this: &mut Shell, window, cx| this.tick(window, cx));
                }
                shell
            })
        };
        #[cfg(feature = "kit")]
        let (any_window, shell) = gpui_kit::open_window(options, cx, build).expect("open window");
        #[cfg(not(feature = "kit"))]
        let (any_window, shell) = {
            let handle = cx.open_window(options, build).expect("open window");
            let shell = handle.update(cx, |_, _, cx| cx.entity()).expect("shell entity");
            (AnyWindowHandle::from(handle), shell)
        };
        if !args.vsync {
            drive_direct(any_window, shell, args.clone(), cx);
        }
        if std::env::var_os("BENCH_DEBUG").is_some() {
            eprintln!("window opened");
        }
        let _ = Duration::ZERO;
    });
}
