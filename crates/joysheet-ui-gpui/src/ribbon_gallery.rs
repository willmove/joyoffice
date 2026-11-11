//! The expanded Home style gallery used when the native window has room.
use crate::desktop::{After, Desktop};
use gpui_kit::{
    component::{
        Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};
use serde_json::json;

impl Desktop {
    pub(crate) fn ribbon_style_gallery(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = [
            [
                ("Normal", "常规", 0xffffff, 0x202020),
                ("Bad", "差", 0xffc7ce, 0x9c0006),
                ("Good", "好", 0xc6efce, 0x006100),
                ("Neutral", "适中", 0xffeb9c, 0x9c6500),
                ("Calculation", "计算", 0xf2f2f2, 0xc17300),
            ],
            [
                ("Input", "输入", 0xfff2cc, 0x202020),
                ("Output", "输出", 0xf2f2f2, 0x202020),
                ("Heading 1", "标题 1", 0xffffff, 0x1f4e78),
                ("Total", "汇总", 0xffffff, 0x202020),
                ("Accent1", "强调", 0xd9e2f3, 0x244062),
            ],
        ];
        div()
            .id("ribbon-style-gallery")
            .w(px(400.))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(3.))
            .children(rows.into_iter().map(|row| {
                div().flex().gap(px(3.)).children(row.into_iter().map(|(name, zh, bg, fg)| {
                    Button::new(ElementId::Name(format!("gallery-{name}").into()))
                        .ghost()
                        .xsmall()
                        .w(px(77.))
                        .h(px(24.))
                        .bg(rgb(if self.preferences.dark { self.palette().surface } else { bg }))
                        .text_color(rgb(if self.preferences.dark { self.palette().text } else { fg }))
                        .border(px(0.5))
                        .border_color(rgb(self.palette().border))
                        .justify_start()
                        .label(self.label(zh, name))
                        .accessibility_label(self.label(zh, name))
                        .tooltip(self.label(zh, name))
                        .child(self.advanced_probe(format!("ribbon-style:{name}"), cx))
                        .on_click(cx.listener(move |v, _, w, cx| {
                            v.commit(After::Command("home.cellStyle".into(), json!({"name":name})), w, cx);
                            v.focus_grid(w, cx);
                        }))
                }))
            }))
            .into_any_element()
    }
}
