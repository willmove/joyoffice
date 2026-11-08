//! JoyOffice Kit pickers/forms adapted to Wordcraft command parameters and selection state.
use super::*;
use gpui::EntityInputHandler;
use gpui::component::{
    Disableable, Selectable, Sizable, WindowExt,
    input::{Input, InputState},
    popover::Popover,
    select::{SearchableVec, Select, SelectState},
};
use joyword_shell::formatting::{
    FormatState, font_families, font_parameters, paragraph_parameters,
};
use serde_json::{Value, json};

pub(super) type FontSelect = gpui::Entity<SelectState<SearchableVec<SharedString>>>;
#[derive(Clone, Copy)]
pub(super) enum Form {
    Font,
    Paragraph,
}

pub(super) fn initial_font_picker(w: &mut Window, cx: &mut Context<JoyWord>) -> FontSelect {
    let names: Vec<SharedString> = font_families().into_iter().map(Into::into).collect();
    cx.new(|cx| SelectState::new(SearchableVec::new(names), None, w, cx).searchable(true))
}
impl JoyWord {
    pub(super) fn format_ready(&self, command: &str) -> bool {
        !self.canvas.loading
            && !self.file_page
            && !self.confirm_discard
            && self.replace_confirm.is_none()
            && self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| r.formatting_disabled(command).is_none())
    }
    pub(super) fn sync_format(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let key = self
            .canvas
            .reader
            .as_ref()
            .map(|r| (r.engine().document_id(), r.engine().rev()));
        if key != self.style_catalog_key {
            self.style_catalog = self
                .canvas
                .reader
                .as_ref()
                .map(|r| r.styles())
                .unwrap_or_default();
            self.style_catalog_key = key;
        }
        if self.format_state.is_some() {
            return;
        }
        let state = self
            .canvas
            .reader
            .as_ref()
            .map(|r| r.formatting())
            .unwrap_or_default();
        let family = state.character["font"].as_str().map(SharedString::from);
        let document = self
            .canvas
            .reader
            .as_ref()
            .map(|r| r.engine().document_id());
        if self.font_document != document
            || self.font_picker.read(cx).selected_value() != family.as_ref()
        {
            // Reader import catalogs system/document fonts after the window is constructed.
            // Keep an unavailable document font visible, as in the host's font picker.
            let mut names: Vec<SharedString> =
                font_families().into_iter().map(Into::into).collect();
            if let Some(name) = &family
                && !names.contains(name)
            {
                names.insert(0, name.clone());
            }
            self.font_picker.update(cx, |s, cx| {
                s.set_items(SearchableVec::new(names), w, cx);
                if let Some(name) = &family {
                    s.set_selected_value(name, w, cx)
                } else {
                    s.set_selected_index(None, w, cx)
                }
            });
            self.font_document = document;
        }
        self.format_state = Some(state);
    }
    fn format_state(&self) -> FormatState {
        self.format_state.clone().unwrap_or_default()
    }
    pub(super) fn run_format(
        &mut self,
        id: &str,
        args: &Value,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.try_edit(id, args, w, cx);
    }
    #[allow(clippy::too_many_arguments)] // Stable ribbon ID, command, state and accessibility metadata.
    fn format_button(
        &self,
        id: &'static str,
        label: &str,
        command: &'static str,
        args: Value,
        key: &str,
        tip: &str,
        cx: &mut Context<Self>,
    ) -> Button {
        let state = self.format_state();
        let mixed = state.mixed(key);
        let selected = if key == "bullets" {
            state.bullets
        } else if key == "numbering" {
            state.numbered
        } else if let Some(align) = key.strip_prefix("align:") {
            state.paragraph["align"].as_str() == Some(align)
        } else if let Some(flag) = key.strip_prefix("para:") {
            state.paragraph[flag].as_bool() == Some(true)
        } else if let Some(style) = key.strip_prefix("style:") {
            state.paragraph["style"].as_str() == Some(style)
        } else {
            state.active(key)
        };
        let label = if mixed {
            format!("{label} ·")
        } else {
            label.to_owned()
        };
        let reason = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.formatting_disabled(command));
        Button::new(id)
            .ghost()
            .xsmall()
            .h(px(27.))
            .label(label)
            .accessibility_label(if mixed {
                format!("{tip}（混合格式）")
            } else {
                tip.to_owned()
            })
            .tooltip(reason.unwrap_or(tip).to_owned())
            .selected(selected)
            .disabled(!self.format_ready(command))
            .on_click(cx.listener(move |v, _, w, cx| v.run_format(command, &args, w, cx)))
    }
    pub(super) fn choices(
        &self,
        id: &'static str,
        label: String,
        command: &'static str,
        choices: Vec<(String, Value)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        let target = self
            .canvas
            .reader
            .as_ref()
            .map(|r| (r.engine().document_id(), r.engine().rev()));
        Popover::new(id)
            .trigger(
                Button::new((id, 0usize))
                    .ghost()
                    .xsmall()
                    .h(px(27.))
                    .label(label.clone())
                    .accessibility_label(label)
                    .disabled(!self.format_ready(command)),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut list = div()
                    .flex()
                    .flex_col()
                    .w(px(190.))
                    .max_h(px(380.))
                    .id((id, 1usize))
                    .overflow_y_scroll();
                for (i, (label, args)) in choices.iter().enumerate() {
                    let owner = owner.clone();
                    let popup = popup.clone();
                    let args = args.clone();
                    list = list.child(
                        Button::new((id, i + 2))
                            .ghost()
                            .xsmall()
                            .label(label.clone())
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = owner.update(cx, |v, cx| {
                                    if v.canvas
                                        .reader
                                        .as_ref()
                                        .map(|r| (r.engine().document_id(), r.engine().rev()))
                                        == target
                                    {
                                        v.run_format(command, &args, w, cx);
                                    }
                                });
                            }),
                    );
                }
                list
            })
    }
    pub(super) fn group_box(&self, title: &str, body: impl IntoElement) -> gpui::Div {
        let p = self.palette();
        div()
            .flex()
            .flex_col()
            .justify_between()
            .h(px(82.))
            .px_2()
            .border_r_1()
            .border_color(rgb(p.separator))
            .child(body)
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(p.muted))
                    .child(title.to_owned()),
            )
    }
    pub(super) fn home_format_groups(&self, cx: &mut Context<Self>) -> gpui::Div {
        let s = self.format_state();
        let tr = |zh, en| self.shell.tr(zh, en);
        let size = s.character["size"]
            .as_f64()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "混合".into());
        let font_row = div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                Select::new(&self.font_picker)
                    .accessibility_label("字体选择")
                    .xsmall()
                    .w(px(170.))
                    .placeholder("混合字体 / 字体")
                    .disabled(!self.format_ready("format.font")),
            )
            .child(
                self.choices(
                    "font-size",
                    format!("{size} 磅 ▾"),
                    "format.size",
                    [
                        8., 9., 10., 10.5, 11., 12., 14., 16., 18., 20., 22., 24., 26., 28., 36.,
                        48., 72.,
                    ]
                    .into_iter()
                    .map(|n| (format!("{n} 磅"), json!({"size":n})))
                    .collect(),
                    cx,
                ),
            )
            .child(
                Button::new("font-dialog")
                    .ghost()
                    .xsmall()
                    .label("字体…")
                    .accessibility_label("字体设置")
                    .disabled(!self.format_ready("format.set"))
                    .on_click(cx.listener(|v, _, w, cx| v.format_dialog(Form::Font, w, cx))),
            );
        let mut font = div().flex().flex_col().gap_1().child(font_row);
        let mut toggles = div().flex().items_center().gap_1();
        for (id, label, cmd, key, tip) in [
            ("bold", "B", "format.bold", "bold", "加粗 (Ctrl+B)"),
            ("italic", "I", "format.italic", "italic", "斜体 (Ctrl+I)"),
            (
                "underline",
                "U",
                "format.underline",
                "underline",
                "下划线 (Ctrl+U)",
            ),
            ("strike", "abc̶", "format.strikethrough", "strike", "删除线"),
            ("subscript", "x₂", "format.subscript", "subscript", "下标"),
            (
                "superscript",
                "x²",
                "format.superscript",
                "superscript",
                "上标",
            ),
        ] {
            toggles = toggles.child(self.format_button(id, label, cmd, json!({}), key, tip, cx));
        }
        toggles = toggles
            .child(
                self.choices(
                    "font-color",
                    "字色 ▾".into(),
                    "format.color",
                    [
                        ("自动", "auto"),
                        ("黑色", "000000"),
                        ("红色", "C42B1C"),
                        ("蓝色", "2459A6"),
                        ("绿色", "147D48"),
                        ("紫色", "7030A0"),
                    ]
                    .into_iter()
                    .map(|(l, c)| (l.into(), json!({"color":c})))
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.choices(
                    "highlight",
                    "高亮 ▾".into(),
                    "format.highlight",
                    [
                        ("无高亮", "none"),
                        ("黄色", "yellow"),
                        ("绿色", "brightGreen"),
                        ("粉色", "pink"),
                        ("青色", "turquoise"),
                    ]
                    .into_iter()
                    .map(|(l, c)| (l.into(), json!({"color":c})))
                    .collect(),
                    cx,
                ),
            );
        font = font.child(toggles);
        let mut para = div().flex().flex_col().gap_1();
        let mut aligns = div().flex().gap_1();
        for (id, label, cmd, key, tip) in [
            (
                "align-left",
                "左",
                "para.alignLeft",
                "align:left",
                "左对齐 (Ctrl+L)",
            ),
            (
                "align-center",
                "中",
                "para.alignCenter",
                "align:center",
                "居中 (Ctrl+E)",
            ),
            (
                "align-right",
                "右",
                "para.alignRight",
                "align:right",
                "右对齐 (Ctrl+R)",
            ),
            (
                "align-justify",
                "齐",
                "para.justify",
                "align:justify",
                "两端对齐 (Ctrl+J)",
            ),
            ("bullets", "•", "para.bullets", "bullets", "项目符号"),
            ("numbering", "1.", "para.numbering", "numbering", "编号"),
            ("outdent", "←", "para.outdent", "", "减少缩进"),
            ("indent", "→", "para.indent", "", "增加缩进"),
        ] {
            aligns = aligns.child(self.format_button(id, label, cmd, json!({}), key, tip, cx));
        }
        aligns = aligns.child(
            self.choices(
                "numbering-kinds",
                "编号样式 ▾".into(),
                "para.numbering",
                [
                    ("1. 2. 3.", "numbered"),
                    ("1) 2) 3)", "numberedParen"),
                    ("A. B. C.", "upperLetter"),
                    ("a. b. c.", "lowerLetter"),
                    ("i. ii. iii.", "lowerRoman"),
                ]
                .into_iter()
                .map(|(l, k)| (l.into(), json!({"kind":k})))
                .collect(),
                cx,
            ),
        );
        para = para.child(aligns).child(
            div()
                .flex()
                .gap_1()
                .child(
                    self.choices(
                        "line-spacing",
                        "行距 ▾".into(),
                        "para.lineSpacing",
                        [1., 1.15, 1.5, 2., 2.5, 3.]
                            .into_iter()
                            .map(|n| (format!("{n} 倍行距"), json!({"value":n})))
                            .collect(),
                        cx,
                    ),
                )
                .child(
                    self.choices(
                        "para-borders",
                        "边框 ▾".into(),
                        "para.borders",
                        [
                            ("无边框", "none"),
                            ("下边框", "bottom"),
                            ("外边框", "outside"),
                            ("全部边框", "all"),
                        ]
                        .into_iter()
                        .map(|(l, k)| (l.into(), json!({"kind":k})))
                        .collect(),
                        cx,
                    ),
                )
                .child(
                    self.choices(
                        "para-fill",
                        "底纹 ▾".into(),
                        "para.shading",
                        [
                            ("无底纹", None),
                            ("浅蓝", Some("DCE6F1")),
                            ("浅黄", Some("FFF2CC")),
                            ("浅绿", Some("E2F0D9")),
                        ]
                        .into_iter()
                        .map(|(l, c)| (l.into(), json!({"color":c})))
                        .collect(),
                        cx,
                    ),
                )
                .child(
                    self.choices(
                        "para-pagination",
                        "分页 ▾".into(),
                        "para.set",
                        [
                            ("与下段同页", "keepNext"),
                            ("段中不分页", "keepLines"),
                            ("段前分页", "pageBreakBefore"),
                            ("孤行控制", "widowControl"),
                        ]
                        .into_iter()
                        .map(|(l, k)| {
                            let on = s.paragraph[k].as_bool() == Some(true);
                            let mut props = serde_json::Map::new();
                            props.insert(k.into(), json!(!on));
                            (
                                format!("{} {l}", if on { "✓" } else { "○" }),
                                json!({"props":props}),
                            )
                        })
                        .collect(),
                        cx,
                    ),
                )
                .child(
                    self.author_button(
                        "formatting-marks",
                        tr("显示格式标记 ¶", "Formatting marks ¶"),
                        "view.marks",
                        json!({}),
                        cx,
                    )
                    .selected(
                        self.canvas
                            .reader
                            .as_ref()
                            .is_some_and(|r| r.engine().view.marks),
                    ),
                )
                .child(
                    Button::new("paragraph-dialog")
                        .ghost()
                        .xsmall()
                        .label("段落…")
                        .accessibility_label("段落设置")
                        .disabled(!self.format_ready("para.set"))
                        .on_click(
                            cx.listener(|v, _, w, cx| v.format_dialog(Form::Paragraph, w, cx)),
                        ),
                ),
        );
        let styles = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(self.format_button(
                        "normal-style",
                        "正文",
                        "para.normal",
                        json!({}),
                        "style:Normal",
                        "正文样式 (Ctrl+Shift+N)",
                        cx,
                    ))
                    .child(self.format_button(
                        "heading1",
                        "标题 1",
                        "para.heading1",
                        json!({}),
                        "style:Heading1",
                        "标题 1 (Ctrl+Alt+1)",
                        cx,
                    ))
                    .child(
                        self.choices(
                            "quick-styles",
                            "快速样式 ▾".into(),
                            "para.style",
                            self.style_catalog
                                .iter()
                                .filter(|s| {
                                    s.quick
                                        && !s.hidden
                                        && matches!(s.kind.as_str(), "paragraph" | "character")
                                })
                                .map(|s| (s.name.clone(), json!({"style":s.id})))
                                .collect(),
                            cx,
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(self.format_button(
                        "heading2",
                        "标题 2",
                        "para.heading2",
                        json!({}),
                        "style:Heading2",
                        "标题 2 (Ctrl+Alt+2)",
                        cx,
                    ))
                    .child(self.format_button(
                        "clear-format",
                        "清除",
                        "format.clear",
                        json!({}),
                        "",
                        "清除格式",
                        cx,
                    ))
                    .child(
                        Button::new("styles-pane")
                            .ghost()
                            .xsmall()
                            .label("样式面板")
                            .on_click(cx.listener(|v, _, _, cx| v.toggle_styles(cx))),
                    ),
            );
        div()
            .flex()
            .items_center()
            .child(self.group_box(tr("字体", "Font"), font))
            .child(self.group_box(tr("段落", "Paragraph"), para))
            .child(self.group_box(tr("样式", "Styles"), styles))
    }
    pub(super) fn format_dialog(&mut self, kind: Form, w: &mut Window, cx: &mut Context<Self>) {
        let command = match kind {
            Form::Font => "format.set",
            Form::Paragraph => "para.set",
        };
        if !self.format_ready(command) {
            return;
        }
        let Some(reader) = self.canvas.reader.as_ref() else {
            return;
        };
        let target = (
            reader.engine().document_id(),
            reader.engine().rev(),
            reader.engine().sel.clone(),
        );
        let state = reader.formatting();
        let val = |v: &Value| {
            if v.is_null() {
                String::new()
            } else if let Some(s) = v.as_str() {
                s.into()
            } else if let Some(n) = v.as_f64() {
                format!("{n:.3}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_owned()
            } else {
                v.to_string()
            }
        };
        let fields = match kind {
            Form::Font => vec![
                ("西文字体", val(&state.character["font"])),
                ("中文字体", val(&state.character["fontEastAsia"])),
                ("字号（1–400 磅）", val(&state.character["size"])),
                ("文字颜色（RGB / auto）", val(&state.character["color"])),
            ],
            Form::Paragraph => vec![
                ("起始缩进（磅）", val(&state.paragraph["indentLeft"])),
                ("结束缩进（磅）", val(&state.paragraph["indentRight"])),
                (
                    "首行缩进（负数为悬挂）",
                    val(&state.paragraph["indentFirst"]),
                ),
                ("段前间距（磅）", val(&state.paragraph["spaceBefore"])),
                ("段后间距（磅）", val(&state.paragraph["spaceAfter"])),
                (
                    "行距（倍数 / exact:磅 / min:磅）",
                    match state.paragraph["lineSpacing"]["rule"].as_str() {
                        Some("exactly") => {
                            format!("exact:{}", val(&state.paragraph["lineSpacing"]["value"]))
                        }
                        Some("atLeast") => {
                            format!("min:{}", val(&state.paragraph["lineSpacing"]["value"]))
                        }
                        Some(_) => val(&state.paragraph["lineSpacing"]["value"]),
                        None => String::new(),
                    },
                ),
            ],
        };
        let initial_values: Vec<String> = fields.iter().map(|(_, value)| value.clone()).collect();
        let inputs: Vec<_> = fields
            .into_iter()
            .map(|(label, value)| {
                let input = cx.new(|cx| {
                    let mut i = InputState::new(w, cx).placeholder("留空保持原值");
                    i.set_value(value, w, cx);
                    i
                });
                (label, input)
            })
            .collect();
        let owner = cx.entity().downgrade();
        let error = cx.new(|_| String::new());
        let close_owner = owner.clone();
        w.open_dialog(cx, move |d, _, cx| {
            let mut body = div()
                .flex()
                .flex_col()
                .gap_2()
                .child("留空保持每个选区的原值；应用为一次撤销。取消保留原文档。");
            for (label, input) in &inputs {
                body = body
                    .child(div().text_sm().child(*label))
                    .child(Input::new(input).aria_label(*label).xsmall());
            }
            let inputs = inputs.clone();
            let owner = owner.clone();
            let error2 = error.clone();
            let target = target.clone();
            let close_owner = close_owner.clone();
            let cancel_owner = owner.clone();
            let initial_values = initial_values.clone();
            d.title(match kind {
                Form::Font => "字体设置",
                Form::Paragraph => "段落设置",
            })
            .width(px(440.))
            .child(body)
            .child(
                div()
                    .text_color(rgb(0xc42b1c))
                    .child(error.read(cx).clone()),
            )
            .on_close(move |_, w, cx| {
                let _ = close_owner.update(cx, |v, cx| w.focus(&v.focus, cx));
            })
            .child(
                Button::new("apply-format")
                    .label("应用")
                    .on_click(move |_, w, cx| {
                        let _ = owner.update(cx, |v, cx| {
                            if v.canvas.reader.as_ref().map(|r| {
                                (
                                    r.engine().document_id(),
                                    r.engine().rev(),
                                    r.engine().sel.clone(),
                                )
                            }) != Some(target.clone())
                            {
                                error2.update(cx, |e, cx| {
                                    *e = "文档或选区已变化，请重新打开设置。".into();
                                    cx.notify();
                                });
                                return;
                            }
                            if inputs.iter().any(|(_, i)| {
                                i.update(cx, |s, cx| s.marked_text_range(w, cx).is_some())
                            }) {
                                return;
                            }
                            let values: Vec<String> = inputs
                                .iter()
                                .enumerate()
                                .map(|(index, (_, i))| {
                                    let value = i.read(cx).value().to_string();
                                    if value.trim() == initial_values[index].trim() {
                                        String::new()
                                    } else {
                                        value
                                    }
                                })
                                .collect();
                            let params = match kind {
                                Form::Font => font_parameters(&values),
                                Form::Paragraph => paragraph_parameters(&values),
                            };
                            match params {
                                Err(e) => error2.update(cx, |s, cx| {
                                    *s = e;
                                    cx.notify();
                                }),
                                Ok(args) => {
                                    if !args["props"].as_object().is_some_and(|m| m.is_empty()) {
                                        v.run_format(command, &args, w, cx);
                                    }
                                    w.close_dialog(cx);
                                    w.focus(&v.focus, cx);
                                }
                            }
                        });
                    }),
            )
            .child(
                Button::new("cancel-format")
                    .ghost()
                    .label("取消")
                    .on_click(move |_, w, cx| {
                        w.close_dialog(cx);
                        let _ = cancel_owner.update(cx, |v, cx| w.focus(&v.focus, cx));
                    }),
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::sync::atomic::AtomicBool;
    #[gpui::test]
    fn native_shortcuts_and_font_picker_edit_same_session(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, cx| {
            let mut r = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            r.edit_command(
                "document.setText",
                &json!({"text":"中文报告\nEnglish text"}),
            )
            .unwrap();
            r.edit_command("select.all", &json!({})).unwrap();
            r.edit_command("format.font", &json!({"name":"Missing R4 Test Family"}))
                .unwrap();
            let doc = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(doc, cx);
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        view.read_with(cx, |v, cx| {
            assert_eq!(
                v.font_picker.read(cx).selected_value().map(|v| v.as_ref()),
                Some("Missing R4 Test Family")
            )
        });
        cx.simulate_keystrokes("ctrl-b ctrl-i ctrl-e");
        view.read_with(cx, |v, _| {
            let f = v.canvas.reader.as_ref().unwrap().formatting();
            assert!(f.active("bold") && f.active("italic"));
            assert_eq!(f.paragraph["align"], "center");
        });
        let picker = view.read_with(cx, |v, _| v.font_picker.clone());
        picker.update(cx, |_, cx| {
            cx.emit(gpui::component::select::SelectEvent::Confirm(Some(
                SharedString::from("Source Sans 3"),
            )))
        });
        view.read_with(cx, |v, _| {
            assert_eq!(
                v.canvas.reader.as_ref().unwrap().formatting().character["font"],
                "Source Sans 3"
            )
        });
        view.update(cx, |v, _| {
            let r = v.canvas.reader.as_mut().unwrap();
            r.edit_command("caret.docEnd", &json!({})).unwrap();
            r.mark_text(None, "pinyin", None).unwrap();
            assert!(!v.format_ready("format.bold"));
        });
    }
}
