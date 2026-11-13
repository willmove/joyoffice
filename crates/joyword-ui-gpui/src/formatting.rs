//! JoyOffice Kit pickers/forms adapted to Wordcraft command parameters and selection state.
use super::*;
use gpui::EntityInputHandler;
use gpui::component::{
    Disableable, Sizable, WindowExt,
    input::{Input, InputState},
    popover::Popover,
    select::{SearchableVec, SelectState},
};
use joyword_shell::formatting::{
    FormatState, font_families, font_parameters, paragraph_parameters,
};
use serde_json::Value;

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
            && !self.file_page_visible()
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
    pub(super) fn format_state(&self) -> FormatState {
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
    pub(super) fn choices(
        &self,
        id: &'static str,
        label: String,
        command: &'static str,
        choices: Vec<(String, Value)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let trigger = Button::new((id, 0usize))
            .ghost()
            .xsmall()
            .h(px(27.))
            .label(label.clone())
            .accessibility_label(label)
            .disabled(!self.format_ready(command));
        self.format_choices(id, trigger, command, choices, cx)
    }

    pub(super) fn format_choices(
        &self,
        id: &'static str,
        trigger: Button,
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
        Popover::new(id).trigger(trigger).content(move |_, _, cx| {
            let popup = cx.entity().downgrade();
            let mut list = div()
                .flex()
                .flex_col()
                .w(px(if command == "format.size" { 56. } else { 190. }))
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
        self.group_frame(
            body,
            div()
                .text_size(px(11.))
                .text_color(rgb(self.palette().muted))
                .child(title.to_owned()),
        )
    }

    pub(super) fn group_frame(
        &self,
        body: impl IntoElement,
        footer: impl IntoElement,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .justify_between()
            .h(px(82.))
            .px_2()
            .border_r_1()
            .border_color(rgb(self.palette().separator))
            .child(body)
            .child(footer)
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
    use serde_json::json;
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
