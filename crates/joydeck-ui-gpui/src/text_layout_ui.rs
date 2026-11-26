//! Text box layout uses the same resolved properties and edit transaction as the canvas.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover};
use joydeck_engine::TextTarget;
use joydeck_model::{
    resolve,
    text::{Anchor, AutoFit, BodyProps, TextDir},
};

#[derive(Clone, Copy)]
enum LayoutChoice {
    Direction(TextDir),
    Anchor(Anchor),
    Fit(AutoFit),
    Wrap(bool),
    Columns(u32),
    ColumnOrder(bool),
    ColumnSettings,
}

impl LayoutChoice {
    fn matches(self, body: &BodyProps) -> bool {
        match self {
            Self::Direction(dir) => body.vert == Some(dir),
            Self::Anchor(anchor) => body.anchor == Some(anchor),
            Self::Fit(AutoFit::Shrink { .. }) => {
                matches!(body.autofit, Some(AutoFit::Shrink { .. }))
            }
            Self::Fit(fit) => body.autofit == Some(fit),
            Self::Wrap(wrap) => body.wrap == Some(wrap),
            Self::Columns(n) => body.columns == Some(n),
            Self::ColumnOrder(rtl) => body.rtl_col == Some(rtl),
            Self::ColumnSettings => false,
        }
    }

    fn apply(self, body: &mut BodyProps) {
        match self {
            Self::Direction(dir) => body.vert = Some(dir),
            Self::Anchor(anchor) => body.anchor = Some(anchor),
            Self::Fit(fit) => body.autofit = Some(fit),
            Self::Wrap(wrap) => body.wrap = Some(wrap),
            Self::Columns(n) => body.columns = Some(n),
            Self::ColumnOrder(rtl) => body.rtl_col = Some(rtl),
            Self::ColumnSettings => {}
        }
    }
}

fn layout_button(id: &'static str, label: &'static str, disabled: bool) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .label(label)
        .h(px(25.))
        .disabled(disabled)
}

fn common<T: Copy + PartialEq>(
    bodies: &[BodyProps],
    value: impl Fn(&BodyProps) -> Option<T>,
) -> Option<T> {
    let first = value(bodies.first()?)?;
    bodies
        .iter()
        .all(|body| value(body) == Some(first))
        .then_some(first)
}

fn field<T: ToString>(value: Option<T>) -> String {
    value.map(|n| n.to_string()).unwrap_or_default()
}

impl Viewer {
    fn text_box_layouts(&self) -> Option<Vec<BodyProps>> {
        let editor = self.editor.as_ref()?;
        let pres = editor.session.model();
        let slide = pres.slides.get(self.page)?;
        let ctx = resolve::Ctx::for_slide(pres, slide)?;
        let targets = self.format_targets();
        if targets.is_empty() || targets.len() > 256 {
            return None;
        }
        targets
            .iter()
            .map(|target| {
                let TextTarget::Shape { shape } = target else {
                    return None;
                };
                let shape = slide.shape(*shape).filter(|shape| !shape.locked)?;
                Some(resolve::body(&ctx, shape))
            })
            .collect()
    }

