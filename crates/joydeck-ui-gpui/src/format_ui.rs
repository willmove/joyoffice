//! Direct ribbon formatting, using the host GPUI Kit controls and the existing text transaction.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover, select::SelectEvent};
use joydeck_model::{ColorRef, ColorTransform, Fill, Rgba, SchemeSlot};
use joyoffice_ui::IconName as I;

const SIZES: &[u16] = &[
    8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 44, 48, 54, 60, 66, 72, 80, 88, 96,
];

// Windows GPUI consumes Shift when it translates OEM punctuation to '<' or '>'.
// Other backends may retain Shift and report the unshifted character instead.
pub(super) fn font_size_direction(command: bool, shift: bool, key: &str) -> Option<bool> {
    if !command {
        return None;
    }
    match key {
        ">" => Some(true),
        "<" => Some(false),
        "." if shift => Some(true),
        "," if shift => Some(false),
        _ => None,
    }
}

pub(super) fn parse_rgb(value: &str) -> Result<ColorRef, String> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("请输入六位 RGB 颜色，例如 D95A1A".into());
    }
    let n = u32::from_str_radix(value, 16).map_err(|_| "请输入六位 RGB 颜色")?;
    Ok(ColorRef::rgb(Rgba::rgb(
        (n >> 16) as u8,
        (n >> 8) as u8,
        n as u8,
    )))
}

impl Viewer {
    pub(super) fn step_font_size(&mut self, larger: bool, w: &mut Window, cx: &mut Context<Self>) {
        let next = |current: f64| {
            if larger {
                SIZES
                    .iter()
                    .copied()
                    .map(f64::from)
                    .find(|n| *n > current)
                    .unwrap_or((current + 10.).min(400.))
            } else {
                SIZES
                    .iter()
                    .copied()
                    .rev()
                    .map(f64::from)
                    .find(|n| *n < current)
                    .unwrap_or((current - 1.).max(1.))
            }
        };
        if self
            .inline
            .as_ref()
            .is_some_and(|i| i.draft.selection.is_empty())
        {
            let current = self.run_props().size.unwrap_or(18.);
            self.format_runs(|p| p.size = Some(next(current)), w, cx);
            return;
        }
        let Some(e) = &self.editor else { return };
        let pres = e.session.model();
        let slide = &pres.slides[self.page];
        let Some(ctx) = joydeck_model::resolve::Ctx::for_slide(pres, slide) else {
            return;
        };
        let mut edits = vec![];
        for target in self.format_targets() {
            let id = match target {
                joydeck_engine::TextTarget::Shape { shape }
                | joydeck_engine::TextTarget::Cell { shape, .. } => shape,
                _ => continue,
            };
            let Some(shape) = slide.shape(id) else {
                continue;
            };
            let Ok(mut body) = joydeck_engine::text_body(pres, self.page, &target) else {
                continue;
            };
            let range = self
                .inline
                .as_ref()
                .filter(|i| i.target == target)
                .map(|i| {
                    body = i.draft.body.clone();
                    i.draft.selection.clone()
                });
            let display = if let (
                joydeck_engine::TextTarget::Cell { row, col, .. },
                joydeck_model::ShapeKind::Table(t),
            ) = (&target, &shape.kind)
                && let Some(cell) = t.cell(*row, *col)
            {
                let mut cell = cell.clone();
                cell.text = body.clone();
                joydeck_render::table_cell_body(&ctx, t, *row, *col, &cell)
            } else {
                body.clone()
            };
            if let Err(error) = joydeck_engine::formatting::format_resolved(
                &mut body,
                range,
                |pi, p, r| {
                    joydeck_model::resolve::run(
                        &ctx,
                        shape,
                        display.paragraphs.get(pi).unwrap_or(p),
                        r,
                    )
                },
                |r, effective| r.size = Some(next(effective.size.unwrap_or(18.))),
            ) {
                self.message = error;
                cx.notify();
                return;
            }
            edits.push(joydeck_engine::TextEdit { target, body });
        }
        if let Some(i) = &mut self.inline {
            if let Some(edit) = edits.into_iter().find(|edit| edit.target == i.target) {
                i.draft.body = edit.body;
                self.publish_inline(w, cx);
            }
        } else if !edits.is_empty() {
            self.dispatch(joydeck_engine::Command::SetTexts { edits }, w, cx);
        }
        self.focus.focus(w, cx);
    }
    pub(super) fn install_font_picker(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.font_subscription =
            Some(cx.subscribe_in(&self.font_picker, w, |s, _, event, w, cx| {
                if let SelectEvent::Confirm(Some(family)) = event {
                    s.format_runs(
                        |p| {
                            p.font = Some(family.to_string());
                            p.font_ea = p.font.clone();
                        },
                        w,
                        cx,
                    );
                    s.focus.focus(w, cx);
                }
            }));
    }

