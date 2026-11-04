//! Host JoyWord style task pane adapted to the single Wordcraft style sheet.
use super::*;
use forms::show;
use gpui::component::{Disableable, Sizable, input::Input};
use gpui::uniform_list;
use joyword_shell::{
    authoring::{StyleInfo, style_name},
    formatting::{font_parameters, paragraph_parameters},
};
use serde_json::{Value, json};
impl JoyWord {
    pub(super) fn toggle_styles(&mut self, cx: &mut Context<Self>) {
        self.styles_open = !self.styles_open;
        if self.styles_open {
            self.advanced.pane = None;
        }
        if self.styles_open && self.style_pick.is_none() {
            self.style_pick = self.canvas.reader.as_ref().map(|r| {
                r.formatting().character["style"]
                    .as_str()
                    .unwrap_or("Normal")
                    .to_string()
            });
        }
        cx.notify();
    }
    fn picked_style(&self) -> Option<StyleInfo> {
        self.style_catalog
            .iter()
            .find(|s| Some(&s.id) == self.style_pick.as_ref())
            .cloned()
    }
    pub(super) fn start_brush(&mut self, sticky: bool, w: &mut Window, cx: &mut Context<Self>) {
        let old = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.engine().painter.as_ref())
            .map(|p| p.2);
        if old.is_some() {
            let _ = self.try_edit("edit.formatPainter", &json!({}), w, cx);
        }
        if old != Some(sticky) {
            let _ = self.try_edit("edit.formatPainter", &json!({"sticky":sticky}), w, cx);
        }
    }
    pub(super) fn create_style_form(
        &mut self,
        character: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let based = self
            .picked_style()
            .filter(|s| (s.kind == "character") == character)
            .map(|s| s.id)
            .unwrap_or(if character {
                String::new()
            } else {
                "Normal".into()
            });
        self.fields_form(if character{"新建字符样式"}else{"新建段落样式"},"styles.create",vec![("样式名称",String::new()),("基于样式（名称或 ID，可空）",based)],false,move|v|Ok(json!({"name":style_name(&v[0])?,"kind":if character{"character"}else{"paragraph"},"basedOn":v[1].trim(),"fromSelection":true})),w,cx);
    }
    pub(super) fn style_metadata(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.picked_style() else {
            return;
        };
        let id = s.id.clone();
        let mut fields = vec![
            ("样式名称", s.name),
            (
                "基于样式（名称或 ID；- 表示无）",
                s.based_on.unwrap_or_else(|| "-".into()),
            ),
        ];
        if s.kind == "paragraph" {
            fields.push((
                "后续段落样式（名称或 ID）",
                s.next.unwrap_or_else(|| "Normal".into()),
            ));
        }
        self.fields_form(
            "修改样式关系",
            "styles.modify",
            fields,
            true,
            move |v| {
                let mut p = json!({"style":id});
                if !v[0].trim().is_empty() {
                    p["name"] = json!(style_name(&v[0])?);
                }
                if !v[1].trim().is_empty() {
                    p["basedOn"] = if v[1].trim() == "-" {
                        Value::Null
                    } else {
                        json!(v[1].trim())
                    };
                }
                if v.len() > 2 && !v[2].trim().is_empty() {
                    p["next"] = json!(v[2].trim());
                }
                Ok(p)
            },
            w,
            cx,
        );
    }
    pub(super) fn style_font(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.picked_style() else {
            return;
        };
        let id = s.id;
        let fields = vec![
            ("西文字体", show(&s.chr["font"])),
            ("中文字体", show(&s.chr["fontEa"])),
            ("字号（磅）", show(&s.chr["size"])),
            ("字色（RGB / auto）", style_color(&s.chr["color"])),
            (
                "粗体（是/否）",
                s.chr["bold"]
                    .as_bool()
                    .map(|b| if b { "是" } else { "否" }.into())
                    .unwrap_or_default(),
            ),
            (
                "斜体（是/否）",
                s.chr["italic"]
                    .as_bool()
                    .map(|b| if b { "是" } else { "否" }.into())
                    .unwrap_or_default(),
            ),
        ];
        self.fields_form(
            "样式字体",
            "styles.modify",
            fields,
            true,
            move |v| {
                let mut props = font_parameters(&v[..4])?["props"].clone();
                for (i, k) in [(4, "bold"), (5, "italic")] {
                    if !v[i].trim().is_empty() {
                        let b = match v[i].trim() {
                            "是" | "true" => true,
                            "否" | "false" => false,
                            _ => return Err("请输入 是 或 否".into()),
                        };
                        props[k] = json!(b);
                        props[if i == 4 { "boldCs" } else { "italicCs" }] = json!(b);
                    }
                }
                Ok(json!({"style":id,"chr":props}))
            },
            w,
            cx,
        );
    }
    pub(super) fn style_paragraph(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = self.picked_style().filter(|s| s.kind == "paragraph") else {
            return;
        };
        let id = s.id;
        let fields = vec![
            ("起始缩进（磅）", show(&s.para["indentLeft"])),
            ("结束缩进（磅）", show(&s.para["indentRight"])),
            ("首行缩进（磅）", show(&s.para["indentFirst"])),
            ("段前间距（磅）", show(&s.para["spaceBefore"])),
            ("段后间距（磅）", show(&s.para["spaceAfter"])),
            ("行距（倍数 / exact:磅 / min:磅）", String::new()),
        ];
        self.fields_form(
            "样式段落",
            "styles.modify",
            fields,
            true,
            move |v| Ok(json!({"style":id,"para":paragraph_parameters(v)?["props"]})),
            w,
            cx,
        );
    }
    pub(super) fn styles_panel(&self, cx: &mut Context<Self>) -> gpui::Div {
        let p = self.palette();
        let query = self.style_search.read(cx).value().to_lowercase();
        let list: Vec<_> = self
            .style_catalog
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.name.to_lowercase().contains(&query) || s.id.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        let count = list.len();
        let rows = uniform_list(
            "style-list",
            count,
            cx.processor(move |v, range: std::ops::Range<usize>, _, cx| {
                range
                    .filter_map(|i| {
                        let s = v.style_catalog.get(*list.get(i)?)?;
                        let id = s.id.clone();
                        let selected = Some(&id) == v.style_pick.as_ref();
                        let name = s.name.clone();
                        let kind = match s.kind.as_str() {
                            "paragraph" => "段落",
                            "character" => "字符",
                            "table" => "表格",
                            _ => "编号",
                        };
                        let current = v
                            .format_state
                            .as_ref()
                            .is_some_and(|a| a.paragraph["style"].as_str() == Some(&id));
                        Some(
                            joyoffice_ui::control(p, true, selected)
                                .id(SharedString::from(format!("style-row-{id}")))
                                .role(gpui::Role::Button)
                                .aria_label(format!("样式 {name} ({kind})"))
                                .aria_selected(selected)
                                .justify_start()
                                .h(px(48.))
                                .cursor_pointer()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(format!("{} {name}", if current { "✓" } else { "" }))
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(rgb(p.muted))
                                                .child(format!(
                                                    "{kind} · {}{}",
                                                    s.based_on.as_deref().unwrap_or("无基样式"),
                                                    if s.builtin { " · 内置" } else { "" }
                                                )),
                                        ),
                                )
                                .on_click(cx.listener(move |v, _, _, cx| {
                                    v.style_pick = Some(id.clone());
                                    cx.notify();
                                })),
                        )
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .flex_1()
        .min_h(px(80.));
        let picked = self.picked_style();
        let valid = picked
            .as_ref()
            .is_some_and(|s| matches!(s.kind.as_str(), "paragraph" | "character" | "table"));
        let mut panel = joyoffice_ui::card(p)
            .w(px(300.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .child(
                div().flex().justify_between().child("样式").child(
                    Button::new("close-styles")
                        .ghost()
                        .xsmall()
                        .label("关闭")
                        .on_click(cx.listener(|v, _, _, cx| v.toggle_styles(cx))),
                ),
            )
            .child(
                Input::new(&self.style_search)
                    .xsmall()
                    .aria_label("搜索样式"),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .child(format!("{count} 个样式 · 空字段保留原值")),
            )
            .child(rows);
        if let Some(s) = &picked {
            panel = panel.child(div().text_size(px(11.)).child(format!(
                "{} · 基于 {} · 后续 {} · {} {} 磅{}{}",
                s.name,
                s.based_on.as_deref().unwrap_or("无"),
                s.next.as_deref().unwrap_or("无"),
                show(&s.resolved["font"]),
                show(&s.resolved["size"]),
                if s.resolved["bold"].as_bool() == Some(true) {
                    " · 粗体"
                } else {
                    ""
                },
                if s.resolved["italic"].as_bool() == Some(true) {
                    " · 斜体"
                } else {
                    ""
                },
            )));
        }
        let apply = picked.clone();
        let update = picked.clone();
        let del = picked.clone();
        let visibility = picked.clone();
        let reset = picked.clone();
        panel = panel.child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(
                    Button::new("reset-style-format")
                        .ghost()
                        .xsmall()
                        .label("恢复继承格式")
                        .disabled(!valid || !self.format_ready("styles.modify"))
                        .on_click(cx.listener(move |v, _, w, cx| {
                            if let Some(s) = &reset {
                                let _ = v.try_edit(
                                    "styles.modify",
                                    &json!({"style":s.id,"reset":true}),
                                    w,
                                    cx,
                                );
                            }
                        })),
                )
                .child(
                    Button::new("apply-style")
                        .xsmall()
                        .label("应用样式")
                        .disabled(
                            !valid
                                || !self.format_ready(
                                    if picked.as_ref().is_some_and(|s| s.kind == "table") {
                                        "table.style"
                                    } else {
                                        "para.style"
                                    },
                                ),
                        )
                        .on_click(cx.listener(move |v, _, w, cx| {
                            if let Some(s) = &apply {
                                let cmd = if s.kind == "table" {
                                    "table.style"
                                } else {
                                    "para.style"
                                };
                                let _ = v.try_edit(cmd, &json!({"style":s.id}), w, cx);
                            }
                        })),
                )
                .child(
                    Button::new("new-para-style")
                        .ghost()
                        .xsmall()
                        .label("新建段落")
                        .disabled(!self.format_ready("styles.create"))
                        .on_click(cx.listener(|v, _, w, cx| v.create_style_form(false, w, cx))),
                )
                .child(
                    Button::new("new-char-style")
                        .ghost()
                        .xsmall()
                        .label("新建字符")
                        .disabled(!self.format_ready("styles.create"))
                        .on_click(cx.listener(|v, _, w, cx| v.create_style_form(true, w, cx))),
                )
                .child(
                    Button::new("edit-style-meta")
                        .ghost()
                        .xsmall()
                        .label("名称/继承")
                        .disabled(!valid || !self.format_ready("styles.modify"))
                        .on_click(cx.listener(|v, _, w, cx| v.style_metadata(w, cx))),
                )
                .child(
                    Button::new("edit-style-font")
                        .ghost()
                        .xsmall()
                        .label("字体…")
                        .disabled(!valid || !self.format_ready("styles.modify"))
                        .on_click(cx.listener(|v, _, w, cx| v.style_font(w, cx))),
                )
                .child(
                    Button::new("edit-style-para")
                        .ghost()
                        .xsmall()
                        .label("段落…")
                        .disabled(
                            picked.as_ref().is_none_or(|s| s.kind != "paragraph")
                                || !self.format_ready("styles.modify"),
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.style_paragraph(w, cx))),
                )
                .child(
                    Button::new("update-style")
                        .ghost()
                        .xsmall()
                        .label("按选区更新")
                        .disabled(
                            picked.as_ref().is_none_or(|s| {
                                !matches!(s.kind.as_str(), "paragraph" | "character")
                            }) || !self.format_ready("styles.updateToMatch"),
                        )
                        .on_click(cx.listener(move |v, _, w, cx| {
                            if let Some(s) = &update {
                                let _ = v.try_edit(
                                    "styles.updateToMatch",
                                    &json!({"style":s.id}),
                                    w,
                                    cx,
                                );
                            }
                        })),
                )
                .child(
                    Button::new("style-gallery")
                        .ghost()
                        .xsmall()
                        .label("显示/隐藏到功能区")
                        .disabled(!valid || !self.format_ready("styles.setVisibility"))
                        .on_click(cx.listener(move |v, _, w, cx| {
                            if let Some(s) = &visibility {
                                let _ = v.try_edit(
                                    "styles.setVisibility",
                                    &json!({"style":s.id,"gallery":!s.quick,"hidden":false}),
                                    w,
                                    cx,
                                );
                            }
                        })),
                )
                .child(
                    Button::new("delete-style")
                        .ghost()
                        .xsmall()
                        .label("删除样式")
                        .disabled(
                            picked.as_ref().is_none_or(|s| s.builtin)
                                || !valid
                                || !self.format_ready("styles.delete"),
                        )
                        .on_click(cx.listener(move |v, _, w, cx| {
                            if let Some(s) = &del
                                && v.try_edit("styles.delete", &json!({"style":s.id}), w, cx)
                                    .is_ok()
                            {
                                v.style_pick = None;
                            }
                        })),
                ),
        );
        panel
    }
}

fn style_color(value: &Value) -> String {
    if value.as_str() == Some("Auto") {
        return "auto".into();
    }
    if let Some(rgb) = value.get("Rgb").and_then(Value::as_array) {
        return rgb
            .iter()
            .map(|v| format!("{:02X}", v.as_u64().unwrap_or(0)))
            .collect();
    }
    show(value)
}
