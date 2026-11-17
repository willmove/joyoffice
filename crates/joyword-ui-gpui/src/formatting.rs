//! JoyOffice Kit pickers/forms adapted to Wordcraft command parameters and selection state.
use super::*;
use gpui::component::{
    Sizable,
    popover::Popover,
    select::{SearchableVec, SelectState},
};
use joyword_shell::formatting::{
    FormatState, font_families, font_parameters, paragraph_parameters,
};
use serde_json::Value;

pub(super) type FontSelect = gpui::Entity<SelectState<SearchableVec<forms::Choice>>>;
#[derive(Clone, Copy)]
pub(super) enum Form {
    Font,
    Paragraph,
}

pub(super) fn initial_font_picker(w: &mut Window, cx: &mut Context<JoyWord>) -> FontSelect {
    let names: Vec<_> = font_families()
        .into_iter()
        .map(|name| forms::Choice {
            title: name.clone().into(),
            value: name,
        })
        .collect();
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
        let family = state.character["font"].as_str().map(str::to_owned);
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
            let mut names = font_families();
            if let Some(name) = &family
                && !names.contains(name)
            {
                names.insert(0, name.clone());
            }
            self.font_picker.update(cx, |s, cx| {
                s.set_items(
                    SearchableVec::new(
                        names
                            .into_iter()
                            .map(|name| forms::Choice {
                                title: name.clone().into(),
                                value: name,
                            })
                            .collect::<Vec<_>>(),
                    ),
                    w,
                    cx,
                );
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
    pub(super) fn format_choices(
        &self,
        id: &'static str,
        trigger: Button,
        command: &'static str,
        choices: Vec<(String, Value)>,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let owner = cx.entity().downgrade();
        let target = self.ribbon_target();
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
                let target = target.clone();
                list = list.child(
                    Button::new((id, i + 2))
                        .ghost()
                        .xsmall()
                        .label(label.clone())
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = owner.update(cx, |v, cx| {
                                if v.ribbon_target() == target {
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
            .h(px(94.))
            .px_2()
            .border_r_1()
            .border_color(rgb(self.palette().separator))
            .child(body)
            .child(footer)
    }

    pub(super) fn format_dialog(&mut self, kind: Form, w: &mut Window, cx: &mut Context<Self>) {
        use forms::{Field, show};
        let Some(reader) = self.canvas.reader.as_ref() else {
            return;
        };
        let state = reader.formatting();
        let unit = reader.engine().units();
        match kind {
            Form::Font => {
                let flag = |key| {
                    if state.mixed(key) {
                        String::new()
                    } else {
                        state.active(key).to_string()
                    }
                };
                self.fields_form(
                    "字体设置",
                    "format.set",
                    vec![
                        Field::font("西文字体", show(&state.character["font"])),
                        Field::font("中文字体", show(&state.character["fontEastAsia"])),
                        Field::text("字号（磅）", show(&state.character["size"])),
                        Field::color("文字颜色", show(&state.character["color"])),
                        Field::check("加粗", flag("bold"), "true", "false"),
                        Field::check("倾斜", flag("italic"), "true", "false"),
                        Field::choice(
                            "下划线",
                            show(&state.character["underlineStyle"]),
                            &[
                                ("none", "无"),
                                ("single", "单下划线"),
                                ("double", "双下划线"),
                                ("dotted", "点线"),
                                ("dash", "虚线"),
                                ("wave", "波浪线"),
                            ],
                        ),
                        Field::check("删除线", flag("strike"), "true", "false"),
                        Field::check("小型大写字母", flag("smallCaps"), "true", "false"),
                        Field::check("全部大写字母", flag("caps"), "true", "false"),
                        Field::check("隐藏文字", flag("hidden"), "true", "false"),
                        Field::choice(
                            "字符位置",
                            show(&state.character["vertAlign"]),
                            &[
                                ("baseline", "正常"),
                                ("superscript", "上标"),
                                ("subscript", "下标"),
                            ],
                        ),
                        Field::text(
                            "字符间距（磅，负值为紧缩）",
                            show(&state.character["spacing"]),
                        ),
                        Field::text("字符缩放（%）", show(&state.character["scale"])),
                        Field::text(
                            "位置偏移（磅，负值为降低）",
                            show(&state.character["position"]),
                        ),
                        Field::text(
                            "字距调整起始字号（磅，0为关闭）",
                            show(&state.character["kern"]),
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, field)| field.in_tab(if i < 12 { "字体" } else { "高级" }))
                    .collect::<Vec<_>>(),
                    true,
                    |values| {
                        let mut args = font_parameters(&values[..4])?;
                        for (i, key) in [
                            (4, "bold"),
                            (5, "italic"),
                            (7, "strike"),
                            (8, "smallCaps"),
                            (9, "caps"),
                            (10, "hidden"),
                        ] {
                            if !values[i].is_empty() {
                                args["props"][key] = serde_json::json!(values[i] == "true");
                            }
                        }
                        for (i, key) in [(6, "underline"), (11, "vertAlign")] {
                            if !values[i].is_empty() {
                                args["props"][key] = serde_json::json!(values[i]);
                            }
                        }
                        for (i, key, min, max) in [
                            (12, "spacing", -100., 100.),
                            (13, "scale", 1., 600.),
                            (14, "position", -1584., 1584.),
                            (15, "kern", 0., 400.),
                        ] {
                            if let Some(n) =
                                joyword_shell::formatting::optional_number(&values[i], min, max)?
                            {
                                args["props"][key] = serde_json::json!(n);
                            }
                        }
                        Ok(args)
                    },
                    w,
                    cx,
                );
            }
            Form::Paragraph => {
                let line = state.paragraph["lineSpacing"].clone();
                self.fields_form(
                    "段落设置",
                    "para.set",
                    vec![
                        Field::length("左缩进", show(&state.paragraph["indentLeft"]), unit),
                        Field::length("右缩进", show(&state.paragraph["indentRight"]), unit),
                        Field::length(
                            "首行缩进（负值为悬挂）",
                            show(&state.paragraph["indentFirst"]),
                            unit,
                        ),
                        Field::length(
                            "段前间距",
                            show(&state.paragraph["spaceBefore"]),
                            joyword_shell::formatting::Unit::Points,
                        ),
                        Field::length(
                            "段后间距",
                            show(&state.paragraph["spaceAfter"]),
                            joyword_shell::formatting::Unit::Points,
                        ),
                        Field::choice(
                            "行距",
                            show(&line["rule"]),
                            &[
                                ("multiple", "多倍行距"),
                                ("exactly", "固定值（磅）"),
                                ("atLeast", "最小值（磅）"),
                            ],
                        ),
                        Field::text("行距设置值（倍数或磅）", show(&line["value"])),
                    ],
                    true,
                    move |values| {
                        let mut legacy = values[..5].to_vec();
                        let rule = if values[5].is_empty() {
                            line["rule"].as_str().unwrap_or("multiple")
                        } else {
                            values[5].as_str()
                        };
                        let amount = if values[6].is_empty() && !values[5].is_empty() {
                            show(&line["value"])
                        } else {
                            values[6].clone()
                        };
                        legacy.push(if amount.is_empty() {
                            String::new()
                        } else {
                            match rule {
                                "exactly" => format!("exact:{amount}"),
                                "atLeast" => format!("min:{amount}"),
                                _ => amount,
                            }
                        });
                        paragraph_parameters(&legacy)
                    },
                    w,
                    cx,
                );
            }
        }
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
                String::from("Source Sans 3"),
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
