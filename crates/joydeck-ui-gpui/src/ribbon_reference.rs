//! Ribbon layouts for presentation features without a desktop authoring command.
use super::*;
use crate::l10n::{t, tf};
use crate::viewer::chrome::{RIBBON_HEIGHT, col, group, row, small, tool};
use gpui_kit::component::{Icon, Sizable, button::ButtonVariants, popover::Popover};
use joyoffice_ui::{IconName as I, UiLocale};

pub(super) const BLUE: u32 = 0x326797;
pub(super) const GREEN: u32 = 0x25805d;

pub(super) fn unavailable(
    locale: UiLocale,
    id: &'static str,
    icon: I,
    label: &'static str,
) -> Button {
    let flat = label.replace('\n', " ");
    tool(id, icon, label, 0x969ca4)
        .accessibility_label(tf(
            locale,
            "common.unsupported_suffix",
            &[("label", flat.trim())],
        ))
        .disabled(true)
}

pub(super) fn unavailable_row(locale: UiLocale, items: &[(&'static str, I, &'static str)]) -> Div {
    row().children(
        items
            .iter()
            .map(|(id, icon, label)| unavailable(locale, id, *icon, label)),
    )
}

pub(super) fn gallery(p: joyoffice_ui::Palette) -> Div {
    row()
        .h(px(58.))
        .gap(px(2.))
        .px_1()
        .border_1()
        .border_color(rgb(p.border))
        .bg(rgb(p.surface))
}

fn effect_tile(
    locale: UiLocale,
    id: &'static str,
    icon: I,
    label: &'static str,
    tint: u32,
    selected: bool,
    p: joyoffice_ui::Palette,
) -> Button {
    Button::new(id)
        .ghost()
        .w(px(72.))
        .h(px(54.))
        .p_0()
        .rounded(px(2.))
        .when(selected, |b| b.border_1().border_color(rgb(p.accent)))
        .accessibility_label(if selected {
            tf(
                locale,
                "common.current_readonly_suffix",
                &[("label", label)],
            )
        } else {
            tf(locale, "common.unsupported_suffix", &[("label", label)])
        })
        .disabled(true)
        .child(
            col()
                .items_center()
                .gap(px(1.))
                .child(Icon::new(icon).size(px(32.)).text_color(rgb(tint)))
                .child(div().text_size(px(9.)).child(label)),
        )
}

fn effects(
    locale: UiLocale,
    p: joyoffice_ui::Palette,
    items: &[(&'static str, I, &'static str)],
    tint: u32,
    current: &str,
) -> Div {
    gallery(p).children(
        items.iter().map(|(id, icon, label)| {
            effect_tile(locale, id, *icon, label, tint, *id == current, p)
        }),
    )
}

