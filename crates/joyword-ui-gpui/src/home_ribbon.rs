//! Home ribbon arranged after the desktop Word Home tab, using JoyWord commands.
use super::*;
use gpui::component::{Icon, popover::Popover, select::Select};
use joyoffice_ui::IconName as I;
use joyword_shell::authoring::StyleInfo;
use serde_json::{Value, json};

type MenuAction = (
    &'static str,
    &'static str,
    bool,
    fn(&mut JoyWord, &mut Window, &mut Context<JoyWord>),
);

fn row() -> gpui::Div {
    div().flex().items_center().gap(px(2.))
}

fn is_quick_style(style: &StyleInfo) -> bool {
    style.quick && !style.hidden && matches!(style.kind.as_str(), "paragraph" | "character")
}

fn menu_button(id: impl Into<gpui::ElementId>, label: &str, enabled: bool) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .h(px(23.))
        .px(px(3.))
        .accessibility_label(label.to_owned())
        .tooltip(label.to_owned())
        .disabled(!enabled)
}

impl JoyWord {
    pub(super) fn home_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_shrink_0()
            .child(self.home_clipboard(cx))
            .child(self.home_font(cx))
            .child(self.home_paragraph(cx))
            .child(self.home_styles(cx))
            .child(self.search_ribbon(cx))
    }

    pub(super) fn home_menu(
        &self,
        id: &'static str,
        trigger: Button,
        actions: Vec<MenuAction>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let owner = cx.weak_entity();
        let target = self.ribbon_target();
        Popover::new(id).trigger(trigger).content(move |_, _, cx| {
            let popup = cx.entity().downgrade();
            div()
                .flex()
                .flex_col()
                .w(px(222.))
                .children(actions.iter().map(|&(id, label, enabled, action)| {
                    let (owner, popup, target) = (owner.clone(), popup.clone(), target.clone());
                    Button::new(id)
                        .ghost()
                        .xsmall()
                        .label(label)
                        .disabled(!enabled)
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = owner.update(cx, |v, cx| {
                                if v.ribbon_target() == target {
                                    action(v, w, cx);
                                }
                            });
                        })
                }))
        })
    }

    fn home_clipboard(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let paste_label = tr("粘贴", "Paste");
        let can_paste = self.format_ready("edit.paste");
        let paste = menu_button("paste-ribbon", paste_label, can_paste)
            .w(px(53.))
            .h(px(47.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.))
                    .child(Icon::new(I::ClipboardPaste).size(px(26.)).text_color(rgb(
                        if can_paste {
                            self.palette().accent
                        } else {
                            self.palette().disabled
                        },
                    )))
                    .child(paste_label),
            )
            .on_click(cx.listener(|v, _, w, cx| v.clipboard("edit.paste", w, cx)));
        let paste_options = self.home_menu(
            "paste-options",
            menu_button(
                "paste-options-trigger",
                tr("粘贴选项", "Paste Options"),
                can_paste,
            )
            .w(px(53.))
            .h(px(16.))
            .icon(I::ChevronDown),
            vec![
                (
                    "paste-source",
                    tr("保留源格式", "Keep Source Formatting"),
                    can_paste,
                    |v, w, cx| {
                        v.clipboard("edit.paste", w, cx);
                    },
                ),
                (
                    "paste-merge",
                    tr("合并格式", "Merge Formatting"),
                    self.format_ready("edit.pasteMerge"),
                    |v, w, cx| {
                        v.clipboard("edit.pasteMerge", w, cx);
                    },
                ),
                (
                    "paste-text",
                    tr("只保留文本", "Keep Text Only"),
                    self.format_ready("edit.pasteText"),
                    |v, w, cx| {
                        v.clipboard("edit.pasteText", w, cx);
                    },
                ),
            ],
            cx,
        );
        let has_selection = self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| !r.engine().sel.is_collapsed());
        let brush = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.engine().painter.as_ref())
            .map(|p| p.2);
        let can_paint = self.format_ready("edit.formatPainter");
        let small = |id, label, icon, enabled| {
            menu_button(id, label, enabled)
                .w(px(26.))
                .h(px(21.))
                .child(Icon::new(icon).size(px(15.)))
        };
        let brush_menu = self.home_menu(
            "brush-options",
            menu_button(
                "brush-options-trigger",
                tr("格式刷选项", "Format Painter Options"),
                can_paint,
            )
            .w(px(15.))
            .h(px(21.))
            .icon(I::ChevronDown)
            .selected(brush == Some(true)),
            vec![(
                "format-brush-sticky",
                tr("连续格式刷", "Continuous Format Painter"),
                can_paint,
                |v, w, cx| {
                    v.start_brush(true, w, cx);
                },
            )],
            cx,
        );
        let tools = div()
            .flex()
            .flex_col()
            .child(
                small(
                    "cut-ribbon",
                    tr("剪切", "Cut"),
                    I::Scissors,
                    has_selection && self.format_ready("edit.cut"),
                )
                .on_click(cx.listener(|v, _, w, cx| v.clipboard("edit.cut", w, cx))),
            )
            .child(
                small(
                    "copy-ribbon",
                    tr("复制", "Copy"),
                    I::Copy,
                    has_selection && self.format_ready("edit.copy"),
                )
                .on_click(cx.listener(|v, _, w, cx| v.clipboard("edit.copy", w, cx))),
            )
            .child(
                div()
                    .flex()
                    .child(
                        small(
                            "format-brush",
                            tr("格式刷", "Format Painter"),
                            I::Paintbrush,
                            can_paint,
                        )
                        .selected(brush == Some(false))
                        .on_click(cx.listener(|v, _, w, cx| v.start_brush(false, w, cx))),
                    )
                    .child(brush_menu),
            );
        self.ribbon_group_with_launcher(
            "home-clipboard",
            tr("剪贴板", "Clipboard"),
            row()
                .child(div().flex().flex_col().child(paste).child(paste_options))
                .child(tools),
            None,
        )
    }

    fn home_format_icon(
        &self,
        (id, icon, command, label): (&'static str, I, &'static str, &'static str),
        selected: bool,
        mixed: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        let label = if mixed {
            format!(
                "{label}{}",
                self.shell.tr("（混合格式）", " (mixed formatting)")
            )
        } else {
            label.to_owned()
        };
        let color = self.palette().muted;
        menu_button(id, &label, self.format_ready(command))
            .w(px(26.))
            .child(Icon::new(icon).size(px(16.)))
            .selected(selected)
            .when(mixed, |button| {
                button.child(
                    div()
                        .absolute()
                        .right(px(2.))
                        .bottom_0()
                        .text_color(rgb(color))
                        .child("·"),
                )
            })
            .on_click(cx.listener(move |v, _, w, cx| v.run_format(command, &json!({}), w, cx)))
    }

    fn icon_choices(
        &self,
        (id, icon, command, label): (&'static str, I, &'static str, &'static str),
        choices: Vec<(String, Value)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ready = self.format_ready(command);
        let swatch = match id {
            "font-color" => Some(0xd92d20),
            "highlight" => Some(0xfacc15),
            _ => None,
        };
        let trigger = menu_button((id, 0usize), label, ready).w(px(37.)).child(
            row()
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .h(px(19.))
                        .child(Icon::new(icon).size(px(15.)))
                        .when_some(swatch.filter(|_| ready), |el, color| {
                            el.child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .h(px(2.))
                                    .bg(rgb(color)),
                            )
                        }),
                )
                .child(Icon::new(I::ChevronDown).size(px(9.))),
        );
        self.format_choices(id, trigger, command, choices, cx)
    }

    fn home_font(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let state = self.format_state();
        let size = state.character["size"]
            .as_f64()
            .map_or_else(|| "—".into(), |n| n.to_string());
        let size_menu = self.format_choices(
            "font-size",
            menu_button(
                ("font-size", 0usize),
                tr("字号", "Font Size"),
                self.format_ready("format.size"),
            )
            .w(px(52.))
            .border_1()
            .border_color(rgb(self.palette().border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .px(px(2.))
                    .child(size)
                    .child(Icon::new(I::ChevronDown).size(px(9.))),
            ),
            "format.size",
            [
                8., 9., 10., 10.5, 11., 12., 14., 16., 18., 20., 22., 24., 26., 28., 36., 48., 72.,
            ]
            .into_iter()
            .map(|size| (size.to_string(), json!({"size":size})))
            .collect(),
            cx,
        );
        let top = row()
            .child(
                Select::new(&self.font_picker)
                    .accessibility_label(tr("字体选择", "Font Family"))
                    .xsmall()
                    .h(px(23.))
                    .w(px(148.))
                    .placeholder(tr("字体", "Font"))
                    .disabled(!self.format_ready("format.font")),
            )
            .child(size_menu)
            .child(self.home_format_icon(
                (
                    "grow-font",
                    I::AArrowUp,
                    "format.growFont",
                    tr("增大字号", "Increase Font Size"),
                ),
                false,
                false,
                cx,
            ))
            .child(self.home_format_icon(
                (
                    "shrink-font",
                    I::AArrowDown,
                    "format.shrinkFont",
                    tr("减小字号", "Decrease Font Size"),
                ),
                false,
                false,
                cx,
            ))
            .child(
                self.icon_choices(
                    (
                        "change-case",
                        I::CaseSensitive,
                        "format.changeCase",
                        tr("更改大小写", "Change Case"),
                    ),
                    [
                        (tr("句首字母大写", "Sentence case"), "sentence"),
                        (tr("小写", "lowercase"), "lower"),
                        (tr("大写", "UPPERCASE"), "upper"),
                        (tr("每个单词首字母大写", "Capitalize Each Word"), "title"),
                        (tr("切换大小写", "tOGGLE cASE"), "toggle"),
                    ]
                    .into_iter()
                    .map(|(label, mode)| (label.into(), json!({"mode":mode})))
                    .collect(),
                    cx,
                ),
            )
            .child(self.home_format_icon(
                (
                    "clear-format",
                    I::Eraser,
                    "format.clear",
                    tr("清除所有格式", "Clear All Formatting"),
                ),
                false,
                false,
                cx,
            ));
        let mut bottom = row();
        for (id, icon, command, key, label) in [
            (
                "bold",
                I::Bold,
                "format.bold",
                "bold",
                tr("加粗 (Ctrl+B)", "Bold (Ctrl+B)"),
            ),
            (
                "italic",
                I::Italic,
                "format.italic",
                "italic",
                tr("倾斜 (Ctrl+I)", "Italic (Ctrl+I)"),
            ),
            (
                "underline",
                I::Underline,
                "format.underline",
                "underline",
                tr("下划线 (Ctrl+U)", "Underline (Ctrl+U)"),
            ),
            (
                "strike",
                I::Strikethrough,
                "format.strikethrough",
                "strike",
                tr("删除线", "Strikethrough"),
            ),
            (
                "subscript",
                I::Subscript,
                "format.subscript",
                "subscript",
                tr("下标", "Subscript"),
            ),
            (
                "superscript",
                I::Superscript,
                "format.superscript",
                "superscript",
                tr("上标", "Superscript"),
            ),
        ] {
            bottom = bottom.child(self.home_format_icon(
                (id, icon, command, label),
                state.active(key),
                state.mixed(key),
                cx,
            ));
            if id == "underline" {
                let label = tr("下划线样式", "Underline Styles");
                bottom = bottom.child(
                    self.format_choices(
                        "underline-styles",
                        menu_button(
                            "underline-styles-trigger",
                            label,
                            self.format_ready(command),
                        )
                        .w(px(14.))
                        .icon(I::ChevronDown),
                        command,
                        [
                            (tr("单线", "Single"), "single"),
                            (tr("双线", "Double"), "double"),
                            (tr("点线", "Dotted"), "dotted"),
                            (tr("波浪线", "Wave"), "wave"),
                        ]
                        .into_iter()
                        .map(|(label, style)| (label.into(), json!({"style":style})))
                        .collect(),
                        cx,
                    ),
                );
            }
        }
        bottom = bottom
            .child(
                self.icon_choices(
                    (
                        "highlight",
                        I::Highlighter,
                        "format.highlight",
                        tr("文本突出显示颜色", "Text Highlight Color"),
                    ),
                    [
                        (tr("无颜色", "No Color"), "none"),
                        (tr("黄色", "Yellow"), "yellow"),
                        (tr("绿色", "Green"), "brightGreen"),
                        (tr("粉色", "Pink"), "pink"),
                        (tr("青色", "Turquoise"), "turquoise"),
                    ]
                    .into_iter()
                    .map(|(label, color)| (label.into(), json!({"color":color})))
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.icon_choices(
                    (
                        "font-color",
                        I::Baseline,
                        "format.color",
                        tr("字体颜色", "Font Color"),
                    ),
                    [
                        (tr("自动", "Automatic"), "auto"),
                        (tr("黑色", "Black"), "000000"),
                        (tr("红色", "Red"), "C42B1C"),
                        (tr("蓝色", "Blue"), "2459A6"),
                        (tr("绿色", "Green"), "147D48"),
                        (tr("紫色", "Purple"), "7030A0"),
                    ]
                    .into_iter()
                    .map(|(label, color)| (label.into(), json!({"color":color})))
                    .collect(),
                    cx,
                ),
            );
        let label = tr("字体设置", "Font Settings");
        let launcher = menu_button("font-dialog", label, self.format_ready("format.set"))
            .w(px(17.))
            .h(px(16.))
            .p_0()
            .icon(I::ArrowDownRight)
            .on_click(cx.listener(|v, _, w, cx| v.format_dialog(formatting::Form::Font, w, cx)));
        self.ribbon_group_with_launcher(
            "home-font",
            tr("字体", "Font"),
            div().flex().flex_col().gap_1().child(top).child(bottom),
            Some(launcher),
        )
    }

    fn home_paragraph(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let state = self.format_state();
        let mut top = row();
        for (id, icon, command, label, selected, choices) in [
            (
                "bullets",
                I::List,
                "para.bullets",
                tr("项目符号", "Bullets"),
                state.bullets,
                vec![
                    ("•".into(), json!({"kind":"bullet"})),
                    ("○".into(), json!({"kind":"○"})),
                    ("▪".into(), json!({"kind":"▪"})),
                    (tr("无", "None").into(), json!({"off":true})),
                ],
            ),
            (
                "numbering",
                I::ListOrdered,
                "para.numbering",
                tr("编号", "Numbering"),
                state.numbered,
                [
                    ("1. 2. 3.", "numbered"),
                    ("1) 2) 3)", "numberedParen"),
                    ("A. B. C.", "upperLetter"),
                    ("a. b. c.", "lowerLetter"),
                    ("i. ii. iii.", "lowerRoman"),
                ]
                .into_iter()
                .map(|(label, kind)| (label.into(), json!({"kind":kind})))
                .collect(),
            ),
        ] {
            let menu_id = if id == "bullets" {
                "bullet-kinds"
            } else {
                "numbering-kinds"
            };
            top = top.child(
                div()
                    .flex()
                    .items_center()
                    .child(self.home_format_icon((id, icon, command, label), selected, false, cx))
                    .child(
                        self.format_choices(
                            menu_id,
                            menu_button((menu_id, 0usize), label, self.format_ready(command))
                                .w(px(14.))
                                .icon(I::ChevronDown),
                            command,
                            choices,
                            cx,
                        ),
                    ),
            );
        }
        top = top
            .child(
                self.icon_choices(
                    (
                        "multilevel-list",
                        I::ListTree,
                        "para.multilevel",
                        tr("多级列表", "Multilevel List"),
                    ),
                    [
                        ("1. 1.1 1.1.1".into(), json!({"kind":"legal"})),
                        ("I. A. 1.".into(), json!({"kind":"outline"})),
                    ]
                    .into(),
                    cx,
                ),
            )
            .child(self.home_format_icon(
                (
                    "outdent",
                    I::ListIndentDecrease,
                    "para.outdent",
                    tr("减少缩进", "Decrease Indent"),
                ),
                false,
                false,
                cx,
            ))
            .child(self.home_format_icon(
                (
                    "indent",
                    I::ListIndentIncrease,
                    "para.indent",
                    tr("增加缩进", "Increase Indent"),
                ),
                false,
                false,
                cx,
            ))
            .child(
                self.icon_choices(
                    (
                        "sort-paragraphs",
                        I::ArrowDownAZ,
                        "para.sort",
                        tr("排序", "Sort"),
                    ),
                    [
                        (tr("升序", "Ascending").into(), json!({"descending":false})),
                        (tr("降序", "Descending").into(), json!({"descending":true})),
                    ]
                    .into(),
                    cx,
                ),
            )
            .child(
                self.home_format_icon(
                    (
                        "formatting-marks",
                        I::Pilcrow,
                        "view.marks",
                        tr("显示／隐藏格式标记", "Show/Hide Formatting Marks"),
                    ),
                    self.canvas
                        .reader
                        .as_ref()
                        .is_some_and(|r| r.engine().view.marks),
                    false,
                    cx,
                ),
            );
        let mut bottom = row();
        for (id, icon, command, align, label) in [
            (
                "align-left",
                I::TextAlignStart,
                "para.alignLeft",
                "left",
                tr("左对齐 (Ctrl+L)", "Align Left (Ctrl+L)"),
            ),
            (
                "align-center",
                I::TextAlignCenter,
                "para.alignCenter",
                "center",
                tr("居中 (Ctrl+E)", "Center (Ctrl+E)"),
            ),
            (
                "align-right",
                I::TextAlignEnd,
                "para.alignRight",
                "right",
                tr("右对齐 (Ctrl+R)", "Align Right (Ctrl+R)"),
            ),
            (
                "align-justify",
                I::TextAlignJustify,
                "para.justify",
                "justify",
                tr("两端对齐 (Ctrl+J)", "Justify (Ctrl+J)"),
            ),
            (
                "align-distribute",
                I::AlignHorizontalDistributeCenter,
                "para.distribute",
                "distribute",
                tr("分散对齐", "Distributed"),
            ),
        ] {
            bottom = bottom.child(self.home_format_icon(
                (id, icon, command, label),
                state.paragraph["align"].as_str() == Some(align),
                false,
                cx,
            ));
        }
        bottom = bottom
            .child(
                self.icon_choices(
                    (
                        "line-spacing",
                        I::ArrowUpDown,
                        "para.lineSpacing",
                        tr("行距", "Line Spacing"),
                    ),
                    [1., 1.15, 1.5, 2., 2.5, 3.]
                        .into_iter()
                        .map(|value| (value.to_string(), json!({"value":value})))
                        .collect(),
                    cx,
                ),
            )
            .child(
                self.icon_choices(
                    (
                        "para-fill",
                        I::PaintBucket,
                        "para.shading",
                        tr("底纹", "Shading"),
                    ),
                    [
                        (tr("无底纹", "No Shading"), None),
                        (tr("浅蓝", "Light Blue"), Some("DCE6F1")),
                        (tr("浅黄", "Light Yellow"), Some("FFF2CC")),
                        (tr("浅绿", "Light Green"), Some("E2F0D9")),
                    ]
                    .into_iter()
                    .map(|(label, color)| (label.into(), json!({"color":color})))
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.icon_choices(
                    (
                        "para-borders",
                        I::Grid2x2,
                        "para.borders",
                        tr("边框", "Borders"),
                    ),
                    [
                        (tr("无边框", "No Border"), "none"),
                        (tr("下边框", "Bottom Border"), "bottom"),
                        (tr("外边框", "Outside Borders"), "outside"),
                        (tr("全部边框", "All Borders"), "all"),
                    ]
                    .into_iter()
                    .map(|(label, kind)| (label.into(), json!({"kind":kind})))
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.icon_choices(
                    (
                        "para-pagination",
                        I::FileText,
                        "para.set",
                        tr("分页选项", "Pagination Options"),
                    ),
                    [
                        (tr("与下段同页", "Keep with Next"), "keepNext"),
                        (tr("段中不分页", "Keep Lines Together"), "keepLines"),
                        (tr("段前分页", "Page Break Before"), "pageBreakBefore"),
                        (tr("孤行控制", "Widow/Orphan Control"), "widowControl"),
                    ]
                    .into_iter()
                    .map(|(label, key)| {
                        let on = state.paragraph[key].as_bool() == Some(true);
                        (
                            format!("{} {label}", if on { "✓" } else { "○" }),
                            json!({"props":{key:!on}}),
                        )
                    })
                    .collect(),
                    cx,
                ),
            );
        let launcher = menu_button(
            "paragraph-dialog",
            tr("段落设置", "Paragraph Settings"),
            self.format_ready("para.set"),
        )
        .w(px(17.))
        .h(px(16.))
        .p_0()
        .icon(I::ArrowDownRight)
        .on_click(cx.listener(|v, _, w, cx| v.format_dialog(formatting::Form::Paragraph, w, cx)));
        self.ribbon_group_with_launcher(
            "home-paragraph",
            tr("段落", "Paragraph"),
            div().flex().flex_col().gap_1().child(top).child(bottom),
            Some(launcher),
        )
    }

    fn home_style_name(&self, style: &StyleInfo) -> String {
        match style.id.as_str() {
            "Normal" if style.name.eq_ignore_ascii_case("Normal") => {
                self.shell.tr("正文", "Normal").into()
            }
            "NoSpacing" if style.name.eq_ignore_ascii_case("No Spacing") => {
                self.shell.tr("无间隔", "No Spacing").into()
            }
            "Heading1" if style.name.eq_ignore_ascii_case("Heading 1") => {
                self.shell.tr("标题 1", "Heading 1").into()
            }
            "Heading2" if style.name.eq_ignore_ascii_case("Heading 2") => {
                self.shell.tr("标题 2", "Heading 2").into()
            }
            _ => style.name.clone(),
        }
    }

    fn home_style_tile(
        &self,
        id: impl Into<gpui::ElementId>,
        style: &StyleInfo,
        active: bool,
    ) -> Button {
        let name = self.home_style_name(style);
        let p = self.palette();
        let enabled = self.format_ready("para.style");
        let size = style.resolved["size"]
            .as_f64()
            .unwrap_or(11.)
            .clamp(10., 23.) as f32;
        let preview = div()
            .w_full()
            .truncate()
            .text_center()
            .text_size(px(size))
            .when(style.resolved["bold"].as_bool() == Some(true), |el| {
                el.font_weight(gpui::FontWeight::BOLD)
            })
            .when(style.resolved["italic"].as_bool() == Some(true), |el| {
                el.italic()
            })
            .child(name.clone());
        menu_button(id, &name, enabled)
            .w(px(116.))
            .h(px(53.))
            .px(px(3.))
            .rounded(px(2.))
            .border_1()
            .border_color(rgb(if active { p.accent } else { p.separator }))
            .selected(active)
            .when(active, |button| button.bg(rgb(p.selected)))
            .child(preview)
    }

    fn home_styles(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let state = self.format_state();
        let active_para = state.paragraph["style"].as_str();
        let mut featured = Vec::new();
        for (id, button) in [
            ("Normal", "normal-style"),
            ("NoSpacing", "no-spacing-style"),
            ("Heading1", "heading1"),
        ] {
            if let Some(style) = self
                .style_catalog
                .iter()
                .find(|style| style.id == id && !style.hidden && style.kind == "paragraph")
            {
                featured.push((style, button));
            }
        }
        let cards = row().children(featured.into_iter().map(|(style, button)| {
            let args = json!({"style":style.id});
            self.home_style_tile(button, style, active_para == Some(style.id.as_str()))
                .on_click(cx.listener(move |v, _, w, cx| v.run_format("para.style", &args, w, cx)))
        }));
        let owner = cx.weak_entity();
        let target = self.ribbon_target();
        let label = tr("更多样式", "More Styles");
        let more = Popover::new("quick-styles")
            .anchor(gpui::Anchor::TopRight)
            .trigger(
                menu_button(
                    "quick-styles-trigger",
                    label,
                    self.format_ready("para.style")
                        && self.style_catalog.iter().any(is_quick_style),
                )
                .w(px(22.))
                .h(px(53.))
                .p_0()
                .border_1()
                .border_color(rgb(self.palette().border))
                .icon(I::ChevronDown),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let target = target.clone();
                owner
                    .read_with(cx, |v, _| {
                        let styles: Vec<_> = v
                            .style_catalog
                            .iter()
                            .filter(|style| is_quick_style(style))
                            .collect();
                        let state = v.format_state();
                        let active_para = state.paragraph["style"].as_str();
                        div()
                            .id("home-styles-gallery")
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_1()
                            .max_h(px(320.))
                            .overflow_y_scroll()
                            .children(styles.chunks(3).enumerate().map(|(row_index, tiles)| {
                                row().children(tiles.iter().enumerate().map(|(column, style)| {
                                    let (owner, target, popup) =
                                        (owner.clone(), target.clone(), popup.clone());
                                    let args = json!({"style":style.id});
                                    v.home_style_tile(
                                        ("quick-style", row_index * 3 + column),
                                        style,
                                        style.kind == "paragraph"
                                            && active_para == Some(style.id.as_str()),
                                    )
                                    .on_click(
                                        move |_, w, cx| {
                                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                            let _ = owner.update(cx, |v, cx| {
                                                if v.ribbon_target() == target {
                                                    v.run_format("para.style", &args, w, cx);
                                                }
                                            });
                                        },
                                    )
                                }))
                            }))
                    })
                    .unwrap_or_else(|_| div().id("home-styles-gallery"))
            });
        let launcher = menu_button(
            "styles-pane",
            tr("样式窗格", "Styles Pane"),
            self.canvas.reader.is_some(),
        )
        .w(px(17.))
        .h(px(16.))
        .p_0()
        .icon(I::ArrowDownRight)
        .selected(self.styles_open)
        .on_click(cx.listener(|v, _, _, cx| v.toggle_styles(cx)));
        self.ribbon_group_with_launcher(
            "home-styles",
            tr("样式", "Styles"),
            cards.child(more),
            Some(launcher),
        )
    }
}
