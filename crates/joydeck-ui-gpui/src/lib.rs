#![forbid(unsafe_code)]
//! Native JoyDeck workspace; product state remains in the headless shell.
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, Theme, ThemeMode, WindowExt,
    button::{Button, ButtonVariants},
    input::{Textarea, TextareaState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use joydeck_shell::{PreviewDeck, PreviewSlide, WorkspaceState};
use joyoffice_ui::{Brand, Palette, metrics};
use std::{cell::RefCell, rc::Rc};

mod layout;
mod viewer;
use layout::WorkspaceLayout;

actions!(
    joydeck,
    [
        Quit,
        NextSlide,
        PreviousSlide,
        ZoomIn,
        ZoomOut,
        FitSlide,
        ToggleTheme,
        ToggleInspector,
        ToggleNotes,
        About
    ]
);

struct Workspace {
    preview: PreviewDeck,
    state: WorkspaceState,
    tab: &'static str,
    focus: FocusHandle,
    notes_input: Entity<TextareaState>,
}

impl Workspace {
    fn new(preview: PreviewDeck, dark: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let mut state = WorkspaceState::new();
        state.dark = dark;
        Self {
            preview,
            state,
            tab: "开始",
            focus,
            notes_input: cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 5)),
        }
    }
    fn palette(&self) -> Palette {
        if self.state.dark {
            Palette::dark(Brand::Deck)
        } else {
            Palette::light(Brand::Deck)
        }
    }
    fn next(&mut self, _: &NextSlide, _: &mut Window, cx: &mut Context<Self>) {
        self.state.step(1, self.preview.slides.len());
        cx.notify();
    }
    fn previous(&mut self, _: &PreviousSlide, _: &mut Window, cx: &mut Context<Self>) {
        self.state.step(-1, self.preview.slides.len());
        cx.notify();
    }
    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.state.zoom_by(1.25);
        cx.notify();
    }
    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.state.zoom_by(0.8);
        cx.notify();
    }
    fn fit(&mut self, _: &FitSlide, _: &mut Window, cx: &mut Context<Self>) {
        self.state.zoom = 1.0;
        cx.notify();
    }
    fn theme(&mut self, _: &ToggleTheme, window: &mut Window, cx: &mut Context<Self>) {
        self.state.dark = !self.state.dark;
        apply_theme(self.state.dark, Some(window), cx);
        cx.notify();
    }
    fn inspector(&mut self, _: &ToggleInspector, _: &mut Window, cx: &mut Context<Self>) {
        self.state.inspector = !self.state.inspector;
        cx.notify();
    }
    fn notes(&mut self, _: &ToggleNotes, _: &mut Window, cx: &mut Context<Self>) {
        self.state.notes = !self.state.notes;
        cx.notify();
    }
    fn about(&mut self, _: &About, window: &mut Window, cx: &mut Context<Self>) {
        window.open_dialog(cx, |dialog, _, _| {
            dialog.title("关于 JoyDeck").child(div().flex().flex_col().gap_3()
                .child("JoyDeck 0.1.0 · R0 原生技术预览")
                .child("Rust 1.98.1 · GPUI Kit 0.8.1 · GPUI pre 0.3.10")
                .child("复用 JoyOffice 公共 UI。当前提供原创示例浏览、缩放、主题与临时备注输入；文稿导入和编辑正在迁移。"))
        });
    }
    fn header(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        div()
            .h(px(metrics::HEADER))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(p.header))
            .border_b_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .rounded(px(7.))
                            .px_2()
                            .py_1()
                            .bg(rgb(p.accent))
                            .text_color(rgb(p.on_accent))
                            .font_weight(FontWeight::BOLD)
                            .child("JD"),
                    )
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("JoyDeck"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(p.muted))
                            .child(self.preview.title.clone()),
                    )
                    .child(
                        div()
                            .rounded_full()
                            .px_2()
                            .py_1()
                            .text_size(px(10.))
                            .bg(rgb(p.selected))
                            .text_color(rgb(p.accent))
                            .child("技术预览 R0"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("header-theme")
                            .small()
                            .ghost()
                            .label(if self.state.dark { "浅色" } else { "深色" })
                            .on_click(cx.listener(|s, _, w, cx| s.theme(&ToggleTheme, w, cx))),
                    )
                    .child(
                        Button::new("header-about")
                            .small()
                            .ghost()
                            .label("关于")
                            .on_click(cx.listener(|s, _, w, cx| s.about(&About, w, cx))),
                    ),
            )
    }
    fn tabs(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        div()
            .flex()
            .flex_shrink_0()
            .h(px(metrics::TAB))
            .px_3()
            .items_center()
            .gap_1()
            .bg(rgb(p.chrome))
            .border_b_1()
            .border_color(rgb(p.border))
            .children(
                [
                    "文件", "开始", "插入", "设计", "切换", "动画", "放映", "视图",
                ]
                .into_iter()
                .enumerate()
                .map(|(i, tab)| {
                    Button::new(("ribbon-tab", i))
                        .small()
                        .ghost()
                        .label(tab)
                        .text_color(rgb(if self.tab == tab { p.accent } else { p.text }))
                        .bg(if self.tab == tab {
                            rgb(p.selected).into()
                        } else {
                            transparent_black()
                        })
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.tab = tab;
                            cx.notify();
                        }))
                }),
            )
    }
    fn ribbon(&self, height: f32, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let mut ribbon = div()
            .h(px(height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .bg(rgb(p.surface))
            .border_b_1()
            .border_color(rgb(p.border));
        if self.tab == "视图" || self.tab == "开始" {
            ribbon = ribbon
                .child(
                    Button::new("ribbon-previous")
                        .outline()
                        .label("上一页")
                        .disabled(self.state.page() == 0)
                        .on_click(cx.listener(|s, _, w, cx| s.previous(&PreviousSlide, w, cx))),
                )
                .child(
                    Button::new("ribbon-next")
                        .outline()
                        .label("下一页")
                        .disabled(self.state.page() + 1 >= self.preview.slides.len())
                        .on_click(cx.listener(|s, _, w, cx| s.next(&NextSlide, w, cx))),
                )
                .child(div().w(px(1.)).h(px(30.)).bg(rgb(p.border)))
                .child(
                    Button::new("ribbon-fit")
                        .ghost()
                        .label("适应窗口")
                        .on_click(cx.listener(|s, _, w, cx| s.fit(&FitSlide, w, cx))),
                )
                .child(
                    Button::new("ribbon-pane")
                        .ghost()
                        .label("属性")
                        .on_click(cx.listener(|s, _, w, cx| s.inspector(&ToggleInspector, w, cx))),
                )
                .child(
                    Button::new("ribbon-notes")
                        .ghost()
                        .label("备注")
                        .on_click(cx.listener(|s, _, w, cx| s.notes(&ToggleNotes, w, cx))),
                );
        } else {
            let actions: &[&str] = match self.tab {
                "文件" => &["新建", "打开", "保存", "导出"],
                "插入" => &["文本框", "形状", "图片", "表格", "图表"],
                "设计" => &["主题", "配色", "母版", "幻灯片大小"],
                "切换" => &["切换效果", "持续时间", "应用到全部"],
                "动画" => &["添加动画", "动画窗格", "预览"],
                _ => &["从头开始", "从当前页", "演示者视图"],
            };
            ribbon = ribbon
                .children(actions.iter().enumerate().map(|(i, label)| {
                    Button::new(("pending-action", i))
                        .outline()
                        .label(*label)
                        .disabled(true)
                }))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(p.muted))
                        .child("此能力正在迁移"),
                );
        }
        ribbon
    }
    fn thumbnails(&self, width: f32, cx: &mut Context<Self>) -> Stateful<Div> {
        let p = self.palette();
        div()
            .id("slide-rail")
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .bg(rgb(p.chrome))
            .border_r_1()
            .border_color(rgb(p.border))
            .px_3()
            .py_4()
            .child(
                div()
                    .mb_4()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .child(format!("幻灯片 · {}", self.preview.slides.len())),
            )
            .children(self.preview.slides.iter().enumerate().map(|(i, slide)| {
                let selected = self.state.page() == i;
                div()
                    .id(("thumbnail", i))
                    .mb_4()
                    .cursor_pointer()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.state.select(i, s.preview.slides.len());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .border_2()
                            .border_color(rgb(if selected { p.accent } else { p.border }))
                            .rounded(px(5.))
                            .overflow_hidden()
                            .child(slide_element(
                                slide,
                                (width - 28.) / self.preview.width,
                                i,
                                true,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_size(px(11.))
                            .text_color(rgb(if selected { p.accent } else { p.muted }))
                            .child(format!("{}", i + 1))
                            .child(slide.title.clone()),
                    )
            }))
    }
    fn inspector_panel(&self, width: f32) -> Div {
        let p = self.palette();
        let current = self.preview.slides.get(self.state.page());
        joyoffice_ui::card(p)
            .w(px(width))
            .h_full()
            .flex_shrink_0()
            .rounded_none()
            .border_r_0()
            .border_t_0()
            .border_b_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("幻灯片属性"))
            .child(
                joyoffice_ui::field(p, "标题").child(
                    div()
                        .text_color(rgb(p.text))
                        .child(current.map(|s| s.title.clone()).unwrap_or_default()),
                ),
            )
            .child(
                joyoffice_ui::field(p, "画布尺寸").child(div().text_color(rgb(p.text)).child(
                    format!("{} × {} · 16:9", self.preview.width, self.preview.height),
                )),
            )
            .child(
                joyoffice_ui::field(p, "主题配色").child(
                    div().flex().gap_2().children(
                        [p.accent, 0x146c43, 0x315fa8, 0xe8c170, 0x26352d]
                            .into_iter()
                            .map(|c| div().size(px(27.)).rounded(px(5.)).bg(rgb(c))),
                    ),
                ),
            )
            .child(joyoffice_ui::section(p, "技术预览"))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .line_height(px(21.))
                    .child(
                        "这是原创浏览样例。可切换页面、调整缩放与主题，展开备注体验原生文字输入。",
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .line_height(px(21.))
                    .child("文件导入、内容编辑与保存将在后续阶段接入。"),
            )
    }
    fn status(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        div()
            .h(px(metrics::STATUS))
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgb(p.border))
            .bg(rgb(p.surface))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(11.))
            .text_color(rgb(p.muted))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(format!(
                        "幻灯片 {} / {}",
                        self.state.page() + 1,
                        self.preview.slides.len()
                    ))
                    .child("原创示例 · 只读浏览"),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new("status-notes")
                            .xsmall()
                            .ghost()
                            .label("备注")
                            .on_click(cx.listener(|s, _, w, cx| s.notes(&ToggleNotes, w, cx))),
                    )
                    .child(
                        Button::new("status-pane")
                            .xsmall()
                            .ghost()
                            .label("属性")
                            .on_click(
                                cx.listener(|s, _, w, cx| s.inspector(&ToggleInspector, w, cx)),
                            ),
                    )
                    .child(
                        Button::new("zoom-out")
                            .xsmall()
                            .ghost()
                            .label("−")
                            .on_click(cx.listener(|s, _, w, cx| s.zoom_out(&ZoomOut, w, cx))),
                    )
                    .child(
                        div()
                            .w(px(42.))
                            .text_center()
                            .child(format!("{:.0}%", self.state.zoom * 100.)),
                    )
                    .child(
                        Button::new("zoom-in")
                            .xsmall()
                            .ghost()
                            .label("＋")
                            .on_click(cx.listener(|s, _, w, cx| s.zoom_in(&ZoomIn, w, cx))),
                    )
                    .child(
                        Button::new("status-fit")
                            .xsmall()
                            .ghost()
                            .label("适应")
                            .on_click(cx.listener(|s, _, w, cx| s.fit(&FitSlide, w, cx))),
                    ),
            )
    }
}

