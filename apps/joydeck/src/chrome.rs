//! Presentation workspace: task ribbon, slide rail, stage and contextual inspector.
use super::*;
use gpui_kit::{
    assets::IconName as I,
    component::{Sizable, button::ButtonVariants},
};
const ACCENT: u32 = 0xb7472a;
fn caption_chrome(s: &str) -> Div {
    label(s.to_owned())
}
fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.))
        .text_color(rgb(0x717171))
        .child(text.into())
}
fn group(name: &str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .pr_3()
        .mr_2()
        .border_r_1()
        .border_color(rgb(0xe8ebf1))
        .child(div().h(px(54.)).child(content))
        .child(label(SharedString::from(name.to_string())))
}
fn tool(id: &'static str, name: &'static str, icon: I) -> Button {
    let accessible = if name.is_empty() {
        match id {
            "save" => "保存",
            "undo" => "撤销",
            "redo" => "重做",
            "rail-add" => "新建幻灯片",
            _ => id,
        }
    } else {
        name
    };
    Button::new(id)
        .ghost()
        .small()
        .icon(icon)
        .label(name)
        .accessibility_label(accessible)
        .tooltip(accessible)
}
pub(super) fn large_tool(id: &'static str, name: &'static str, icon: I) -> Button {
    Button::new(id)
        .accessibility_label(name)
        .tooltip(name)
        .ghost()
        .h(px(54.))
        .px_2()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(gpui_kit::component::Icon::from(icon).size(px(27.)))
                .child(div().text_size(px(12.)).child(name)),
        )
}
impl Editor {
    fn pane_width(&self) -> f32 {
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
            320.
        } else if self.inspector_open || matches!(self.ribbon.as_str(), "文件" | "设计" | "表格")
        {
            280.
        } else {
            0.
        }
    }
    pub(super) fn home_text(&mut self, op: &str, w: &mut Window, cx: &mut Context<Self>) {
        if ["left", "center", "right"].contains(&op) {
            self.finish_text(cx);
            if let Some((i, mut o)) = self.author_object() {
                o.text_align = op.into();
                self.command(Command::ReplaceObject(self.page, i, o), cx);
            }
            return;
        }
        if self.text_edit.is_none()
            && let Some(i) = self.selected
        {
            self.begin_text(i, w, cx);
            if let Some(t) = &mut self.text_edit {
                t.selection = 0..t.object.text.len();
            }
        }
        self.format_text(op, w, cx);
    }

    pub(super) fn canvas_scale(&self, w: &Window) -> f32 {
        let vw = f32::from(w.viewport_size().width);
        let vh = f32::from(w.viewport_size().height);
        let scale = ((vw - 250. - self.pane_width() - 48.) / self.session.deck.width)
            .min(
                (vh - 142. - 28. - 48. - if self.show_notes { 124. } else { 32. })
                    / self.session.deck.height,
            )
            .max(0.1)
            * self.zoom;
        // Integral aspect-ratio steps avoid introducing a one-pixel distortion at fit.
        let mut a = self.session.deck.width.round() as u32;
        let mut b = self.session.deck.height.round() as u32;
        while b != 0 {
            let n = a % b;
            a = b;
            b = n;
        }
        let step = self.session.deck.width / a.max(1) as f32;
        if step <= 32. {
            ((self.session.deck.width * scale / step).floor().max(1.) * step)
                / self.session.deck.width
        } else {
            scale
        }
    }
    pub(super) fn file_dialog(&mut self, kind: &str, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let kind = kind.to_owned();
        let window = w.window_handle();
        if kind == "open" || kind == "image" || kind == "replace-image" {
            let receiver = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(
                    if kind.contains("image") {
                        "插入图片"
                    } else {
                        "打开演示文稿"
                    }
                    .into(),
                ),
            });
            cx.spawn(async move |view, cx| {
                if let Ok(Ok(Some(paths))) = receiver.await
                    && let Some(path) = paths.first()
                {
                    let path = path.clone();
                    let _ = window.update(cx, |_, w, cx| {
                        let _ = view.update(cx, |s, cx| {
                            if kind.contains("image") {
                                s.media_path.update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.import_media(None, kind == "replace-image", w, cx);
                            } else if path
                                .extension()
                                .is_some_and(|e| e.eq_ignore_ascii_case("pptx"))
                            {
                                s.delivery_fields[0].update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.delivery("import", w, cx);
                            } else {
                                s.path.update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.open(false, w, cx);
                            }
                        });
                    });
                }
            })
            .detach();
        } else {
            let ext = match kind.as_str() {
                "pptx" | "preserve" => "pptx",
                "pdf" | "pdf-notes" => "pdf",
                _ => "joydeck",
            };
            let suggestion = format!("演示文稿.{ext}");
            let directory = self
                .disk
                .as_ref()
                .and_then(|p| p.0.parent())
                .unwrap_or_else(|| std::path::Path::new("."));
            let receiver = cx.prompt_for_new_path(directory, Some(&suggestion));
            cx.spawn(async move |view, cx| {
                if let Ok(Ok(Some(path))) = receiver.await {
                    let _ = window.update(cx, |_, w, cx| {
                        let _ = view.update(cx, |s, cx| {
                            if kind == "save" || kind == "save-as" {
                                s.path.update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.save(&Save, w, cx);
                            } else {
                                s.delivery_fields[0].update(cx, |f, cx| {
                                    f.set_value(path.to_string_lossy().to_string(), w, cx)
                                });
                                s.delivery(&kind, w, cx);
                            }
                        });
                    });
                }
            })
            .detach();
        }
    }
    fn ribbon_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let row = div().flex().items_start().gap_1();
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
                .h(px(80.))
                .px_4()
                .py_2()
                .bg(rgb(0xffffff))
                .border_b_1()
                .border_color(rgb(0xd4d4d4))
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
        let content = match self.ribbon.as_str() {
            "插入" => row
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
                        .child(large_tool("insert-chart", "图表", I::ChartColumn).on_click(
                            cx.listener(|s, _, w, cx| s.insert_authored("column", w, cx)),
                        )),
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
                                    tool(kind, name, icon).on_click(cx.listener(
                                        move |s, _, _, cx| {
                                            s.command(
                                                Command::AddObject(s.page, edit::object(kind)),
                                                cx,
                                            )
                                        },
                                    ))
                                }))
                        }),
                    ),
                )),
            "表格" => row
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
                                    .on_click(
                                        cx.listener(move |s, _, _, cx| s.table_action(op, cx)),
                                    )
                            }),
                    ),
                )),
            "设计" => row
                .child(group(
                    "主题与版式",
                    div().flex().gap_1().children(
                        [
                            ("starter", "创建设计"),
                            ("apply", "应用版式"),
                            ("reset", "重置版式"),
                            ("theme", "更新主题"),
                        ]
                        .into_iter()
                        .map(|(op, name)| {
                            Button::new(op)
                                .ghost()
                                .small()
                                .label(name)
                                .on_click(cx.listener(move |s, _, _, cx| s.design_op(op, cx)))
                        }),
                    ),
                ))
                .child(group(
                    "幻灯片",
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            tool("page-size", "应用页面尺寸", I::Maximize)
                                .on_click(cx.listener(|s, _, _, cx| s.page_size(cx))),
                        )
                        .child(
                            tool("hidden", "隐藏 / 显示", I::EyeOff)
                                .on_click(cx.listener(|s, _, _, cx| s.page_command("hide", cx))),
                        ),
                )),
            "视图" => row
                .child(group(
                    "工作区",
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            tool("new-window", "同文稿新窗口", I::PanelsTopLeft).on_click(
                                cx.listener(|s, _, _, cx| {
                                    let _ = super::open_session(cx, Some(s.session.clone()));
                                }),
                            ),
                        )
                        .child(tool("new-document", "新文稿窗口", I::FilePlus).on_click(
                            cx.listener(|_, _, _, cx| {
                                let _ = super::open(cx);
                            }),
                        ))
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
                    tool("clear-gpu", "重建缓存", I::RefreshCw).on_click(cx.listener(
                        |s, _, _, cx| {
                            s.gpu_cache.clear();
                            cx.notify();
                        },
                    )),
                )),
            "文件" => row
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
                            tool("save-as", "另存为", I::Save).on_click(
                                cx.listener(|s, _, w, cx| s.file_dialog("save-as", w, cx)),
                            ),
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
                )),
            _ => row
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
                                        .on_click(
                                            cx.listener(|s, _, _, cx| s.delete_selection(cx)),
                                        ),
                                ),
                        ),
                ))
                .child(group(
                    "幻灯片",
                    div()
                        .flex()
                        .gap_1()
                        .child(
                            large_tool("add-page", "新建幻灯片", I::FilePlus).on_click(
                                cx.listener(|s, _, _, cx| s.command(Command::AddPage, cx)),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(tool("copy-page", "复制幻灯片", I::Copy).on_click(
                                    cx.listener(|s, _, _, cx| s.page_command("copy", cx)),
                                ))
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
                                .child(Input::new(&self.fields[12]).small().w(px(132.)))
                                .child(self.font_menu(cx))
                                .child(Input::new(&self.author_fields[8]).small().w(px(48.)))
                                .child(tool("apply-font", "应用", I::Check).on_click(
                                    cx.listener(|s, _, w, cx| s.home_text("font", w, cx)),
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .children(
                                    [("bold", "粗体", I::Bold), ("italic", "斜体", I::Italic)]
                                        .into_iter()
                                        .map(|(op, name, icon)| {
                                            tool(op, name, icon).on_click(cx.listener(
                                                move |s, _, w, cx| s.home_text(op, w, cx),
                                            ))
                                        }),
                                )
                                .child(Input::new(&self.fields[13]).small().w(px(104.))),
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
                                    tool(op, name, icon).on_click(
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
                            large_tool("format-pane", "形状格式", I::Paintbrush).on_click(
                                cx.listener(|s, _, w, cx| {
                                    s.inspector_open = !s.inspector_open;
                                    s.sync_fields(w, cx);
                                    cx.notify();
                                }),
                            ),
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
                )),
        };
        div()
            .id("ribbon-content")
            .h(px(80.))
            .px_4()
            .py_1()
            .bg(rgb(0xffffff))
            .border_b_1()
            .border_color(rgb(0xdde2eb))
            .overflow_x_scroll()
            .child(content)
            .into_any_element()
    }
    fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut p = div()
            .id("inspector")
            .w(px(280.))
            .flex_shrink_0()
            .bg(rgb(0xffffff))
            .border_l_1()
            .border_color(rgb(0xdde2eb))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(48.))
                    .px_5()
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(if self.ribbon == "设计" {
                        "设计与版式"
                    } else if self.table_target().is_some() {
                        "表格格式"
                    } else if self.ribbon == "文件" {
                        "文稿与导出"
                    } else {
                        "对象格式"
                    }),
            );
        let mut body = div()
            .id("inspector-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_5()
            .pb_5()
            .flex()
            .flex_col()
            .gap_4()
            .text_size(px(12.));
        if self.ribbon == "文件" {
            body = body
                .child(label("文稿信息"))
                .child("标题")
                .child(Input::new(&self.doc_fields[0]).small())
                .child("作者")
                .child(Input::new(&self.doc_fields[1]).small())
                .child(
                    Button::new("properties")
                        .small()
                        .label("更新文稿信息")
                        .on_click(cx.listener(|s, _, _, cx| s.properties(cx))),
                )
                .children(self.delivery_panel.then(|| self.delivery_controls(cx)));
        } else if self.ribbon == "设计" {
            body = body
                .child(label("页面尺寸"))
                .child(self.property_pair(10, "宽", 11, "高"))
                .child(label("主题色与背景"))
                .child(Input::new(&self.doc_fields[7]).small())
                .child(Input::new(&self.doc_fields[11]).small())
                .child(label("版式"))
                .children(self.session.deck.design.layouts.iter().map(|(id, l)| {
                    let id = id.clone();
                    Button::new(SharedString::from(format!("layout-{id}")))
                        .small()
                        .label(l.name.clone())
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.chosen_layout = id.clone();
                            s.design_op("apply", cx);
                        }))
                }))
                .child(
                    tool("advanced-design", "管理母版与文稿", I::Settings2).on_click(cx.listener(
                        |s, _, _, cx| {
                            s.workspace_panel = !s.workspace_panel;
                            cx.notify();
                        },
                    )),
                )
                .children(self.workspace_panel.then(|| self.workspace_controls(cx)));
        } else if let Some((_, t)) = self.table_target() {
            let pick = self.table_pick.as_ref().unwrap();
            body = body
                .child(
                    div()
                        .p_3()
                        .rounded_lg()
                        .bg(rgb(0xfff0ea))
                        .text_color(rgb(ACCENT))
                        .child(format!(
                            "{} 行 × {} 列   ·   R{} : C{}",
                            t.rows.len(),
                            t.columns.len(),
                            pick.focus.0 + 1,
                            pick.focus.1 + 1
                        )),
                )
                .child(label("单元格对齐"))
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
                .child(label("行高 / 列宽"))
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
                .child(label("表格样式"))
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
                .child(label("所选单元格填充"))
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
                        .text_color(rgb(0x9aa5b5))
                        .child(gpui_kit::component::Icon::from(I::MousePointer2).size(px(28.)))
                        .child("选择对象以编辑格式"),
                )
                .child(label("当前幻灯片"))
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
                .child(label("位置与大小"))
                .child(self.property_pair(0, "X", 1, "Y"))
                .child(self.property_pair(2, "宽", 3, "高"))
                .child(self.property_pair(6, "旋转", 7, "透明度"))
                .child(label("填充与边框"))
                .child(Input::new(&self.fields[4]).small())
                .child(self.property_pair(8, "边框色", 9, "线宽"))
                .child(label("文字"))
                .child(Input::new(&self.fields[13]).small())
                .child(
                    Button::new("apply-text-color")
                        .ghost()
                        .small()
                        .label("应用文字颜色 / 字体")
                        .disabled(self.text_edit.is_none())
                        .on_click(cx.listener(|s, _, w, cx| s.format_text("style", w, cx))),
                )
                .child(
                    Button::new("apply")
                        .small()
                        .label("应用对象属性")
                        .on_click(cx.listener(|s, _, _, cx| s.apply(cx))),
                );
            body = body.child(label("对齐与分布")).child(
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
                    .child(Input::new(&self.fields[a]).small()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(label(SharedString::from(bl.to_owned())))
                    .child(Input::new(&self.fields[b]).small()),
            )
    }

    fn slide_surface(&self, scale: f32, cx: &mut Context<Self>) -> AnyElement {
        let objects = self.display_objects();
        let width = self.session.deck.width * scale;
        let height = self.session.deck.height * scale;
        div()
            .relative()
            .w(px(width))
            .h(px(height))
            .bg(rgb(0xffffff))
            .children(
                self.image
                    .as_ref()
                    .filter(|_| !self.accelerated)
                    .map(|i| img(i.clone()).w(px(width)).h(px(height))),
            )
            .id("slide-surface")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |s, e: &MouseDownEvent, w, cx| s.down(e, scale, w, cx)),
            )
            .child({
                let weak = cx.entity().downgrade();
                let paint_weak = weak.clone();
                canvas(
                    move |bounds, _, cx| {
                        let _ = weak.update(cx, |s, _| {
                            s.canvas_origin =
                                (f32::from(bounds.origin.x), f32::from(bounds.origin.y))
                        });
                    },
                    move |bounds, _, w, cx| {
                        let _ = paint_weak.update(cx, |s, cx| {
                            s.paint_scene(bounds, scale, w, cx);
                            s.paint_text(bounds, scale, w, cx)
                        });
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            })
            .children(
                objects
                    .iter()
                    .filter(|o| self.selection.contains(&o.id))
                    .map(|o| {
                        let r = geometry::bounds(o);
                        div()
                            .absolute()
                            .left(px(r.x * scale))
                            .top(px(r.y * scale))
                            .w(px(r.w * scale))
                            .h(px(r.h * scale))
                            .border_1()
                            .border_color(rgb(ACCENT))
                    }),
            )
            .children({
                let selected: Vec<_> = objects
                    .iter()
                    .filter(|o| self.selection.contains(&o.id))
                    .cloned()
                    .collect();
                if selected.is_empty() {
                    Vec::new()
                } else {
                    let r = geometry::union(&selected);
                    let mut handles = Vec::new();
                    for (hx, hy) in [
                        (-1, -1),
                        (0, -1),
                        (1, -1),
                        (-1, 0),
                        (1, 0),
                        (-1, 1),
                        (0, 1),
                        (1, 1),
                    ] {
                        handles.push(
                            div()
                                .absolute()
                                .left(px((r.x + r.w * (hx as f32 + 1.) / 2.) * scale - 5.))
                                .top(px((r.y + r.h * (hy as f32 + 1.) / 2.) * scale - 5.))
                                .w(px(10.))
                                .h(px(10.))
                                .bg(rgb(ACCENT)),
                        );
                    }
                    handles.push(
                        div()
                            .absolute()
                            .left(px((r.x + r.w / 2.) * scale - 5.))
                            .top(px(r.y * scale - 29.))
                            .w(px(10.))
                            .h(px(10.))
                            .rounded_full()
                            .bg(rgb(ACCENT)),
                    );
                    handles
                }
            })
            .children(self.gesture.as_ref().filter(|g| g.mode.is_none()).map(|g| {
                div()
                    .absolute()
                    .left(px(g.start.0.min(g.end.0) * scale))
                    .top(px(g.start.1.min(g.end.1) * scale))
                    .w(px((g.end.0 - g.start.0).abs() * scale))
                    .h(px((g.end.1 - g.start.1).abs() * scale))
                    .border_1()
                    .border_color(rgb(ACCENT))
            }))
            .children(self.table_overlays(scale))
            .shadow_lg()
            .into_any_element()
    }
    fn table_overlays(&self, scale: f32) -> Vec<Div> {
        let Some((o, t)) = self.table_target() else {
            return vec![];
        };
        if o.rotation != 0. {
            return vec![];
        }
        let p = self.table_pick.as_ref().unwrap();
        let Ok(range) = t.selection_range(p.anchor, p.focus) else {
            return vec![];
        };
        let mut seen = BTreeSet::new();
        let mut result = vec![];
        for r in range.top..=range.bottom {
            for c in range.left..=range.right {
                if let Ok((rr, cc)) = t.owner(r, c)
                    && seen.insert((rr, cc))
                    && let Ok([x, y, w, h]) = t.bounds(rr, cc)
                {
                    result.push(
                        div()
                            .absolute()
                            .left(px((o.x + x) * scale))
                            .top(px((o.y + y) * scale))
                            .w(px(w * scale))
                            .h(px(h * scale))
                            .border_2()
                            .border_color(rgb(ACCENT)),
                    );
                }
            }
        }
        result
    }
    pub(super) fn render_workspace(
        &mut self,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.focus_initialized {
            self.focus.focus(w, cx);
            self.focus_initialized = true;
        }
        self.sync_document(cx);
        if self
            .control_service
            .as_ref()
            .is_some_and(|service| service.document_id != self.session.id)
        {
            self.control_service = None;
            self.message = "文稿已切换，本机控制已关闭".into();
        }
        if self.av_player.is_some()
            && (self.av_document_id != self.session.id
                || self
                    .av_slide
                    .as_ref()
                    .is_some_and(|id| id != &self.session.deck.slides[self.page].id)
                    && !self
                        .session
                        .deck
                        .playback
                        .clips
                        .get(self.clip_pick)
                        .is_some_and(|c| c.cross_slide))
        {
            self.av_player = None;
            self.av_image = None;
        }

        if self.doc_page != self.session.deck.slides[self.page].id {
            self.sync_doc_fields(w, cx);
        }
        if !w.is_window_active() && self.gesture.is_some() {
            self.cancel_gesture(&CancelGesture, w, cx);
        }
        self.request_render(cx);
        let pages = self.session.deck.slides.len();
        let scale = self.canvas_scale(w);
        self.last_scale = scale;
        let dirty = self.session.dirty();
        let title = self.session.deck.title.clone();
        let surface = self.slide_surface(scale, cx);
        let inspector = if matches!(
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
            Some(if matches!(self.ribbon.as_str(), "比较" | "维护") {
                self.maintenance_pane(cx)
            } else if matches!(self.ribbon.as_str(), "审阅" | "媒体" | "输出") {
                self.extended_pane(cx)
            } else {
                self.author_pane(cx)
            })
        } else if self.pane_width() > 0. {
            Some(self.inspector(cx))
        } else {
            None
        };
        let ribbon = self.ribbon_content(cx);
        div()
            .id("editor")
            .key_context("Editor")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xe9e9e9))
            .text_color(rgb(0x334155))
            .text_size(px(13.))
            .font_family("PingFang SC")
            .on_key_down(cx.listener(|s, e: &KeyDownEvent, w, cx| {
                if s.table_key(e, w, cx) {
                    return;
                }
                if s.text_edit.is_none()
                    && s.focus.is_focused(w)
                    && (e.keystroke.modifiers.platform || e.keystroke.modifiers.control)
                {
                    match e.keystroke.key.as_str() {
                        "c" => {
                            s.copy_objects(cx);
                            cx.stop_propagation();
                            return;
                        }
                        "v" => {
                            s.paste_media(w, cx);
                            cx.stop_propagation();
                            return;
                        }
                        _ => {}
                    }
                }
                if s.text_edit.is_none()
                    && s.focus.is_focused(w)
                    && matches!(e.keystroke.key.as_str(), "delete" | "backspace")
                {
                    s.delete_selection(cx);
                    cx.stop_propagation();
                    return;
                }
                s.text_key(e, w, cx);
            }))
            .on_drop::<ExternalPaths>(cx.listener(|s, paths: &ExternalPaths, w, cx| {
                if let Some(path) = paths.paths().first() {
                    s.media_path.update(cx, |f, cx| {
                        f.set_value(path.to_string_lossy().to_string(), w, cx)
                    });
                    s.import_media(None, false, w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &CellNext, w, cx| s.cell_tab(false, w, cx)))
            .on_action(cx.listener(|s, _: &CellPrevious, w, cx| s.cell_tab(true, w, cx)))
            .on_action(cx.listener(|_, _: &NewDocument, _, cx| {
                let _ = super::open(cx);
            }))
            .on_action(cx.listener(|s, _: &NewSlide, _, cx| s.command(Command::AddPage, cx)))
            .on_action(cx.listener(|s, _: &ShowFirst, w, cx| s.delivery("show-first", w, cx)))
            .on_action(cx.listener(|s, _: &ShowCurrent, w, cx| s.delivery("show-current", w, cx)))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(|s, _: &OpenFile, w, cx| s.file_dialog("open", w, cx)))
            .on_action(cx.listener(|s, _: &SaveAs, w, cx| s.file_dialog("save-as", w, cx)))
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::cancel_gesture))
            .on_mouse_move(cx.listener(|s, e, _, cx| s.move_pointer(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| {
                    s.end_table_resize(cx);
                    s.text_anchor = None;
                    s.end_gesture(cx);
                    s.page_drag = None;
                    s.sync_fields(w, cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|s, _, w, cx| {
                    s.table_resize = None;
                    s.table_drag = false;
                    s.page_drag = None;
                    s.cancel_gesture(&CancelGesture, w, cx);
                    s.visual_revision += 1;
                    s.pending = None;
                    cx.notify();
                }),
            )
            .on_action(cx.listener(|s, _: &crate::Quit, w, cx| {
                cx.stop_propagation();
                s.close(&Close, w, cx);
            }))
            .child(
                gpui_kit::component::TitleBar::new()
                    .h(px(30.))
                    .bg(rgb(0xf4f4f4))
                    .on_close_window({
                        let weak = cx.entity().downgrade();
                        move |_, w, cx| {
                            let _ = weak.update(cx, |s, cx| s.close(&Close, w, cx));
                        }
                    })
                    .child(
                        div()
                            .h(px(30.))
                            .w_full()
                            .flex_shrink_0()
                            .px_4()
                            .flex()
                            .items_center()
                            .gap_1()
                            .bg(rgb(0xf4f4f4))
                            .child(
                                div()
                                    .w(px(330.))
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        tool("save", "", I::Save).disabled(self.saving).on_click(
                                            cx.listener(|s, _, w, cx| s.save(&Save, w, cx)),
                                        ),
                                    )
                                    .child(
                                        tool("undo", "", I::Undo2)
                                            .disabled(!self.session.can_undo())
                                            .on_click(
                                                cx.listener(|s, _, w, cx| s.undo(&Undo, w, cx)),
                                            ),
                                    )
                                    .child(
                                        tool("redo", "", I::Redo2)
                                            .disabled(!self.session.can_redo())
                                            .on_click(
                                                cx.listener(|s, _, w, cx| s.redo(&Redo, w, cx)),
                                            ),
                                    )
                                    .child(label(if dirty {
                                        "未保存"
                                    } else {
                                        "已保存到本机"
                                    })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_center()
                                    .truncate()
                                    .text_size(px(12.))
                                    .child(if title.is_empty() {
                                        "未命名演示文稿 — JoyDeck".to_owned()
                                    } else {
                                        format!("{title} — JoyDeck")
                                    }),
                            )
                            .child(
                                div()
                                    .w(px(330.))
                                    .flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(tool("export", "导出", I::Upload).on_click(cx.listener(
                                        |s, _, _, cx| {
                                            s.ribbon = "文件".into();
                                            s.delivery_panel = true;
                                            cx.notify();
                                        },
                                    )))
                                    .child(tool("show", "放映", I::Presentation).on_click(
                                        cx.listener(|s, _, w, cx| {
                                            s.delivery("show-current", w, cx)
                                        }),
                                    )),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_1()
                    .bg(rgb(0xffffff))
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
                            "媒体",
                            "输出",
                            "视图",
                            "比较",
                            "维护",
                            "表格",
                            "图表",
                            "图示",
                        ]
                        .into_iter()
                        .filter(|name| match *name {
                            "表格" => {
                                self.ribbon == "表格"
                                    || self.author_object().is_some_and(|(_, o)| o.table.is_some())
                            }
                            "图表" => {
                                self.ribbon == "图表"
                                    || self.author_object().is_some_and(|(_, o)| o.chart.is_some())
                            }
                            "图示" => {
                                self.ribbon == "图示"
                                    || self
                                        .author_object()
                                        .is_some_and(|(_, o)| o.diagram.is_some())
                            }
                            _ => true,
                        })
                        .map(|name| {
                            let active = self.ribbon == name;
                            Button::new(name)
                                .ghost()
                                .small()
                                .label(match name {
                                    "图表" => "图表设计",
                                    "图示" => "SmartArt 设计",
                                    "表格" => "表格布局",
                                    _ => name,
                                })
                                .h(px(32.))
                                .px_4()
                                .border_b_2()
                                .border_color(rgb(if active { ACCENT } else { 0xffffff }))
                                .text_color(rgb(if active { ACCENT } else { 0x64748b }))
                                .on_click(cx.listener(move |s, _, w, cx| {
                                    if matches!(name, "文件" | "设计" | "视图") {
                                        s.finish_text(cx);
                                    }
                                    s.ribbon = name.into();
                                    if name == "幻灯片放映" {
                                        let order = if s.session.deck.presentation.order.is_empty()
                                        {
                                            (1..=s.session.deck.slides.len())
                                                .map(|i| i.to_string())
                                                .collect::<Vec<_>>()
                                                .join(",")
                                        } else {
                                            s.session
                                                .deck
                                                .presentation
                                                .order
                                                .iter()
                                                .filter_map(|id| {
                                                    s.session
                                                        .deck
                                                        .slides
                                                        .iter()
                                                        .position(|p| &p.id == id)
                                                })
                                                .map(|i| (i + 1).to_string())
                                                .collect::<Vec<_>>()
                                                .join(",")
                                        };
                                        s.author_data.update(cx, |f, cx| f.set_value(order, w, cx));
                                    }
                                    if name == "切换" {
                                        let t = s
                                            .session
                                            .deck
                                            .presentation
                                            .timing(&s.session.deck.slides[s.page].id);
                                        for (i, value) in
                                            [(0, t.duration_ms), (2, t.advance_ms.unwrap_or(0))]
                                        {
                                            s.motion_fields[i].update(cx, |f, cx| {
                                                f.set_value(value.to_string(), w, cx)
                                            });
                                        }
                                    }
                                    if matches!(name, "图表" | "图示" | "动画") {
                                        s.sync_author(w, cx);
                                    }
                                    if name == "文件" || name == "设计" {
                                        s.sync_doc_fields(w, cx);
                                    }
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(ribbon)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .w(px(250.))
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(rgb(0xdde2eb))
                            .bg(rgb(0xf3f3f3))
                            .flex()
                            .flex_col()
                            .child(
                                uniform_list(
                                    "edit-page-thumbnails",
                                    pages,
                                    cx.processor(|s, range: std::ops::Range<usize>, _, cx| {
                                        s.request_thumbs(range.clone(), cx);
                                        range
                                            .map(|p| {
                                                let active = s.page_selection.contains(&p);
                                                let hidden = s.session.deck.slides[p].hidden;
                                                let aspect =
                                                    s.session.deck.width / s.session.deck.height;
                                                let h = 112f32.min(200. / aspect);
                                                div()
                                                    .id(("page", p))
                                                    .h(px(132.))
                                                    .px_4()
                                                    .py_2()
                                                    .cursor_pointer()
                                                    .flex()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .pt_1()
                                                            .w(px(14.))
                                                            .text_size(px(11.))
                                                            .text_color(rgb(if active {
                                                                ACCENT
                                                            } else {
                                                                0x94a3b8
                                                            }))
                                                            .child(format!("{}", p + 1)),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .h(px(116.))
                                                                    .w_full()
                                                                    .bg(rgb(0xffffff))
                                                                    .rounded_md()
                                                                    .border_2()
                                                                    .border_color(rgb(if active {
                                                                        ACCENT
                                                                    } else {
                                                                        0xe1e5ed
                                                                    }))
                                                                    .flex()
                                                                    .items_center()
                                                                    .justify_center()
                                                                    .overflow_hidden()
                                                                    .children(
                                                                        s.thumbs.get(&p).map(|i| {
                                                                            img(i.clone())
                                                                                .w(px(h * aspect))
                                                                                .h(px(h))
                                                                        }),
                                                                    ),
                                                            )
                                                            .children(hidden.then(|| {
                                                                div()
                                                                    .absolute()
                                                                    .bottom_0()
                                                                    .right_0()
                                                                    .text_size(px(10.))
                                                                    .bg(rgb(0xf3f3f3))
                                                                    .child("隐藏")
                                                            })),
                                                    )
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |s, e: &MouseDownEvent, w, cx| {
                                                                s.finish_text(cx);
                                                                s.table_pick = None;
                                                                s.page_click(
                                                                    p,
                                                                    e.modifiers.shift,
                                                                    e.modifiers.control
                                                                        || e.modifiers.platform,
                                                                    cx,
                                                                );
                                                                s.page_drag = Some(s.page_ids());
                                                                s.focus.focus(w, cx);
                                                            },
                                                        ),
                                                    )
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |s, _, _, cx| {
                                                            if let Some(ids) = s.page_drag.take()
                                                                && !(ids.len() == 1 && ids[0] == p)
                                                            {
                                                                let target = if p >= ids[0] {
                                                                    p + 1
                                                                } else {
                                                                    p
                                                                };
                                                                s.command(
                                                                    Command::ReorderPages(
                                                                        ids, target,
                                                                    ),
                                                                    cx,
                                                                );
                                                                cx.stop_propagation();
                                                            }
                                                        }),
                                                    )
                                            })
                                            .collect::<Vec<_>>()
                                    }),
                                )
                                .flex_1()
                                .min_h_0(),
                            )
                            .child(
                                div().h(px(40.)).flex().justify_center().gap_1().children(
                                    [("up", "上移"), ("down", "下移"), ("delete", "删除")]
                                        .into_iter()
                                        .map(|(op, name)| {
                                            Button::new(SharedString::from(format!("slide-{op}")))
                                                .ghost()
                                                .small()
                                                .label(name)
                                                .on_click(cx.listener(move |s, _, _, cx| {
                                                    s.page_command(op, cx)
                                                }))
                                        }),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id("stage")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_scroll()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p_8()
                                    .bg(rgb(0xf3f3f3))
                                    .child(surface),
                            )
                            .children((!self.show_notes).then(|| {
                                div()
                                    .id("notes-collapsed")
                                    .h(px(32.))
                                    .flex_shrink_0()
                                    .px_3()
                                    .bg(rgb(0xf8f8f8))
                                    .border_t_1()
                                    .border_color(rgb(0xd8d8d8))
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .text_color(rgb(0x737373))
                                    .child("单击此处添加备注")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|s, _, w, cx| {
                                            s.show_notes = true;
                                            s.sync_doc_fields(w, cx);
                                            cx.notify();
                                        }),
                                    )
                            }))
                            .children(self.show_notes.then(|| {
                                div()
                                    .h(px(124.))
                                    .flex_shrink_0()
                                    .px_5()
                                    .py_3()
                                    .bg(rgb(0xffffff))
                                    .border_t_1()
                                    .border_color(rgb(0xdde2eb))
                                    .flex()
                                    .gap_4()
                                    .child(label("演讲者备注"))
                                    .child(div().flex_1().child(Textarea::new(&self.notes_input)))
                                    .child(
                                        Button::new("notes")
                                            .small()
                                            .label("保存备注")
                                            .on_click(cx.listener(|s, _, _, cx| s.notes(cx))),
                                    )
                            })),
                    )
                    .children(inspector),
            )
            .child(
                div()
                    .h(px(28.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_4()
                    .border_t_1()
                    .border_color(rgb(0xdde2eb))
                    .bg(rgb(0xffffff))
                    .text_size(px(11.))
                    .text_color(rgb(0x7c899b))
                    .child(format!("幻灯片 {} / {}", self.page + 1, pages))
                    .child(div().flex_1().truncate().child(self.message.clone()))
                    .child(
                        Button::new("notes-toggle")
                            .ghost()
                            .small()
                            .label("备注")
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.show_notes = !s.show_notes;
                                s.sync_doc_fields(w, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("zoom-out")
                            .ghost()
                            .small()
                            .icon(I::Minus)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.zoom = (s.zoom / 1.15).max(0.25);
                                cx.notify();
                            })),
                    )
                    .child(format!("{}%", (scale * 100.).round() as u32))
                    .child(
                        Button::new("zoom-in")
                            .ghost()
                            .small()
                            .icon(I::Plus)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.zoom = (s.zoom * 1.15).min(3.);
                                cx.notify();
                            })),
                    )
                    .child(
                        tool("zoom-fit", "适应窗口", I::Maximize).on_click(cx.listener(
                            |s, _, _, cx| {
                                s.zoom = 1.;
                                cx.notify();
                            },
                        )),
                    ),
            )
            .into_any_element()
    }
}