    // Programmatic synchronization does not emit Confirm. Keep an active search intact.
    pub(super) fn sync_font_picker(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let state = self.formatting();
        if state.is_empty() {
            return;
        }
        if state.mixed.contains("font") || state.mixed.contains("font_ea") {
            if self.font_picker.read(cx).selected_value().is_some() {
                self.font_picker
                    .update(cx, |s, cx| s.set_selected_index(None, w, cx));
            }
            return;
        }
        let family: SharedString = state.run.font.unwrap_or("主题字体".into()).into();
        if self.font_picker.read(cx).selected_value() == Some(&family) {
            return;
        }
        if !self.font_names.contains(&family) {
            self.font_names.insert(0, family.clone());
            self.font_picker.update(cx, |s, cx| {
                s.set_items(
                    gpui_kit::component::select::SearchableVec::new(self.font_names.clone()),
                    w,
                    cx,
                )
            });
        }
        self.font_picker
            .update(cx, |s, cx| s.set_selected_value(&family, w, cx));
    }

    pub(super) fn font_size_picker(
        &self,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.formatting();
        let current = state.run.size.unwrap_or(18.);
        let label = if state.mixed.contains("size") {
            "—⌄".into()
        } else {
            format!("{current}⌄")
        };
        let view = cx.entity().downgrade();
        Popover::new("font-size-popup")
            .trigger(
                Button::new("font-size")
                    .xsmall()
                    .label(label)
                    .w(px(52.))
                    .h(px(23.))
                    .accessibility_label(if state.mixed.contains("size") {
                        "字号（混合格式）"
                    } else {
                        "字号"
                    })
                    .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let mut list = div().flex().flex_col().w(px(140.)).gap_1();
                let popup = cx.entity().downgrade();
                for row in SIZES.chunks(4) {
                    let mut line = div().flex().gap_1();
                    for &size in row {
                        let view = view.clone();
                        let popup = popup.clone();
                        line = line.child(
                            Button::new(("size-option", size as usize))
                                .ghost()
                                .xsmall()
                                .label(size.to_string())
                                .accessibility_label(format!("{size} 磅"))
                                .w(px(30.))
                                .on_click(move |_, w, cx| {
                                    let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
                                    let _ = view.update(cx, |s, cx| {
                                        s.format_runs(|p| p.size = Some(size as f64), w, cx)
                                    });
                                }),
                        );
                    }
                    list = list.child(line);
                }
                let view = view.clone();
                list.child(
                    Button::new("custom-size")
                        .ghost()
                        .xsmall()
                        .label("其他字号…")
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
                            let _ = view.update(cx, |s, cx| {
                                s.fields_dialog(
                                    "字号",
                                    vec![("字号（磅）", current.to_string())],
                                    w,
                                    cx,
                                    |s, v, w, cx| {
                                        let size: f64 =
                                            v[0].parse().map_err(|_| "请输入有效字号")?;
                                        if !size.is_finite() || !(1.0..=400.).contains(&size) {
                                            return Err("字号须在 1–400 磅之间".into());
                                        }
                                        s.format_runs(|p| p.size = Some(size), w, cx);
                                        Ok(())
                                    },
                                )
                            });
                        }),
                )
            })
    }

    pub(super) fn font_color_picker(
        &self,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let theme = self
            .editor
            .as_ref()
            .and_then(|e| {
                e.session
                    .model()
                    .master_for(&e.session.model().slides[self.page])
            })
            .map(|(m, _)| m.theme.colors.clone())
            .unwrap_or_else(|| joydeck_model::theme::Theme::default().colors);
        let state = self.formatting();
        let mixed = state.mixed.contains("fill");
        let selected = state.run.fill;
        let indicator = match selected {
            Some(Fill::Solid { color }) => color.resolve(&theme, None),
            _ => Rgba::BLACK,
        };
        let rgb_value = |c: Rgba| (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
        Popover::new("font-color-popup")
            .trigger(
                Button::new("font-color")
                    .ghost()
                    .xsmall()
                    .icon(I::Baseline)
                    .child(div().w(px(12.)).h(px(3.)).bg(rgb(if mixed {
                        0x999999
                    } else {
                        rgb_value(indicator)
                    })))
                    .h(px(25.))
                    .accessibility_label(if mixed {
                        "字体颜色（混合格式）"
                    } else {
                        "字体颜色"
                    })
                    .tooltip("字体颜色")
                    .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut panel = div().flex().flex_col().w(px(244.)).gap_2().text_sm();
                panel = panel.child("主题颜色");
                let slots = [
                    SchemeSlot::Lt1,
                    SchemeSlot::Dk1,
                    SchemeSlot::Lt2,
                    SchemeSlot::Dk2,
                    SchemeSlot::Accent1,
                    SchemeSlot::Accent2,
                    SchemeSlot::Accent3,
                    SchemeSlot::Accent4,
                    SchemeSlot::Accent5,
                    SchemeSlot::Accent6,
                ];
                for row in 0..6 {
                    let mut line = div().flex().gap_1();
                    for (col, slot) in slots.into_iter().enumerate() {
                        let mut color = ColorRef::scheme(slot);
                        if row > 0 {
                            color = color.with(match row {
                                1 => ColorTransform::Tint(80_000),
                                2 => ColorTransform::Tint(60_000),
                                3 => ColorTransform::Tint(40_000),
                                4 => ColorTransform::Shade(75_000),
                                _ => ColorTransform::Shade(50_000),
                            });
                        }
                        let label = format!("主题色 {} 变体 {row}", slot.xml_name());
                        line = line.child(color_button(
                            row * 10 + col,
                            &label,
                            color,
                            &theme,
                            view.clone(),
                            popup.clone(),
                        ));
                    }
                    panel = panel.child(line);
                }
                panel = panel.child("标准颜色");
                let mut line = div().flex().gap_1();
                for (index, (name, n)) in [
                    ("深红", 0xc00000),
                    ("红色", 0xff0000),
                    ("橙色", 0xffc000),
                    ("黄色", 0xffff00),
                    ("浅绿", 0x92d050),
                    ("绿色", 0x00b050),
                    ("浅蓝", 0x00b0f0),
                    ("蓝色", 0x0070c0),
                    ("深蓝", 0x002060),
                    ("紫色", 0x7030a0),
                ]
                .into_iter()
                .enumerate()
                {
                    let color = ColorRef::rgb(Rgba::rgb((n >> 16) as u8, (n >> 8) as u8, n as u8));
                    line = line.child(color_button(
                        60 + index,
                        name,
                        color,
                        &theme,
                        view.clone(),
                        popup.clone(),
                    ));
                }
                let view = view.clone();
                panel.child(line).child(
                    Button::new("custom-font-color")
                        .ghost()
                        .small()
                        .label("其他颜色…")
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
                            let _ = view.update(cx, |s, cx| {
                                s.fields_dialog(
                                    "字体颜色",
                                    vec![("颜色（RGB）", format!("{:06X}", rgb_value(indicator)))],
                                    w,
                                    cx,
                                    |s, v, w, cx| {
                                        let color = parse_rgb(&v[0])?;
                                        s.format_runs(
                                            |p| p.fill = Some(Fill::solid(color.clone())),
                                            w,
                                            cx,
                                        );
                                        Ok(())
                                    },
                                )
                            });
                        }),
                )
            })
    }
}

