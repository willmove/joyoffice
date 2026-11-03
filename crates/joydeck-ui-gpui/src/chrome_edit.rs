//! Editing affordances shared by the native ribbon, dialogs and keyboard routes.
use super::*;
use gpui_kit::component::{Icon, Sizable, button::ButtonVariants};
use joydeck_engine::{Command, TextTarget};
use joydeck_model::{
    edit,
    text::{Align, Bullet, RunProps, TextBody},
};
use joyoffice_ui::IconName as I;

impl Viewer {
    fn text_target(&self) -> Option<TextTarget> {
        if let Some((shape, row, col)) = self.cell {
            return Some(TextTarget::Cell { shape, row, col });
        }
        let e = self.editor.as_ref()?;
        let shape = *e.session.selection().shapes.first()?;
        Some(TextTarget::Shape { shape })
    }
    pub(super) fn selected_body(&self) -> Option<(TextTarget, TextBody)> {
        let target = self.text_target()?;
        let e = self.editor.as_ref()?;
        let mut body = joydeck_engine::text_body(e.session.model(), self.page, &target).ok()?;
        if body.paragraphs.is_empty() {
            body.paragraphs.push(joydeck_model::Paragraph::default());
        }
        Some((target, body))
    }
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
            let text = i.draft.body.text();
            let a = joydeck_engine::text::pos(&text, i.draft.selection.start);
            let b = joydeck_engine::text::pos(&text, i.draft.selection.end);
            edit::format(&mut i.draft.body, a, b, &f);
            self.publish_inline(w, cx);
        } else if let Some((target, mut body)) = self.selected_body() {
            edit::format_all(&mut body, &f);
            self.dispatch(
                Command::SetText {
                    target,
                    body: Box::new(body),
                },
                w,
                cx,
            );
        }
    }
    fn format_paras(
        &mut self,
        f: impl Fn(&mut joydeck_model::text::ParaProps),
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.composing() {
            return;
        }
        if let Some(i) = &mut self.inline {
            let text = i.draft.body.text();
            let a = joydeck_engine::text::pos(&text, i.draft.selection.start).0;
            let b = joydeck_engine::text::pos(&text, i.draft.selection.end).0;
            for p in i.draft.body.paragraphs.iter_mut().take(b + 1).skip(a) {
                f(&mut p.props);
            }
            self.publish_inline(w, cx);
        } else if let Some((target, mut body)) = self.selected_body() {
            for p in &mut body.paragraphs {
                f(&mut p.props);
            }
            self.dispatch(
                Command::SetText {
                    target,
                    body: Box::new(body),
                },
                w,
                cx,
            );
        }
    }
    pub(super) fn run_props(&self) -> RunProps {
        let (body, at) = if let Some(i) = &self.inline {
            (
                i.draft.body.clone(),
                joydeck_engine::text::pos(&i.draft.body.text(), i.draft.cursor()),
            )
        } else {
            let Some((_, body)) = self.selected_body() else {
                return RunProps::default();
            };
            (body, (0, 0))
        };
        let props = edit::props_at(&body, at);
        if self.cell.is_none()
            && let Some(e) = &self.editor
            && let Some(id) = e.session.selection().shapes.first()
            && let Some(shape) = e.session.model().slides[self.page].shape(*id)
            && let Some(ctx) = joydeck_model::resolve::Ctx::for_slide(
                e.session.model(),
                &e.session.model().slides[self.page],
            )
            && let Some(para) = body.paragraphs.get(at.0)
        {
            let mut resolved = joydeck_model::resolve::run(&ctx, shape, para, &props);
            resolved.font = Some(joydeck_model::resolve::font_family(&ctx, &resolved));
            return resolved;
        }
        props
    }
    pub(super) fn font_group(&self, cx: &mut Context<Self>) -> Div {
        let props = self.run_props();
        let disabled = self.selected_body().is_none() || self.loading || self.composing();
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
            formats = formats.child(
                Button::new(id)
                    .ghost()
                    .xsmall()
                    .icon(icon)
                    .h(px(25.))
                    .w(px(26.))
                    .rounded(px(3.))
                    .accessibility_label(label)
                    .tooltip(label)
                    .disabled(disabled)
                    .when(on, |b| b.bg(rgb(p.selected)))
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
        let family = props
            .font
            .filter(|s| !s.starts_with('+'))
            .unwrap_or("主题字体".into());
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
        let disabled = self.selected_body().is_none() || self.loading;
        let mut first = div().flex().gap(px(1.));
        for (id, label, numbered) in [("bullets", "项目符号", false), ("numbering", "编号", true)]
        {
            first = first.child(
                Button::new(id)
                    .ghost()
                    .xsmall()
                    .icon(if numbered { I::ListOrdered } else { I::List })
                    .h(px(25.))
                    .w(px(28.))
                    .accessibility_label(label)
                    .tooltip(label)
                    .disabled(disabled)
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.format_paras(
                            |p| {
                                p.bullet = Some(
                                    if p.bullet
                                        .as_ref()
                                        .is_some_and(|b| !matches!(b, Bullet::None))
                                    {
                                        Bullet::None
                                    } else if numbered {
                                        Bullet::AutoNum {
                                            scheme: "arabicPeriod".into(),
                                            start_at: 1,
                                        }
                                    } else {
                                        Bullet::Char { char: "•".into() }
                                    },
                                );
                            },
                            w,
                            cx,
                        )
                    })),
            );
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
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.format_paras(|p| p.align = Some(align), w, cx)
                    })),
            );
        }
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
                                    Ok(()) => w.close_dialog(cx),
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
        let props = self.run_props();
        let color = match &props.fill {
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
        self.fields_dialog(
            "字体",
            vec![
                ("字体名称", props.font.unwrap_or("Microsoft YaHei".into())),
                ("字号（磅）", props.size.unwrap_or(18.).to_string()),
                ("颜色（RGB）", color),
            ],
            w,
            cx,
            |s, v, w, cx| {
                let size: f64 = v[1].parse().map_err(|_| "请输入有效字号")?;
                if !(1.0..=400.).contains(&size) || v[0].trim().is_empty() {
                    return Err("字号须在 1–400 磅之间，字体名称不能为空".into());
                }
                let color = format_ui::parse_rgb(&v[2])?;
                s.format_runs(
                    |p| {
                        p.font = Some(v[0].trim().into());
                        p.font_ea = p.font.clone();
                        p.size = Some(size);
                        p.fill = Some(joydeck_model::Fill::solid(color.clone()));
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
        let x = joydeck_model::resolve::xfrm(&ctx, shape);
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
                    Command::Transform {
                        dx: v[0] - x.x,
                        dy: v[1] - x.y,
                        dw: v[2] - x.w,
                        dh: v[3] - x.h,
                        rotation: v[4] - x.rot,
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
