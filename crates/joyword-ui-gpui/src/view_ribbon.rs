//! Native View ribbon, screen-only guides and navigation over the existing session.
use super::ribbon::Tool;
use super::*;
use gpui::component::{checkbox::Checkbox, popover::Popover};
use gpui::{canvas, fill, point};
use joyoffice_ui::IconName;
use serde_json::json;

fn fit_percent(mode: &str, page: (f32, f32), viewport: (f32, f32)) -> u16 {
    let width = (viewport.0 - 64.).max(1.) / (page.0.max(1.) * 96. / 72.);
    let height = (viewport.1 - 66.).max(1.) / (page.1.max(1.) * 96. / 72.);
    let scale = if mode == "onePage" {
        width.min(height)
    } else {
        width
    };
    (scale * 100.).floor().clamp(10., 500.) as u16
}

impl JoyWord {
    pub(super) fn sync_view_zoom(&mut self) {
        let Some(r) = &self.canvas.reader else { return };
        let view = &r.engine().view;
        self.shell.zoom = if matches!(view.fit.as_str(), "onePage" | "pageWidth") {
            let page = r
                .summary
                .pages
                .iter()
                .fold((1_f32, 1_f32), |p, s| (p.0.max(s.width), p.1.max(s.height)));
            let viewport = self
                .viewport_bounds
                .map_or((900., self.canvas.viewport), |b| {
                    (f32::from(b.size.width), f32::from(b.size.height))
                });
            fit_percent(&view.fit, page, viewport)
        } else {
            (view.zoom * 100.).round().clamp(10., 500.) as u16
        };
    }
    pub(super) fn view_focus(&self) -> bool {
        !self.file_page
            && self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.engine().view.focus_mode)
    }
    pub(super) fn view_navigation(&self) -> bool {
        !self.file_page
            && !self.view_focus()
            && self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.engine().view.nav_pane)
    }
    fn view_group(&self, zh: &'static str, en: &'static str) -> gpui::Stateful<gpui::Div> {
        div()
            .id(zh)
            .flex()
            .items_center()
            .h(px(66.))
            .px(px(5.))
            .gap(px(2.))
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(self.palette().separator))
            .role(gpui::Role::Group)
            .aria_label(self.shell.tr(zh, en))
    }
    fn view_tool(
        &self,
        id: &'static str,
        tool: Tool,
        command: &'static str,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        self.ribbon_tool(id, tool, self.format_ready(command), false)
            .selected(selected)
            .on_click(cx.listener(move |v, _, w, cx| v.run_format(command, &json!({}), w, cx)))
    }
    pub(super) fn view_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.canvas.reader.as_ref().map(|r| &r.engine().view);
        let chinese = self.shell.locale == joyword_shell::l10n::Locale::ZhCn;
        let width = |zh, en| if chinese { zh } else { en };
        let unavailable = |id, tool| {
            self.ribbon_tool(id, tool, false, false)
                .tooltip(self.shell.tr(
                    "此桌面版本尚未接入该视图或窗口功能",
                    "This desktop view or window feature is not available yet",
                ))
        };
        let views = self
            .view_group("视图", "Views")
            .child(
                self.view_tool(
                    "view-print",
                    Tool::new("页面视图", "Print Layout", IconName::FileText)
                        .width(width(38., 58.))
                        .caption_width(width(28., 54.)),
                    "view.printLayout",
                    true,
                    cx,
                ),
            )
            .child(unavailable(
                "view-web",
                Tool::new("Web 版式", "Web Layout", IconName::Globe)
                    .width(width(38., 58.))
                    .caption_width(width(28., 54.)),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(unavailable(
                        "view-outline",
                        Tool::compact("大纲", "Outline", IconName::ListTree).width(width(60., 88.)),
                    ))
                    .child(unavailable(
                        "view-draft",
                        Tool::compact("草稿", "Draft", IconName::File).width(width(60., 88.)),
                    )),
            );
        let immersive = self
            .view_group("沉浸式", "Immersive")
            .child(self.view_tool(
                "view-focus",
                Tool::new("专注", "Focus", IconName::Focus).width(width(40., 48.)),
                "view.focus",
                state.is_some_and(|s| s.focus_mode),
                cx,
            ))
            .child(unavailable(
                "view-reader",
                Tool::new("沉浸式阅读器", "Immersive Reader", IconName::BookOpen)
                    .width(width(50., 68.))
                    .caption_width(width(44., 64.)),
            ));
        let mut show = self
            .view_group("显示", "Show")
            .flex_col()
            .items_start()
            .justify_center()
            .gap(px(1.))
            .w(px(
                if self.shell.locale == joyword_shell::l10n::Locale::ZhCn {
                    96.
                } else {
                    134.
                },
            ));
        for (id, zh, en, command, checked) in [
            (
                "view-ruler",
                "标尺",
                "Ruler",
                "view.ruler",
                state.is_some_and(|s| s.ruler),
            ),
            (
                "view-grid",
                "网格线",
                "Gridlines",
                "view.gridlines",
                state.is_some_and(|s| s.gridlines),
            ),
            (
                "view-navigation",
                "导航窗格",
                "Navigation Pane",
                "view.navigationPane",
                state.is_some_and(|s| s.nav_pane),
            ),
        ] {
            show = show.child(
                Checkbox::new(id)
                    .label(self.shell.tr(zh, en))
                    .text_size(px(11.))
                    .xsmall()
                    .h(px(21.))
                    .checked(checked)
                    .disabled(!self.format_ready(command))
                    .on_click(cx.listener(move |v, checked, w, cx| {
                        v.run_format(command, &json!({"value":checked}), w, cx)
                    })),
            );
        }
        let zoom = self
            .view_group("缩放", "Zoom")
            .child(
                self.ribbon_tool(
                    "view-zoom",
                    Tool::new("缩放", "Zoom", IconName::Search).width(40.),
                    self.format_ready("view.zoom"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.open_zoom_dialog(w, cx))),
            )
            .child(
                self.view_tool(
                    "view-100",
                    Tool::new("缩放至 100%", "100%", IconName::FileDigit)
                        .width(42.)
                        .caption_width(38.),
                    "view.zoom100",
                    false,
                    cx,
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(self.view_tool(
                        "view-one-page",
                        Tool::compact("单页", "One Page", IconName::File).width(width(60., 116.)),
                        "view.onePage",
                        state.is_some_and(|s| s.fit == "onePage"),
                        cx,
                    ))
                    .child(unavailable(
                        "view-multiple",
                        Tool::compact("多页", "Multiple Pages", IconName::Columns2)
                            .width(width(60., 116.)),
                    ))
                    .child(
                        self.view_tool(
                            "view-page-width",
                            Tool::compact("页宽", "Page Width", IconName::BetweenHorizontalStart)
                                .width(width(60., 116.)),
                            "view.pageWidth",
                            state.is_some_and(|s| s.fit == "pageWidth"),
                            cx,
                        ),
                    ),
            );
        let windows = self
            .view_group("窗口", "Window")
            .child(unavailable(
                "view-new-window",
                Tool::new("新建窗口", "New Window", IconName::SquarePlus)
                    .width(width(42., 56.))
                    .caption_width(width(30., 52.)),
            ))
            .child(unavailable(
                "view-arrange",
                Tool::new("全部重排", "Arrange All", IconName::PanelsTopLeft)
                    .width(width(42., 56.))
                    .caption_width(width(30., 52.)),
            ))
            .child(unavailable(
                "view-split",
                Tool::new("拆分", "Split", IconName::Rows2).width(34.),
            ));
        let switch = self.view_group("切换窗口", "Switch Windows").child(
            self.ribbon_tool(
                "view-switch",
                Tool::new("切换窗口", "Switch Windows", IconName::AppWindow)
                    .width(width(56., 76.))
                    .caption_width(width(48., 70.)),
                false,
                true,
            )
            .tooltip(self.shell.tr(
                "当前桌面仅有一个文档窗口",
                "This desktop currently has one document window",
            )),
        );
        let owner = cx.weak_entity();
        let macro_ready = self.format_ready("tools.macros");
        let recording = self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.engine().recording.is_some());
        let chinese = self.shell.locale == joyword_shell::l10n::Locale::ZhCn;
        let macros = Popover::new("view-macros-menu")
            .trigger(self.ribbon_tool(
                "view-macros",
                Tool::new("宏", "Macros", IconName::SquareTerminal).width(48.),
                macro_ready,
                true,
            ))
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut list = div().flex().flex_col().p_1().w(px(190.));
                for (id, zh, en, enabled) in [
                    ("macro-list", "查看宏…", "View Macros…", macro_ready),
                    (
                        "macro-record",
                        "录制宏…",
                        "Record Macro…",
                        macro_ready && !recording,
                    ),
                    (
                        "macro-stop",
                        "停止录制",
                        "Stop Recording",
                        macro_ready && recording,
                    ),
                ] {
                    let owner = owner.clone();
                    let popup = popup.clone();
                    list = list.child(
                        Button::new(id)
                            .ghost()
                            .label(if chinese { zh } else { en })
                            .disabled(!enabled)
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = owner.update(cx, |v, cx| v.advanced_action(id, w, cx));
                            }),
                    );
                }
                list
            });
        div()
            .flex()
            .items_center()
            .px(px(5.))
            .flex_shrink_0()
            .child(views)
            .child(immersive)
            .child(show)
            .child(zoom)
            .child(windows)
            .child(switch)
            .child(macros)
    }
    fn open_zoom_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form(
            self.shell.tr("缩放", "Zoom"),
            "view.zoom",
            vec![(
                self.shell.tr("百分比（10–500）", "Percent (10–500)"),
                self.shell.zoom.to_string(),
            )],
            false,
            |values| {
                let value = values
                    .first()
                    .and_then(|s| s.trim().trim_end_matches('%').trim().parse::<f64>().ok())
                    .filter(|v| v.is_finite() && (10. ..=500.).contains(v))
                    .ok_or("请输入 10–500 之间的缩放百分比 / Enter a percent from 10 to 500")?;
                Ok(json!({"value":value}))
            },
            w,
            cx,
        );
    }
    pub(super) fn focus_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .justify_end()
            .h(px(32.))
            .bg(rgb(self.palette().chrome))
            .child(
                Button::new("view-exit-focus")
                    .ghost()
                    .xsmall()
                    .label(self.shell.tr("退出专注（Esc）", "Exit Focus (Esc)"))
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.run_format("view.focus", &json!({"value":false}), w, cx)
                    })),
            )
    }
    pub(super) fn navigation_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let mut panel = div()
            .id("view-navigation-panel")
            .w(px(240.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(p.surface))
            .border_r_1()
            .border_color(rgb(p.border))
            .p_3()
            .gap_2()
            .role(gpui::Role::Group)
            .aria_label(self.shell.tr("导航", "Navigation"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(self.shell.tr("导航", "Navigation"))
                    .child(
                        Button::new("view-close-navigation")
                            .ghost()
                            .xsmall()
                            .icon(IconName::X)
                            .accessibility_label(
                                self.shell.tr("关闭导航窗格", "Close Navigation Pane"),
                            )
                            .on_click(cx.listener(|v, _, w, cx| {
                                v.run_format("view.navigationPane", &json!({"value":false}), w, cx)
                            })),
                    ),
            )
            .child(
                Button::new("view-navigation-find")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Search)
                    .label(self.shell.tr("在文档中搜索…", "Search in document…"))
                    .disabled(!self.format_ready("edit.findNext"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_find(false, w, cx))),
            )
            .child(
                div()
                    .text_color(rgb(p.accent))
                    .child(self.shell.tr("标题", "Headings")),
            );
        let mut list = div()
            .id("view-navigation-headings")
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        if let Some(r) = &self.canvas.reader {
            let entries = r.outline_entries();
            if entries.is_empty() {
                list = list.child(div().text_size(px(12.)).text_color(rgb(p.muted)).child(
                    self.shell.tr(
                        "应用标题样式后，标题将在此处显示。",
                        "Apply heading styles to see headings here.",
                    ),
                ));
            }
            let document = r.engine().document_id();
            let revision = r.engine().rev();
            for (index, (label, args)) in entries.into_iter().enumerate() {
                list = list.child(
                    Button::new(("view-heading", index))
                        .ghost()
                        .xsmall()
                        .label(label)
                        .disabled(!self.format_ready("select.range"))
                        .on_click(cx.listener(move |v, _, w, cx| {
                            let _ = v.select_navigation_entry(document, revision, &args, w, cx);
                        })),
                );
            }
        }
        panel = panel.child(list);
        panel
    }
    pub(super) fn page_ruler(&self, page: usize) -> impl IntoElement {
        let p = self.palette();
        let scale = self.canvas.factor(self.shell.zoom);
        let r = self.canvas.reader.as_ref();
        let geometry = r.and_then(|r| r.geometry().pages.get(page));
        let (width, left, body) = geometry.map_or((0., 0., 0.), |g| {
            (g.w * scale, g.body.x * scale, g.body.w * scale)
        });
        let unit = r.map_or(72. / 2.54, |r| r.engine().units().pt_per_unit()) * scale;
        let ticks = (width / (unit / 4.).max(1.)).ceil().min(1200.) as usize;
        div()
            .id(("view-page-ruler", page))
            .w(px(width))
            .h(px(18.))
            .relative()
            .overflow_hidden()
            .bg(rgb(p.chrome))
            .role(gpui::Role::Group)
            .aria_label(
                self.shell
                    .tr("水平标尺（只读）", "Horizontal ruler (read only)"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(left))
                    .w(px(body))
                    .h_full()
                    .bg(rgb(p.surface)),
            )
            .children((0..ticks).filter_map(move |i| {
                let value = i as f32 / 4. - (left / unit).ceil();
                let x = left + value * unit;
                (x >= 0. && x <= width).then(|| {
                    div()
                        .absolute()
                        .left(px(x))
                        .bottom_0()
                        .h(px(if i % 4 == 0 { 7. } else { 3. }))
                        .border_l_1()
                        .border_color(rgb(p.muted))
                })
            }))
            .children(
                (0..(width / unit).ceil().min(300.) as usize).filter_map(move |i| {
                    let x = left + i as f32 * unit;
                    (x < width && x <= left + body).then(|| {
                        div()
                            .absolute()
                            .left(px(x + 2.))
                            .top_0()
                            .text_size(px(9.))
                            .text_color(rgb(p.muted))
                            .child(i.to_string())
                    })
                }),
            )
    }
    pub(super) fn page_grid(&self, page: usize) -> impl IntoElement {
        let scale = self.canvas.factor(self.shell.zoom);
        let body = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.geometry().pages.get(page))
            .map(|p| p.body);
        div().absolute().top_0().left_0().size_full().child(
            canvas(
                |_, _, _| (),
                move |bounds, _, w, _| {
                    if let Some(body) = body {
                        // Screen-only guides; they never enter the page scene or exported bytes.
                        let color = gpui::rgba(0x7f8d9b30);
                        let step = (18. * scale).max(4.);
                        let origin = point(
                            bounds.left() + px(body.x * scale),
                            bounds.top() + px(body.y * scale),
                        );
                        let width = body.w * scale;
                        let height = body.h * scale;
                        for i in 0..=(width / step).min(1600.) as usize {
                            w.paint_quad(fill(
                                Bounds::new(
                                    point(
                                        px(f32::from(origin.x + px(i as f32 * step)).round()),
                                        origin.y,
                                    ),
                                    size(px(1.), px(height)),
                                ),
                                color,
                            ));
                        }
                        for i in 0..=(height / step).min(1600.) as usize {
                            w.paint_quad(fill(
                                Bounds::new(
                                    point(
                                        origin.x,
                                        px(f32::from(origin.y + px(i as f32 * step)).round()),
                                    ),
                                    size(px(width), px(1.)),
                                ),
                                color,
                            ));
                        }
                    }
                },
            )
            .size_full(),
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "GUI regression setup and assertions")]
mod tests {
    use super::*;
    use gpui::{EntityInputHandler, TestAppContext};
    use std::sync::atomic::AtomicBool;
    #[test]
    fn fitting_respects_page_geometry_viewport_and_limits() {
        assert_eq!(fit_percent("pageWidth", (600., 800.), (864., 666.)), 100);
        assert_eq!(fit_percent("onePage", (600., 800.), (864., 666.)), 56);
        assert_eq!(fit_percent("onePage", (800., 600.), (864., 666.)), 75);
        assert_eq!(fit_percent("onePage", (600., 800.), (10., 10.)), 10);
        assert_eq!(
            fit_percent("pageWidth", (600., 800.), (10000., 10000.)),
            500
        );
    }
    #[gpui::test]
    fn view_controls_change_visible_state_without_editing_and_share_zoom(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let r = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            let id = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(id, cx);
            v.tab = 5;
        });
        let original = view.read_with(cx, |v, _| v.canvas.reader.as_ref().unwrap().engine().rev());
        for (command, args) in [
            ("view.ruler", json!({"value":true})),
            ("view.gridlines", json!({"value":true})),
            ("view.navigationPane", json!({"value":true})),
            ("view.focus", json!({"value":true})),
            ("view.zoom", json!({"value":175})),
        ] {
            cx.update(|w, cx| view.update(cx, |v, cx| v.run_format(command, &args, w, cx)));
        }
        cx.update(|w, cx| w.draw(cx).clear(cx));
        view.read_with(cx, |v, _| {
            let r = v.canvas.reader.as_ref().unwrap();
            assert_eq!(r.engine().rev(), original);
            assert!(!r.is_dirty());
            assert!(r.engine().view.gridlines && r.engine().view.ruler);
            assert!(v.view_focus());
            assert!(!v.view_navigation());
            assert_eq!(v.shell.zoom, 175);
        });
        // Escape must cancel an active composition before leaving Focus.
        let input = view.read_with(cx, |v, _| v.input.clone().unwrap());
        cx.update(|w, cx| {
            input.update(cx, |i, cx| {
                i.replace_and_mark_text_in_range(None, "zhong", None, w, cx)
            })
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |v, _| {
            assert!(v.view_focus());
            assert!(!v.canvas.reader.as_ref().unwrap().is_composing());
        });
        cx.simulate_keystrokes("escape");
        cx.update(|w, cx| view.update(cx, |v, cx| v.execute("view.zoom_in", w, cx)));
        cx.update(|w, cx| w.draw(cx).clear(cx));
        view.read_with(cx, |v, _| {
            assert!(!v.view_focus());
            assert!(v.view_navigation());
            assert_eq!(v.shell.zoom, 185);
        });
        cx.update(|w, cx| view.update(cx, |v, cx| v.run_format("view.onePage", &json!({}), w, cx)));
        cx.update(|w, cx| w.draw(cx).clear(cx));
        view.read_with(cx, |v, _| {
            assert!(v.shell.zoom < 100);
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().engine().view.fit,
                "onePage"
            );
        });
        cx.update(|w, cx| view.update(cx, |v, cx| v.execute("view.zoom_reset", w, cx)));
        cx.update(|w, cx| w.draw(cx).clear(cx));
        view.read_with(cx, |v, _| {
            assert_eq!(v.shell.zoom, 100);
            assert!(
                v.canvas
                    .reader
                    .as_ref()
                    .unwrap()
                    .engine()
                    .view
                    .fit
                    .is_empty()
            );
        });
    }
}
