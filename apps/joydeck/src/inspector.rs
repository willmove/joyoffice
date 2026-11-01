//! Contextual forms and previews; field drafts remain owned by the editor.
use super::ribbon_menus::ColorTarget;
use super::ui::*;
use super::*;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    assets::IconName as I,
    component::{Sizable, button::ButtonVariants},
};
impl Editor {
    pub(super) fn inspector(&self, pane_width: f32, cx: &mut Context<Self>) -> AnyElement {
        let title = if self.ribbon == "设计" {
            "设计与版式"
        } else if self.table_target().is_some() {
            "表格格式"
        } else if self.ribbon == "文件" {
            "文稿与导出"
        } else {
            "对象格式"
        };
        let mut p = pane(pane_width)
            .id("inspector")
            .child(self.pane_header(title, cx));
        let mut body = div()
            .id("inspector-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_4()
            .pb_5()
            .flex()
            .flex_col()
            .gap_2()
            .text_size(px(12.));
        if self.ribbon == "文件" {
            body = body
                .child(section("文稿信息"))
                .child(field("标题", &self.doc_fields[0]))
                .child(field("作者", &self.doc_fields[1]))
                .child(
                    Button::new("properties")
                        .small()
                        .label("更新文稿信息")
                        .on_click(cx.listener(|s, _, _, cx| s.properties(cx))),
                )
                .children(self.delivery_panel.then(|| self.delivery_controls(cx)));
        } else if self.ribbon == "设计" {
            body = body
                .child(section("页面尺寸"))
                .child(self.property_pair(10, "宽度 · pt", 11, "高度 · pt"))
                .child(
                    tool("inspector-page-size", "应用页面尺寸", I::Maximize)
                        .on_click(cx.listener(|s, _, _, cx| s.page_size(cx))),
                )
                .child(section("主题配色"))
                .child(self.color_field("theme-accent", "强调色", ColorTarget::Accent, cx))
                .child(self.color_field("theme-background", "背景色", ColorTarget::Background, cx))
                .child(
                    Button::new("apply-theme")
                        .label("应用主题配色")
                        .disabled(self.session.deck.design.layouts.is_empty())
                        .on_click(cx.listener(|s, _, _, cx| s.design_op("theme", cx))),
                )
                .child(section("幻灯片版式"))
                .child(label("占位符预览 · 点击应用到当前页"))
                .child(div().flex().flex_wrap().gap_2().children(
                    self.session.deck.design.layouts.iter().map(|(id, l)| {
                        let selected =
                            self.session.deck.slides[self.page].layout.as_ref() == Some(id);
                        let preview = self.layout_preview(l);
                        let id = id.clone();
                        Button::new(SharedString::from(format!("layout-{id}")))
                            .ghost()
                            .h(px(94.))
                            .w(px(116.))
                            .p_1()
                            .accessibility_label(l.name.clone())
                            .tooltip(l.name.clone())
                            .border_1()
                            .border_color(rgb(if selected { ACCENT } else { PALETTE.border }))
                            .rounded(px(6.))
                            .bg(rgb(if selected {
                                PALETTE.selected
                            } else {
                                PALETTE.surface
                            }))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(preview)
                                    .child(label(l.name.clone()).truncate()),
                            )
                            .on_click(cx.listener(move |s, _, w, cx| {
                                s.chosen_layout = id.clone();
                                s.design_op("apply", cx);
                                s.sync_doc_fields(w, cx);
                            }))
                    }),
                ))
                .when(self.session.deck.design.layouts.is_empty(), |el| {
                    el.child(label("此文稿尚无主题或版式")).child(
                        tool("create-design", "创建设计", I::Palette)
                            .on_click(cx.listener(|s, _, _, cx| s.design_op("starter", cx))),
                    )
                })
                .child(section("母版与文稿"))
                .child(
                    tool("advanced-design", "管理母版与文稿", I::Settings2).on_click(cx.listener(
                        |s, _, _, cx| {
                            s.workspace_panel = !s.workspace_panel;
                            cx.notify();
                        },
                    )),
                )
                .children(self.workspace_panel.then(|| self.workspace_controls(cx)));
        } else if let Some((_, t)) = self.table_target()
            && let Some(pick) = self.table_pick.as_ref()
        {
            body = body
                .child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(PALETTE.selected))
                        .text_color(rgb(ACCENT))
                        .child(format!(
                            "{} 行 × {} 列   ·   R{} : C{}",
                            t.rows.len(),
                            t.columns.len(),
                            pick.focus.0 + 1,
                            pick.focus.1 + 1
                        )),
                )
                .child(section("单元格对齐"))
                .child(
                    div().flex().gap_1().children(
                        [("left", "左对齐"), ("center", "居中"), ("right", "右对齐")]
                            .into_iter()
                            .map(|(op, name)| {
                                Button::new(op).ghost().small().label(name).on_click(
                                    cx.listener(move |s, _, _, cx| s.table_action(op, cx)),
                                )
                            }),
                    ),
                )
                .child(section("行高 / 列宽"))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(Input::new(&self.table_fields[2]).small().w(px(96.)))
                        .child(Input::new(&self.table_fields[3]).small().w(px(96.))),
                )
                .child(
                    div().flex().gap_2().children(
                        [("height", "设置行高"), ("width", "设置列宽")]
                            .into_iter()
                            .map(|(op, name)| {
                                Button::new(op).small().label(name).on_click(
                                    cx.listener(move |s, _, _, cx| s.table_action(op, cx)),
                                )
                            }),
                    ),
                )
                .child(section("表格样式"))
                .children(
                    [(4, "内边距"), (5, "边框宽度"), (6, "边框颜色"), (7, "字号")]
                        .into_iter()
                        .map(|(i, name)| {
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(name)
                                .child(Input::new(&self.table_fields[i]).small().w(px(116.)))
                        }),
                )
                .child(
                    Button::new("apply-table-style")
                        .small()
                        .label("应用表格样式")
                        .on_click(cx.listener(|s, _, _, cx| s.table_action("style", cx))),
                )
                .child(section("所选单元格填充"))
                .child(Input::new(&self.table_fields[8]).small())
                .child(
                    div().flex().gap_2().children(
                        [("fill", "应用填充"), ("inherit", "恢复继承")]
                            .into_iter()
                            .map(|(op, name)| {
                                Button::new(op).small().label(name).on_click(
                                    cx.listener(move |s, _, _, cx| s.table_action(op, cx)),
                                )
                            }),
                    ),
                )
                .child(label(
                    "双击输入 · Tab 下一格 · Shift 选择区域\n拖动分隔线调整尺寸 · Alt 拖动整个表格",
                ));
        } else if self.selection.is_empty() {
            body = body
                .child(
                    div()
                        .py_8()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .text_color(rgb(PALETTE.muted))
                        .child(gpui_kit::component::Icon::from(I::MousePointer2).size(px(28.)))
                        .child("选择对象以编辑格式"),
                )
                .child(section("当前幻灯片"))
                .child(format!(
                    "{} × {}",
                    self.session.deck.width, self.session.deck.height
                ))
                .child(
                    tool("insert-text-hint", "添加文本框", I::Type).on_click(cx.listener(
                        |s, _, _, cx| {
                            s.command(Command::AddObject(s.page, edit::object("text")), cx)
                        },
                    )),
                );
        } else {
            body = body
                .child(section("位置与大小"))
                .child(self.property_pair(0, "水平位置 · pt", 1, "垂直位置 · pt"))
                .child(self.property_pair(2, "宽度 · pt", 3, "高度 · pt"))
                .child(self.property_pair(6, "旋转 · °", 7, "不透明度 · 0–1"))
                .child(section("填充与边框"))
                .child(self.color_field("object-fill", "填充颜色", ColorTarget::Fill, cx))
                .child(self.color_field("object-stroke", "边框颜色", ColorTarget::Stroke, cx))
                .child(field("线宽 · pt", &self.fields[9]))
                .child(section("文字"))
                .child(self.font_menu(cx))
                .child(field("自定义字体", &self.fields[12]))
                .child(field("字号 · pt", &self.author_fields[8]))
                .child(self.color_field("inspector-text-color", "文字颜色", ColorTarget::Text, cx))
                .child(
                    Button::new("apply-text-color")
                        .ghost()
                        .small()
                        .label("应用文字格式")
                        .on_click(cx.listener(|s, _, w, cx| s.home_text("font", w, cx))),
                )
                .child(
                    Button::new("apply")
                        .primary()
                        .label("应用对象属性")
                        .on_click(cx.listener(|s, _, _, cx| s.apply(cx))),
                );
            body = body.child(section("对齐与分布")).child(
                div().flex().flex_wrap().gap_1().children(
                    [
                        ("左对齐", geometry::Arrange::Left),
                        ("水平居中", geometry::Arrange::Center),
                        ("右对齐", geometry::Arrange::Right),
                        ("顶端对齐", geometry::Arrange::Top),
                        ("垂直居中", geometry::Arrange::Middle),
                        ("底端对齐", geometry::Arrange::Bottom),
                        ("横向分布", geometry::Arrange::Horizontal),
                        ("纵向分布", geometry::Arrange::Vertical),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, mode))| {
                        Button::new(("arrange", i))
                            .ghost()
                            .small()
                            .label(name)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.finish_text(cx);
                                match geometry::arrange(&s.selected_objects(), mode) {
                                    Ok(objects) => {
                                        s.command(Command::ReplaceMany(s.page, objects), cx)
                                    }
                                    Err(e) => {
                                        s.message = e;
                                        cx.notify();
                                    }
                                }
                            }))
                    }),
                ),
            );
            if self.selected_objects().iter().any(|o| o.kind == "image") {
                body = body
                    .child(
                        tool("replace-image", "替换图片", I::Image).on_click(
                            cx.listener(|s, _, w, cx| s.file_dialog("replace-image", w, cx)),
                        ),
                    )
                    .child(label("图片裁剪 · 左 / 上 / 宽 / 高"))
                    .children(self.crop_fields.iter().map(|f| Input::new(f).small()))
                    .child(
                        Button::new("crop")
                            .small()
                            .label("应用裁剪")
                            .on_click(cx.listener(|s, _, _, cx| s.crop_media(false, cx))),
                    )
                    .child(
                        Button::new("crop-reset")
                            .ghost()
                            .small()
                            .label("重置裁剪")
                            .on_click(cx.listener(|s, _, _, cx| s.crop_media(true, cx))),
                    )
            }
        }
        p = p.child(body);
        p.into_any_element()
    }
    fn property_pair(&self, a: usize, al: &str, b: usize, bl: &str) -> Div {
        div()
            .flex()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(label(SharedString::from(al.to_owned())))
                    .child(
                        Input::new(&self.fields[a])
                            .aria_label(al.to_owned())
                            .h(px(30.)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(label(SharedString::from(bl.to_owned())))
                    .child(
                        Input::new(&self.fields[b])
                            .aria_label(bl.to_owned())
                            .h(px(30.)),
                    ),
            )
    }
}

impl Editor {
    fn layout_preview(&self, layout: &joydeck::design::Layout) -> Div {
        let width = 104.;
        let height = 58.;
        let mut frames = BTreeMap::new();
        if let Some(master) = self.session.deck.design.masters.get(&layout.master) {
            for p in master.placeholders.iter().chain(&layout.placeholders) {
                if let Some(frame) = p.style.frame {
                    frames.insert((p.kind.clone(), p.index), frame);
                }
            }
        }
        div()
            .relative()
            .w(px(width))
            .h(px(height))
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(PALETTE.border))
            .overflow_hidden()
            .children(frames.values().map(|r| {
                div()
                    .absolute()
                    .left(px(r[0] / self.session.deck.width * width))
                    .top(px(r[1] / self.session.deck.height * height))
                    .w(px(r[2] / self.session.deck.width * width))
                    .h(px(r[3] / self.session.deck.height * height))
                    .border_1()
                    .border_color(rgb(PALETTE.muted))
                    .bg(rgb(PALETTE.chrome))
            }))
    }
}
