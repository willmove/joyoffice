//! Document identity, navigation and advanced-workflow entry points.
use super::ui::*;
use super::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    assets::IconName as I,
    component::{
        Sizable,
        button::ButtonVariants,
        menu::{DropdownMenu, PopupMenuItem},
    },
};
impl Editor {
    pub(super) fn select_ribbon(
        &mut self,
        name: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            name,
            "文件" | "设计" | "视图" | "维护" | "比较" | "输出" | "媒体"
        ) {
            self.finish_text(cx);
        }
        self.ribbon = name.into();
        if name == "幻灯片放映" {
            let order = if self.session.deck.presentation.order.is_empty() {
                (1..=self.session.deck.slides.len())
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                self.session
                    .deck
                    .presentation
                    .order
                    .iter()
                    .filter_map(|id| self.session.deck.slides.iter().position(|p| &p.id == id))
                    .map(|i| (i + 1).to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            };
            self.author_data
                .update(cx, |f, cx| f.set_value(order, w, cx));
        }
        if name == "切换" {
            let t = self
                .session
                .deck
                .presentation
                .timing(&self.session.deck.slides[self.page].id);
            for (i, value) in [(0, t.duration_ms), (2, t.advance_ms.unwrap_or(0))] {
                self.motion_fields[i].update(cx, |f, cx| f.set_value(value.to_string(), w, cx));
            }
        }
        if matches!(name, "图表" | "图示" | "动画") {
            self.sync_author(w, cx);
        }
        if matches!(name, "文件" | "设计") {
            self.sync_doc_fields(w, cx);
        }
        cx.notify();
    }
    pub(super) fn document_header(&self, w: &Window, cx: &mut Context<Self>) -> AnyElement {
        let compact = f32::from(w.viewport_size().width) < 1000.;
        let title = if self.session.deck.title.is_empty() {
            "未命名演示文稿".to_owned()
        } else {
            self.session.deck.title.clone()
        };
        gpui_kit::component::TitleBar::new()
            .h(px(joyoffice_ui::metrics::HEADER))
            .bg(rgb(PALETTE.header))
            .on_close_window({
                let weak = cx.entity().downgrade();
                move |_, w, cx| {
                    let _ = weak.update(cx, |s, cx| s.close(&Close, w, cx));
                }
            })
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ACCENT))
                            .child("JoyDeck"),
                    )
                    .child(
                        tool("save", "", I::Save)
                            .disabled(self.saving)
                            .on_click(cx.listener(|s, _, w, cx| s.save(&Save, w, cx))),
                    )
                    .child(
                        tool("undo", "", I::Undo2)
                            .disabled(!self.session.can_undo())
                            .on_click(cx.listener(|s, _, w, cx| s.undo(&Undo, w, cx))),
                    )
                    .child(
                        tool("redo", "", I::Redo2)
                            .disabled(!self.session.can_redo())
                            .on_click(cx.listener(|s, _, w, cx| s.redo(&Redo, w, cx))),
                    )
                    .child(div().w(px(1.)).h(px(20.)).mx_2().bg(rgb(PALETTE.border)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .when(!compact, |el| {
                        el.child(label(if self.saving {
                            "正在保存…"
                        } else if self.session.dirty() {
                            "有未保存的更改"
                        } else if self.disk.is_some() {
                            "已保存到本机"
                        } else {
                            "新文稿"
                        }))
                    })
                    .child(tool("export", "导出", I::Upload).on_click(cx.listener(
                        |s, _, w, cx| {
                            s.select_ribbon("文件", w, cx);
                            s.delivery_panel = true;
                        },
                    )))
                    .child(
                        tool("show", "放映", I::Presentation)
                            .primary()
                            .on_click(cx.listener(|s, _, w, cx| s.delivery("show-current", w, cx))),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn workflow_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let weak = cx.entity().downgrade();
        tool("more-workflows", "更多", I::Ellipsis)
            .dropdown_menu(move |mut menu, _, _| {
                for (name, label) in [
                    ("媒体", "插入音视频与播放"),
                    ("输出", "视频、字幕与高级输出"),
                    ("比较", "比较文稿与接受更改"),
                    ("维护", "设置、最近文稿与帮助"),
                ] {
                    let weak = weak.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, w, cx| {
                        let _ = weak.update(cx, |s, cx| s.select_ribbon(name, w, cx));
                    }));
                }
                menu
            })
            .into_any_element()
    }
    pub(super) fn ribbon_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .h(px(joyoffice_ui::metrics::TAB))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_1()
            .bg(rgb(PALETTE.chrome))
            .border_b_1()
            .border_color(rgb(PALETTE.border))
            .child(
                div()
                    .id("ribbon-tabs-scroll")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .overflow_x_scroll()
                    .children(
                        [
                            "文件",
                            "开始",
                            "插入",
                            "设计",
                            "切换",
                            "动画",
                            "幻灯片放映",
                            "审阅",
                            "视图",
                            "表格",
                            "图表",
                            "图示",
                        ]
                        .into_iter()
                        .filter(|name| match *name {
                            "表格" => {
                                self.ribbon == *name
                                    || self.author_object().is_some_and(|(_, o)| o.table.is_some())
                            }
                            "图表" => {
                                self.ribbon == *name
                                    || self.author_object().is_some_and(|(_, o)| o.chart.is_some())
                            }
                            "图示" => {
                                self.ribbon == *name
                                    || self
                                        .author_object()
                                        .is_some_and(|(_, o)| o.diagram.is_some())
                            }
                            _ => true,
                        })
                        .map(|name| {
                            let active = self.ribbon == name;
                            let text = match name {
                                "幻灯片放映" => "放映",
                                "表格" => "表格布局",
                                "图表" => "图表设计",
                                "图示" => "图示设计",
                                _ => name,
                            };
                            Button::new(name)
                                .accessibility_id(name)
                                .ghost()
                                .small()
                                .label(text)
                                .accessibility_label(text)
                                .h(px(joyoffice_ui::metrics::TAB))
                                .px_3()
                                .rounded_none()
                                .border_b_2()
                                .border_color(rgb(if active { ACCENT } else { PALETTE.chrome }))
                                .text_color(rgb(if active { ACCENT } else { PALETTE.muted }))
                                .on_click(
                                    cx.listener(move |s, _, w, cx| s.select_ribbon(name, w, cx)),
                                )
                        }),
                    ),
            )
            .child(self.workflow_menu(cx))
            .child(
                tool("inspector-toggle", "格式", I::Settings2).on_click(cx.listener(
                    |s, _, w, cx| {
                        s.inspector_open = !s.inspector_open;
                        if !s.inspector_open {
                            s.select_ribbon("开始", w, cx);
                        }
                        s.sync_fields(w, cx);
                        cx.notify();
                    },
                )),
            )
            .into_any_element()
    }
}