pub(super) fn ribbon_checks(locale: UiLocale, items: &[(&'static str, bool)]) -> Div {
    col()
        .gap(px(2.))
        .children(items.iter().map(|(label, checked)| {
            Button::new(*label)
                .ghost()
                .xsmall()
                .h(px(20.))
                .p_0()
                .justify_start()
                .icon(if *checked { I::SquareCheck } else { I::Square })
                .label(*label)
                .accessibility_label(if *checked {
                    tf(locale, "common.checked_readonly", &[("label", label)])
                } else {
                    tf(locale, "common.unchecked_readonly", &[("label", label)])
                })
                .disabled(true)
        }))
}

impl Viewer {
    pub(super) fn current_slide(&self) -> Option<&joydeck_model::Slide> {
        self.editor
            .as_ref()?
            .session
            .model()
            .slides
            .get(self.page)
            .map(AsRef::as_ref)
    }

    #[inline(never)]
    pub(super) fn ribbon_draw(&self, _: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let pens = gallery(self.palette()).children(
            [
                (
                    "ink-black",
                    I::PenLine,
                    t(locale, "draw.pen_black"),
                    0x53575d,
                ),
                ("ink-red", I::PenLine, t(locale, "draw.pen_red"), 0xc77d7d),
                ("ink-pencil", I::Pencil, t(locale, "draw.pencil"), 0x8b9ba5),
                (
                    "ink-highlight",
                    I::Highlighter,
                    t(locale, "draw.highlighter"),
                    0xada051,
                ),
            ]
            .into_iter()
            .map(|(id, icon, label, tint)| {
                effect_tile(locale, id, icon, label, tint, false, self.palette())
            }),
        );
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "draw.group.tools"),
                unavailable_row(
                    locale,
                    &[
                        ("ink-draw", I::PenLine, t(locale, "draw.draw")),
                        ("ink-erase", I::Eraser, t(locale, "draw.eraser")),
                        ("ink-lasso", I::Lasso, t(locale, "draw.lasso")),
                    ],
                ),
            ))
            .child(group(
                t(locale, "draw.group.pens"),
                row().child(pens).child(unavailable(
                    locale,
                    "ink-add",
                    I::Plus,
                    t(locale, "draw.add"),
                )),
            ))
    }

    #[inline(never)]
    pub(super) fn ribbon_transitions(&self, cx: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let p = self.palette();
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let transition = self.current_slide().and_then(|s| s.transition.as_ref());
        let current = transition.map_or("none", |t| t.kind.as_str());
        let current_opt = transition.map_or("", |t| t.option.as_str());
        let on_click = transition.is_none_or(|t| t.advance_on_click);
        let advance = transition.and_then(|t| t.advance_after_ms);
        let duration_label = transition.map_or("—".into(), |tr| {
            let secs = format!("{:.2}", f64::from(tr.duration_ms) / 1000.);
            tf(locale, "trans.duration_sec", &[("seconds", &secs)])
        });
        let after_label = advance.map_or_else(
            || t(locale, "trans.auto_advance_off").to_owned(),
            |ms| {
                let secs = format!("{:.1}", f64::from(ms) / 1000.);
                tf(locale, "trans.auto_advance_sec", &[("seconds", &secs)])
            },
        );
        let primary: &[(&'static str, I, &'static str)] = &[
            ("none", I::PanelTop, t(locale, "trans.none")),
            ("morph", I::Layers2, t(locale, "trans.morph")),
            ("fade", I::Blend, t(locale, "trans.fade")),
            ("push", I::ArrowUpToLine, t(locale, "trans.push")),
            ("wipe", I::PanelLeft, t(locale, "trans.wipe")),
            ("split", I::Columns2, t(locale, "trans.split")),
            (
                "reveal",
                I::SquareSplitHorizontal,
                t(locale, "trans.reveal"),
            ),
            ("cut", I::Layers, t(locale, "trans.cut")),
            (
                "randomBar",
                I::ChartNoAxesColumn,
                t(locale, "trans.random_bar"),
            ),
            ("shape", I::Shapes, t(locale, "trans.shape")),
            ("uncover", I::ArrowLeftToLine, t(locale, "trans.uncover")),
        ];
        let extra: Vec<(&'static str, &'static str)> = vec![
            ("cover", t(locale, "trans.cover")),
            ("flash", t(locale, "trans.flash")),
            ("dissolve", t(locale, "trans.dissolve")),
            ("blinds", t(locale, "trans.blinds")),
            ("clock", t(locale, "trans.clock")),
            ("checker", t(locale, "trans.checker")),
            ("zoom", t(locale, "trans.zoom")),
            ("cube", t(locale, "trans.cube")),
            ("flip", t(locale, "trans.flip")),
            ("orbit", t(locale, "trans.orbit")),
            ("wheel", t(locale, "trans.wheel")),
            ("comb", t(locale, "trans.comb")),
            ("curtains", t(locale, "trans.curtains")),
            ("doors", t(locale, "trans.doors")),
            ("origami", t(locale, "trans.origami")),
            ("vortex", t(locale, "trans.vortex")),
        ];
        let tiles =
            gallery(p).children(primary.iter().enumerate().map(|(i, (id, icon, label))| {
                let selected = *id == current;
                let effect_id = *id;
                Button::new(("trans-tile", i))
                    .ghost()
                    .w(px(64.))
                    .h(px(54.))
                    .p_0()
                    .rounded(px(2.))
                    .disabled(disabled)
                    .when(selected, |b| b.border_1().border_color(rgb(p.accent)))
                    .accessibility_label(tf(locale, "trans.aria_effect", &[("label", label)]))
                    .child(
                        col()
                            .items_center()
                            .gap(px(1.))
                            .child(Icon::new(*icon).size(px(28.)).text_color(rgb(0x698eb4)))
                            .child(div().text_size(px(9.)).child(*label)),
                    )
                    .on_click(cx.listener(move |s, _, w, cx| {
                        s.set_transition_effect(effect_id, w, cx);
                    }))
            }));

        let more_view = cx.entity().downgrade();
        let current_owned = current.to_string();
        let more_btn = Popover::new("transition-more")
            .trigger(
                tool(
                    "transition-more-btn",
                    I::Sparkles,
                    t(locale, "common.more"),
                    0x698eb4,
                )
                .dropdown_caret(true)
                .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut left_col = col().gap_1();
                let mut right_col = col().gap_1();
                for (idx, (id, label)) in extra.iter().enumerate() {
                    let view = more_view.clone();
                    let popup = popup.clone();
                    let active = current_owned == *id;
                    let effect_id = *id;
                    let btn = Button::new(("trans-extra", idx))
                        .ghost()
                        .xsmall()
                        .h(px(22.))
                        .label(if active {
                            format!("✓ {label}")
                        } else {
                            (*label).into()
                        })
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = view.update(cx, |s, cx| {
                                s.set_transition_effect(effect_id, w, cx);
                            });
                        });
                    if idx % 2 == 0 {
                        left_col = left_col.child(btn);
                    } else {
                        right_col = right_col.child(btn);
                    }
                }
                row().gap_2().child(left_col).child(right_col)
            });

        let options: &'static [&'static str] = joydeck_model::anim::TRANSITIONS
            .iter()
            .find(|(id, _, _, _, _)| *id == current)
            .map_or(&[], |(_, _, _, _, opts)| *opts);
        let opt_view = cx.entity().downgrade();
        let current_for_opt = current.to_string();
        let current_opt_owned = current_opt.to_string();
        let options_btn = Popover::new("transition-options-pop")
            .trigger(
                tool(
                    "transition-options",
                    I::SlidersHorizontal,
                    t(locale, "trans.effect_options"),
                    BLUE,
                )
                .dropdown_caret(true)
                .disabled(disabled || options.is_empty()),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                col()
                    .gap_1()
                    .children(options.iter().enumerate().map(|(i, opt)| {
                        let view = opt_view.clone();
                        let popup = popup.clone();
                        let opt_str = *opt;
                        let active = current_opt_owned == opt_str;
                        let text =
                            joydeck_model::anim::transition_option_label(&current_for_opt, opt_str);
                        Button::new(("trans-opt", i))
                            .ghost()
                            .xsmall()
                            .h(px(22.))
                            .label(if active { format!("✓ {text}") } else { text })
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| {
                                    s.set_transition_option(opt_str, w, cx);
                                });
                            })
                    }))
            });

        let dur_view = cx.entity().downgrade();
        let duration_btn = Popover::new("transition-duration-pop")
            .trigger(
                Button::new("transition-duration")
                    .ghost()
                    .xsmall()
                    .h(px(23.))
                    .label(tf(
                        locale,
                        "trans.duration_label",
                        &[("duration", &duration_label)],
                    ))
                    .disabled(disabled || current == "none"),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let custom_view = dur_view.clone();
                let custom_popup = popup.clone();
                col()
                    .gap_1()
                    .children(
                        [
                            (300u32, "0.30"),
                            (500, "0.50"),
                            (700, "0.70"),
                            (1000, "1.00"),
                            (1500, "1.50"),
                            (2000, "2.00"),
                            (3000, "3.00"),
                        ]
                        .into_iter()
                        .map(|(ms, secs)| {
                            let view = dur_view.clone();
                            let popup = popup.clone();
                            Button::new(("trans-dur", ms))
                                .ghost()
                                .xsmall()
                                .h(px(22.))
                                .label(tf(locale, "trans.duration_sec", &[("seconds", secs)]))
                                .on_click(move |_, w, cx| {
                                    let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = view.update(cx, |s, cx| {
                                        s.set_transition_duration(ms, w, cx);
                                    });
                                })
                        }),
                    )
                    .child(
                        small(
                            "trans-dur-custom",
                            I::Timer,
                            t(locale, "trans.duration_custom"),
                        )
                        .on_click(move |_, w, cx| {
                            let _ = custom_popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = custom_view.update(cx, |s, cx| {
                                s.content_form(content_ui::Form::TransitionDuration, w, cx);
                            });
                        }),
                    )
            });

        let after_view = cx.entity().downgrade();
        let after_btn = Popover::new("transition-after-pop")
            .trigger(
                Button::new("transition-after")
                    .ghost()
                    .xsmall()
                    .h(px(23.))
                    .label(after_label)
                    .disabled(disabled),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                col().gap_1().children(
                    [
                        (None, t(locale, "trans.auto_advance_disable").to_owned()),
                        (
                            Some(2000u32),
                            tf(locale, "trans.auto_advance_after", &[("seconds", "2")]),
                        ),
                        (
                            Some(3000),
                            tf(locale, "trans.auto_advance_after", &[("seconds", "3")]),
                        ),
                        (
                            Some(5000),
                            tf(locale, "trans.auto_advance_after", &[("seconds", "5")]),
                        ),
                        (
                            Some(10000),
                            tf(locale, "trans.auto_advance_after", &[("seconds", "10")]),
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(idx, (ms, text))| {
                        let view = after_view.clone();
                        let popup = popup.clone();
                        Button::new(("trans-after-opt", idx))
                            .ghost()
                            .xsmall()
                            .h(px(22.))
                            .label(text)
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = view.update(cx, |s, cx| {
                                    s.set_transition_after(ms, w, cx);
                                });
                            })
                    }),
                )
            });

        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "trans.group.preview"),
                tool(
                    "transition-preview",
                    I::MonitorPlay,
                    t(locale, "trans.preview"),
                    GREEN,
                )
                .disabled(disabled)
                .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
            ))
            .child(group(
                t(locale, "trans.group.style"),
                row().child(tiles).child(more_btn).child(options_btn),
            ))
            .child(group(
                t(locale, "trans.group.timing"),
                row()
                    .gap_2()
                    .child(
                        col().child(duration_btn).child(
                            Button::new("transition-on-click")
                                .ghost()
                                .xsmall()
                                .h(px(23.))
                                .disabled(disabled)
                                .label(if on_click {
                                    t(locale, "trans.on_click_checked")
                                } else {
                                    t(locale, "trans.on_click_unchecked")
                                })
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.toggle_transition_on_click(w, cx);
                                })),
                        ),
                    )
                    .child(col().child(after_btn))
                    .child(
                        tool(
                            "transition-apply-all",
                            I::Layers,
                            t(locale, "trans.apply_all"),
                            BLUE,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.apply_transition_all(w, cx))),
                    ),
            ))
    }

    #[inline(never)]
    pub(super) fn ribbon_animations(&self, _: &mut Context<Self>) -> Div {
        let locale = self.locale;
        let entrance = effects(
            locale,
            self.palette(),
            &[
                ("anim-appear", I::Star, t(locale, "anim.appear")),
                ("anim-fly-in", I::ArrowUp, t(locale, "anim.fly_in")),
                ("anim-blinds", I::TextAlignJustify, t(locale, "anim.blinds")),
                ("anim-checker", I::Grid2x2, t(locale, "anim.checker")),
                ("anim-dissolve", I::Sparkles, t(locale, "anim.dissolve_in")),
            ],
            0x73927b,
            "",
        );
        let emphasis = effects(
            locale,
            self.palette(),
            &[
                ("anim-fill", I::PaintBucket, t(locale, "anim.fill_color")),
                ("anim-scale", I::MoveDiagonal, t(locale, "anim.grow_shrink")),
                ("anim-line-color", I::PenLine, t(locale, "anim.line_color")),
                ("anim-spin", I::RotateCw, t(locale, "anim.spin")),
                ("anim-opacity", I::Blend, t(locale, "anim.transparency")),
            ],
            0xa08a58,
            "",
        );
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "anim.group.preview"),
                unavailable(
                    locale,
                    "animation-preview",
                    I::MonitorPlay,
                    t(locale, "trans.preview"),
                ),
            ))
            .child(group(t(locale, "anim.group.entrance"), entrance))
            .child(group(t(locale, "anim.group.emphasis"), emphasis))
            .child(group(
                t(locale, "anim.group.other"),
                unavailable_row(
                    locale,
                    &[
                        ("anim-exit", I::Star, t(locale, "anim.exit")),
                        ("anim-path", I::Spline, t(locale, "anim.path")),
                    ],
                ),
            ))
            .child(group(
                t(locale, "anim.group.advanced"),
                unavailable_row(
                    locale,
                    &[
                        (
                            "anim-options",
                            I::SlidersHorizontal,
                            t(locale, "anim.options"),
                        ),
                        ("anim-pane", I::PanelRight, t(locale, "anim.pane")),
                        ("anim-trigger", I::Zap, t(locale, "anim.trigger")),
                        ("anim-painter", I::Paintbrush, t(locale, "anim.painter")),
                    ],
                ),
            ))
            .child(group(
                t(locale, "anim.group.timing"),
                col()
                    .child(small_unavailable(
                        locale,
                        "anim-start",
                        I::Play,
                        t(locale, "anim.start"),
                    ))
                    .child(small_unavailable(
                        locale,
                        "anim-duration",
                        I::Timer,
                        t(locale, "anim.duration"),
                    )),
            ))
    }

    #[inline(never)]
    pub(super) fn ribbon_record(&self, _: &mut Context<Self>) -> Div {
        let locale = self.locale;
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                t(locale, "record.group.camera"),
                unavailable(locale, "record-cameo", I::Camera, "Cameo"),
            ))
            .child(group(
                t(locale, "record.group.record"),
                unavailable_row(
                    locale,
                    &[
                        (
                            "record-start",
                            I::Presentation,
                            t(locale, "record.from_beginning"),
                        ),
                        (
                            "record-current",
                            I::MonitorPlay,
                            t(locale, "record.from_current"),
                        ),
                    ],
                ),
            ))
            .child(group(
                t(locale, "record.group.manage"),
                unavailable_row(
                    locale,
                    &[
                        ("record-clear", I::Film, t(locale, "record.clear")),
                        ("record-reset", I::RotateCw, t(locale, "record.reset_cameo")),
                    ],
                ),
            ))
    }
}

pub(super) fn small_unavailable(
    locale: UiLocale,
    id: &'static str,
    icon: I,
    label: &'static str,
) -> Button {
    chrome::small(id, icon, label)
        .accessibility_label(tf(locale, "common.unsupported_suffix", &[("label", label)]))
        .disabled(true)
}
