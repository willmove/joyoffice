use super::*;
use gpui::component::WindowExt;
use serde_json::{Value, json};
use std::rc::Rc;

impl JoyWord {
    pub(super) fn agent_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.agent.enabled();
        let tr = |zh, en| self.shell.tr(zh, en);
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                Button::new("agent-toggle")
                    .ghost()
                    .xsmall()
                    .label(if enabled {
                        tr("关闭本地提案", "Disable local proposals")
                    } else {
                        tr("启用本地提案", "Enable local proposals")
                    })
                    .on_click(cx.listener(move |v, _, _, cx| {
                        if let Err(error) = v.agent.set_enabled(!enabled) {
                            v.shell.notice = Some(error);
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new("agent-draft")
                    .ghost()
                    .xsmall()
                    .label(tr("审阅替换提案…", "Review replacement…"))
                    .disabled(!enabled || !self.format_ready("text.insert"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_agent_draft(w, cx))),
            )
    }

    fn open_agent_draft(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let tr = |zh, en| self.shell.tr(zh, en);
        self.fields_action(
            tr("本地替换提案", "Local replacement draft"),
            "text.insert",
            vec![(
                tr("替换文本（留空表示删除）", "Replacement (blank to delete)"),
                String::new(),
            )],
            false,
            |values| Ok(json!({"replacement":values.first()})),
            |v, args, w, cx| {
                let (pending, preview) = v.agent.propose(
                    v.canvas.reader.as_mut().ok_or("No document")?,
                    args["replacement"].as_str().unwrap_or_default(),
                )?;
                let pending = Rc::new(pending);
                let before = preview.before().to_owned();
                let after = preview.after().to_owned();
                cx.defer_in(w, move |v, w, cx| {
                    let owner = cx.weak_entity();
                    let tr = |zh, en| v.shell.tr(zh, en);
                    let title = tr("确认修订", "Confirm tracked replacement");
                    let original = tr("原文", "Original");
                    let replacement = tr("替换文本", "Replacement");
                    let accept = tr("接受修订", "Accept tracked replacement");
                    let cancel = tr("取消", "Cancel");
                    w.open_dialog(cx, move |dialog, _, _| {
                        let owner = owner.clone();
                        let pending = pending.clone();
                        dialog
                            .title(title)
                            .width(px(560.))
                            .child(
                                div()
                                    .id("agent-diff")
                                    .max_h(px(320.))
                                    .overflow_y_scroll()
                                    .child(div().child(original))
                                    .child(div().child(before.clone()))
                                    .child(div().child(replacement))
                                    .child(div().child(after.clone())),
                            )
                            .child(
                                Button::new("agent-cancel")
                                    .label(cancel)
                                    .on_click(|_, w, cx| w.close_dialog(cx)),
                            )
                            .child(
                                Button::new("agent-accept")
                                    .primary()
                                    .label(accept)
                                    .on_click(move |_, w, cx| {
                                        w.close_dialog(cx);
                                        let _ = owner.update(cx, |v, cx| {
                                            let agent = std::mem::take(&mut v.agent);
                                            v.apply_input(w, cx, |r| agent.accept(r, &pending));
                                            v.agent = agent;
                                            w.focus(&v.focus, cx);
                                        });
                                    }),
                            )
                    });
                });
                Ok(Value::Null)
            },
            w,
            cx,
        );
    }
}
