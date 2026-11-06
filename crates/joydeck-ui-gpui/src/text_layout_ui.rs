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
            "文字方向",
            &bodies,
            disabled,
            [
                ("水平文字", LayoutChoice::Direction(TextDir::Horizontal)),
                ("旋转 90°", LayoutChoice::Direction(TextDir::Vertical)),
                ("旋转 270°", LayoutChoice::Direction(TextDir::Vertical270)),
                ("中文竖排", LayoutChoice::Direction(TextDir::EaVertical)),
                ("堆叠文字", LayoutChoice::Direction(TextDir::Stacked)),
                ("从右到左堆叠", LayoutChoice::Direction(TextDir::StackedRtl)),
                (
                    "蒙古文竖排",
                    LayoutChoice::Direction(TextDir::MongolianVertical),
                ),
            ],
            cx,
        );
        let columns = self.text_layout_menu(
            "text-columns",
            "分栏",
            &bodies,
            disabled,
            [
                ("一栏", LayoutChoice::Columns(1)),
                ("两栏", LayoutChoice::Columns(2)),
                ("三栏", LayoutChoice::Columns(3)),
                ("从左到右排列", LayoutChoice::ColumnOrder(false)),
                ("从右到左排列", LayoutChoice::ColumnOrder(true)),
                ("更多分栏…", LayoutChoice::ColumnSettings),
            ],
            cx,
        );
        let alignment = self.text_layout_menu(
            "text-anchor",
            "文字对齐",
            &bodies,
            disabled,
            [
                ("顶端对齐", LayoutChoice::Anchor(Anchor::Top)),
                ("垂直居中", LayoutChoice::Anchor(Anchor::Middle)),
                ("底端对齐", LayoutChoice::Anchor(Anchor::Bottom)),
                ("两端对齐", LayoutChoice::Anchor(Anchor::Justified)),
                ("分散对齐", LayoutChoice::Anchor(Anchor::Distributed)),
            ],
            cx,
        );
        let fit = self.text_layout_menu(
            "text-fit",
            "自动调整",
            &bodies,
            disabled,
            [
                ("不自动调整", LayoutChoice::Fit(AutoFit::None)),
                (
                    "溢出时缩小文字",
                    LayoutChoice::Fit(AutoFit::Shrink {
                        font_scale: Some(1.),
                        line_reduction: 0.,
                    }),
                ),
                ("形状内自动换行", LayoutChoice::Wrap(true)),
                ("不自动换行", LayoutChoice::Wrap(false)),
            ],
            cx,
        );
        chrome::group(
            "文字布局",
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
                        layout_button("text-insets", "文字边距…", disabled)
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
        let palette = chrome::palette(self.dark);
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
            "文字分栏（留空保持原值）",
            vec![
                ("栏数（1–16）", original[0].clone()),
                ("栏间距（磅）", original[1].clone()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                if s.page != page || s.format_targets() != targets {
                    return Err("文字对象已变化，请重新打开此对话框".into());
                }
                let columns = if v[0] == original[0] || v[0].trim().is_empty() {
                    None
                } else {
                    Some(
                        v[0].trim()
                            .parse::<u32>()
                            .ok()
                            .filter(|n| (1..=16).contains(n))
                            .ok_or("栏数须为 1–16 的整数")?,
                    )
                };
                let gap = if v[1] == original[1] {
                    None
                } else {
                    paragraph_ui::optional_number(&v[1], 0., 4000.)?
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
            "文字边距（留空保持原值）",
            vec![
                ("左边距（磅）", original[0].clone()),
                ("上边距（磅）", original[1].clone()),
                ("右边距（磅）", original[2].clone()),
                ("下边距（磅）", original[3].clone()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                if s.page != page || s.format_targets() != targets {
                    return Err("文字对象已变化，请重新打开此对话框".into());
                }
                let mut patch = [None; 4];
                for (i, slot) in patch.iter_mut().enumerate() {
                    if v[i] != original[i] {
                        *slot = paragraph_ui::optional_number(&v[i], 0., 4000.)?;
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