/// Original preview content painted with native GPUI layout elements.
fn slide_element(slide: &PreviewSlide, scale: f32, index: usize, thumbnail: bool) -> Div {
    let scale = scale.max(0.01);
    let mut page = div()
        .relative()
        .w(px(960. * scale))
        .h(px(540. * scale))
        .flex_shrink_0()
        .overflow_hidden()
        .bg(rgb(0xffffff))
        .text_color(rgb(0x26352d))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .w(px(12. * scale))
                .h_full()
                .bg(rgb(slide.accent)),
        )
        .child(
            div()
                .absolute()
                .left(px(60. * scale))
                .top(px(55. * scale))
                .text_size(px(14. * scale))
                .text_color(rgb(slide.accent))
                .font_weight(FontWeight::SEMIBOLD)
                .child("J O Y D E C K"),
        )
        .child(
            div()
                .absolute()
                .left(px(60. * scale))
                .top(px(118. * scale))
                .text_size(px(44. * scale))
                .font_weight(FontWeight::BOLD)
                .child(slide.title.clone()),
        )
        .child(
            div()
                .absolute()
                .left(px(62. * scale))
                .top(px(196. * scale))
                .text_size(px(20. * scale))
                .text_color(rgb(0x5b6b63))
                .child(slide.subtitle.clone()),
        )
        .child(
            div()
                .absolute()
                .left(px(62. * scale))
                .bottom(px(38. * scale))
                .text_size(px(12. * scale))
                .text_color(rgb(0x7c8982))
                .child(format!(
                    "JOYOFFICE                                  {:02}",
                    index + 1
                )),
        );
    for (i, point) in slide.points.iter().take(3).enumerate() {
        page = page.child(
            div()
                .absolute()
                .left(px((62. + i as f32 * 280.) * scale))
                .top(px(310. * scale))
                .w(px(250. * scale))
                .h(px(100. * scale))
                .rounded(px(9. * scale))
                .bg(rgb(0xf3f6f4))
                .flex()
                .flex_col()
                .justify_center()
                .px(px(22. * scale))
                .gap(px(10. * scale))
                .child(
                    div()
                        .w(px(28. * scale))
                        .h(px(4. * scale))
                        .rounded_full()
                        .bg(rgb(slide.accent)),
                )
                .child(
                    div()
                        .text_size(px(21. * scale))
                        .font_weight(FontWeight::MEDIUM)
                        .child(point.clone()),
                ),
        );
    }
    if !thumbnail {
        page = page.shadow_lg();
    }
    page
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let layout = WorkspaceLayout::new(
            f32::from(window.viewport_size().width),
            f32::from(window.viewport_size().height),
            self.state.inspector,
            self.state.notes,
            self.tab == "开始",
        );
        let scale = layout.fit(self.preview.width, self.preview.height) * self.state.zoom;
        let mut center = div().flex_1().min_w_0().h_full().flex().flex_col().child(
            div()
                .id("slide-stage")
                .flex_1()
                .min_h_0()
                .overflow_scroll()
                .bg(rgb(p.canvas))
                .child(
                    div()
                        .min_w_full()
                        .min_h_full()
                        .w(px(self.preview.width * scale + layout.padding * 2.))
                        .h(px(self.preview.height * scale + layout.padding * 2.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .children(
                            self.preview
                                .slides
                                .get(self.state.page())
                                .map(|s| slide_element(s, scale, self.state.page(), false)),
                        ),
                ),
        );
        center = center.child(
            div()
                .h(px(layout.notes))
                .flex_shrink_0()
                .border_t_1()
                .border_color(rgb(p.border))
                .bg(rgb(p.surface))
                .px_3()
                .py_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("notes-toggle")
                        .xsmall()
                        .ghost()
                        .label(if self.state.notes {
                            "演讲备注 · 本窗口临时输入"
                        } else {
                            "点击添加演讲备注"
                        })
                        .on_click(cx.listener(|s, _, w, cx| s.notes(&ToggleNotes, w, cx))),
                )
                .when(self.state.notes, |el| {
                    el.child(Textarea::new(&self.notes_input).w_full().flex_1())
                }),
        );
        div()
            .id("joydeck-workspace")
            .track_focus(&self.focus)
            .key_context("Workspace")
            .size_full()
            .font_family(if cfg!(target_os = "windows") {
                "Microsoft YaHei"
            } else if cfg!(target_os = "macos") {
                "Helvetica"
            } else {
                "DejaVu Sans"
            })
            .text_size(px(if layout.compact { 12. } else { 13. }))
            .text_color(rgb(p.text))
            .bg(rgb(p.surface))
            .flex()
            .flex_col()
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::fit))
            .on_action(cx.listener(Self::theme))
            .on_action(cx.listener(Self::inspector))
            .on_action(cx.listener(Self::notes))
            .on_action(cx.listener(Self::about))
            .child(self.header(cx))
            .child(self.tabs(cx))
            .child(self.ribbon(layout.ribbon, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.thumbnails(layout.rail, cx))
                    .child(center)
                    .when(self.state.inspector, |el| {
                        el.child(self.inspector_panel(layout.pane))
                    }),
            )
            .child(self.status(cx))
    }
}

