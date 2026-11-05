//! Editing affordances shared by the native ribbon, dialogs and keyboard routes.
use super::*;
use gpui_kit::component::{Icon, Sizable, button::ButtonVariants};
use joydeck_engine::Command;
use joydeck_model::{
    edit,
    text::{Align, Bullet, RunProps, TextBody},
};
use joyoffice_ui::IconName as I;

impl Viewer {
    pub(super) fn format_runs(
        &mut self,
        f: impl Fn(&mut RunProps),
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.composing() {
            return;
        }
        if let Some(i) = &mut self.inline {
            if i.draft.selection.is_empty() {
                let text = i.draft.body.text();
                let pos = joydeck_engine::text::pos(&text, i.draft.selection.start);
                let mut props = i
                    .draft
                    .typing_props
                    .clone()
                    .unwrap_or_else(|| edit::props_at(&i.draft.body, pos));
                f(&mut props);
                i.draft.typing_props = Some(props);
                self.focus.focus(w, cx);
                cx.notify();
                return;
            }
            let text = i.draft.body.text();
            let a = joydeck_engine::text::pos(&text, i.draft.selection.start);
            let b = joydeck_engine::text::pos(&text, i.draft.selection.end);
            edit::format(&mut i.draft.body, a, b, &f);
            self.publish_inline(w, cx);
        } else {
            self.format_bodies(|body| edit::format_all(body, &f), w, cx);
        }
        self.focus.focus(w, cx);
    }
    pub(super) fn format_bodies(
        &mut self,
        f: impl Fn(&mut TextBody),
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading || self.composing() {
            return;
        }
        if let Some(i) = &mut self.inline {
            f(&mut i.draft.body);
            self.publish_inline(w, cx);
            return;
        }
        let Some(e) = &self.editor else { return };
        let mut edits = vec![];
        for target in self.format_targets() {
            let mut body = match joydeck_engine::text_body(e.session.model(), self.page, &target) {
                Ok(body) => body,
                Err(error) => {
                    self.message = error;
                    cx.notify();
                    return;
                }
            };
            if body.paragraphs.is_empty() {
                body.paragraphs.push(Default::default());
            }
            f(&mut body);
            edits.push(joydeck_engine::TextEdit { target, body });
        }
        if !edits.is_empty() {
            self.dispatch(Command::SetTexts { edits }, w, cx);
        }
    }
    pub(super) fn format_paras(
        &mut self,
        f: impl Fn(&mut joydeck_model::text::ParaProps),
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.composing() {
            return;
        }
        if let Some(i) = &mut self.inline {
            let Ok(range) =
                joydeck_engine::formatting::paragraphs(&i.draft.body, i.draft.selection.clone())
            else {
                return;
            };
            for p in i
                .draft
                .body
                .paragraphs
                .iter_mut()
                .take(range.end)
                .skip(range.start)
            {
                f(&mut p.props);
            }
            self.publish_inline(w, cx);
        } else {
            self.format_bodies(
                |body| {
                    for p in &mut body.paragraphs {
                        f(&mut p.props);
                    }
                },
                w,
                cx,
            );
        }
        self.focus.focus(w, cx);
    }
    pub(super) fn run_props(&self) -> RunProps {
        self.formatting().run
    }
    pub(super) fn font_group(&self, cx: &mut Context<Self>) -> Div {
        let state = self.formatting();
        let disabled = state.is_empty() || self.loading || self.composing();
        let props = state.run;
        let p = chrome::palette(self.dark);
        let mut formats = div().flex().items_center().gap(px(1.));
        for (id, label, icon, on) in [
            ("font-bold", "加粗", I::Bold, props.bold == Some(true)),
            ("font-italic", "倾斜", I::Italic, props.italic == Some(true)),
            (
                "font-underline",
                "下划线",
                I::Underline,
                props.underline.as_deref().is_some_and(|u| u != "none"),
            ),
        ] {
            let mixed = state.mixed.contains(match id {
                "font-bold" => "bold",
                "font-italic" => "italic",
                _ => "underline",
            });
            let accessible = if mixed {
                format!("{label}（混合格式）")
            } else {
                label.into()
            };
            formats = formats.child(
                Button::new(id)
                    .ghost()
                    .xsmall()
                    .icon(icon)
                    .h(px(25.))
                    .w(px(26.))
                    .rounded(px(3.))
                    .accessibility_label(accessible.clone())
                    .tooltip(accessible)
                    .disabled(disabled)
                    .when(on, |b| b.bg(rgb(p.selected)))
                    .when(mixed, |b| b.border_1().border_color(rgb(p.accent)))
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.format_runs(
                            |p| match id {
                                "font-bold" => p.bold = Some(!on),
                                "font-italic" => p.italic = Some(!on),
                                _ => p.underline = Some(if on { "none" } else { "sng" }.into()),
                            },
                            w,
                            cx,
                        )
                    })),
            );
        }
        formats = formats.child(self.font_color_picker(disabled, cx)).child(
            Button::new("clear-format")
                .ghost()
                .xsmall()
                .icon(I::RemoveFormatting)
                .h(px(25.))
                .accessibility_label("清除字符格式")
                .tooltip("清除字符格式")
                .disabled(disabled)
                .on_click(
                    cx.listener(|s, _, w, cx| s.format_runs(|p| *p = RunProps::default(), w, cx)),
                ),
        );
        let family = props.font.filter(|s| !s.starts_with('+')).unwrap_or(
            if state.mixed.contains("font") || state.mixed.contains("font_ea") {
                "混合字体"
            } else {
                "主题字体"
            }
            .into(),
        );
        let content = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        gpui_kit::component::select::Select::new(&self.font_picker)
                            .xsmall()
                            .accessibility_label("字体名称")
                            .search_placeholder("搜索已安装字体")
                            .placeholder(family)
                            .menu_width(px(260.))
                            .menu_max_h(px(360.))
                            .w(px(138.))
                            .h(px(23.))
                            .disabled(disabled),
                    )
                    .child(self.font_size_picker(disabled, cx))
                    .child(
                        Button::new("grow-font")
                            .ghost()
                            .xsmall()
                            .label("A+")
                            .h(px(23.))
                            .w(px(28.))
                            .accessibility_label("增大字号")
                            .tooltip("增大字号 (Ctrl+Shift+.)")
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.step_font_size(true, w, cx))),
                    )
                    .child(
                        Button::new("shrink-font")
                            .ghost()
                            .xsmall()
                            .label("A−")
                            .h(px(23.))
                            .w(px(28.))
                            .accessibility_label("减小字号")
                            .tooltip("减小字号 (Ctrl+Shift+,)")
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.step_font_size(false, w, cx))),
                    ),
            )
            .child(
                formats.child(
                    Button::new("font-more")
                        .ghost()
                        .xsmall()
                        .label("字体…")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.font_dialog(w, cx))),
                ),
            );
        chrome::group("字体", content)
    }
    pub(super) fn paragraph_group(&self, cx: &mut Context<Self>) -> Div {
        let state = self.formatting();
        let disabled = state.is_empty() || self.loading || self.composing();
        let palette = chrome::palette(self.dark);
        let mut first = div().flex().gap(px(1.));
        for (id, label, numbered) in [("bullets", "项目符号", false), ("numbering", "编号", true)]
        {
            let active = if numbered {
                matches!(state.para.bullet, Some(Bullet::AutoNum { .. }))
            } else {
                matches!(state.para.bullet, Some(Bullet::Char { .. }))
            };
            first = first
                .child(
                    Button::new(id)
                        .ghost()
                        .xsmall()
                        .icon(if numbered { I::ListOrdered } else { I::List })
                        .h(px(25.))
                        .w(px(28.))
                        .accessibility_label(label)
                        .tooltip(label)
                        .disabled(disabled)
                        .when(active, |b| b.bg(rgb(palette.selected)))
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.format_paras(
                                |p| {
                                    p.bullet = Some(if active {
                                        Bullet::None
                                    } else if numbered {
                                        Bullet::AutoNum {
                                            scheme: "arabicPeriod".into(),
                                            start_at: 1,
                                        }
                                    } else {
                                        Bullet::Char { char: "•".into() }
                                    });
                                },
                                w,
                                cx,
                            )
                        })),
                )
                .child(self.list_picker(numbered, disabled, cx));
        }
        first = first
            .child(
                Button::new("indent-less")
                    .ghost()
                    .xsmall()
                    .icon(I::ListIndentDecrease)
                    .h(px(25.))
                    .accessibility_label("减少缩进")
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.format_paras(
                            |p| p.margin_left = Some((p.margin_left.unwrap_or(0.) - 18.).max(0.)),
                            w,
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("indent-more")
                    .ghost()
                    .xsmall()
                    .icon(I::ListIndentIncrease)
                    .h(px(25.))
                    .accessibility_label("增加缩进")
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.format_paras(
                            |p| p.margin_left = Some((p.margin_left.unwrap_or(0.) + 18.).min(720.)),
                            w,
                            cx,
                        )
                    })),
            );
        let mut second = div().flex().gap(px(1.));
        for (id, label, icon, align) in [
            ("align-left", "左对齐", I::TextAlignStart, Align::Left),
            ("align-center", "居中", I::TextAlignCenter, Align::Center),
            ("align-right", "右对齐", I::TextAlignEnd, Align::Right),
            (
                "align-justify",
                "两端对齐",
                I::TextAlignJustify,
                Align::Justify,
            ),
        ] {
            second = second.child(
                Button::new(id)
                    .ghost()
                    .xsmall()
                    .icon(icon)
                    .h(px(25.))
                    .w(px(28.))
                    .accessibility_label(label)
                    .tooltip(label)
                    .disabled(disabled)
                    .when(state.para.align == Some(align), |b| {
                        b.bg(rgb(palette.selected))
                    })
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.format_paras(|p| p.align = Some(align), w, cx)
                    })),
            );
        }
        second = second.child(self.line_spacing_picker(disabled, cx)).child(
            Button::new("paragraph-dialog")
                .ghost()
                .xsmall()
                .label("段落…")
                .h(px(25.))
                .accessibility_label("段落设置")
                .disabled(disabled)
                .on_click(cx.listener(|s, _, w, cx| s.paragraph_dialog(w, cx))),
        );
        chrome::group(
            "段落",
            div().flex().flex_col().gap_1().child(first).child(second),
        )
    }
    pub(super) fn fields_dialog(
        &mut self,
        title: &'static str,
        fields: Vec<(&'static str, String)>,
        w: &mut Window,
        cx: &mut Context<Self>,
        apply: impl Fn(&mut Self, Vec<String>, &mut Window, &mut Context<Self>) -> Result<(), String>
        + 'static,
    ) {
        if self.composing() {
            return;
        }
        let inputs: Vec<_> = fields
            .into_iter()
            .map(|(label, value)| {
                let input = cx.new(|cx| {
                    let mut i = InputState::new(w, cx);
                    i.set_value(value, w, cx);
                    i
                });
                (label, input)
            })
            .collect();
        let view = cx.entity().downgrade();
        let generation = self.generation;
        let revision = self.editor.as_ref().map(|e| e.session.revision());
        let error = cx.new(|_| String::new());
        let apply = std::rc::Rc::new(apply);
        w.open_dialog(cx, move |d, _, cx| {
            let mut content = div().flex().flex_col().gap_3();
            for (label, input) in &inputs {
                content = content
                    .child(div().text_sm().child(*label))
                    .child(Input::new(input).aria_label(*label));
            }
            let inputs = inputs.clone();
            let view = view.clone();
            let error2 = error.clone();
            let apply = apply.clone();
            d.title(title)
                .child(content)
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0xc42b1c))
                        .child(error.read(cx).clone()),
                )
                .child(
                    Button::new("apply-fields")
                        .label("确定")
                        .on_click(move |_, w, cx| {
                            let _ = view.update(cx, |s, cx| {
                                if s.generation != generation
                                    || s.editor.as_ref().map(|e| e.session.revision()) != revision
                                {
                                    error2.update(cx, |e, cx| {
                                        *e = "文稿已变化，请重新打开此对话框".into();
                                        cx.notify();
                                    });
                                    return;
                                }
                                if inputs.iter().any(|(_, i)| {
                                    i.update(cx, |s, cx| s.marked_text_range(w, cx).is_some())
                                }) {
                                    return;
                                }
                                let values = inputs
                                    .iter()
                                    .map(|(_, i)| i.read(cx).value().to_string())
                                    .collect();
                                match apply(s, values, w, cx) {
                                    Ok(()) => {
                                        w.close_dialog(cx);
                                        s.focus.focus(w, cx);
                                    }
                                    Err(e) => error2.update(cx, |value, cx| {
                                        *value = e;
                                        cx.notify();
                                    }),
                                }
                            });
                        }),
                )
        });
    }
    pub(super) fn font_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let state = self.formatting();
        let props = state.run;
        let mut color = match &props.fill {
            Some(joydeck_model::Fill::Solid { color }) => self
                .editor
                .as_ref()
                .and_then(|e| {
                    joydeck_model::resolve::Ctx::for_slide(
                        e.session.model(),
                        &e.session.model().slides[self.page],
                    )
                })
                .map(|ctx| ctx.color(color, None))
                .map(|c| format!("{:02X}{:02X}{:02X}", c.r, c.g, c.b))
                .unwrap_or("242424".into()),
            _ => "242424".into(),
        };
        if state.mixed.contains("fill") {
            color.clear();
        }
        let font = props.font.unwrap_or_default();
        let size = props.size.map(|n| n.to_string()).unwrap_or_default();
        let original = [font.clone(), size.clone(), color.clone()];
        self.fields_dialog(
            "字体",
            vec![
                ("字体名称（混合时留空）", font),
                ("字号（磅，混合时留空）", size),
                ("颜色（RGB）", color),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                let size = if v[1] == original[1] || v[1].trim().is_empty() {
                    None
                } else {
                    let n: f64 = v[1].trim().parse().map_err(|_| "请输入有效字号")?;
                    if !n.is_finite() || !(1. ..=400.).contains(&n) {
                        return Err("字号须在 1–400 磅之间".into());
                    }
                    Some(n)
                };
                let color = if v[2] == original[2] || v[2].trim().is_empty() {
                    None
                } else {
                    Some(format_ui::parse_rgb(&v[2])?)
                };
                let font = (v[0] != original[0] && !v[0].trim().is_empty())
                    .then(|| v[0].trim().to_owned());
                s.format_runs(
                    |p| {
                        if let Some(font) = &font {
                            p.font = Some(font.clone());
                            p.font_ea = p.font.clone();
                        }
                        if let Some(size) = size {
                            p.size = Some(size);
                        }
                        if let Some(color) = &color {
                            p.fill = Some(joydeck_model::Fill::solid(color.clone()));
                        }
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
    pub(super) fn find_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_dialog(
            "查找文字",
            vec![("查找内容", String::new())],
            w,
            cx,
            |s, v, w, cx| {
                let query = v[0].trim().to_lowercase();
                if query.is_empty() {
                    return Err("请输入查找内容".into());
                }
                let Some(e) = &s.editor else {
                    return Err("请先打开演示文稿".into());
                };
                let mut found = None;
                for (page, slide) in e.session.model().slides.iter().enumerate() {
                    for shape in &slide.shapes {
                        let text = shape.text.as_ref().map(|t| t.text()).unwrap_or_default();
                        if text.to_lowercase().contains(&query) {
                            found = Some((page, shape.id));
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                let Some((page, id)) = found else {
                    return Err("未找到匹配的文字".into());
                };
                s.select(page, cx);
                if let Some(e) = &mut s.editor {
                    let _ = e.session.select(page, vec![id]);
                }
                s.focus.focus(w, cx);
                cx.notify();
                Ok(())
            },
        );
    }
    pub(super) fn insert_textbox(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.dispatch(
            Command::InsertText {
                x: 100.,
                y: 100.,
                text: String::new(),
            },
            w,
            cx,
        );
        self.begin_selected_text(w, cx);
    }
    pub(super) fn insert_table_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_dialog(
            "插入表格",
            vec![("列数", "3".into()), ("行数", "3".into())],
            w,
            cx,
            |s, v, w, cx| {
                let cols: usize = v[0].parse().map_err(|_| "列数须为整数")?;
                let rows: usize = v[1].parse().map_err(|_| "行数须为整数")?;
                if !(1..=32).contains(&cols) || !(1..=64).contains(&rows) {
                    return Err("列数范围 1–32，行数范围 1–64".into());
                }
                s.dispatch(
                    Command::InsertTable {
                        x: 100.,
                        y: 120.,
                        rows,
                        cols,
                        width: 600.,
                        row_height: 32.,
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
    pub(super) fn geometry_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &self.editor else { return };
        let Some(id) = e.session.selection().shapes.first() else {
            return;
        };
        let slide = &e.session.model().slides[self.page];
        let Some(shape) = slide.shape(*id) else {
            return;
        };
        let Some(ctx) = joydeck_model::resolve::Ctx::for_slide(e.session.model(), slide) else {
            return;
        };
        let mut x = joydeck_model::resolve::xfrm(&ctx, shape);
        if e.session.selection().shapes.len() > 1
            && let Some(b) = self.selected_bounds()
        {
            x.x = b.x0;
            x.y = b.y0;
            x.w = b.width();
            x.h = b.height();
            x.rot = 0.;
        }
        self.fields_dialog(
            "大小和位置",
            vec![
                ("水平位置（磅）", x.x.to_string()),
                ("垂直位置（磅）", x.y.to_string()),
                ("宽度（磅）", x.w.to_string()),
                ("高度（磅）", x.h.to_string()),
                ("旋转角度", x.rot.to_string()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                let v: Vec<f64> = v
                    .iter()
                    .map(|s| s.parse::<f64>().map_err(|_| "请输入数值".to_string()))
                    .collect::<Result<_, _>>()?;
                if v.iter().any(|x| !x.is_finite() || x.abs() > 100000.) || v[2] <= 0. || v[3] <= 0.
                {
                    return Err("尺寸须大于零，数值须为有限值".into());
                }
                s.dispatch(
                    Command::Workspace {
                        action: joydeck_engine::WorkspaceAction::TransformSelection {
                            dx: v[0] - x.x + (v[2] - x.w) / 2.,
                            dy: v[1] - x.y + (v[3] - x.h) / 2.,
                            scale_x: v[2] / x.w.max(1.),
                            scale_y: v[3] / x.h.max(1.),
                            rotation: v[4] - x.rot,
                        },
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
    pub(super) fn layout_gallery(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &self.editor else { return };
        let layouts: Vec<_> = e
            .session
            .model()
            .masters
            .iter()
            .flat_map(|m| m.layouts.iter())
            .map(|l| (l.id, l.name.clone()))
            .collect();
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let mut list = div().flex().flex_wrap().gap_2();
            for (id, name) in &layouts {
                let id = *id;
                let view = view.clone();
                list = list.child(
                    Button::new(SharedString::from(format!("layout-{}", id.0)))
                        .label(name.clone())
                        .icon(I::PanelsTopLeft)
                        .w(px(180.))
                        .h(px(54.))
                        .on_click(move |_, w, cx| {
                            let _ = view.update(cx, |s, cx| {
                                s.dispatch(
                                    Command::Content {
                                        action: joydeck_engine::ContentAction::Layout { id },
                                    },
                                    w,
                                    cx,
                                );
                                w.close_dialog(cx);
                            });
                        }),
                );
            }
            d.title("幻灯片版式").child(list)
        });
    }
    pub(super) fn shape_gallery(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        w.open_dialog(cx, move |d, _, _| {
            let mut list = div().flex().gap_3();
            for (preset, label, icon) in [
                ("rect", "矩形", I::Square),
                ("roundRect", "圆角矩形", I::RectangleHorizontal),
                ("ellipse", "椭圆", I::Circle),
                ("triangle", "三角形", I::Triangle),
                ("chevron", "燕尾形", I::ChevronRight),
            ] {
                let view = view.clone();
                list = list.child(
                    Button::new(preset)
                        .label(label)
                        .icon(Icon::new(icon).size(px(30.)))
                        .h(px(70.))
                        .on_click(move |_, w, cx| {
                            let _ = view.update(cx, |s, cx| {
                                s.dispatch(
                                    Command::InsertShape {
                                        preset: preset.into(),
                                        x: 100.,
                                        y: 140.,
                                        width: 200.,
                                        height: 120.,
                                    },
                                    w,
                                    cx,
                                );
                                w.close_dialog(cx);
                            });
                        }),
                );
            }
            d.title("形状").child(list)
        });
    }
}
