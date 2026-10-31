use crate::desktop::{After, Desktop, Pending};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};
pub struct TrustRequest {
    pub doc: u64,
    pub revision: u64,
    pub name: String,
    pub run: bool,
}
impl Desktop {
    pub fn script_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut panel = div()
            .absolute()
            .right_0()
            .top(px(90.))
            .bottom_0()
            .w(px(420.))
            .p_3()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x20332a))
            .shadow_lg()
            .id("scripts-panel")
            .occlude()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().id("close-scripts").cursor_pointer().child("动作与脚本 / Scripts ×").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 0;
                cx.notify();
            })));
        let trusted = self.view().is_some_and(|v| v.scripts_trusted);
        let recording = self.view().is_some_and(|v| v.recording);
        panel = panel
            .child(if trusted { "此会话已信任 / Trusted this session" } else { "来自文件的脚本尚未信任 / Scripts disabled" })
            .child(
                div()
                    .id("record-script")
                    .p_2()
                    .bg(rgb(0xe3f0e7))
                    .cursor_pointer()
                    .child(if recording { "停止录制并命名…" } else { "开始录制动作" })
                    .on_click(cx.listener(|this, _, w, cx| {
                        if this.view().is_some_and(|v| v.recording) {
                            this.open_command("automate.recordActions", w, cx);
                        } else {
                            this.commit(After::Command("automate.recordActions".into(), json!({})), w, cx);
                        }
                    })),
            )
            .child(
                div()
                    .id("new-script")
                    .p_2()
                    .cursor_pointer()
                    .child("新建脚本…")
                    .on_click(cx.listener(|this, _, w, cx| this.open_command("automate.newScript", w, cx))),
            )
            .child(div().id("revoke-trust").p_2().cursor_pointer().child("撤销此会话信任").on_click(
                cx.listener(|this, _, w, cx| this.commit(After::Command("automate.trustScripts".into(), json!({"trusted":false})), w, cx)),
            ));
        if self.script_doc == self.view().map(|v| v.doc)
            && let Some(scripts) = self.script_bodies.as_object()
        {
            for (index, (name, steps)) in scripts.iter().enumerate() {
                let run_name = name.clone();
                let edit_name = name.clone();
                let body = serde_json::to_string_pretty(steps).unwrap_or_default();
                let del_name = name.clone();
                let mut row = div()
                    .p_2()
                    .border_1()
                    .border_color(rgb(0xd8e3db))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(format!("{name} · {} 步", steps.as_array().map_or(0, Vec::len)));
                for step in steps.as_array().into_iter().flatten().take(8) {
                    row = row.child(div().text_xs().child(format!(
                        "{} {}",
                        step["command"].as_str().unwrap_or("?"),
                        step["params"].to_string().chars().take(160).collect::<String>()
                    )));
                }
                row = row.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .id(("run-script", index))
                                .relative()
                                .cursor_pointer()
                                .child("运行 / Run")
                                .child(self.advanced_probe(format!("script-run:{name}"), cx))
                                .on_click(cx.listener(move |this, _, w, cx| this.run_stored_script(run_name.clone(), w, cx))),
                        )
                        .child(div().id(("edit-script", index)).cursor_pointer().child("查看/编辑全部步骤").on_click(cx.listener(
                            move |this, _, w, cx| this.open_prefilled("automate.newScript", json!({"name":edit_name,"commandsText":body}), w, cx),
                        )))
                        .child(div().id(("delete-script", index)).cursor_pointer().child("删除").on_click(cx.listener(move |this, _, w, cx| {
                            this.commit(After::Command("automate.deleteScript".into(), json!({"name":del_name})), w, cx)
                        }))),
                );
                if self.view().is_some_and(|v| v.workbook.vba_project.is_some()) {
                    panel = panel.child("包含 VBA 项目：仅保留，不执行 / VBA preserved, never executed");
                }
                panel = panel.child(row);
            }
        }
        panel = panel.child("运行队列 / Tasks（取消在命令之间生效）");
        for task in self.engine.tasks().iter().take(12) {
            let id = task.id;
            let active = matches!(task.state.as_str(), "queued" | "running" | "cancelling");
            let document = self
                .snapshot
                .windows
                .iter()
                .find(|v| Some(v.doc) == task.doc)
                .map(|v| v.title.clone())
                .unwrap_or_else(|| task.doc.map(|d| format!("文档 #{d}")).unwrap_or("—".into()));
            let state = if self.chinese() {
                match task.state.as_str() {
                    "queued" => "排队中",
                    "running" => "运行中",
                    "cancelling" => "正在停止",
                    "completed" => "已完成",
                    "cancelled" => "已取消",
                    _ => "失败",
                }
            } else {
                task.state.as_str()
            };
            let mut row = div()
                .p_2()
                .border_b_1()
                .border_color(rgb(0xe0e8e2))
                .flex()
                .flex_col()
                .gap_1()
                .child(format!("#{id} · {state} · {} 步 · {document}", task.completed))
                .child(task.message.clone());
            if active {
                row = row.child(
                    div()
                        .id(SharedString::from(format!("cancel-task-{id}")))
                        .relative()
                        .cursor_pointer()
                        .child("取消 / Cancel")
                        .child(self.advanced_probe(format!("cancel-task:{id}"), cx))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Err(e) = this.engine.cancel_task(id) {
                                this.alert(e);
                            }
                            cx.notify();
                        })),
                );
            }
            panel = panel.child(row);
        }
        panel
    }
    pub fn run_stored_script(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.view().is_some_and(|v| v.scripts_trusted) {
            self.commit(After::Command("automate.runScript".into(), json!({"name":name})), window, cx);
        } else {
            self.request_trust(name, true, window, cx);
        }
    }
    pub fn request_trust(&mut self, name: String, run: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = self.view() else { return };
        let (doc, revision, bodies) = (v.doc, v.revision, gridcraft_engine::cmd::extra::script_definitions(&v.workbook));
        self.script_bodies = bodies;
        self.script_doc = Some(doc);
        self.trust_request = Some(TrustRequest { doc, revision, name, run });
        self.palette_open = false;
        window.focus(&self.trust_focus, cx);
        cx.notify();
    }
    pub fn confirm_script_trust(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(request) = self.trust_request.take() else { return };
        if !self.view().is_some_and(|v| v.doc == request.doc && v.revision == request.revision) {
            self.alert("工作簿已更改，请重新审核脚本 / Workbook changed; review again");
            cx.notify();
            return;
        }
        let after = if request.run { After::Command("automate.runScript".into(), json!({"name":request.name})) } else { After::None };
        match self.engine.submit_guarded("automate.trustScripts", json!({"trusted":true}), self.target(), Some(request.revision)) {
            Ok(ticket) => self.pending.push(Pending { ticket, after, commit: false, clipboard: false }),
            Err(e) => self.alert(e),
        }
        self.focus_grid(window, cx);
        cx.notify();
    }
    pub fn trust_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let request = self.trust_request.as_ref();
        let mut body = div()
            .w(px(620.))
            .max_h(px(700.))
            .p_5()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x20332a))
            .rounded_lg()
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_3()
            .child("审核并信任此工作簿脚本？ / Trust workbook scripts?")
            .child("确认将信任此工作簿的全部脚本，仅在本次打开会话有效。文件、链接和跨窗口命令仍被沙箱拒绝。重新打开后需要再次确认。")
            .child(format!(
                "工作簿：{} · 脚本：{}",
                self.view().map(|v| v.title.as_str()).unwrap_or(""),
                request.map(|r| r.name.as_str()).unwrap_or("")
            ));
        let mut steps = div().id("trust-steps").max_h(px(420.)).overflow_y_scroll().flex().flex_col().gap_1();
        if request.is_some_and(|r| r.name.is_empty())
            && let Some(scripts) = self.script_bodies.as_object()
        {
            for (name, body) in scripts.iter().take(200) {
                steps = steps.child(format!("{name} · {} 步", body.as_array().map_or(0, Vec::len)));
                for step in body.as_array().into_iter().flatten().take(5) {
                    steps = steps.child(step.to_string().chars().take(300).collect::<String>());
                }
            }
        }
        if let Some(request) = request
            && let Some(list) = self.script_bodies.get(&request.name).and_then(Value::as_array)
        {
            for (i, st) in list.iter().take(200).enumerate() {
                steps = steps.child(format!(
                    "{}. {} {}",
                    i + 1,
                    st["command"].as_str().unwrap_or("?"),
                    st["params"].to_string().chars().take(500).collect::<String>()
                ));
            }
            if list.len() > 200 {
                steps = steps.child(format!("共 {} 步；面板可查看/编辑完整脚本。", list.len()));
            }
        }
        body = body.child(steps).child(
            div()
                .flex()
                .justify_end()
                .gap_4()
                .child(
                    div()
                        .id("decline-trust")
                        .relative()
                        .p_2()
                        .cursor_pointer()
                        .child("不信任 / Cancel")
                        .child(self.advanced_probe("trust-decline".into(), cx))
                        .on_click(cx.listener(|this, _, w, cx| {
                            this.trust_request = None;
                            this.focus_grid(w, cx);
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id("accept-trust")
                        .relative()
                        .p_2()
                        .bg(rgb(0xdceee2))
                        .cursor_pointer()
                        .child("信任此会话 / Trust")
                        .child(self.advanced_probe("trust-accept".into(), cx))
                        .on_click(cx.listener(|this, _, w, cx| this.confirm_script_trust(w, cx))),
                ),
        );
        div().absolute().inset_0().occlude().track_focus(&self.trust_focus).bg(rgba(0x10231a77)).flex().items_center().justify_center().child(body)
    }
}