    #[inline(never)]
    pub(super) fn text_layout_group(&self, cx: &mut Context<Self>) -> Div {
        let bodies = self.text_box_layouts().unwrap_or_default();
        let disabled = bodies.is_empty() || self.loading || self.composing();
        let direction = self.text_layout_menu(
            "text-direction",
            self.t("text_layout.direction"),
            &bodies,
            disabled,
            [
                (
                    self.t("text_dir.horizontal"),
                    LayoutChoice::Direction(TextDir::Horizontal),
                ),
                (
                    self.t("text_dir.rot90"),
                    LayoutChoice::Direction(TextDir::Vertical),
                ),
                (
                    self.t("text_dir.rot270"),
                    LayoutChoice::Direction(TextDir::Vertical270),
                ),
                (
                    self.t("text_dir.ea_vertical"),
                    LayoutChoice::Direction(TextDir::EaVertical),
                ),
                (
                    self.t("text_dir.stacked"),
                    LayoutChoice::Direction(TextDir::Stacked),
                ),
                (
                    self.t("text_dir.stacked_rtl"),
                    LayoutChoice::Direction(TextDir::StackedRtl),
                ),
                (
                    self.t("text_dir.mongolian"),
                    LayoutChoice::Direction(TextDir::MongolianVertical),
                ),
            ],
            cx,
        );
        let columns = self.text_layout_menu(
            "text-columns",
            self.t("text_layout.columns"),
            &bodies,
            disabled,
            [
                (self.t("text_layout.col1"), LayoutChoice::Columns(1)),
                (self.t("text_layout.col2"), LayoutChoice::Columns(2)),
                (self.t("text_layout.col3"), LayoutChoice::Columns(3)),
                (
                    self.t("text_layout.col_ltr"),
                    LayoutChoice::ColumnOrder(false),
                ),
                (
                    self.t("text_layout.col_rtl"),
                    LayoutChoice::ColumnOrder(true),
                ),
                (
                    self.t("text_layout.more_cols"),
                    LayoutChoice::ColumnSettings,
                ),
            ],
            cx,
        );
        let alignment = self.text_layout_menu(
            "text-anchor",
            self.t("text_layout.align"),
            &bodies,
            disabled,
            [
                (self.t("align.top"), LayoutChoice::Anchor(Anchor::Top)),
                (
                    self.t("align.middle_v"),
                    LayoutChoice::Anchor(Anchor::Middle),
                ),
                (self.t("align.bottom"), LayoutChoice::Anchor(Anchor::Bottom)),
                (
                    self.t("para.align_justify"),
                    LayoutChoice::Anchor(Anchor::Justified),
                ),
                (
                    self.t("para.align_distribute"),
                    LayoutChoice::Anchor(Anchor::Distributed),
                ),
            ],
            cx,
        );
        let fit = self.text_layout_menu(
            "text-fit",
            self.t("text_layout.autofit"),
            &bodies,
            disabled,
            [
                (
                    self.t("text_layout.autofit_none"),
                    LayoutChoice::Fit(AutoFit::None),
                ),
                (
                    self.t("text_layout.autofit_shrink"),
                    LayoutChoice::Fit(AutoFit::Shrink {
                        font_scale: Some(1.),
                        line_reduction: 0.,
                    }),
                ),
                (self.t("text_layout.wrap_on"), LayoutChoice::Wrap(true)),
                (self.t("text_layout.wrap_off"), LayoutChoice::Wrap(false)),
            ],
            cx,
        );
        chrome::group(
            self.t("text_layout.group"),
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .child(direction)
                        .child(columns)
                        .child(alignment),
                )
                .child(
                    div().flex().child(fit).child(
                        layout_button("text-insets", self.t("text_layout.insets_btn"), disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.text_insets_dialog(w, cx))),
                    ),
                ),
        )
    }

    fn text_layout_menu<const N: usize>(
        &self,
        id: &'static str,
        title: &'static str,
        bodies: &[BodyProps],
        disabled: bool,
        items: [(&'static str, LayoutChoice); N],
        cx: &mut Context<Self>,
    ) -> Popover {
        let view = cx.entity().downgrade();
        let selected =
            items.map(|(_, action)| !bodies.is_empty() && bodies.iter().all(|b| action.matches(b)));
        let palette = self.palette();
        Popover::new((id, usize::MAX))
            .trigger(layout_button(id, title, disabled).accessibility_label(title))
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut panel = div().flex().flex_col().w(px(170.)).gap_1();
                for (index, (label, action)) in items.into_iter().enumerate() {
                    let view = view.clone();
                    let popup = popup.clone();
                    panel = panel.child(
                        Button::new((id, index))
                            .ghost()
                            .xsmall()
                            .label(label)
                            .when(selected[index], |b| b.bg(rgb(palette.selected)))
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = view
                                    .update(cx, |s, cx| s.apply_text_layout_choice(action, w, cx));
                            }),
                    );
                }
                panel
            })
    }

    fn apply_text_layout_choice(
        &mut self,
        choice: LayoutChoice,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.text_box_layouts().is_none() || self.loading || self.composing() {
            return;
        }
        if matches!(choice, LayoutChoice::ColumnSettings) {
            self.text_columns_dialog(w, cx);
        } else {
            self.format_bodies(|body| choice.apply(&mut body.body), w, cx);
            self.focus.focus(w, cx);
        }
    }

    fn text_columns_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(bodies) = self.text_box_layouts() else {
            return;
        };
        let original = [
            field(common(&bodies, |b| b.columns)),
            field(common(&bodies, |b| b.col_spacing)),
        ];
        let targets = self.format_targets();
        let page = self.page;
        self.fields_dialog(
            self.t("text_layout.cols_title"),
            vec![
                (self.t("text_layout.cols_count"), original[0].clone()),
                (self.t("text_layout.cols_spacing"), original[1].clone()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                if s.page != page || s.format_targets() != targets {
                    return Err(s.t("text_layout.err_shape_changed").into());
                }
                let columns = if v[0] == original[0] || v[0].trim().is_empty() {
                    None
                } else {
                    Some(
                        v[0].trim()
                            .parse::<u32>()
                            .ok()
                            .filter(|n| (1..=16).contains(n))
                            .ok_or(s.t("text_layout.err_cols_range"))?,
                    )
                };
                let gap = if v[1] == original[1] {
                    None
                } else {
                    paragraph_ui::optional_number(s.locale, &v[1], 0., 4000.)?
                };
                if columns.is_some() || gap.is_some() {
                    s.format_bodies(
                        |body| {
                            if let Some(columns) = columns {
                                body.body.columns = Some(columns);
                            }
                            if let Some(gap) = gap {
                                body.body.col_spacing = Some(gap);
                            }
                        },
                        w,
                        cx,
                    );
                }
                Ok(())
            },
        );
    }

    fn text_insets_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(bodies) = self.text_box_layouts() else {
            return;
        };
        let original = [
            field(common(&bodies, |b| b.inset_l)),
            field(common(&bodies, |b| b.inset_t)),
            field(common(&bodies, |b| b.inset_r)),
            field(common(&bodies, |b| b.inset_b)),
        ];
        let targets = self.format_targets();
        let page = self.page;
        self.fields_dialog(
            self.t("text_layout.insets_title"),
            vec![
                (self.t("table.margin_left"), original[0].clone()),
                (self.t("table.margin_top"), original[1].clone()),
                (self.t("table.margin_right"), original[2].clone()),
                (self.t("table.margin_bottom"), original[3].clone()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                if s.page != page || s.format_targets() != targets {
                    return Err(s.t("text_layout.err_shape_changed").into());
                }
                let mut patch = [None; 4];
                for (i, slot) in patch.iter_mut().enumerate() {
                    if v[i] != original[i] {
                        *slot = paragraph_ui::optional_number(s.locale, &v[i], 0., 4000.)?;
                    }
                }
                if patch.iter().any(Option::is_some) {
                    s.format_bodies(
                        |body| {
                            let b = &mut body.body;
                            for (value, dst) in patch.into_iter().zip([
                                &mut b.inset_l,
                                &mut b.inset_t,
                                &mut b.inset_r,
                                &mut b.inset_b,
                            ]) {
                                if let Some(value) = value {
                                    *dst = Some(value);
                                }
                            }
                        },
                        w,
                        cx,
                    );
                }
                Ok(())
            },
        );
    }
}