fn color_button(
    index: usize,
    label: &str,
    color: ColorRef,
    theme: &joydeck_model::ColorScheme,
    view: WeakEntity<Viewer>,
    popup: WeakEntity<gpui_kit::component::popover::PopoverState>,
) -> impl IntoElement {
    let c = color.resolve(theme, None);
    let n = (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
    Button::new(("font-color-swatch", index))
        .ghost()
        .xsmall()
        .w(px(20.))
        .h(px(20.))
        .p_0()
        .accessibility_label(label.to_owned())
        .tooltip(format!("{label} #{n:06X}"))
        .child(
            div()
                .size(px(16.))
                .bg(rgb(n))
                .border_1()
                .border_color(rgb(0xb0b0b0)),
        )
        .on_click(move |_, w, cx| {
            let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
            let _ = view.update(cx, |s, cx| {
                s.format_runs(|p| p.fill = Some(Fill::solid(color.clone())), w, cx)
            });
        })
}

#[cfg(test)]
mod tests {
    use super::font_size_direction;

    #[test]
    fn shifted_punctuation_matches_windows_and_other_backends() {
        assert_eq!(font_size_direction(true, false, ">"), Some(true));
        assert_eq!(font_size_direction(true, false, "<"), Some(false));
        assert_eq!(font_size_direction(true, true, "."), Some(true));
        assert_eq!(font_size_direction(true, true, ","), Some(false));
        assert_eq!(font_size_direction(true, false, "."), None);
        assert_eq!(font_size_direction(true, false, ","), None);
        assert_eq!(font_size_direction(false, true, ">"), None);
    }
}