fn apply_theme(dark: bool, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let p = viewer::deck_palette(dark);
    let theme = Theme::global_mut(cx);
    theme.colors.primary = rgb(p.accent).into();
    theme.colors.primary_hover = rgb(p.accent_hover).into();
    theme.colors.primary_active = rgb(p.accent_hover).into();
    theme.colors.primary_foreground = rgb(p.on_accent).into();
    theme.colors.ring = rgb(p.accent).into();
    theme.colors.button_primary = rgb(p.accent).into();
    theme.colors.button_primary_hover = rgb(p.accent_hover).into();
    theme.colors.button_primary_active = rgb(p.accent_hover).into();
    theme.colors.button_primary_foreground = rgb(p.on_accent).into();
    theme.tokens.button_primary = theme.colors.button_primary.into();
    theme.tokens.button_primary_hover = theme.colors.button_primary_hover.into();
    theme.tokens.button_primary_active = theme.colors.button_primary_active.into();
    theme.radius = px(metrics::RADIUS);
}

pub fn run(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    let mut dark = false;
    let mut window_size = (1440., 900.);
    let mut input = None;
    let mut sample_workspace = false;
    let mut control_root = None;
    for arg in arguments {
        match arg.as_str() {
            "--dark" => dark = true,
            "--light" => dark = false,
            "--sample-workspace" => sample_workspace = true,
            "--help" => {
                println!(
                    "JoyDeck [INPUT.pptx|INPUT.joydeck|INPUT.deckcraft] [--dark|--light] [--window-size=WIDTHxHEIGHT] [--control-stdio=AUTHORIZED_ROOT]\nR8 native ribbon editor, save/reopen and recovery. --sample-workspace opens the R0 UI sample."
                );
                return Ok(());
            }
            "--version" => {
                println!("JoyDeck {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ if arg.starts_with("--control-stdio=") => {
                let root = std::path::PathBuf::from(arg.trim_start_matches("--control-stdio="));
                joydeck_shell::automation::Dispatcher::new(&root)?;
                control_root = Some(root);
            }
            _ if arg.starts_with("--window-size=") => {
                let value = arg.trim_start_matches("--window-size=");
                let (width, height) = value
                    .split_once('x')
                    .ok_or("window size must be WIDTHxHEIGHT")?;
                let width: u32 = width.parse().map_err(|_| "invalid window width")?;
                let height: u32 = height.parse().map_err(|_| "invalid window height")?;
                if !(800..=8192).contains(&width) || !(600..=8192).contains(&height) {
                    return Err("window size must be between 800x600 and 8192x8192".into());
                }
                window_size = (width as f32, height as f32);
            }
            _ if !arg.starts_with('-') && input.is_none() => {
                input = Some(std::path::PathBuf::from(arg))
            }
            _ => return Err(format!("unsupported argument: {arg}; use --help")),
        }
    }
    if sample_workspace && control_root.is_some() {
        return Err("desktop control requires the editor workspace".into());
    }
    let preview = PreviewDeck::bundled()?;
    let failure = Rc::new(RefCell::new(None));
    let startup_failure = failure.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            apply_theme(dark, None, cx);
            viewer::bind(cx);
            cx.on_action(move |_: &Quit, cx| {
                if sample_workspace {
                    cx.quit()
                } else {
                    viewer::quit(cx)
                }
            });
            cx.bind_keys([
                KeyBinding::new("ctrl-q", Quit, None),
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("pagedown", NextSlide, Some("Workspace")),
                KeyBinding::new("pageup", PreviousSlide, Some("Workspace")),
                KeyBinding::new("ctrl-0", FitSlide, Some("Workspace")),
                KeyBinding::new("ctrl-shift-t", ToggleTheme, Some("Workspace")),
            ]);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let result = if !sample_workspace {
                viewer::open_window(dark, window_size, cx).map(|(handle, view)| {
                    if let Some(input) = input {
                        viewer::load_path(&view, input, cx);
                    } else {
                        viewer::new_document(&view, handle, cx);
                    }
                    if let Some(root) = control_root
                        && let Err(error) =
                            viewer::desktop_control::install(&view, handle, root, cx)
                    {
                        *startup_failure.borrow_mut() = Some(error);
                    }
                    handle
                })
            } else {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(window_size.0), px(window_size.1)),
                            cx,
                        ))),
                        window_min_size: Some(size(px(800.), px(600.))),
                        ..Default::default()
                    },
                    cx,
                    move |window, cx| {
                        window.set_window_title("JoyDeck · R0 原生技术预览");
                        cx.new(|cx| Workspace::new(preview, dark, window, cx))
                    },
                )
                .map(|(handle, _)| handle)
            };
            match result {
                Ok(_) => {
                    eprintln!("JoyDeck window opened · GPUI Kit 0.8.1 / pre 0.3.10");
                    cx.activate(true);
                }
                Err(error) => {
                    *startup_failure.borrow_mut() = Some(error.to_string());
                    cx.quit();
                }
            }
        });
    match failure.borrow_mut().take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
