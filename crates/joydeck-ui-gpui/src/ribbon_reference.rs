//! Ribbon layouts for presentation features without a desktop authoring command.
use super::*;
use crate::viewer::chrome::{RIBBON_HEIGHT, col, group, row, tool};
use gpui_kit::component::{Icon, Sizable, button::ButtonVariants};
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

pub(super) fn gallery(dark: bool) -> Div {
    let p = chrome::palette(dark);
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
    dark: bool,
) -> Button {
    let p = chrome::palette(dark);
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

fn effects(dark: bool, items: &[(&'static str, I, &'static str)], tint: u32, current: &str) -> Div {
    gallery(dark).children(
        items
            .iter()
            .map(|(id, icon, label)| effect_tile(id, *icon, label, tint, *id == current, dark)),
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
        let pens = gallery(self.dark).children(
            [
                ("ink-black", I::PenLine, "黑色笔", 0x53575d),
                ("ink-red", I::PenLine, "红色笔", 0xc77d7d),
                ("ink-pencil", I::Pencil, "铅笔", 0x8b9ba5),
                ("ink-highlight", I::Highlighter, "荧光笔", 0xada051),
            ]
            .into_iter()
            .map(|(id, icon, label, tint)| effect_tile(id, icon, label, tint, false, self.dark)),
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
    pub(super) fn ribbon_transitions(&self, _: &mut Context<Self>) -> Div {
        let transition = self.current_slide().and_then(|s| s.transition.as_ref());
        let current = transition.map_or("none", |t| t.kind.as_str());
        let tiles = effects(
            self.dark,
            &[
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
            ],
            0x698eb4,
            current,
        );
        let timing = |id, label: &str, value: String| {
            Button::new(id)
                .ghost()
                .xsmall()
                .h(px(23.))
                .label(format!("{label}    {value}"))
                .accessibility_label(format!("{label} {value}（暂不支持编辑）"))
                .disabled(true)
        };
        let duration = transition.map_or("—".into(), |t| {
            format!("{:.2} 秒", f64::from(t.duration_ms) / 1000.)
        });
        let advance = transition.and_then(|t| t.advance_after_ms);
        let sound = if transition.is_some_and(|t| t.sound.is_some() || t.stop_sound) {
            "已设置"
        } else {
            "无声音"
        };
        row()
            .h(px(RIBBON_HEIGHT))
            .child(group(
                "预览",
                unavailable("transition-preview", I::MonitorPlay, "预览"),
            ))
            .child(group(
                "切换样式",
                row().child(tiles).child(unavailable(
                    "transition-options",
                    I::SlidersHorizontal,
                    "效果选项",
                )),
            ))
            .child(group(
                "计时",
                row()
                    .gap_2()
                    .child(
                        col()
                            .child(timing("transition-duration", "持续时间", duration))
                            .child(timing("transition-sound", "声音", sound.into())),
                    )
                    .child(ribbon_checks(&[
                        ("单击鼠标时", transition.is_none_or(|t| t.advance_on_click)),
                        ("自动换片间隔", advance.is_some()),
                    ]))
                    .child(unavailable(
                        "transition-apply-all",
                        I::Layers,
                        "应用到\n所有",
                    )),
            ))
    }

    #[inline(never)]
    pub(super) fn ribbon_animations(&self, _: &mut Context<Self>) -> Div {
        let entrance = effects(
            self.dark,
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
            self.dark,
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
