//! Native editor chrome. Layout and colour tokens adapted from DeckCraft's MIT
//! ribbon/widgets/theme; GPUI Kit controls and assets follow JoyOffice's baseline.
use super::ribbon_reference::{
    BLUE, GREEN, gallery, ribbon_checks, small_unavailable, unavailable, unavailable_row,
};
use super::*;
use gpui_kit::component::{Icon, Selectable, Sizable, button::ButtonVariants, popover::Popover};
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
        p.chrome = 0xf3f3f3;
        p.surface = 0xffffff;
        p.text = 0x242424;
        p.muted = 0x5c5c5c;
        p.border = 0xd4d4d4;
        p.hover = 0xebebeb;
        p.selected = 0xe6e6e6;
        p.accent = 0xa63d24;
    }
    p
}

pub(super) fn small(id: &'static str, icon: I, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .label(label)
        .h(px(17.))
        .text_size(px(11.))
        .rounded(px(3.))
}
pub(super) const RIBBON_HEIGHT: f32 = 66.;
const TABS: &[&str] = &[
    "开始",
    "插入",
    "绘图",
    "设计",
    "切换",
    "动画",
    "幻灯片放映",
    "录制",
    "审阅",
    "视图",
];

pub(super) fn tool(id: &'static str, icon: I, label: &'static str, tint: u32) -> Button {
    let lines = label.split('\n');
    let width = lines
        .clone()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    let accessible = label.replace('\n', "");
    Button::new(id)
        .ghost()
        .accessibility_label(accessible.clone())
        .tooltip(accessible)
        .w(px((width as f32 * 11. + 8.).max(38.)))
        .h(px(63.))
        .p_0()
        .rounded(px(3.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_start()
                .pt(px(2.))
                .h_full()
                .gap(px(4.))
                .child(Icon::new(icon).size(px(29.)).text_color(rgb(tint)))
                .child(
                    div()
                        .text_center()
                        .text_size(px(10.5))
                        .line_height(px(12.))
                        .children(lines.map(|line| div().child(line))),
                ),
        )
}

pub(super) fn group(label: &'static str, content: impl IntoElement) -> Div {
    div()
        .h(px(RIBBON_HEIGHT))
        .flex_shrink_0()
        .px(px(6.))
        .border_r_1()
        .border_color(gpui::rgb(0x888888).opacity(0.27))
        .flex()
        .items_start()
        .child(
            div()
                .id(label)
                .role(gpui::Role::Group)
                .aria_label(label)
                .child(content),
        )
}

pub(super) fn row() -> Div {
    div().flex().items_center().gap(px(1.))
}

pub(super) fn col() -> Div {
    div().flex().flex_col().gap(px(4.))
}

fn theme_preview(
    index: usize,
    theme: &joydeck_model::Theme,
    current: bool,
    p: Palette,
) -> impl IntoElement {
    use joydeck_model::SchemeSlot as Slot;
    let color = |slot| {
        let c = theme.colors.get(slot);
        rgb(u32::from_be_bytes([0, c.r, c.g, c.b]))
    };
    let status = if current {
        "当前主题"
    } else {
        "主题预览，暂不支持应用"
    };
    div()
        .id(("theme-preview", index))
        .role(gpui::Role::Image)
        .aria_label(format!("{}（{status}）", theme.name))
        .w(px(83.))
        .h(px(48.))
        .px_2()
        .py(px(3.))
        .flex()
        .flex_col()
        .justify_between()
        .border_1()
        .border_color(rgb(if current { p.accent } else { p.border }))
        .bg(color(Slot::Lt1))
        .text_color(color(Slot::Dk1))
        .child(div().text_size(px(20.)).child("Aa"))
        .child(
            row().children(
                [
                    Slot::Accent1,
                    Slot::Accent2,
                    Slot::Accent3,
                    Slot::Accent4,
                    Slot::Accent5,
                    Slot::Accent6,
                ]
                .map(|slot| div().w(px(8.)).h(px(5.)).bg(color(slot))),
            ),
        )
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
            .h(px(33.))
            .flex_shrink_0()
            .relative()
            .flex()
            .items_center()
            .px_3()
            .when(cfg!(target_os = "macos"), |d| d.pl(px(82.)))
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
                div().flex_1().min_w_0().flex().justify_center().child(
                    div()
                        .max_w_full()
                        .text_center()
                        .text_size(px(12.))
                        .text_ellipsis()
                        .child(format!("{name}{}", if dirty { " *" } else { "" })),
                ),
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
            .id("ribbon-tabs")
            .role(gpui::Role::TabList)
            .aria_label("功能区")
            .h(px(29.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .overflow_x_scroll();
        if !cfg!(target_os = "macos") {
            tabs = tabs.child(self.ribbon_tab("文件", self.backstage, p.accent, cx));
        }
        let contextual = self
            .editor
            .as_ref()
            .and_then(|e| {
                let selected = e.session.selection().shapes.first()?;
                e.session.model().slides.get(self.page)?.shape(*selected)
            })
            .map_or(&[][..], |shape| match shape.kind {
                joydeck_model::ShapeKind::Picture { .. } => &["图片格式"][..],
                joydeck_model::ShapeKind::Chart(_) => &["图表设计", "格式"][..],
                joydeck_model::ShapeKind::Table(_) => &["表格设计", "表格布局"][..],
                _ => &["形状格式"][..],
            });
        let active = if TABS.contains(&self.ribbon) || contextual.contains(&self.ribbon) {
            self.ribbon
        } else {
            "开始"
        };
        for tab in TABS.iter().chain(contextual).copied() {
            tabs = tabs.child(self.ribbon_tab(tab, active == tab && !self.backstage, p.accent, cx));
        }
        tabs = tabs.child(div().flex_1()).child(
            Button::new("ribbon-comments")
                .ghost()
                .xsmall()
                .icon(I::MessageSquare)
                .label("批注")
                .on_click(cx.listener(|s, _, w, cx| s.comments_dialog(w, cx))),
        );
        let disabled = self.editor.is_none() || self.loading;
        let selected = self
            .editor
            .as_ref()
            .is_some_and(|e| !e.session.selection().shapes.is_empty());
        let bar = match active {
            "开始" => self.ribbon_home(cx, disabled, selected),
            "插入" => self.ribbon_insert(cx),
            "设计" => self.ribbon_design(cx, disabled),
            "图片格式" => self.ribbon_picture(cx, selected),
            "图表设计" => self.ribbon_chart(cx, selected),
            "格式" => self.ribbon_chart_format(cx),
            "表格设计" => self.table_controls(true, cx),
            "表格布局" => self.table_controls(false, cx),
            "形状格式" => self.ribbon_shape(cx),
            "绘图" => self.ribbon_draw(cx),
            "切换" => self.ribbon_transitions(cx),
            "动画" => self.ribbon_animations(cx),
            "幻灯片放映" => self.ribbon_show(cx),
            "录制" => self.ribbon_record(cx),
            "审阅" => self.ribbon_review(cx),
            "视图" => self.ribbon_view(cx),
            _ => row().h(px(RIBBON_HEIGHT)).items_start(),
        };
        let narrow = f32::from(w.viewport_size().width) < 1280.;
        div().flex_shrink_0().child(tabs).child(
            div()
                .h(px(75.))
                .pt(px(5.))
                .pb(px(3.))
                .border_b_1()
                .border_color(rgb(p.border))
                .bg(rgb(p.chrome))
                .flex()
                .when(narrow, |d| {
                    d.child(
                        Button::new("ribbon-left")
                            .ghost()
                            .icon(I::ChevronLeft)
                            .w(px(18.))
                            .h(px(RIBBON_HEIGHT))
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
                            .h(px(RIBBON_HEIGHT))
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
    fn ribbon_tab(
        &self,
        tab: &'static str,
        selected: bool,
        accent: u32,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(tab)
            .ghost()
            .xsmall()
            .h(px(28.))
            .p_0()
            .rounded_none()
            .flex_shrink_0()
            .accessibility_label(tab)
            .role(gpui::Role::Tab)
            .selected(selected)
            .toggled(selected)
            .when(selected, |b| b.bg(gpui::transparent_black()))
            .child(
                div()
                    .mx(px(11.5))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .when(selected, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                    .border_b(px(2.))
                    .border_color(if selected {
                        rgb(accent).into()
                    } else {
                        gpui::transparent_black()
                    })
                    .child(tab),
            )
            .on_click(cx.listener(move |s, _, _, cx| {
                if tab == "文件" {
                    s.backstage = !s.backstage;
                    if s.backstage {
                        s.refresh_recent(cx);
                    }
                } else {
                    s.ribbon = tab;
                    s.backstage = false;
                    s.ribbon_scroll.set_offset(point(px(0.), px(0.)));
                }
                cx.notify();
            }))
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_home(&self, cx: &mut Context<Self>, disabled: bool, selected: bool) -> Div {
        let mut bar = row().h(px(RIBBON_HEIGHT)).items_start();
        bar = bar
            .child(self.home_clipboard(cx, disabled, selected))
            .child(self.home_slides(cx, disabled))
            .child(self.font_group(cx))
            .child(self.paragraph_group(cx))
            .child(self.home_drawing(cx, disabled))
            .child(self.home_arrange(cx))
            .child(group(
                "扩展",
                unavailable_row(&[
                    ("home-addins", I::LayoutGrid, "加载项"),
                    ("home-designer", I::Zap, "设计器"),
                ]),
            ));
        bar
    }
    #[inline(never)]
    fn home_clipboard(&self, cx: &mut Context<Self>, disabled: bool, selected: bool) -> Div {
        group(
            "剪贴板",
            row()
                .child(
                    tool("paste", I::ClipboardPaste, "粘贴", 0x2e6fd8)
                        .disabled(disabled || self.composing())
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
                        .child(small_unavailable("format-painter", I::Paintbrush, "格式刷")),
                ),
        )
    }
    #[inline(never)]
    fn home_slides(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        let view = cx.entity().downgrade();
        group(
            "幻灯片",
            row().child(self.new_slide_split(cx, "new-slide")).child(
                col()
                    .child(
                        small("layout", I::PanelsTopLeft, "版式")
                            .dropdown_caret(true)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.layout_gallery(w, cx))),
                    )
                    .child(small_unavailable("reset-slide", I::RotateCw, "重置"))
                    .child(
                        Popover::new("slide-section-menu")
                            .trigger(
                                small("section-slides", I::GalleryVerticalEnd, "节")
                                    .dropdown_caret(true)
                                    .disabled(disabled),
                            )
                            .content(move |_, _, cx| {
                                let popup = cx.entity().downgrade();
                                let new_section = view.clone();
                                let move_slides = view.clone();
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(small("create-section", I::Plus, "新建节…").on_click({
                                        let popup = popup.clone();
                                        move |_, w, cx| {
                                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ = new_section
                                                .update(cx, |s, cx| s.create_slide_section(w, cx));
                                        }
                                    }))
                                    .child(small("move-slides", I::Move, "移动幻灯片…").on_click(
                                        move |_, w, cx| {
                                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ = move_slides
                                                .update(cx, |s, cx| s.organize_dialog(w, cx));
                                        },
                                    ))
                            }),
                    ),
            ),
        )
    }

    fn new_slide_split(&self, cx: &mut Context<Self>, id: &'static str) -> Div {
        let view = cx.entity().downgrade();
        let can_duplicate = self
            .basic_edit_unavailable(BasicEdit::DuplicateSlides, cx)
            .is_none();
        let can_delete = self
            .basic_edit_unavailable(BasicEdit::DeleteSlides, cx)
            .is_none();
        row()
            .gap_0()
            .child(self.basic_button(
                tool(id, I::GalleryVerticalEnd, "新建\n幻灯片", GREEN),
                BasicEdit::NewSlide,
                cx,
            ))
            .child(
                Popover::new((id, 0usize))
                    .trigger(
                        Button::new((id, 1usize))
                            .ghost()
                            .xsmall()
                            .p_0()
                            .w(px(14.))
                            .h(px(35.))
                            .icon(I::ChevronDown)
                            .accessibility_label("幻灯片选项")
                            .disabled(self.editor.is_none() || self.loading || self.composing()),
                    )
                    .content(move |_, _, cx| {
                        let popup = cx.entity().downgrade();
                        col().children(
                            [
                                (
                                    "duplicate-slide",
                                    I::Copy,
                                    "复制幻灯片",
                                    BasicEdit::DuplicateSlides,
                                    can_duplicate,
                                ),
                                (
                                    "delete-slide",
                                    I::Trash,
                                    "删除幻灯片",
                                    BasicEdit::DeleteSlides,
                                    can_delete,
                                ),
                            ]
                            .into_iter()
                            .map(|(id, icon, label, edit, enabled)| {
                                let view = view.clone();
                                let popup = popup.clone();
                                small(id, icon, label).disabled(!enabled).on_click(
                                    move |_, w, cx| {
                                        let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                        let _ =
                                            view.update(cx, |s, cx| s.run_basic_edit(edit, w, cx));
                                    },
                                )
                            }),
                        )
                    }),
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
        let view = cx.entity().downgrade();
        let moves = [
            (
                "forward",
                I::BringToFront,
                "上移一层",
                BasicEdit::BringForward,
            ),
            (
                "backward",
                I::SendToBack,
                "下移一层",
                BasicEdit::SendBackward,
            ),
            ("duplicate", I::CopyPlus, "创建副本", BasicEdit::Duplicate),
        ]
        .map(|(id, icon, label, edit)| {
            (
                id,
                icon,
                label,
                edit,
                self.basic_edit_unavailable(edit, cx).is_none(),
            )
        });
        group(
            "排列",
            row()
                .child(
                    Popover::new("arrange-menu")
                        .trigger(tool("arrange", I::Layers2, "排列", 0xa77436).dropdown_caret(true))
                        .content(move |_, _, cx| {
                            let popup = cx.entity().downgrade();
                            let selection_view = view.clone();
                            col()
                                .children(moves.into_iter().map(
                                    |(id, icon, label, edit, enabled)| {
                                        let view = view.clone();
                                        let popup = popup.clone();
                                        small(id, icon, label).disabled(!enabled).on_click(
                                            move |_, w, cx| {
                                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                                let _ = view.update(cx, |s, cx| {
                                                    s.run_basic_edit(edit, w, cx)
                                                });
                                            },
                                        )
                                    },
                                ))
                                .child(small("selection", I::PanelRight, "选择窗格").on_click(
                                    move |_, w, cx| {
                                        let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                        let _ = selection_view.update(cx, |s, cx| {
                                            s.inspector = !s.inspector;
                                            cx.notify();
                                        });
                                    },
                                ))
                        }),
                )
                .child(unavailable("quick-shape-style", I::Brush, "快速样式"))
                .child(
                    col()
                        .pt(px(4.))
                        .child(
                            small("home-shape-fill", I::PaintBucket, "形状填充")
                                .disabled(
                                    self.editor
                                        .as_ref()
                                        .is_none_or(|e| e.session.selection().shapes.is_empty())
                                        || self.loading
                                        || self.composing(),
                                )
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.content_form(content_ui::Form::ShapeFill, w, cx)
                                })),
                        )
                        .child(small_unavailable(
                            "home-shape-outline",
                            I::PenLine,
                            "形状轮廓",
                        )),
                ),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_insert(&self, cx: &mut Context<Self>) -> Div {
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let mut bar = row().h(px(RIBBON_HEIGHT)).items_start();
        bar = bar
            .child(group("幻灯片", self.new_slide_split(cx, "insert-slide")))
            .child(group(
                "表格",
                tool("insert-table", I::Table, "表格", BLUE)
                    .dropdown_caret(true)
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| s.insert_table_dialog(w, cx))),
            ))
            .child(group(
                "图像",
                row()
                    .child(
                        tool("insert-picture", I::Image, "图片", BLUE)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ImportPicture(false), w, cx)
                            })),
                    )
                    .child(unavailable("insert-screenshot", I::Scan, "屏幕截图")),
            ))
            .child(group(
                "摄像头",
                unavailable("insert-cameo", I::Camera, "Cameo"),
            ))
            .child(group(
                "插图",
                row()
                    .child(
                        tool("insert-shapes", I::Shapes, "形状", BLUE)
                            .dropdown_caret(true)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.shape_gallery(w, cx))),
                    )
                    .child(unavailable_row(&[
                        ("insert-icons", I::Shapes, "图标"),
                        ("insert-3d", I::Box, "3D 模型"),
                        ("insert-smartart", I::Network, "SmartArt"),
                        ("insert-chart", I::ChartColumn, "图表"),
                    ])),
            ))
            .child(group(
                "链接",
                row()
                    .child(unavailable("insert-zoom", I::Scan, "缩放定位"))
                    .child(self.text_link_controls(cx))
                    .child(unavailable("insert-action", I::MousePointer, "动作")),
            ))
            .child(group(
                "批注",
                tool("insert-comment", I::MessageSquarePlus, "批注", GREEN)
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| s.add_comment(w, cx))),
            ))
            .child(self.insert_text_group(disabled, cx))
            .child(group(
                "符号",
                unavailable_row(&[
                    ("insert-equation", I::Sigma, "公式"),
                    ("insert-symbol", I::Omega, "符号"),
                ]),
            ))
            .child(group(
                "媒体",
                unavailable_row(&[
                    ("insert-video", I::Video, "视频"),
                    ("insert-audio", I::Volume2, "音频"),
                ]),
            ));
        bar
    }

    #[inline(never)]
    fn insert_text_group(&self, disabled: bool, cx: &mut Context<Self>) -> Div {
        group(
            "文本",
            row()
                .child(
                    tool("insert-textbox", I::TextCursorInput, "文本框", BLUE)
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.insert_textbox(w, cx))),
                )
                .child(self.footer_control(cx))
                .child(unavailable("insert-wordart", I::Type, "艺术字"))
                .children(self.text_field_controls(cx))
                .child(unavailable("insert-object", I::FileBox, "对象")),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_design(&self, cx: &mut Context<Self>, disabled: bool) -> Div {
        let editor = self.editor.as_ref();
        let master = editor.and_then(|e| {
            let model = e.session.model();
            model
                .master_for(model.slides.get(self.page)?)
                .map(|(master, _)| master)
        });
        let p = palette(self.dark);
        let builtin = joydeck_model::theme::builtin_themes();
        let themes = gallery(self.dark).children(
            master
                .into_iter()
                .map(|m| (&m.theme, true))
                .chain(
                    builtin
                        .iter()
                        .filter(|theme| master.is_none_or(|m| m.theme.name != theme.name))
                        .map(|theme| (theme, false)),
                )
                .enumerate()
                .map(|(i, (theme, current))| theme_preview(i, theme, current, p)),
        );
        let mut variants = row().child(unavailable("design-variants", I::Brush, "变体"));
        for (id, label, icon, form) in [
            ("theme-colors", "颜色", I::Palette, content_ui::Form::Accent),
            ("theme-fonts", "字体", I::Type, content_ui::Form::Fonts),
            (
                "slide-background",
                "背景样式",
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
            variants = variants.child(
                tool(id, icon, label, BLUE)
                    .disabled(disabled)
                    .on_click(cx.listener(move |s, _, w, cx| s.content_form(form, w, cx))),
            );
        }
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group("主题", themes))
            .child(group("变体和背景", variants))
            .child(group(
                "自定义",
                row()
                    .child(
                        tool("design-layout", I::PanelsTopLeft, "版式", BLUE)
                            .dropdown_caret(true)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.layout_gallery(w, cx))),
                    )
                    .child(
                        tool("slide-size", I::RectangleHorizontal, "幻灯片\n大小", BLUE)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::Size, w, cx)
                            })),
                    ),
            ))
            .child(group(
                "设计建议",
                unavailable("designer", I::Zap, "设计\n建议"),
            ))
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_picture(&self, cx: &mut Context<Self>, selected: bool) -> Div {
        let adjustment = row()
            .child(unavailable("picture-remove-bg", I::ImageOff, "删除\n背景"))
            .child(
                tool("picture-corrections", I::Sun, "更正", 0xae6f30)
                    .disabled(!selected)
                    .on_click(
                        cx.listener(|s, _, w, cx| s.content_form(content_ui::Form::Picture, w, cx)),
                    ),
            )
            .child(unavailable("picture-color", I::Image, "颜色"))
            .child(unavailable("picture-effects", I::Sparkles, "艺术效果"))
            .child(
                tool("picture-alpha", I::Blend, "透明度", BLUE)
                    .disabled(!selected)
                    .on_click(
                        cx.listener(|s, _, w, cx| s.content_form(content_ui::Form::Picture, w, cx)),
                    ),
            )
            .child(
                col()
                    .child(small_unavailable(
                        "picture-compress",
                        I::Minimize2,
                        "压缩图片",
                    ))
                    .child(
                        small("replace-picture", I::ImagePlus, "更改图片")
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ImportPicture(true), w, cx)
                            })),
                    )
                    .child(small_unavailable("picture-reset", I::RotateCw, "重置图片")),
            );
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group("调整", adjustment))
            .child(group(
                "图片样式",
                unavailable_row(&[
                    ("picture-style", I::Image, "图片样式"),
                    ("picture-border", I::PenLine, "图片边框"),
                    ("picture-layout", I::PanelsTopLeft, "图片布局"),
                ]),
            ))
            .child(self.object_alt_text(cx))
            .child(self.object_arrange(cx))
            .child(group(
                "大小",
                row()
                    .child(
                        tool("crop-picture", I::Crop, "裁剪", BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::Picture, w, cx)
                            })),
                    )
                    .child(
                        tool("picture-size", I::Move, "大小和\n位置", BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
                    ),
            ))
    }

    fn object_alt_text(&self, cx: &mut Context<Self>) -> Div {
        group(
            "辅助功能",
            tool("object-alt-text", I::FilePenLine, "替代\n文本", BLUE)
                .on_click(cx.listener(|s, _, w, cx| s.accessibility_dialog(w, cx))),
        )
    }

    fn object_arrange(&self, cx: &mut Context<Self>) -> Div {
        group(
            "排列",
            row()
                .child(self.basic_button(
                    tool("object-forward", I::BringToFront, "前移一层", 0xa77436),
                    BasicEdit::BringForward,
                    cx,
                ))
                .child(self.basic_button(
                    tool("object-backward", I::SendToBack, "后移一层", 0xa77436),
                    BasicEdit::SendBackward,
                    cx,
                ))
                .child(
                    tool("object-selection-pane", I::PanelRight, "选择\n窗格", BLUE)
                        .toggled(self.inspector)
                        .selected(self.inspector)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.inspector = !s.inspector;
                            cx.notify();
                        })),
                ),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_chart(&self, cx: &mut Context<Self>, selected: bool) -> Div {
        use joydeck_model::chart::ChartType;
        let current = self.editor.as_ref().and_then(|e| {
            let id = e.session.selection().shapes.first()?;
            let shape = self.current_slide()?.shape(*id)?;
            match &shape.kind {
                joydeck_model::ShapeKind::Chart(chart) => Some(&chart.kind),
                _ => None,
            }
        });
        let mut kinds = gallery(self.dark);
        for (id, label, icon, kind) in [
            ("chart-column", "柱形图", I::ChartColumn, ChartType::Column),
            ("chart-line", "折线图", I::ChartLine, ChartType::Line),
            ("chart-pie", "饼图", I::ChartPie, ChartType::Pie),
        ] {
            kinds = kinds.child(
                tool(id, icon, label, BLUE)
                    .w(px(80.))
                    .h(px(52.))
                    .selected(current == Some(&kind))
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
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "图表布局",
                row()
                    .child(
                        tool("chart-title", I::Type, "图表\n标题", BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ChartTitle, w, cx)
                            })),
                    )
                    .child(unavailable("chart-layout", I::PanelsTopLeft, "快速布局")),
            ))
            .child(group(
                "图表样式",
                unavailable("chart-colors", I::Palette, "更改颜色"),
            ))
            .child(group("更改图表类型", kinds))
            .child(group(
                "数据",
                row()
                    .child(unavailable("chart-select-data", I::Table, "选择\n数据"))
                    .child(
                        tool("chart-data", I::Table, "编辑\n数据", BLUE)
                            .disabled(!selected)
                            .on_click(cx.listener(|s, _, w, cx| {
                                s.content_form(content_ui::Form::ChartValue, w, cx)
                            })),
                    )
                    .child(unavailable("chart-refresh-data", I::RotateCw, "刷新\n数据")),
            ))
    }

    #[inline(never)]
    fn ribbon_chart_format(&self, cx: &mut Context<Self>) -> Div {
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "当前所选内容",
                unavailable("chart-selection", I::ChartColumn, "图表区域"),
            ))
            .child(group(
                "形状样式",
                unavailable_row(&[
                    ("chart-shape-style", I::Brush, "快速样式"),
                    ("chart-shape-outline", I::PenLine, "形状轮廓"),
                ]),
            ))
            .child(self.object_arrange(cx))
            .child(group(
                "大小",
                tool("chart-size", I::Move, "大小和\n位置", BLUE)
                    .on_click(cx.listener(|s, _, w, cx| s.geometry_dialog(w, cx))),
            ))
    }
    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_shape(&self, cx: &mut Context<Self>) -> Div {
        let mut bar = row().h(px(RIBBON_HEIGHT)).items_start();
        bar = bar
            .child(group(
                "插入形状",
                row()
                    .child(
                        tool("format-shapes", I::Shapes, "形状", BLUE)
                            .dropdown_caret(true)
                            .on_click(cx.listener(|s, _, w, cx| s.shape_gallery(w, cx))),
                    )
                    .child(
                        tool("format-textbox", I::TextCursorInput, "文本框", BLUE)
                            .on_click(cx.listener(|s, _, w, cx| s.insert_textbox(w, cx))),
                    ),
            ))
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
            .child(self.object_alt_text(cx))
            .child(self.object_arrange(cx))
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
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let hidden = self.current_slide().is_some_and(|slide| slide.hidden);
        let show = self.editor.as_ref().map(|e| &e.session.model().show);
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "开始放映",
                row()
                    .child(
                        tool("show-start", I::Presentation, "从头\n播放", GREEN)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, _, cx| s.start_show(true, false, cx))),
                    )
                    .child(
                        tool("show-current", I::MonitorPlay, "从当前\n幻灯片播放", GREEN)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
                    )
                    .child(
                        tool("show-presenter", I::PanelsTopLeft, "演示者\n视图", BLUE)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, _, cx| s.start_show(false, true, cx))),
                    )
                    .child(unavailable("custom-show", I::PanelsTopLeft, "自定义\n放映")),
            ))
            .child(group(
                "设置",
                row()
                    .child(unavailable("show-setup", I::Settings, "设置\n幻灯片放映"))
                    .child(
                        tool("hide-slides", I::EyeOff, "隐藏\n幻灯片", BLUE)
                            .toggled(hidden)
                            .selected(hidden)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.toggle_hidden_slides(w, cx))),
                    ),
            ))
            .child(group(
                "计时和录制",
                row()
                    .child(unavailable("show-rehearse", I::Timer, "排练\n计时"))
                    .child(unavailable("show-record", I::Video, "录制"))
                    .child(ribbon_checks(&[
                        ("保持幻灯片更新", false),
                        (
                            "播放旁白",
                            show.is_some_and(|settings| !settings.without_narration),
                        ),
                    ]))
                    .child(ribbon_checks(&[
                        (
                            "使用计时",
                            show.is_some_and(|settings| settings.use_timings),
                        ),
                        (
                            "显示媒体控件",
                            show.is_some_and(|settings| settings.show_media_controls),
                        ),
                    ])),
            ))
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_review(&self, cx: &mut Context<Self>) -> Div {
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let mut bar = row().h(px(RIBBON_HEIGHT));
        bar = bar
            .child(group(
                "校对",
                unavailable_row(&[
                    ("review-spelling", I::SpellCheck, "拼写"),
                    ("review-thesaurus", I::BookOpen, "同义词\n库"),
                ]),
            ))
            .child(group(
                "辅助功能",
                tool("accessibility", I::Accessibility, "替代\n文字", BLUE)
                    .disabled(
                        disabled
                            || self
                                .editor
                                .as_ref()
                                .is_none_or(|e| e.session.selection().shapes.is_empty()),
                    )
                    .on_click(cx.listener(|s, _, w, cx| s.accessibility_dialog(w, cx))),
            ))
            .child(group(
                "语言",
                unavailable_row(&[
                    ("review-translate", I::Languages, "翻译"),
                    ("review-language", I::Languages, "语言"),
                ]),
            ))
            .child(group(
                "比较",
                tool("compare-documents", I::FileCheck, "比较\n文稿", BLUE)
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| s.compare_file(w, cx))),
            ))
            .child(group(
                "批注",
                row()
                    .child(
                        tool("add-comment", I::MessageSquarePlus, "新建\n批注", GREEN)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.add_comment(w, cx))),
                    )
                    .child(unavailable_row(&[
                        ("review-delete-comment", I::Trash, "删除"),
                        ("review-prev-comment", I::ArrowLeft, "上一条"),
                        ("review-next-comment", I::ArrowRight, "下一条"),
                    ]))
                    .child(
                        tool("comments", I::MessageSquare, "显示批注", BLUE)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.comments_dialog(w, cx))),
                    ),
            ))
            .child(self.review_document_tools(cx));
        bar
    }

    #[inline(never)]
    fn review_document_tools(&self, cx: &mut Context<Self>) -> Div {
        group(
            "文稿",
            row()
                .child(
                    tool("compatibility", I::FileCheck, "兼容性\n检查", GREEN)
                        .on_click(cx.listener(|s, _, w, cx| s.report(&Report, w, cx))),
                )
                .child(
                    tool("metadata", I::FilePenLine, "文稿\n属性", BLUE).on_click(
                        cx.listener(|s, _, w, cx| {
                            s.content_form(content_ui::Form::Metadata, w, cx)
                        }),
                    ),
                )
                .child(unavailable("review-hide-ink", I::PenLine, "隐藏墨迹")),
        )
    }

    // Keep ribbon construction in bounded frames on the Windows default stack.
    #[inline(never)]
    fn ribbon_view(&self, cx: &mut Context<Self>) -> Div {
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "演示文稿视图",
                row()
                    .child(
                        tool("normal", I::PanelsTopLeft, "普通", BLUE)
                            .toggled(!self.sorter)
                            .selected(!self.sorter)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.sorter = false;
                                cx.notify();
                            })),
                    )
                    .child(unavailable("outline-view", I::ListTree, "大纲\n视图"))
                    .child(
                        tool("sorter", I::LayoutGrid, "幻灯片\n浏览", BLUE)
                            .toggled(self.sorter)
                            .selected(self.sorter)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.sorter = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        tool("review-notes", I::NotebookPen, "备注页", BLUE)
                            .on_click(cx.listener(|s, _, w, cx| s.edit_notes(w, cx))),
                    )
                    .child(unavailable("reading-view", I::BookOpen, "阅读\n视图")),
            ))
            .child(group(
                "母版视图",
                unavailable_row(&[
                    ("slide-master", I::PanelsTopLeft, "幻灯片\n母版"),
                    ("handout-master", I::LayoutGrid, "讲义\n母版"),
                    ("notes-master", I::NotebookText, "备注\n母版"),
                ]),
            ))
            .child(group(
                "显示",
                row()
                    .child(ribbon_checks(&[
                        ("标尺", false),
                        ("网格线", false),
                        ("参考线", false),
                    ]))
                    .child(
                        tool("notes-view", I::NotebookPen, "备注", BLUE)
                            .toggled(self.notes_visible)
                            .selected(self.notes_visible)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.notes_visible = !s.notes_visible;
                                cx.notify();
                            })),
                    ),
            ))
            .child(group(
                "缩放",
                row()
                    .child(
                        tool("view-zoom", I::Search, "缩放", BLUE)
                            .on_click(cx.listener(|s, _, w, cx| s.ribbon_zoom_dialog(w, cx))),
                    )
                    .child(
                        tool("view-fit", I::Scan, "适应窗口\n大小", BLUE)
                            .on_click(cx.listener(|s, _, w, cx| s.fit(&Fit, w, cx))),
                    ),
            ))
            .child(self.view_window_tools(cx))
    }

    fn ribbon_zoom_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_dialog(
            "缩放",
            vec![(
                "缩放百分比（10–400）",
                format!("{:.0}", self.scale(w) * 100.),
            )],
            w,
            cx,
            |s, values, _, cx| {
                let value = values[0]
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| "请输入 10–400 的数值")?;
                if !(10. ..=400.).contains(&value) {
                    return Err("缩放百分比须介于 10 与 400 之间".into());
                }
                s.zoom = Some(value / 100.);
                cx.notify();
                Ok(())
            },
        );
    }

    #[inline(never)]
    fn view_window_tools(&self, cx: &mut Context<Self>) -> Div {
        group(
            "窗口和诊断",
            row()
                .child(
                    tool("shared-view", I::PanelsTopLeft, "新建\n文稿视图", BLUE)
                        .disabled(self.editor.is_none() || self.loading)
                        .on_click(cx.listener(|s, _, _, cx| s.new_shared_view(cx))),
                )
                .child(
                    tool("properties", I::PanelRight, "选择\n窗格", BLUE)
                        .toggled(self.inspector)
                        .selected(self.inspector)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.inspector = !s.inspector;
                            cx.notify();
                        })),
                )
                .child(
                    tool("render-report", I::Activity, "渲染\n报告", GREEN)
                        .on_click(cx.listener(|s, _, w, cx| s.scene_report(w, cx))),
                )
                .child(
                    tool("render-mode", I::Layers, "CPU / GPUI", 0x8a5cd8).on_click(cx.listener(
                        |s, _, w, cx| {
                            s.cpu_view = !s.cpu_view;
                            s.gpu_cache.clear(w);
                            cx.notify();
                        },
                    )),
                ),
        )
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
