//! Native editor chrome. Layout and colour tokens adapted from DeckCraft's MIT
//! ribbon/widgets/theme; GPUI Kit controls and assets follow JoyOffice's baseline.
use super::*;
use gpui_kit::component::{Icon, Sizable, button::ButtonVariants};
use joydeck_engine::Command;
use joyoffice_ui::{IconName as I, Palette};

pub(super) fn palette(dark: bool) -> Palette {
    let mut p = if dark {
        Palette::dark(joyoffice_ui::Brand::Deck)
    } else {
        Palette::light(joyoffice_ui::Brand::Deck)
    };
    if dark {
        p.canvas = 0x1e1e1e;
        p.chrome = 0x262626;
        p.surface = 0x333333;
        p.text = 0xeeeeee;
        p.muted = 0xbbbbbb;
        p.border = 0x4a4a4a;
        p.hover = 0x404040;
        p.selected = 0x474747;
        p.accent = 0xf26b1d;
    } else {
        p.canvas = 0xf0f0f0;
        p.chrome = 0xf5f5f5;
        p.surface = 0xffffff;
        p.text = 0x242424;
        p.muted = 0x5c5c5c;
        p.border = 0xe1e1e1;
        p.hover = 0xebebeb;
        p.selected = 0xe6e6e6;
        p.accent = 0xd95a1a;
    }
    p
}

fn small(id: &'static str, icon: I, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .label(label)
        .h(px(17.))
        .text_size(px(11.))
        .rounded(px(3.))
}
fn tool(id: &'static str, icon: I, label: &'static str, tint: u32) -> Button {
    Button::new(id)
        .ghost()
        .accessibility_label(label)
        .tooltip(label)
        .w(px(68.))
        .h(px(50.))
        .p_1()
        .rounded(px(4.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(Icon::new(icon).size(px(27.)).text_color(rgb(tint)))
                .child(div().text_size(px(11.)).child(label)),
        )
}
pub(super) fn group(label: &'static str, content: impl IntoElement) -> Div {
    div()
        .h(px(66.))
        .flex_shrink_0()
        .px_2()
        .border_r_1()
        .border_color(rgb(0xe1e1e1))
        .flex()
        .flex_col()
        .relative()
        .child(content)
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .w_full()
                .h(px(11.))
                .text_center()
                .text_size(px(9.))
                .child(label),
        )
}

fn row() -> Div {
    div().flex().items_center().gap_1()
}
fn col() -> Div {
    div().flex().flex_col().gap(px(1.))
}

impl Viewer {
    fn basic_button(&self, button: Button, edit: BasicEdit, cx: &mut Context<Self>) -> Button {
        let button = if let Some(reason) = self.basic_edit_unavailable(edit, cx) {
            button.tooltip(reason).disabled(true)
        } else {
            button
        };
        button.on_click(cx.listener(move |s, _, w, cx| s.run_basic_edit(edit, w, cx)))
    }

