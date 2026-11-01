//! Task tools; activation always uses the existing editor commands.
use super::ribbon_menus::ColorTarget;
use super::ui::*;
use super::*;
use gpui_kit::{
    assets::IconName as I,
    component::{Selectable, Sizable, button::ButtonVariants},
};
impl Editor {
    pub(super) fn ribbon_content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.ribbon == "设计" {
            return self.design_ribbon(cx);
        }
        if matches!(
            self.ribbon.as_str(),
            "图表"
                | "图示"
                | "动画"
                | "切换"
                | "幻灯片放映"
                | "审阅"
                | "媒体"
                | "输出"
                | "比较"
                | "维护"
        ) {
            return div()
                .h(px(96.))
                .flex_shrink_0()
                .px_4()
                .py_2()
                .bg(rgb(PALETTE.surface))
                .border_b_1()
                .border_color(rgb(PALETTE.border))
                .child(if matches!(self.ribbon.as_str(), "比较" | "维护") {
                    div()
                        .child(if self.ribbon == "比较" {
                            "比较文稿与接受对象更改"
                        } else {
                            "布局偏好、最近文件与帮助"
                        })
                        .into_any_element()
                } else if matches!(self.ribbon.as_str(), "审阅" | "媒体" | "输出") {
                    self.extended_ribbon(cx)
                } else {
                    self.author_ribbon(cx)
                })
                .into_any_element();
        }
        // Keep each tab in its own frame: unoptimized GPUI builders retain large
        // temporaries, including those for branches that are not being rendered.
        let content = match self.ribbon.as_str() {
            "插入" => self.insert_ribbon(cx),
            "表格" => self.table_ribbon(cx),
            "视图" => self.view_ribbon(cx),
            "文件" => self.file_ribbon(cx),
            _ => self.home_ribbon(cx),
        };
        div()
            .id("ribbon-content")
            .h(px(96.))
            .flex_shrink_0()
            .px_4()
            .py_2()
            .bg(rgb(PALETTE.surface))
            .border_b_1()
            .border_color(rgb(PALETTE.border))
            .overflow_x_scroll()
            .child(content)
            .into_any_element()
    }

    fn insert_ribbon(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_start()
            .gap_1()
            .child(group(
                "幻灯片",
                large_tool("insert-slide", "新建幻灯片", I::FilePlus)
                    .on_click(cx.listener(|s, _, _, cx| s.command(Command::AddPage, cx))),
            ))
            .child(group(
                "表格",
                large_tool("insert-table", "表格", I::Table2)
                    .on_click(cx.listener(|s, _, w, cx| s.table_dialog(w, cx))),
            ))
            .child(group(
                "图像",
                large_tool("image-picker", "图片", I::Image)
                    .on_click(cx.listener(|s, _, w, cx| s.file_dialog("image", w, cx))),
            ))
            .child(group(
                "插图",
                div()
                    .flex()
                    .gap_1()
                    .child(self.shape_menu(cx))
                    .child(
                        large_tool("insert-diagram", "SmartArt 图示", I::Network).on_click(
                            cx.listener(|s, _, w, cx| s.insert_authored("process", w, cx)),
                        ),
                    )
                    .child(
                        large_tool("insert-chart", "图表", I::ChartColumn).on_click(
                            cx.listener(|s, _, w, cx| s.insert_authored("column", w, cx)),
                        ),
                    ),
            ))
            .child(group(
                "文本",
                div()
                    .flex()
                    .gap_1()
                    .child(large_tool("text", "文本框", I::Type).on_click(cx.listener(
                        |s, _, _, cx| {
                            s.command(Command::AddObject(s.page, edit::object("text")), cx)
                        },
                    ))),
            ))
            .child(group(
                "形状库",
                div().flex().flex_col().gap_1().children(
                    [
                        [
                            ("roundRect", "圆角矩形", I::Square),
                            ("ellipse", "椭圆", I::Circle),
                        ],
                        [
                            ("line", "直线", I::Minus),
                            ("arrow", "箭头", I::ArrowUpRight),
                        ],
                    ]
                    .into_iter()
                    .map(|row| {
                        div()
                            .flex()
                            .children(row.into_iter().map(|(kind, name, icon)| {
                                tool(kind, name, icon).on_click(cx.listener(move |s, _, _, cx| {
                                    s.command(Command::AddObject(s.page, edit::object(kind)), cx)
                                }))
                            }))
                    }),
                ),
            ))
    }

    fn table_ribbon(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_start()
            .gap_1()
            .child(group(
                "行与列",
                div().flex().gap_1().children(
                    [
                        ("row+", "插入行"),
                        ("col+", "插入列"),
                        ("row-", "删除行"),
                        ("col-", "删除列"),
                    ]
                    .into_iter()
                    .map(|(op, name)| {
                        Button::new(op)
                            .ghost()
                            .small()
                            .label(name)
                            .disabled(self.table_target().is_none())
                            .on_click(cx.listener(move |s, _, _, cx| s.table_action(op, cx)))
                    }),
                ),
            ))
            .child(group(
                "单元格",
                div().flex().gap_1().children(
                    [
                        ("merge", "合并"),
                        ("split", "拆分"),
                        ("rows=", "均分行"),
                        ("cols=", "均分列"),
                    ]
                    .into_iter()
                    .map(|(op, name)| {
                        Button::new(op)
                            .ghost()
                            .small()
                            .label(name)
                            .disabled(self.table_target().is_none())
                            .on_click(cx.listener(move |s, _, _, cx| s.table_action(op, cx)))
                    }),
                ),
            ))
            .child(group(
                "样式",
                div().flex().gap_1().children(
                    [("header", "标题行"), ("banded", "条纹行")]
                        .into_iter()
                        .map(|(op, name)| {
                            Button::new(op)
                                .ghost()
                                .small()
                                .label(name)
                                .disabled(self.table_target().is_none())
                                .on_click(cx.listener(move |s, _, _, cx| s.table_action(op, cx)))
                        }),
                ),
            ))
    }

    fn view_ribbon(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_start()
            .gap_1()
            .child(group(
                "工作区",
                div()
                    .flex()
                    .gap_1()
                    .child(
                        tool("new-window", "同文稿新窗口", I::PanelsTopLeft).on_click(cx.listener(
                            |s, _, _, cx| {
                                let _ = super::open_session(cx, Some(s.session.clone()));
                            },
                        )),
                    )
                    .child(
                        tool("new-document", "新文稿窗口", I::FilePlus).on_click(cx.listener(
                            |_, _, _, cx| {
                                let _ = super::open(cx);
                            },
                        )),
                    )
                    .child(tool("notes-view", "演讲者备注", I::MessageSquare).on_click(
                        cx.listener(|s, _, w, cx| {
                            s.show_notes = !s.show_notes;
                            s.sync_doc_fields(w, cx);
                            cx.notify();
                        }),
                    )),
            ))
            .child(group(
                "绘制",
                tool("renderer", "CPU / GPUI", I::Layers)
                    .on_click(cx.listener(|s, _, _, cx| s.toggle_renderer(cx))),
            ))
            .child(group(
                "缓存",
                tool("clear-gpu", "重建缓存", I::RefreshCw).on_click(cx.listener(|s, _, _, cx| {
                    s.gpu_cache.clear();
                    cx.notify();
                })),
            ))
    }

    fn file_ribbon(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_start()
            .gap_1()
            .child(group(
                "文稿",
                div()
                    .flex()
                    .gap_1()
                    .child(
                        tool("reset", "新建", I::FilePlus)
                            .on_click(cx.listener(|s, _, w, cx| s.reset(w, cx))),
                    )
                    .child(
                        tool("open", "打开", I::FolderOpen)
                            .on_click(cx.listener(|s, _, w, cx| s.file_dialog("open", w, cx))),
                    )
                    .child(
                        tool("save-as", "另存为", I::Save)
                            .on_click(cx.listener(|s, _, w, cx| s.file_dialog("save-as", w, cx))),
                    )
                    .child(
                        tool("recover", "恢复", I::RotateCcw)
                            .on_click(cx.listener(|s, _, w, cx| s.recovery_list(w, cx))),
                    ),
            ))
            .child(group(
                "导出",
                div().flex().gap_1().children(
                    [
                        ("pptx", "PowerPoint"),
                        ("pdf", "PDF"),
                        ("preserve", "保留源包"),
                    ]
                    .into_iter()
                    .map(|(op, name)| {
                        Button::new(op)
                            .ghost()
                            .small()
                            .label(name)
                            .on_click(cx.listener(move |s, _, w, cx| s.file_dialog(op, w, cx)))
                    }),
                ),
            ))
            .child(group(
                "更多",
                tool("export-options", "导出选项", I::Settings2).on_click(cx.listener(
                    |s, _, _, cx| {
                        s.delivery_panel = !s.delivery_panel;
                        cx.notify();
                    },
                )),
            ))
    }

    fn home_ribbon(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_start()
            .gap_1()
            .child(group(
                "剪贴板",
                div()
                    .flex()
                    .gap_1()
                    .child(
                        large_tool("paste", "粘贴", I::ClipboardPaste)
                            .on_click(cx.listener(|s, _, w, cx| s.paste_at_focus(w, cx))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                tool("copy", "复制", I::Copy)
                                    .on_click(cx.listener(|s, _, _, cx| s.copy_objects(cx))),
                            )
                            .child(
                                tool("delete-object", "删除", I::Trash)
                                    .disabled(self.selection.is_empty())
                                    .on_click(cx.listener(|s, _, _, cx| s.delete_selection(cx))),
                            ),
                    ),
            ))
            .child(group(
                "幻灯片",
                div()
                    .flex()
                    .gap_1()
                    .child(
                        large_tool("add-page", "新建幻灯片", I::FilePlus)
                            .on_click(cx.listener(|s, _, _, cx| s.command(Command::AddPage, cx))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                tool("copy-page", "复制幻灯片", I::Copy).on_click(
                                    cx.listener(|s, _, _, cx| s.page_command("copy", cx)),
                                ),
                            )
                            .child(tool("layout-pane", "版式", I::PanelsTopLeft).on_click(
                                cx.listener(|s, _, w, cx| {
                                    s.ribbon = "设计".into();
                                    s.sync_doc_fields(w, cx);
                                    cx.notify();
                                }),
                            )),
                    ),
            ))
            .child(group(
                "字体",
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(self.font_menu(cx))
                            .child(
                                Input::new(&self.author_fields[8])
                                    .aria_label("字号")
                                    .h(px(30.))
                                    .w(px(48.)),
                            )
                            .child(
                                tool("apply-font", "应用", I::Check).on_click(
                                    cx.listener(|s, _, w, cx| s.home_text("font", w, cx)),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .children(
                                [("bold", "粗体", I::Bold), ("italic", "斜体", I::Italic)]
                                    .into_iter()
                                    .map(|(op, name, icon)| {
                                        self.text_toggle(op, name, icon, false, cx)
                                    }),
                            )
                            .child(self.color_menu("text-color", "字色", ColorTarget::Text, cx)),
                    ),
            ))
            .child(group(
                "段落",
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div().flex().gap_1().children(
                            [
                                ("left", "左对齐", I::TextAlignStart),
                                ("center", "居中", I::TextAlignCenter),
                                ("right", "右对齐", I::TextAlignEnd),
                            ]
                            .into_iter()
                            .map(|(op, name, icon)| {
                                tool(op, "", icon)
                                    .accessibility_label(name)
                                    .tooltip(name)
                                    .on_click(
                                        cx.listener(move |s, _, w, cx| s.home_text(op, w, cx)),
                                    )
                            }),
                        ),
                    )
                    .child(caption_chrome("文字框的水平对齐")),
            ))
            .child(group(
                "绘图",
                div()
                    .flex()
                    .gap_1()
                    .child(
                        large_tool("format-pane", "对象格式", I::Paintbrush).on_click(cx.listener(
                            |s, _, w, cx| {
                                s.inspector_open = !s.inspector_open;
                                s.sync_fields(w, cx);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                tool("front", "置于顶层", I::BringToFront)
                                    .on_click(cx.listener(|s, _, _, cx| s.layer(true, cx))),
                            )
                            .child(
                                tool("back", "置于底层", I::SendToBack)
                                    .on_click(cx.listener(|s, _, _, cx| s.layer(false, cx))),
                            ),
                    ),
            ))
    }
}

impl Editor {
    fn text_toggle(
        &self,
        op: &'static str,
        name: &'static str,
        icon: I,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        let style = self
            .text_edit
            .as_ref()
            .map(|t| t.typing.clone())
            .or_else(|| {
                self.author_object().map(|(_, o)| {
                    o.runs
                        .first()
                        .map(|r| r.style.clone())
                        .unwrap_or_else(|| joydeck::text::Style::for_object(&o))
                })
            });
        let enabled =
            self.text_edit.is_some() || self.author_object().is_some_and(|(_, o)| o.kind == "text");
        let selected = style.is_some_and(|s| if op == "bold" { s.bold } else { s.italic });
        tool(op, if compact { "" } else { name }, icon)
            .accessibility_label(name)
            .tooltip(name)
            .selected(selected)
            .disabled(!enabled)
            .on_click(cx.listener(move |s, _, w, cx| s.home_text(op, w, cx)))
    }
    pub(super) fn compact_ribbon(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("compact-ribbon")
            .h(px(64.))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .bg(rgb(PALETTE.surface))
            .border_b_1()
            .border_color(rgb(PALETTE.border))
            .overflow_x_scroll()
            .child(
                tool("paste", "粘贴", I::ClipboardPaste)
                    .on_click(cx.listener(|s, _, w, cx| s.paste_at_focus(w, cx))),
            )
            .child(
                tool("add-page", "新建页", I::FilePlus)
                    .on_click(cx.listener(|s, _, _, cx| s.command(Command::AddPage, cx))),
            )
            .child(self.font_menu(cx))
            .child(
                Input::new(&self.author_fields[8])
                    .aria_label("字号")
                    .h(px(30.))
                    .w(px(48.)),
            )
            .child(
                tool("apply-font", "应用", I::Check)
                    .on_click(cx.listener(|s, _, w, cx| s.home_text("font", w, cx))),
            )
            .children(
                [("bold", "粗体", I::Bold), ("italic", "斜体", I::Italic)]
                    .into_iter()
                    .map(|(op, name, icon)| self.text_toggle(op, name, icon, true, cx)),
            )
            .child(self.color_menu("text-color", "字色", ColorTarget::Text, cx))
            .child(
                tool("compact-format", "对象", I::Settings2).on_click(cx.listener(
                    |s, _, w, cx| {
                        s.inspector_open = !s.inspector_open;
                        s.sync_fields(w, cx);
                        cx.notify();
                    },
                )),
            )
            .child(self.home_more(cx))
            .into_any_element()
    }
    fn home_more(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
        let weak = cx.entity().downgrade();
        tool("home-more", "更多工具", I::Ellipsis)
            .dropdown_menu(move |mut menu, _, _| {
                for (op, name) in [
                    ("copy", "复制对象"),
                    ("delete", "删除对象"),
                    ("copy-page", "复制幻灯片"),
                    ("left", "文字左对齐"),
                    ("center", "文字居中"),
                    ("right", "文字右对齐"),
                    ("front", "置于顶层"),
                    ("back", "置于底层"),
                    ("design", "版式与主题"),
                ] {
                    let weak = weak.clone();
                    menu = menu.item(PopupMenuItem::new(name).on_click(move |_, w, cx| {
                        let _ = weak.update(cx, |s, cx| match op {
                            "copy" => s.copy_objects(cx),
                            "delete" => s.delete_selection(cx),
                            "copy-page" => s.page_command("copy", cx),
                            "front" => s.layer(true, cx),
                            "back" => s.layer(false, cx),
                            "design" => s.select_ribbon("设计", w, cx),
                            _ => s.home_text(op, w, cx),
                        });
                    }));
                }
                menu
            })
            .into_any_element()
    }
    fn design_ribbon(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut themes = div().flex().gap_2();
        for theme in self.session.deck.design.themes.values().take(8) {
            themes = themes.child(
                joyoffice_ui::card(PALETTE).p_2().min_w(px(116.)).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .children(theme.colors.values().take(6).map(|c| swatch(c))),
                        )
                        .child(label(theme.name.clone())),
                ),
            );
        }
        if self.session.deck.design.themes.is_empty() {
            themes = themes
                .child(
                    large_tool("starter", "创建设计", I::Palette)
                        .on_click(cx.listener(|s, _, _, cx| s.design_op("starter", cx))),
                )
                .child(label("创建文稿主题\n再选择版式与配色").pt_2());
        }
        div()
            .id("design-ribbon")
            .h(px(96.))
            .flex_shrink_0()
            .px_3()
            .py_2()
            .bg(rgb(PALETTE.surface))
            .border_b_1()
            .border_color(rgb(PALETTE.border))
            .overflow_x_scroll()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(group("文稿主题", themes))
                    .child(group(
                        "版式",
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                large_tool("apply-layout", "应用版式", I::PanelsTopLeft)
                                    .disabled(self.session.deck.design.layouts.is_empty())
                                    .on_click(cx.listener(|s, _, _, cx| s.design_op("apply", cx))),
                            )
                            .child(
                                large_tool("reset-layout", "重置版式", I::RotateCcw)
                                    .disabled(self.session.deck.design.layouts.is_empty())
                                    .on_click(cx.listener(|s, _, _, cx| s.design_op("reset", cx))),
                            ),
                    ))
                    .child(group(
                        "幻灯片",
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                tool("page-size", "应用页面尺寸", I::Maximize)
                                    .on_click(cx.listener(|s, _, _, cx| s.page_size(cx))),
                            )
                            .child(
                                tool("hidden", "隐藏 / 显示", I::EyeOff).on_click(
                                    cx.listener(|s, _, _, cx| s.page_command("hide", cx)),
                                ),
                            ),
                    )),
            )
            .into_any_element()
    }
}
