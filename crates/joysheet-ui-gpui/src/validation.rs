use crate::desktop::{After, Desktop};
use gpui_kit::{component::button::Button, prelude::*, *};
pub struct ValidationPrompt {
    pub after: After,
    pub message: String,
    pub revision: u64,
}
impl Desktop {
    pub fn accept_validation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.validation_prompt.take() else { return };
        if self.view().is_none_or(|v| v.revision != p.revision) {
            self.alert("工作簿已更改，请重新确认 / Workbook changed; review again");
            cx.notify();
            return;
        }
        self.validation_accepted = true;
        self.commit(p.after, window, cx);
        self.validation_accepted = false;
    }
    pub fn validation_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let message = self.validation_prompt.as_ref().map(|p| p.message.clone()).unwrap_or_default();
        div()
            .id("validation-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .role(Role::Dialog)
            .aria_label("Data validation confirmation")
            .track_focus(&self.trust_focus)
            .bg(rgba(0x10231a77))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(520.))
                    .p_5()
                    .bg(rgb(0xffffff))
                    .text_color(rgb(0x20332a))
                    .rounded_lg()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child("输入不符合数据验证，仍要使用？ / Keep this value?")
                    .child(message)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_3()
                            .child(
                                Button::new("validation-cancel")
                                    .label("返回编辑 / Edit")
                                    .relative()
                                    .child(self.advanced_probe("validation-cancel".into(), cx))
                                    .on_click(cx.listener(|this, _, w, cx| {
                                        this.validation_prompt = None;
                                        this.focus_grid(w, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("validation-accept")
                                    .label("仍然使用 / Keep")
                                    .relative()
                                    .child(self.advanced_probe("validation-accept".into(), cx))
                                    .on_click(cx.listener(|this, _, w, cx| this.accept_validation(w, cx))),
                            ),
                    ),
            )
    }
}
