//! Native paragraph choices; blank mixed fields preserve each target's value.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover};
use joydeck_model::text::{Bullet, Spacing};

pub(super) fn optional_number(
    locale: joyoffice_ui::UiLocale,
    value: &str,
    min: f64,
    max: f64,
) -> Result<Option<f64>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let n: f64 = value
        .trim()
        .parse()
        .map_err(|_| crate::l10n::t(locale, "para.err_number"))?;
    if !n.is_finite() || !(min..=max).contains(&n) {
        let min_s = min.to_string();
        let max_s = max.to_string();
        return Err(crate::l10n::tf(
            locale,
            "para.err_range",
            &[("min", &min_s), ("max", &max_s)],
        ));
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
fn parse_spacing(
    locale: joyoffice_ui::UiLocale,
    value: &str,
    line: bool,
) -> Result<Option<Spacing>, String> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return optional_number(locale, percent, 0., 400000.)
            .map(|n| n.map(|n| Spacing::Pct(n / 100.)));
    }
    if let Some(pts) = value.strip_suffix("pt") {
        return optional_number(locale, pts, if line { 1. } else { 0. }, 4000.)
            .map(|n| n.map(Spacing::Pts));
    }
    optional_number(
        locale,
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
        let locale = self.locale;
        let label = if numbered {
            self.t("para.numbering_style")
        } else {
            self.t("para.bullet_style")
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
            let t = |key: &'static str| crate::l10n::t(locale, key);
            let popup = cx.entity().downgrade();
            let mut list = div().flex().flex_col().w(px(175.)).gap_1();
            let choices = if numbered {
                vec![
                    (t("common.none"), Bullet::None),
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
                    (t("common.none"), Bullet::None),
                    (
                        t("para.bullet_filled_circle"),
                        Bullet::Char { char: "•".into() },
                    ),
                    (
                        t("para.bullet_hollow_circle"),
                        Bullet::Char { char: "○".into() },
                    ),
                    (t("para.bullet_square"), Bullet::Char { char: "▪".into() }),
                    (t("para.bullet_dash"), Bullet::Char { char: "–".into() }),
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
        let locale = self.locale;
        Popover::new("line-spacing-popup")
            .trigger(
                Button::new("line-spacing")
                    .ghost()
                    .xsmall()
                    .label(self.t("para.line_spacing_btn"))
                    .h(px(25.))
                    .accessibility_label(self.t("para.line_spacing_aria"))
                    .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut list = div().flex().flex_col().w(px(155.)).gap_1();
                for (index, n) in [1., 1.15, 1.5, 2., 2.5, 3.].into_iter().enumerate() {
                    let view = view.clone();
                    let popup = popup.clone();
                    let n_s = n.to_string();
                    list = list.child(
                        Button::new(("line-spacing-choice", index))
                            .ghost()
                            .xsmall()
                            .label(crate::l10n::tf(
                                locale,
                                "para.line_spacing_multiple",
                                &[("n", &n_s)],
                            ))
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
                        .label(crate::l10n::t(locale, "para.settings_item"))
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
            self.t("para.dialog_title"),
            vec![
                (self.t("para.margin_left"), show(p.margin_left)),
                (self.t("para.margin_right"), show(p.margin_right)),
                (self.t("para.indent"), show(p.indent)),
                (
                    self.t("para.space_before"),
                    display_spacing(p.space_before, false),
                ),
                (
                    self.t("para.space_after"),
                    display_spacing(p.space_after, false),
                ),
                (
                    self.t("para.line_spacing_field"),
                    display_spacing(p.line_spacing, true),
                ),
            ],
            w,
            cx,
            |s, v, w, cx| {
                let locale = s.locale;
                let left = optional_number(locale, &v[0], 0., 4000.)?;
                let right = optional_number(locale, &v[1], 0., 4000.)?;
                let indent = optional_number(locale, &v[2], -4000., 4000.)?;
                let before = parse_spacing(locale, &v[3], false)?;
                let after = parse_spacing(locale, &v[4], false)?;
                let line = parse_spacing(locale, &v[5], true)?;
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