    pub(super) fn title_chrome(&self, cx: &mut Context<Self>) -> Div {
        let p = palette(self.dark);
        let dirty = self.editor.as_ref().is_some_and(|e| e.session.dirty());
        let name = self.document_name();
        div()
            .h(px(36.))
            .flex_shrink_0()
            .relative()
            .flex()
            .items_center()
            .px_3()
            .gap_1()
            .child(
                Button::new("file-home")
                    .ghost()
                    .xsmall()
                    .icon(I::House)
                    .accessibility_label("文件")
                    .tooltip("文件")
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.backstage = !s.backstage;
                        if s.backstage {
                            s.refresh_recent(cx);
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("quick-save")
                    .ghost()
                    .xsmall()
                    .icon(I::Save)
                    .accessibility_label("保存")
                    .tooltip("保存 · Ctrl+S")
                    .disabled(self.save_unavailable().is_some())
                    .on_click(cx.listener(|s, _, w, cx| s.save(&Save, w, cx))),
            )
            .child(
                self.basic_button(
                    Button::new("quick-undo")
                        .ghost()
                        .xsmall()
                        .icon(I::Undo2)
                        .accessibility_label("撤销")
                        .tooltip("撤销 · Ctrl+Z"),
                    BasicEdit::Undo,
                    cx,
                ),
            )
            .child(
                self.basic_button(
                    Button::new("quick-redo")
                        .ghost()
                        .xsmall()
                        .icon(I::Redo2)
                        .accessibility_label("重做")
                        .tooltip("重做 · Ctrl+Y"),
                    BasicEdit::Redo,
                    cx,
                ),
            )
            .child(
                Button::new("quick-present")
                    .ghost()
                    .xsmall()
                    .icon(I::Presentation)
                    .accessibility_label("从头开始放映")
                    .tooltip("从头开始 · F5")
                    .on_click(cx.listener(|s, _, _, cx| s.start_show(true, false, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .text_center()
                    .text_size(px(12.))
                    .text_ellipsis()
                    .child(format!("{name}{}", if dirty { " *" } else { "" })),
            )
            .child(
                Button::new("quick-find")
                    .ghost()
                    .xsmall()
                    .icon(I::Search)
                    .accessibility_label("查找文字")
                    .tooltip("查找文字")
                    .on_click(cx.listener(|s, _, w, cx| s.find_dialog(w, cx))),
            )
            .child(
                Button::new("quick-theme")
                    .ghost()
                    .xsmall()
                    .icon(if self.dark { I::Sun } else { I::Moon })
                    .accessibility_label("切换明暗主题")
                    .tooltip("切换明暗主题")
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.dark = !s.dark;
                        crate::apply_theme(s.dark, Some(w), cx);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("present")
                    .label("放映")
                    .icon(I::Play)
                    .h(px(26.))
                    .rounded(px(16.))
                    .bg(rgb(p.accent))
                    .text_color(rgb(0xffffff))
                    .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
            )
    }
    pub(super) fn editor_ribbon(&self, w: &Window, cx: &mut Context<Self>) -> Div {
        let p = palette(self.dark);
        let mut tabs = div()
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_3()
            .gap_1();
        for tab in ["文件", "开始", "插入", "设计", "放映", "审阅", "视图"] {
            tabs = tabs.child(
                Button::new(tab)
                    .ghost()
                    .xsmall()
                    .h(px(29.))
                    .px_3()
                    .rounded_none()
                    .label(tab)
                    .border_b(px(if self.ribbon == tab && !self.backstage {
                        2.
                    } else {
                        0.
                    }))
                    .border_color(rgb(p.accent))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if tab == "文件" {
                            s.backstage = !s.backstage;
                            if s.backstage {
                                s.refresh_recent(cx);
                            }
                        } else {
                            s.ribbon = tab;
                            s.backstage = false;
                        }
                        cx.notify();
                    })),
            );
        }
        if let Some(e) = &self.editor {
            let contextual = e
                .session
                .selection()
                .shapes
                .first()
                .and_then(|id| e.session.model().slides[self.page].shape(*id))
                .map(|s| match s.kind {
                    joydeck_model::ShapeKind::Picture { .. } => "图片",
                    joydeck_model::ShapeKind::Chart(_) => "图表",
                    joydeck_model::ShapeKind::Table(_) => "表格",
                    _ => "形状格式",
                });
            if let Some(tab) = contextual {
                tabs = tabs.child(
                    Button::new("contextual-tab")
                        .ghost()
                        .xsmall()
                        .label(tab)
                        .text_color(rgb(p.accent))
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.ribbon = tab;
                            s.backstage = false;
                            cx.notify();
                        })),
                );
            }
        }
        let disabled = self.editor.is_none() || self.loading;
        let selected = self
            .editor
            .as_ref()
            .is_some_and(|e| !e.session.selection().shapes.is_empty());
        let bar = match self.ribbon {
            "开始" => self
                .ribbon_home(cx, disabled, selected)
                .child(self.organization_controls(cx)),
            "插入" => self.ribbon_insert(cx),
            "设计" => self.ribbon_design(cx, disabled),
            "图片" => self.ribbon_picture(cx, selected),
            "图表" => self.ribbon_chart(cx, selected),
            "表格" => self.ribbon_table(cx),
            "形状格式" => self.ribbon_shape(cx),
            "放映" => self.ribbon_show(cx),
            "审阅" => self.ribbon_review(cx),
            "视图" => self.ribbon_view(cx),
            _ => row().h(px(66.)).items_start(),
        };
        let narrow = f32::from(w.viewport_size().width) < 1100.;
        div().flex_shrink_0().child(tabs).child(
            div()
                .mx_2()
                .mb(px(6.))
                .p_1()
                .rounded(px(10.))
                .bg(rgb(p.surface))
                .flex()
                .when(narrow, |d| {
                    d.child(
                        Button::new("ribbon-left")
                            .ghost()
                            .icon(I::ChevronLeft)
                            .w(px(18.))
                            .h(px(66.))
                            .p_0()
                            .accessibility_label("向左滚动功能区")
                            .on_click(cx.listener(|s, _, _, cx| {
                                let at = s.ribbon_scroll.offset();
                                s.ribbon_scroll.set_offset(point(at.x + px(240.), px(0.)));
                                cx.notify();
                            })),
                    )
                })
                .child(
                    div()
                        .id("ribbon-scroll")
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_start()
                        .overflow_x_scroll()
                        .track_scroll(&self.ribbon_scroll)
                        .child(bar.flex_shrink_0()),
                )
                .when(narrow, |d| {
                    d.child(
                        Button::new("ribbon-right")
                            .ghost()
                            .icon(I::ChevronRight)
                            .w(px(18.))
                            .h(px(66.))
                            .p_0()
                            .accessibility_label("向右滚动功能区")
                            .on_click(cx.listener(|s, _, _, cx| {
                                let at = s.ribbon_scroll.offset();
                                s.ribbon_scroll.set_offset(point(at.x - px(240.), px(0.)));
                                cx.notify();
                            })),
                    )
                }),
        )
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_home(&self, cx: &mut Context<Self>, disabled: bool, selected: bool) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        bar = bar
            .child(self.home_clipboard(cx, disabled, selected))
            .child(self.home_slides(cx, disabled))
            .child(self.font_group(cx))
            .child(self.paragraph_group(cx))
            .child(self.home_drawing(cx, disabled))
            .child(self.home_arrange(cx));
        bar
    }
    #[inline(never)]
    fn home_clipboard(&self, cx: &mut Context<Self>, disabled: bool, selected: bool) -> Div {
        group(
            "剪贴板",
            row()
                .child(
                    tool("paste", I::ClipboardPaste, "粘贴", 0x2e6fd8)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.paste_grid(w, cx))),
                )
                .child(
                    col()
                        .child(
                            small("cut", I::Scissors, "剪切")
                                .disabled(
                                    self.inline.is_none()
                                        && self
                                            .basic_edit_unavailable(BasicEdit::Delete, cx)
                                            .is_some(),
                                )
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.clipboard_copy(true, w, cx);
                                })),
                        )
                        .child(
                            small("copy", I::Copy, "复制").disabled(!selected).on_click(
                                cx.listener(|s, _, w, cx| s.clipboard_copy(false, w, cx)),
                            ),
                        )
                        .child(self.basic_button(
                            small("duplicate", I::CopyPlus, "创建副本"),
                            BasicEdit::Duplicate,
                            cx,
                        )),
                ),
        )
    }
    #[inline(never)]
    fn home_slides(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        group(
            "幻灯片",
            row()
                .child(self.basic_button(
                    tool("new-slide", I::GalleryVerticalEnd, "新建幻灯片", 0x2ea36f),
                    BasicEdit::NewSlide,
                    cx,
                ))
                .child(
                    col()
                        .child(
                            small("layout", I::PanelsTopLeft, "版式")
                                .disabled(disabled)
                                .on_click(cx.listener(|s, _, w, cx| s.layout_gallery(w, cx))),
                        )
                        .child(self.basic_button(
                            small("duplicate-slide", I::Copy, "复制幻灯片"),
                            BasicEdit::DuplicateSlides,
                            cx,
                        ))
                        .child(self.basic_button(
                            small("delete-slide", I::Trash, "删除幻灯片"),
                            BasicEdit::DeleteSlides,
                            cx,
                        )),
                ),
        )
    }
    #[inline(never)]
    fn home_drawing(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        group(
            "绘图",
            row()
                .child(
                    tool("picture", I::Image, "图片", 0x2ea36f)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::ImportPicture(false), w, cx)
                        })),
                )
                .child(
                    tool("shapes", I::Shapes, "形状", 0x2e6fd8)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.shape_gallery(w, cx))),
                )
                .child(
                    tool("text-box", I::TextCursorInput, "文本框", 0x2e6fd8)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.insert_textbox(w, cx))),
                ),
        )
    }
    #[inline(never)]
    fn home_arrange(&self, cx: &mut Context<Self>) -> Div {
        group(
            "排列",
            col()
                .child(self.basic_button(
                    small("forward", I::BringToFront, "上移一层"),
                    BasicEdit::BringForward,
                    cx,
                ))
                .child(self.basic_button(
                    small("backward", I::SendToBack, "下移一层"),
                    BasicEdit::SendBackward,
                    cx,
                ))
                .child(
                    small("selection", I::PanelRight, "选择窗格").on_click(cx.listener(
                        |s, _, _, cx| {
                            s.inspector = !s.inspector;
                            cx.notify();
                        },
                    )),
                ),
        )
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_insert(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        bar = bar
            .child(group(
                "幻灯片",
                self.basic_button(
                    tool(
                        "insert-slide",
                        I::GalleryVerticalEnd,
                        "新建幻灯片",
                        0x2ea36f,
                    ),
                    BasicEdit::NewSlide,
                    cx,
                ),
            ))
            .child(group(
                "表格",
                tool("insert-table", I::Table, "表格", 0x2e6fd8)
                    .on_click(cx.listener(|s, _, w, cx| s.insert_table_dialog(w, cx))),
            ))
            .child(group(
                "图像",
                tool("insert-picture", I::Image, "图片", 0x2ea36f).on_click(cx.listener(
                    |s, _, w, cx| s.content_form(content_ui::Form::ImportPicture(false), w, cx),
                )),
            ))
            .child(group(
                "插图",
                tool("insert-shapes", I::Shapes, "形状", 0x2e6fd8)
                    .on_click(cx.listener(|s, _, w, cx| s.shape_gallery(w, cx))),
            ))
            .child(group(
                "文本",
                tool("insert-textbox", I::TextCursorInput, "文本框", 0x2e6fd8)
                    .on_click(cx.listener(|s, _, w, cx| s.insert_textbox(w, cx))),
            ))
            .child(group(
                "演讲备注",
                tool("insert-notes", I::NotebookPen, "备注", 0xe3b21c)
                    .on_click(cx.listener(|s, _, w, cx| s.edit_notes(w, cx))),
            ));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_design(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        for (id, label, icon, form) in [
            (
                "theme-colors",
                "主题颜色",
                I::Palette,
                content_ui::Form::Accent,
            ),
            ("theme-fonts", "主题字体", I::Type, content_ui::Form::Fonts),
            (
                "slide-size",
                "幻灯片大小",
                I::RectangleHorizontal,
                content_ui::Form::Size,
            ),
            (
                "slide-background",
                "设置背景",
                I::PaintBucket,
                content_ui::Form::Background,
            ),
            (
                "master-background",
                "母版背景",
                I::PanelsTopLeft,
                content_ui::Form::MasterBackground,
            ),
        ] {
            bar = bar.child(group(
                label,
                tool(id, icon, label, 0xd95a1a)
                    .disabled(disabled)
                    .on_click(cx.listener(move |s, _, w, cx| s.content_form(form, w, cx))),
            ));
        }
        bar = bar.child(group(
            "版式",
            tool("design-layout", I::PanelsTopLeft, "幻灯片版式", 0x2e6fd8)
                .disabled(disabled)
                .on_click(cx.listener(|s, _, w, cx| s.layout_gallery(w, cx))),
        ));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_picture(&self, cx: &mut Context<Self>, selected: bool) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        for (id, label, icon, form) in [
            (
                "replace-picture",
                "更改图片",
                I::ImagePlus,
                content_ui::Form::ImportPicture(true),
            ),
            ("crop-picture", "裁剪", I::Crop, content_ui::Form::Picture),
            (
                "picture-corrections",
                "校正",
                I::SlidersHorizontal,
                content_ui::Form::Picture,
            ),
            (
                "picture-alpha",
                "透明度",
                I::Blend,
                content_ui::Form::Picture,
            ),
        ] {
            bar = bar.child(group(
                label,
                tool(id, icon, label, 0x2e6fd8)
                    .disabled(!selected)
                    .on_click(cx.listener(move |s, _, w, cx| s.content_form(form, w, cx))),
            ));
        }
        bar = bar.child(group(
            "大小",
            tool("picture-size", I::Move, "大小和位置", 0x2e6fd8)
                .disabled(!selected)
                .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
        ));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_chart(&self, cx: &mut Context<Self>, selected: bool) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        for (id, label, icon, form) in [
            (
                "chart-data",
                "编辑数据",
                I::Table,
                content_ui::Form::ChartValue,
            ),
            (
                "chart-title",
                "图表标题",
                I::Type,
                content_ui::Form::ChartTitle,
            ),
        ] {
            bar = bar.child(group(
                label,
                tool(id, icon, label, 0x2e6fd8)
                    .disabled(!selected)
                    .on_click(cx.listener(move |s, _, w, cx| s.content_form(form, w, cx))),
            ));
        }
        let mut kinds = row();
        for (id, label, icon, kind) in [
            (
                "chart-column",
                "柱形图",
                I::ChartColumn,
                joydeck_model::chart::ChartType::Column,
            ),
            (
                "chart-line",
                "折线图",
                I::ChartLine,
                joydeck_model::chart::ChartType::Line,
            ),
            (
                "chart-pie",
                "饼图",
                I::ChartPie,
                joydeck_model::chart::ChartType::Pie,
            ),
        ] {
            kinds = kinds.child(
                tool(id, icon, label, 0x8a5cd8)
                    .disabled(!selected)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.dispatch(
                            Command::Content {
                                action: joydeck_engine::ContentAction::ChartType {
                                    chart_type: kind,
                                },
                            },
                            w,
                            cx,
                        )
                    })),
            );
        }
        bar = bar.child(group("更改图表类型", kinds));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_table(&self, cx: &mut Context<Self>) -> Div {
        self.table_controls(cx)
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_shape(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        bar = bar
            .child(group(
                "形状样式",
                row()
                    .child(
                        tool("shape-fill", I::PaintBucket, "形状填充", 0xf26b1d).on_click(
                            cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ShapeFill, w, cx)
                            }),
                        ),
                    )
                    .child(
                        tool("shape-gradient", I::Blend, "渐变填充", 0x8a5cd8).on_click(
                            cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ShapeGradient, w, cx)
                            }),
                        ),
                    ),
            ))
            .child(self.font_group(cx))
            .child(self.paragraph_group(cx))
            .child(self.text_layout_group(cx))
            .child(group(
                "大小与位置",
                row()
                    .child(
                        tool("geometry", I::Move, "大小和位置", 0x2e6fd8)
                            .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
                    )
                    .child(tool("rotate", I::RotateCw, "向右旋转", 0x2e6fd8).on_click(
                        cx.listener(|s, _, w, cx| {
                            s.dispatch(
                                Command::Transform {
                                    dx: 0.,
                                    dy: 0.,
                                    dw: 0.,
                                    dh: 0.,
                                    rotation: 90.,
                                },
                                w,
                                cx,
                            )
                        }),
                    )),
            ));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_show(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        bar = bar.child(group(
            "开始放映",
            row()
                .child(
                    tool("show-start", I::Presentation, "从头开始", 0x2ea36f)
                        .on_click(cx.listener(|s, _, _, cx| s.start_show(true, false, cx))),
                )
                .child(
                    tool("show-current", I::Play, "从当前页", 0x2ea36f)
                        .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
                )
                .child(
                    tool("show-presenter", I::PanelsTopLeft, "演示者视图", 0x2e6fd8)
                        .on_click(cx.listener(|s, _, _, cx| s.start_show(false, true, cx))),
                ),
        ));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_review(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        bar = bar.child(group(
            "批注与比较",
            row()
                .child(
                    tool("add-comment", I::NotebookPen, "新增批注", 0x2e6fd8)
                        .on_click(cx.listener(|s, _, w, cx| s.add_comment(w, cx))),
                )
                .child(
                    tool("comments", I::NotebookPen, "批注列表", 0x2e6fd8)
                        .on_click(cx.listener(|s, _, w, cx| s.comments_dialog(w, cx))),
                )
                .child(
                    tool("accessibility", I::FilePenLine, "替代文字", 0x2e6fd8)
                        .on_click(cx.listener(|s, _, w, cx| s.accessibility_dialog(w, cx))),
                )
                .child(
                    tool("compare-documents", I::FileCheck, "比较文稿", 0x2e6fd8)
                        .on_click(cx.listener(|s, _, w, cx| s.compare_file(w, cx))),
                ),
        ));
        bar = bar
            .child(group(
                "检查",
                row()
                    .child(
                        tool("compatibility", I::FileCheck, "兼容性检查", 0x2ea36f)
                            .on_click(cx.listener(|s, _, w, cx| s.report(&Report, w, cx))),
                    )
                    .child(
                        tool("metadata", I::FilePenLine, "文稿属性", 0x2e6fd8).on_click(
                            cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::Metadata, w, cx)
                            }),
                        ),
                    ),
            ))
            .child(group(
                "备注",
                tool("review-notes", I::NotebookPen, "演讲备注", 0xe3b21c)
                    .on_click(cx.listener(|s, _, w, cx| s.edit_notes(w, cx))),
            ));
        bar
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_view(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = row().h(px(66.)).items_start();
        bar = bar
            .child(group(
                "演示文稿视图",
                row()
                    .child(tool("normal", I::PanelsTopLeft, "普通", 0x2e6fd8).on_click(
                        cx.listener(|s, _, _, cx| {
                            s.sorter = false;
                            cx.notify();
                        }),
                    ))
                    .child(
                        tool("sorter", I::LayoutGrid, "幻灯片浏览", 0x2e6fd8).on_click(
                            cx.listener(|s, _, _, cx| {
                                s.sorter = true;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        tool("notes-view", I::NotebookPen, "备注", 0xe3b21c).on_click(cx.listener(
                            |s, _, _, cx| {
                                s.notes_visible = !s.notes_visible;
                                cx.notify();
                            },
                        )),
                    ),
            ))
            .child(group(
                "窗口",
                tool("shared-view", I::PanelsTopLeft, "新建文稿视图", 0x2e6fd8)
                    .disabled(self.editor.is_none() || self.loading)
                    .on_click(cx.listener(|s, _, _, cx| s.new_shared_view(cx))),
            ))
            .child(group(
                "显示",
                row()
                    .child(
                        tool("view-fit", I::Scan, "适应窗口", 0x2e6fd8)
                            .on_click(cx.listener(|s, _, w, cx| s.fit(&Fit, w, cx))),
                    )
                    .child(
                        tool("properties", I::PanelRight, "选择窗格", 0x2e6fd8).on_click(
                            cx.listener(|s, _, _, cx| {
                                s.inspector = !s.inspector;
                                cx.notify();
                            }),
                        ),
                    ),
            ))
            .child(group(
                "诊断",
                row()
                    .child(
                        tool("render-report", I::Activity, "渲染报告", 0x2ea36f)
                            .on_click(cx.listener(|s, _, w, cx| s.scene_report(w, cx))),
                    )
                    .child(
                        tool("render-mode", I::Layers, "CPU / GPUI", 0x8a5cd8).on_click(
                            cx.listener(|s, _, w, cx| {
                                s.cpu_view = !s.cpu_view;
                                s.gpu_cache.clear(w);
                                cx.notify();
                            }),
                        ),
                    ),
            ));
        bar
    }
    pub(super) fn file_backstage(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(self.dark);
        div()
            .size_full()
            .flex()
            .bg(rgb(p.surface))
            .child(
                div()
                    .w(px(208.))
                    .h_full()
                    .p_4()
                    .bg(rgb(p.chrome))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        Button::new("file-back")
                            .ghost()
                            .icon(I::ArrowLeft)
                            .label("返回")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.backstage = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("file-new")
                            .ghost()
                            .icon(I::FilePlus)
                            .label("新建")
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.new_document(&NewDocument, w, cx);
                                s.backstage = false;
                            })),
                    )
                    .child(
                        Button::new("file-open")
                            .ghost()
                            .icon(I::FolderOpen)
                            .label("打开 · Ctrl+O")
                            .on_click(cx.listener(|s, _, w, cx| s.open(&Open, w, cx))),
                    )
                    .child(
                        Button::new("file-save")
                            .ghost()
                            .icon(I::Save)
                            .label("保存 · Ctrl+S")
                            .on_click(cx.listener(|s, _, w, cx| s.save(&Save, w, cx))),
                    )
                    .child(
                        Button::new("file-save-as")
                            .ghost()
                            .icon(I::Save)
                            .label("另存为")
                            .on_click(cx.listener(|s, _, w, cx| s.save_as(&SaveAs, w, cx))),
                    )
                    .child(
                        Button::new("file-recover")
                            .ghost()
                            .icon(I::FolderClock)
                            .label("恢复未保存文稿")
                            .on_click(cx.listener(|s, _, w, cx| s.recover(w, cx))),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(div().text_2xl().child("文件"))
                    .child(
                        div().text_lg().child(
                            self.deck
                                .as_ref()
                                .map(|d| d.info.name.clone())
                                .unwrap_or("新演示文稿".into()),
                        ),
                    )
                    .child(self.templates_panel(cx))
                    .child(self.recent_panel(cx))
                    .child("导出与打印")
                    .child(self.delivery_toolbar(cx))
                    .child(div().mt_4().text_sm().text_color(rgb(p.muted)).child(
                        "支持 PPTX、JoyDeck 与 DeckCraft 文稿。导入与兼容性详情可在“审阅”中查看。",
                    )),
            )
            .into_any_element()
    }
    pub(super) fn status_chrome(&self, scale: f32, cx: &mut Context<Self>) -> Div {
        let p = palette(self.dark);
        let count = self.deck.as_ref().map_or(0, |d| d.info.slides.len());
        div()
            .h(px(26.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgb(p.border))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(11.))
            .child(format!(
                "幻灯片 {} / {count}",
                if count == 0 { 0 } else { self.page + 1 }
            ))
            .child(div().w(px(1.)).h(px(12.)).bg(rgb(p.border)))
            .child("简体中文")
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_ellipsis()
                    .text_color(rgb(p.muted))
                    .child(if self.loading {
                        "正在打开文稿…".into()
                    } else if self.is_saving() {
                        "正在保存…".into()
                    } else {
                        self.message.clone()
                    }),
            )
            .when(self.loading, |bar| {
                bar.child(
                    Button::new("cancel-open")
                        .ghost()
                        .xsmall()
                        .label("取消打开")
                        .on_click(cx.listener(|s, _, _, cx| s.cancel_read(cx))),
                )
            })
            .child(
                Button::new("status-notes")
                    .ghost()
                    .xsmall()
                    .icon(I::NotebookPen)
                    .label("备注")
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.notes_visible = !s.notes_visible;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("status-normal")
                    .ghost()
                    .xsmall()
                    .icon(I::PanelsTopLeft)
                    .accessibility_label("普通视图")
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.sorter = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("status-sorter")
                    .ghost()
                    .xsmall()
                    .icon(I::LayoutGrid)
                    .accessibility_label("幻灯片浏览")
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.sorter = true;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("status-show")
                    .ghost()
                    .xsmall()
                    .icon(I::Presentation)
                    .accessibility_label("从当前页放映")
                    .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
            )
            .child(
                Button::new("status-minus")
                    .ghost()
                    .xsmall()
                    .icon(I::Minus)
                    .accessibility_label("缩小")
                    .on_click(cx.listener(|s, _, w, cx| s.zoom_out(&ZoomOut, w, cx))),
            )
            .child(
                div()
                    .id("zoom-track")
                    .w(px(100.))
                    .h(px(20.))
                    .relative()
                    .cursor_pointer()
                    .child(
                        canvas(
                            cx.processor(|s, bounds, _, _| s.zoom_bounds = bounds),
                            |_, _, _, _| {},
                        )
                        .size_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(10.))
                            .h(px(1.))
                            .bg(rgb(p.muted)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px((scale / 4.).clamp(0., 1.) * 96.))
                            .top(px(5.))
                            .w(px(4.))
                            .h(px(11.))
                            .bg(rgb(p.text)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|s, e: &MouseDownEvent, _, cx| {
                            let mouse = f32::from(e.position.x);
                            let left = f32::from(s.zoom_bounds.left());
                            let width = f32::from(s.zoom_bounds.size.width).max(1.);
                            s.zoom = Some(((mouse - left) / width * 4.).clamp(0.1, 4.));
                            cx.notify();
                        }),
                    ),
            )
            .child(
                Button::new("status-plus")
                    .ghost()
                    .xsmall()
                    .icon(I::Plus)
                    .accessibility_label("放大")
                    .on_click(cx.listener(|s, _, w, cx| s.zoom_in(&ZoomIn, w, cx))),
            )
            .child(
                div()
                    .w(px(40.))
                    .text_center()
                    .child(format!("{:.0}%", scale * 100.)),
            )
            .child(
                Button::new("status-fit")
                    .ghost()
                    .xsmall()
                    .icon(I::Scan)
                    .accessibility_label("适应窗口")
                    .tooltip("适应窗口 · Ctrl+0")
                    .on_click(cx.listener(|s, _, w, cx| s.fit(&Fit, w, cx))),
            )
    }
    pub(super) fn start_show(&mut self, first: bool, presenter: bool, cx: &mut Context<Self>) {
        if self.composing() {
            return;
        }
        self.end_inline(cx);
        if let Some(e) = &self.editor
            && let Err(error) = slideshow::open(
                e.session.model_snapshot(),
                if first { None } else { Some(self.page) },
                true,
                presenter,
                cx,
            )
        {
            self.message = error;
            cx.notify();
        }
    }
}
