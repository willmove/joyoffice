//! Editing affordances shared by the native ribbon, dialogs and keyboard routes.
use super::*;
use crate::l10n::{t, tf};
use gpui_kit::component::{Sizable, button::ButtonVariants, tooltip::Tooltip};
use joydeck_engine::Command;
use joydeck_model::{
    edit,
    text::{Align, Bullet, RunProps, Strike, TextBody},
};
use joyoffice_ui::{IconName as I, UiLocale};

fn unavailable_format(locale: UiLocale, id: &'static str, icon: I, label: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(icon)
        .h(px(25.))
        .w(px(26.))
        .accessibility_label(tf(locale, "common.unsupported_suffix", &[("label", label)]))
        .disabled(true)
}

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
        let locale = self.locale;
        let state = self.formatting();
        let empty = state.is_empty();
        let disabled = empty || self.loading || self.composing();
        let props = state.run;
        let p = self.palette();
        let mut formats = div().flex().items_center().gap(px(1.));
        for (id, label, icon, property, on) in [
            (
                "font-bold",
                t(locale, "font.bold"),
                I::Bold,
                "bold",
                props.bold == Some(true),
            ),
            (
                "font-italic",
                t(locale, "font.italic"),
                I::Italic,
                "italic",
                props.italic == Some(true),
            ),
            (
                "font-underline",
                t(locale, "font.underline"),
                I::Underline,
                "underline",
                props.underline.as_deref().is_some_and(|u| u != "none"),
            ),
            (
                "font-strike",
                t(locale, "font.strikethrough"),
                I::Strikethrough,
                "strike",
                props.strike.is_some_and(|strike| strike != Strike::None),
            ),
            (
                "font-super",
                t(locale, "font.superscript"),
                I::Superscript,
                "baseline",
                props.baseline.is_some_and(|value| value > 0.),
            ),
            (
                "font-sub",
                t(locale, "font.subscript"),
                I::Subscript,
                "baseline",
                props.baseline.is_some_and(|value| value < 0.),
            ),
        ] {
            let mixed = state.mixed.contains(property);
            let accessible = if mixed {
                tf(locale, "common.mixed_format_suffix", &[("label", label)])
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
                                "font-underline" => {
                                    p.underline = Some(if on { "none" } else { "sng" }.into())
                                }
                                "font-strike" => {
                                    p.strike = Some(if on { Strike::None } else { Strike::Single })
                                }
                                "font-super" => p.baseline = Some(if on { 0. } else { 0.30 }),
                                _ => p.baseline = Some(if on { 0. } else { -0.25 }),
                            },
                            w,
                            cx,
                        )
                    })),
            );
        }
        let clear_format = Button::new("clear-format")
            .ghost()
            .xsmall()
            .icon(I::RemoveFormatting)
            .h(px(23.))
            .accessibility_label(t(locale, "font.clear_formatting"))
            .tooltip(t(locale, "font.clear_formatting"))
            .disabled(disabled)
            .on_click(cx.listener(|s, _, w, cx| {
                s.format_runs(joydeck_engine::formatting::clear_character_format, w, cx)
            }));
        formats = formats
            .child(unavailable_format(
                locale,
                "font-spacing",
                I::MoveHorizontal,
                t(locale, "font.char_spacing"),
            ))
            .child(unavailable_format(
                locale,
                "font-case",
                I::CaseSensitive,
                t(locale, "font.change_case"),
            ))
            .child(unavailable_format(
                locale,
                "font-highlight",
                I::Highlighter,
                t(locale, "font.highlight_color"),
            ))
            .child(self.font_color_picker(disabled, cx));
        let family = props.font.filter(|s| !s.starts_with('+')).unwrap_or(
            if empty {
                ""
            } else if state.mixed.contains("font") || state.mixed.contains("font_ea") {
                t(locale, "font.mixed")
            } else {
                t(locale, "font.theme_font")
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
                            .accessibility_label(t(locale, "font.name_aria"))
                            .search_placeholder(t(locale, "font.search_placeholder"))
                            .placeholder(family)
                            .menu_width(px(260.))
                            .menu_max_h(px(360.))
                            .w(px(169.))
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
                            .accessibility_label(t(locale, "font.increase_size"))
                            .tooltip(t(locale, "font.increase_size_tip"))
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
                            .accessibility_label(t(locale, "font.decrease_size"))
                            .tooltip(t(locale, "font.decrease_size_tip"))
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.step_font_size(false, w, cx))),
                    )
                    .child(clear_format),
            )
            .child(
                formats.child(
                    Button::new("font-more")
                        .ghost()
                        .xsmall()
                        .label(t(locale, "font.dialog_btn"))
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.font_dialog(w, cx))),
                ),
            );
        chrome::group(t(locale, "home.group.font"), content)
    }
    pub(super) fn paragraph_group(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let state = self.formatting();
        let disabled = state.is_empty() || self.loading || self.composing();
        let palette = self.palette();
        let mut first = div().flex().gap(px(1.));
        for (id, label, numbered) in [
            ("bullets", t(locale, "para.bullets"), false),
            ("numbering", t(locale, "para.numbering"), true),
        ] {
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
                    .accessibility_label(t(locale, "para.decrease_indent"))
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
                    .accessibility_label(t(locale, "para.increase_indent"))
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, w, cx| {
                        s.format_paras(
                            |p| p.margin_left = Some((p.margin_left.unwrap_or(0.) + 18.).min(720.)),
                            w,
                            cx,
                        )
                    })),
            )
            .child(self.line_spacing_picker(disabled, cx))
            .child(unavailable_format(
                locale,
                "paragraph-columns",
                I::Columns2,
                t(locale, "para.columns"),
            ));
        let mut second = div().flex().gap(px(1.));
        for (id, label, icon, align) in [
            (
                "align-left",
                t(locale, "para.align_left"),
                I::TextAlignStart,
                Align::Left,
            ),
            (
                "align-center",
                t(locale, "para.align_center"),
                I::TextAlignCenter,
                Align::Center,
            ),
            (
                "align-right",
                t(locale, "para.align_right"),
                I::TextAlignEnd,
                Align::Right,
            ),
            (
                "align-justify",
                t(locale, "para.align_justify"),
                I::TextAlignJustify,
                Align::Justify,
            ),
            (
                "align-distributed",
                t(locale, "para.align_distribute"),
                I::TextAlignJustify,
                Align::Distributed,
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
        second = second.child(
            Button::new("paragraph-dialog")
                .ghost()
                .xsmall()
                .label(t(locale, "para.dialog_btn"))
                .h(px(25.))
                .accessibility_label(t(locale, "para.settings_aria"))
                .disabled(disabled)
                .on_click(cx.listener(|s, _, w, cx| s.paragraph_dialog(w, cx))),
        );
        chrome::group(
            t(locale, "home.group.paragraph"),
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
        let locale = self.locale;
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
                        .label(t(locale, "common.ok"))
                        .on_click(move |_, w, cx| {
                            let _ = view.update(cx, |s, cx| {
                                if s.generation != generation
                                    || s.editor.as_ref().map(|e| e.session.revision()) != revision
                                {
                                    error2.update(cx, |e, cx| {
                                        *e = s.t("dialog.doc_changed").into();
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
        let locale = self.locale;
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
            t(locale, "font.dialog_title"),
            vec![
                (t(locale, "font.dialog_name_field"), font),
                (t(locale, "font.dialog_size_field"), size),
                (t(locale, "font.color_rgb_field"), color),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                let size = if v[1] == original[1] || v[1].trim().is_empty() {
                    None
                } else {
                    let n: f64 = v[1]
                        .trim()
                        .parse()
                        .map_err(|_| s.t("font.err_invalid_size"))?;
                    if !n.is_finite() || !(1. ..=400.).contains(&n) {
                        return Err(s.t("font.err_size_range").into());
                    }
                    Some(n)
                };
                let color = if v[2] == original[2] || v[2].trim().is_empty() {
                    None
                } else {
                    Some(format_ui::parse_rgb_for(s.locale, &v[2])?)
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
        let locale = self.locale;
        self.fields_dialog(
            t(locale, "dialog.find_title"),
            vec![(t(locale, "dialog.find_field"), String::new())],
            w,
            cx,
            |s, v, w, cx| {
                let query = v[0].trim().to_lowercase();
                if query.is_empty() {
                    return Err(s.t("dialog.find_err_empty").into());
                }
                let Some(e) = &s.editor else {
                    return Err(s.t("dialog.find_err_no_doc").into());
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
                    return Err(s.t("dialog.find_err_not_found").into());
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
        let locale = self.locale;
        self.fields_dialog(
            t(locale, "dialog.insert_table_title"),
            vec![
                (t(locale, "dialog.table_cols"), "3".into()),
                (t(locale, "dialog.table_rows"), "3".into()),
            ],
            w,
            cx,
            |s, v, w, cx| {
                let cols: usize = v[0].parse().map_err(|_| s.t("dialog.table_err_cols"))?;
                let rows: usize = v[1].parse().map_err(|_| s.t("dialog.table_err_rows"))?;
                if !(1..=32).contains(&cols) || !(1..=64).contains(&rows) {
                    return Err(s.t("dialog.table_err_range").into());
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
        let locale = self.locale;
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
            t(locale, "dialog.geom_title"),
            vec![
                (t(locale, "dialog.geom_x"), x.x.to_string()),
                (t(locale, "dialog.geom_y"), x.y.to_string()),
                (t(locale, "dialog.geom_w"), x.w.to_string()),
                (t(locale, "dialog.geom_h"), x.h.to_string()),
                (t(locale, "dialog.geom_rot"), x.rot.to_string()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                let err_num = s.t("dialog.geom_err_num");
                let v: Vec<f64> = v
                    .iter()
                    .map(|s| s.parse::<f64>().map_err(|_| err_num.to_string()))
                    .collect::<Result<_, _>>()?;
                if v.iter().any(|x| !x.is_finite() || x.abs() > 100000.) || v[2] <= 0. || v[3] <= 0.
                {
                    return Err(s.t("dialog.geom_err_positive").into());
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
        let locale = self.locale;
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
            d.title(t(locale, "dialog.slide_layout_title")).child(list)
        });
    }
    pub(super) fn shape_gallery(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.open_shape_picker(false, w, cx);
    }
    pub(super) fn shape_preset_gallery(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.open_shape_picker(true, w, cx);
    }
    fn open_shape_picker(
        &mut self,
        replace_selected: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = self.locale;
        let view = cx.entity().downgrade();
        let p = self.palette();
        w.open_dialog(cx, move |d, _, _| {
            const GROUPS: &[(&str, &[(&str, &str)])] = &[
                (
                    "shapes.cat.basic",
                    &[
                        ("rect", "shapes.rect"),
                        ("roundRect", "shapes.round_rect"),
                        ("snip1Rect", "shapes.snip1_rect"),
                        ("ellipse", "shapes.ellipse"),
                        ("triangle", "shapes.triangle"),
                        ("rtTriangle", "shapes.rt_triangle"),
                        ("diamond", "shapes.diamond"),
                        ("parallelogram", "shapes.parallelogram"),
                        ("trapezoid", "shapes.trapezoid"),
                        ("pentagon", "shapes.pentagon"),
                        ("hexagon", "shapes.hexagon"),
                        ("octagon", "shapes.octagon"),
                        ("plus", "shapes.plus"),
                        ("donut", "shapes.donut"),
                        ("frame", "shapes.frame"),
                        ("heart", "shapes.heart"),
                        ("cloud", "shapes.cloud"),
                        ("lightningBolt", "shapes.lightning"),
                    ],
                ),
                (
                    "shapes.cat.arrows",
                    &[
                        ("rightArrow", "shapes.right_arrow"),
                        ("leftArrow", "shapes.left_arrow"),
                        ("upArrow", "shapes.up_arrow"),
                        ("downArrow", "shapes.down_arrow"),
                        ("leftRightArrow", "shapes.left_right_arrow"),
                        ("upDownArrow", "shapes.up_down_arrow"),
                        ("chevron", "shapes.chevron"),
                        ("homePlate", "shapes.home_plate"),
                        ("bentArrow", "shapes.bent_arrow"),
                        ("uturnArrow", "shapes.uturn_arrow"),
                    ],
                ),
                (
                    "shapes.cat.flowchart",
                    &[
                        ("flowChartProcess", "shapes.fc_process"),
                        ("flowChartDecision", "shapes.fc_decision"),
                        ("flowChartTerminator", "shapes.fc_terminator"),
                        ("flowChartDocument", "shapes.fc_document"),
                        ("flowChartPredefinedProcess", "shapes.fc_predefined"),
                        ("flowChartConnector", "shapes.fc_connector"),
                    ],
                ),
                (
                    "shapes.cat.stars",
                    &[
                        ("star4", "shapes.star4"),
                        ("star5", "shapes.star5"),
                        ("star6", "shapes.star6"),
                        ("star8", "shapes.star8"),
                        ("wedgeRectCallout", "shapes.callout_rect"),
                        ("wedgeRoundRectCallout", "shapes.callout_round_rect"),
                        ("wedgeEllipseCallout", "shapes.callout_ellipse"),
                        ("cloudCallout", "shapes.callout_cloud"),
                    ],
                ),
                (
                    "shapes.cat.lines",
                    &[
                        ("line", "shapes.line"),
                        ("straightConnector1", "shapes.straight_connector"),
                        ("bentConnector3", "shapes.bent_connector"),
                        ("curvedConnector3", "shapes.curved_connector"),
                    ],
                ),
            ];
            let mut body = div()
                .id("shape-gallery-body")
                .flex()
                .flex_col()
                .gap_3()
                .max_h(px(460.))
                .w_full()
                .overflow_y_scroll()
                .pr_2();
            let mut idx = 0usize;
            for (heading_key, items) in GROUPS {
                let mut grid = div().flex().flex_wrap().gap(px(6.));
                for &(preset, label_key) in *items {
                    let label = t(locale, label_key);
                    let view = view.clone();
                    let stroke_color = rgb(p.text);
                    let fill_color = rgb(p.accent);
                    let tile = div()
                        .id(("shape-tile", idx))
                        .w(px(72.))
                        .h(px(58.))
                        .p(px(4.))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_between()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(rgb(p.border))
                        .bg(rgb(p.surface))
                        .cursor_pointer()
                        .tooltip(move |w, cx| Tooltip::new(label).build(w, cx))
                        .on_click(move |_, w, cx| {
                            let _ = view.update(cx, |s, cx| {
                                let cmd = if replace_selected {
                                    Command::Content {
                                        action: joydeck_engine::ContentAction::ShapePreset {
                                            name: preset.into(),
                                        },
                                    }
                                } else {
                                    Command::InsertShape {
                                        preset: preset.into(),
                                        x: 100.,
                                        y: 140.,
                                        width: 200.,
                                        height: 120.,
                                    }
                                };
                                s.dispatch(cmd, w, cx);
                                w.close_dialog(cx);
                            });
                        })
                        .child(shape_preset_preview(preset, fill_color, stroke_color))
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(rgb(p.muted))
                                .truncate()
                                .max_w(px(66.))
                                .child(label),
                        );
                    grid = grid.child(tile);
                    idx += 1;
                }
                body = body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(p.muted))
                                .child(t(locale, heading_key)),
                        )
                        .child(grid),
                );
            }
            d.title(if replace_selected {
                t(locale, "shapes.change_title")
            } else {
                t(locale, "shapes.insert_title")
            })
            .child(body)
        });
    }
}

fn shape_preset_preview(preset: &'static str, fill: Rgba, stroke: Rgba) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, w, _| {
            let pad = 3.0_f64;
            let bw = (f64::from(bounds.size.width) - pad * 2.0).max(4.0);
            let bh = (f64::from(bounds.size.height) - pad * 2.0).max(4.0);
            let ox = f32::from(bounds.origin.x) + pad as f32;
            let oy = f32::from(bounds.origin.y) + pad as f32;
            let Some(geom) = joydeck_geom::preset::build(preset, bw, bh, &[]) else {
                return;
            };
            let is_open = geom.is_open() || joydeck_geom::preset::is_line_like(preset);
            let pt = |p: joydeck_geom::Point| point(px(ox + p.x as f32), px(oy + p.y as f32));
            for sub in &geom.paths {
                let mut builder =
                    if is_open || matches!(sub.fill, joydeck_geom::preset::FillMode::None) {
                        PathBuilder::stroke(px(1.6))
                    } else {
                        PathBuilder::fill()
                    };
                for el in sub.path.elements() {
                    match *el {
                        joydeck_geom::PathEl::MoveTo(p) => builder.move_to(pt(p)),
                        joydeck_geom::PathEl::LineTo(p) => builder.line_to(pt(p)),
                        joydeck_geom::PathEl::QuadTo(a, b) => builder.curve_to(pt(b), pt(a)),
                        joydeck_geom::PathEl::CurveTo(a, b, c) => {
                            builder.cubic_bezier_to(pt(c), pt(a), pt(b))
                        }
                        joydeck_geom::PathEl::ClosePath => builder.close(),
                    }
                }
                if let Ok(path) = builder.build() {
                    let paint =
                        if is_open || matches!(sub.fill, joydeck_geom::preset::FillMode::None) {
                            stroke
                        } else {
                            fill
                        };
                    w.paint_path(path, paint);
                }
            }
        },
    )
    .w(px(36.))
    .h(px(28.))
}
