//! Ribbon layouts for presentation features without a desktop authoring command.
use super::*;
use crate::viewer::chrome::{RIBBON_HEIGHT, col, group, row, small, tool};
use gpui_kit::component::{Icon, Sizable, button::ButtonVariants, popover::Popover};
use joyoffice_ui::IconName as I;

pub(super) const BLUE: u32 = 0x326797;
pub(super) const GREEN: u32 = 0x25805d;

pub(super) fn unavailable(id: &'static str, icon: I, label: &'static str) -> Button {
    tool(id, icon, label, 0x969ca4)
        .accessibility_label(format!("{}（暂不支持）", label.replace('\n', "")))
        .disabled(true)
}

pub(super) fn unavailable_row(items: &[(&'static str, I, &'static str)]) -> Div {
    row().children(
        items
            .iter()
            .map(|(id, icon, label)| unavailable(id, *icon, label)),
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
            format!("{label}（当前效果，暂不支持编辑）")
        } else {
            format!("{label}（暂不支持）")
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
    p: joyoffice_ui::Palette,
    items: &[(&'static str, I, &'static str)],
    tint: u32,
    current: &str,
) -> Div {
    gallery(p).children(
        items
            .iter()
            .map(|(id, icon, label)| effect_tile(id, *icon, label, tint, *id == current, p)),
    )
}

pub(super) fn ribbon_checks(items: &[(&'static str, bool)]) -> Div {
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
                .accessibility_label(format!(
                    "{label}（{}，暂不支持编辑）",
                    if *checked { "已选中" } else { "未选中" }
                ))
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
        let pens = gallery(self.palette()).children(
            [
                ("ink-black", I::PenLine, "黑色笔", 0x53575d),
                ("ink-red", I::PenLine, "红色笔", 0xc77d7d),
                ("ink-pencil", I::Pencil, "铅笔", 0x8b9ba5),
                ("ink-highlight", I::Highlighter, "荧光笔", 0xada051),
            ]
            .into_iter()
            .map(|(id, icon, label, tint)| {
                effect_tile(id, icon, label, tint, false, self.palette())
            }),
        );
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "绘图工具",
                unavailable_row(&[
                    ("ink-draw", I::PenLine, "绘图"),
                    ("ink-erase", I::Eraser, "橡皮擦"),
                    ("ink-lasso", I::Lasso, "套索\n选择"),
                ]),
            ))
            .child(group(
                "笔",
                row()
                    .child(pens)
                    .child(unavailable("ink-add", I::Plus, "添加")),
            ))
    }

    #[inline(never)]
    pub(super) fn ribbon_transitions(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let disabled = self.editor.is_none() || self.loading || self.composing();
        let transition = self.current_slide().and_then(|s| s.transition.as_ref());
        let current = transition.map_or("none", |t| t.kind.as_str());
        let current_opt = transition.map_or("", |t| t.option.as_str());
        let on_click = transition.is_none_or(|t| t.advance_on_click);
        let advance = transition.and_then(|t| t.advance_after_ms);
        let duration_label = transition.map_or("—".into(), |t| {
            format!("{:.2} 秒", f64::from(t.duration_ms) / 1000.)
        });
        let after_label = advance.map_or("自动换片：关".into(), |ms| {
            format!("自动换片：{:.1} 秒", f64::from(ms) / 1000.)
        });
        let primary: &[(&'static str, I, &'static str)] = &[
            ("none", I::PanelTop, "无"),
            ("morph", I::Layers2, "平滑"),
            ("fade", I::Blend, "淡入/淡出"),
            ("push", I::ArrowUpToLine, "推入"),
            ("wipe", I::PanelLeft, "擦除"),
            ("split", I::Columns2, "分割"),
            ("reveal", I::SquareSplitHorizontal, "显示"),
            ("cut", I::Layers, "切入"),
            ("randomBar", I::ChartNoAxesColumn, "随机线条"),
            ("shape", I::Shapes, "形状"),
            ("uncover", I::ArrowLeftToLine, "揭开"),
        ];
        let extra: &[(&'static str, &'static str)] = &[
            ("cover", "覆盖"),
            ("flash", "闪光"),
            ("dissolve", "溶解"),
            ("blinds", "百叶窗"),
            ("clock", "时钟"),
            ("checker", "棋盘"),
            ("zoom", "缩放"),
            ("cube", "立方体"),
            ("flip", "翻转"),
            ("orbit", "轨道旋转"),
            ("wheel", "轮子"),
            ("comb", "梳理"),
            ("curtains", "帘式"),
            ("doors", "门"),
            ("origami", "折纸"),
            ("vortex", "漩涡"),
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
                    .accessibility_label(format!("切换效果：{label}"))
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
                tool("transition-more-btn", I::Sparkles, "更多", 0x698eb4)
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
                tool("transition-options", I::SlidersHorizontal, "效果选项", BLUE)
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
                    .label(format!("持续时间  {duration_label}"))
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
                            (300u32, "0.30 秒"),
                            (500, "0.50 秒"),
                            (700, "0.70 秒"),
                            (1000, "1.00 秒"),
                            (1500, "1.50 秒"),
                            (2000, "2.00 秒"),
                            (3000, "3.00 秒"),
                        ]
                        .into_iter()
                        .map(|(ms, text)| {
                            let view = dur_view.clone();
                            let popup = popup.clone();
                            Button::new(("trans-dur", ms))
                                .ghost()
                                .xsmall()
                                .h(px(22.))
                                .label(text)
                                .on_click(move |_, w, cx| {
                                    let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = view.update(cx, |s, cx| {
                                        s.set_transition_duration(ms, w, cx);
                                    });
                                })
                        }),
                    )
                    .child(small("trans-dur-custom", I::Timer, "自定义时长…").on_click(
                        move |_, w, cx| {
                            let _ = custom_popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = custom_view.update(cx, |s, cx| {
                                s.content_form(content_ui::Form::TransitionDuration, w, cx);
                            });
                        },
                    ))
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
                        (None, "关闭自动换片"),
                        (Some(2000u32), "2 秒后换片"),
                        (Some(3000), "3 秒后换片"),
                        (Some(5000), "5 秒后换片"),
                        (Some(10000), "10 秒后换片"),
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
                "预览",
                tool("transition-preview", I::MonitorPlay, "预览", GREEN)
                    .disabled(disabled)
                    .on_click(cx.listener(|s, _, _, cx| s.start_show(false, false, cx))),
            ))
            .child(group(
                "切换样式",
                row().child(tiles).child(more_btn).child(options_btn),
            ))
            .child(group(
                "计时",
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
                                    "☑ 单击鼠标时"
                                } else {
                                    "☐ 单击鼠标时"
                                })
                                .on_click(cx.listener(|s, _, w, cx| {
                                    s.toggle_transition_on_click(w, cx);
                                })),
                        ),
                    )
                    .child(col().child(after_btn))
                    .child(
                        tool("transition-apply-all", I::Layers, "应用到\n所有", BLUE)
                            .disabled(disabled)
                            .on_click(cx.listener(|s, _, w, cx| s.apply_transition_all(w, cx))),
                    ),
            ))
    }

    #[inline(never)]
    pub(super) fn ribbon_animations(&self, _: &mut Context<Self>) -> Div {
        let entrance = effects(
            self.palette(),
            &[
                ("anim-appear", I::Star, "出现"),
                ("anim-fly-in", I::ArrowUp, "飞入"),
                ("anim-blinds", I::TextAlignJustify, "百叶窗"),
                ("anim-checker", I::Grid2x2, "棋盘"),
                ("anim-dissolve", I::Sparkles, "向内溶解"),
            ],
            0x73927b,
            "",
        );
        let emphasis = effects(
            self.palette(),
            &[
                ("anim-fill", I::PaintBucket, "填充颜色"),
                ("anim-scale", I::MoveDiagonal, "放大/缩小"),
                ("anim-line-color", I::PenLine, "线条颜色"),
                ("anim-spin", I::RotateCw, "陀螺旋"),
                ("anim-opacity", I::Blend, "透明"),
            ],
            0xa08a58,
            "",
        );
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "预览",
                unavailable("animation-preview", I::MonitorPlay, "预览"),
            ))
            .child(group("进入效果", entrance))
            .child(group("强调效果", emphasis))
            .child(group(
                "其他效果",
                unavailable_row(&[
                    ("anim-exit", I::Star, "退出效果"),
                    ("anim-path", I::Spline, "路径动画"),
                ]),
            ))
            .child(group(
                "高级动画",
                unavailable_row(&[
                    ("anim-options", I::SlidersHorizontal, "效果选项"),
                    ("anim-pane", I::PanelRight, "动画\n窗格"),
                    ("anim-trigger", I::Zap, "触发器"),
                    ("anim-painter", I::Paintbrush, "动画刷"),
                ]),
            ))
            .child(group(
                "计时",
                col()
                    .child(small_unavailable("anim-start", I::Play, "开始"))
                    .child(small_unavailable("anim-duration", I::Timer, "持续时间")),
            ))
    }

    #[inline(never)]
    pub(super) fn ribbon_record(&self, _: &mut Context<Self>) -> Div {
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "摄像头",
                unavailable("record-cameo", I::Camera, "Cameo"),
            ))
            .child(group(
                "录制",
                unavailable_row(&[
                    ("record-start", I::Presentation, "从头\n开始"),
                    ("record-current", I::MonitorPlay, "从当前\n幻灯片开始"),
                ]),
            ))
            .child(group(
                "管理录制",
                unavailable_row(&[
                    ("record-clear", I::Film, "清除录制"),
                    ("record-reset", I::RotateCw, "重置为\nCameo"),
                ]),
            ))
    }
}

pub(super) fn small_unavailable(id: &'static str, icon: I, label: &'static str) -> Button {
    chrome::small(id, icon, label)
        .accessibility_label(format!("{label}（暂不支持）"))
        .disabled(true)
}
