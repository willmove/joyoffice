//! Native paragraph choices; blank mixed fields preserve each target's value.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover};
use joydeck_model::text::{Bullet, Spacing};

pub(super) fn optional_number(value: &str, min: f64, max: f64) -> Result<Option<f64>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let n: f64 = value
        .trim()
        .parse()
        .map_err(|_| "请输入有效数值；留空保持原值")?;
    if !n.is_finite() || !(min..=max).contains(&n) {
        return Err(format!("数值范围为 {min}–{max}"));
    }
    Ok(Some(n))
}
fn display_spacing(value: Option<Spacing>, line: bool) -> String {
    match value {
        Some(Spacing::Pct(v)) => {
            if line {
                v.to_string()
            } else {
                format!("{}%", v * 100.)
            }
        }
        Some(Spacing::Pts(v)) => format!("{v}pt"),
        None => String::new(),
    }
}
fn parse_spacing(value: &str, line: bool) -> Result<Option<Spacing>, String> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return optional_number(percent, 0., 400000.).map(|n| n.map(|n| Spacing::Pct(n / 100.)));
    }
    if let Some(pts) = value.strip_suffix("pt") {
        return optional_number(pts, if line { 1. } else { 0. }, 4000.)
            .map(|n| n.map(Spacing::Pts));
    }
    optional_number(
        value,
        if line { 0.5 } else { 0. },
        if line { 10. } else { 4000. },
    )
    .map(|n| n.map(if line { Spacing::Pct } else { Spacing::Pts }))
}
impl Viewer {
    pub(super) fn list_picker(
        &self,
        numbered: bool,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let label = if numbered {
            "编号样式"
        } else {
            "项目符号样式"
        };
        Popover::new(if numbered {
            "numbering-popup"
        } else {
            "bullets-popup"
        })
        .trigger(
            Button::new(if numbered {
                "numbering-choices"
            } else {
                "bullet-choices"
            })
            .ghost()
            .xsmall()
            .label("⌄")
            .w(px(16.))
            .h(px(25.))
            .accessibility_label(label)
            .disabled(disabled),
        )
        .content(move |_, _, cx| {
            let popup = cx.entity().downgrade();
            let mut list = div().flex().flex_col().w(px(175.)).gap_1();
            let choices = if numbered {
                vec![
                    ("无", Bullet::None),
                    (
                        "1. 2. 3.",
                        Bullet::AutoNum {
                            scheme: "arabicPeriod".into(),
                            start_at: 1,
                        },
                    ),
                    (
                        "a. b. c.",
                        Bullet::AutoNum {
                            scheme: "alphaLcPeriod".into(),
                            start_at: 1,
                        },
                    ),
                    (
                        "I. II. III.",
                        Bullet::AutoNum {
                            scheme: "romanUcPeriod".into(),
                            start_at: 1,
                        },
                    ),
                ]
            } else {
                vec![
                    ("无", Bullet::None),
                    ("• 实心圆", Bullet::Char { char: "•".into() }),
                    ("○ 空心圆", Bullet::Char { char: "○".into() }),
                    ("▪ 方形", Bullet::Char { char: "▪".into() }),
                    ("– 短横线", Bullet::Char { char: "–".into() }),
                ]
            };
            for (index, (label, bullet)) in choices.into_iter().enumerate() {
                let view = view.clone();
                let popup = popup.clone();
                list = list.child(
                    Button::new(("list-choice", index))
                        .ghost()
                        .xsmall()
                        .label(label)
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
                            let _ = view.update(cx, |s, cx| {
                                s.format_paras(|p| p.bullet = Some(bullet.clone()), w, cx)
                            });
                        }),
                );
            }
            list
        })
    }
    pub(super) fn line_spacing_picker(
        &self,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().downgrade();
        Popover::new("line-spacing-popup")
            .trigger(
                Button::new("line-spacing")
                    .ghost()
                    .xsmall()
                    .label("行距⌄")
                    .h(px(25.))
                    .accessibility_label("行距")
                    .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut list = div().flex().flex_col().w(px(155.)).gap_1();
                for (index, n) in [1., 1.15, 1.5, 2., 2.5, 3.].into_iter().enumerate() {
                    let view = view.clone();
                    let popup = popup.clone();
                    list = list.child(
                        Button::new(("line-spacing-choice", index))
                            .ghost()
                            .xsmall()
                            .label(format!("{n} 倍行距"))
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| {
                                    s.format_paras(
                                        |p| p.line_spacing = Some(Spacing::Pct(n)),
                                        w,
                                        cx,
                                    )
                                });
                            }),
                    );
                }
                let view = view.clone();
                list.child(
                    Button::new("paragraph-options")
                        .ghost()
                        .xsmall()
                        .label("段落设置…")
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
                            let _ = view.update(cx, |s, cx| s.paragraph_dialog(w, cx));
                        }),
                )
            })
    }
    pub(super) fn paragraph_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let p = self.formatting().para;
        let show = |v: Option<f64>| v.map(|n| n.to_string()).unwrap_or_default();
        self.fields_dialog(
            "段落（留空保持原值）",
            vec![
                ("左缩进（磅）", show(p.margin_left)),
                ("右缩进（磅）", show(p.margin_right)),
                ("首行缩进（磅，负值为悬挂）", show(p.indent)),
                ("段前间距（磅）", display_spacing(p.space_before, false)),
                ("段后间距（磅）", display_spacing(p.space_after, false)),
                (
                    "行距（倍数或磅，如 1.5 或 24pt）",
                    display_spacing(p.line_spacing, true),
                ),
            ],
            w,
            cx,
            |s, v, w, cx| {
                let left = optional_number(&v[0], 0., 4000.)?;
                let right = optional_number(&v[1], 0., 4000.)?;
                let indent = optional_number(&v[2], -4000., 4000.)?;
                let before = parse_spacing(&v[3], false)?;
                let after = parse_spacing(&v[4], false)?;
                let line = parse_spacing(&v[5], true)?;
                s.format_paras(
                    |p| {
                        if let Some(n) = left {
                            p.margin_left = Some(n);
                        }
                        if let Some(n) = right {
                            p.margin_right = Some(n);
                        }
                        if let Some(n) = indent {
                            p.indent = Some(n);
                        }
                        if let Some(n) = before {
                            p.space_before = Some(n);
                        }
                        if let Some(n) = after {
                            p.space_after = Some(n);
                        }
                        if let Some(n) = line {
                            p.line_spacing = Some(n);
                        }
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
}
