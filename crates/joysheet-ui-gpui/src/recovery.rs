use crate::desktop::{After, Desktop};
use gpui_kit::{component::button::Button, prelude::*, *};
use joysheet_app::recovery::{RecoveryDocument, RecoveryEntry, RecoveryService};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
pub struct RecoveryRuntime {
    pub service: Arc<RecoveryService>,
    pub entries: Vec<RecoveryEntry>,
    pub drafts: HashMap<u64, Value>,
    pub pending: HashMap<u64, Value>,
    pub last: Instant,
}
impl Global for RecoveryRuntime {}
impl Desktop {
    pub fn recovery_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(doc) = self.view().map(|v| v.doc)
            && let Some(draft) = cx.update_global::<RecoveryRuntime, _>(|r, _| r.pending.remove(&doc))
        {
            self.issue("sheet.activate", json!({"sheet":draft["sheet"]}), After::RestoreDraft(draft), cx);
        }
        if self.snapshot.active_window != self.window_id {
            return;
        }
        if let Some(error) = cx.global::<RecoveryRuntime>().service.take_error() {
            self.alert(format!("恢复副本失败 / Recovery: {error}"));
            cx.notify();
        }
        let due = cx.global::<RecoveryRuntime>().last.elapsed() > Duration::from_secs(5);
        if due {
            let enabled = self.preferences.recovery;
            let documents = cx.update_global::<RecoveryRuntime, _>(|r, _| {
                r.last = Instant::now();
                let mut seen = std::collections::HashSet::new();
                self.snapshot
                    .windows
                    .iter()
                    .filter(|w| seen.insert(w.doc))
                    .filter_map(|w| {
                        let drafts: Vec<_> = r.drafts.values().filter(|d| d["doc"].as_u64() == Some(w.doc)).cloned().collect();
                        if !enabled || (!w.dirty && drafts.is_empty()) {
                            return None;
                        }
                        Some(RecoveryDocument {
                            doc: w.doc,
                            title: w.title.clone(),
                            original: w.path.clone(),
                            revision: w.revision,
                            workbook: w.workbook.clone(),
                            drafts,
                        })
                    })
                    .collect()
            });
            if let Err(e) = cx.global::<RecoveryRuntime>().service.submit(documents) {
                self.alert(e);
            }
        }
        let _ = window;
    }
    pub fn restore_recovery(&mut self, id: String, draft_index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.is_some() {
            self.alert("请先提交或取消当前草稿 / Finish the current edit first");
            cx.notify();
            return;
        }
        let service = cx.global::<RecoveryRuntime>().service.clone();
        let task = cx.background_executor().spawn(async move { service.restore(&id) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, _, cx| match result {
                Ok((entry, bytes)) => {
                    let draft = entry.drafts.get(draft_index).cloned().unwrap_or(Value::Null);
                    this.issue(
                        "file.open",
                        json!({"name":"Recovered.json","base64":joysheet_engine::io::base64_encode(&bytes)}),
                        After::Recovered(draft),
                        cx,
                    );
                }
                Err(e) => this.alert(e),
            });
        })
        .detach();
    }
    pub fn recovery_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let entries = cx.global::<RecoveryRuntime>().entries.clone();
        let mut panel = div()
            .id("recovery-panel")
            .absolute()
            .right_0()
            .top(px(54.))
            .bottom_0()
            .w(px(460.))
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x20332a))
            .occlude()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .role(Role::Dialog)
            .aria_label("Recovery copies")
            .child(Button::new("close-recovery").label("恢复副本 · 关闭 × / Recovery").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 0;
                cx.notify();
            })))
            .child("恢复为独立工作簿；源文件不被覆盖。副本包含未提交草稿，脚本须重新信任。")
            .child(Button::new("refresh-recovery").label("刷新副本 / Refresh").on_click(cx.listener(|this, _, _, cx| {
                let result = cx.global::<RecoveryRuntime>().service.list();
                match result {
                    Ok(entries) => cx.update_global::<RecoveryRuntime, _>(|r, _| r.entries = entries),
                    Err(e) => this.alert(e),
                };
                cx.notify();
            })));
        if entries.is_empty() {
            panel = panel.child("没有其他会话留下的副本 / No copies from other sessions");
        }
        for entry in entries {
            panel = panel.child(format!("{} · {}", entry.title, entry.original.as_deref().unwrap_or("未保存")));
            for index in 0..entry.drafts.len().max(1) {
                let id = entry.id.clone();
                let label = entry
                    .drafts
                    .get(index)
                    .map(|d| format!("恢复 {} · 草稿 {} {}", entry.title, d["sheetName"].as_str().unwrap_or(""), d["cell"].as_str().unwrap_or("")))
                    .unwrap_or_else(|| format!("恢复 {}", entry.title));
                panel = panel.child(
                    Button::new(SharedString::from(format!("restore-{}-{index}", entry.id)))
                        .label(label)
                        .on_click(cx.listener(move |this, _, w, cx| this.restore_recovery(id.clone(), index, w, cx))),
                );
            }
            let id = entry.id.clone();
            panel = panel.child(Button::new(SharedString::from(format!("discard-{}", entry.id))).label("丢弃此恢复副本 / Discard copy").on_click(
                cx.listener(move |this, _, _, cx| {
                    let result = cx.global::<RecoveryRuntime>().service.discard(&id);
                    match result {
                        Ok(()) => cx.update_global::<RecoveryRuntime, _>(|r, _| r.entries.retain(|e| e.id != id)),
                        Err(e) => this.alert(e),
                    };
                    cx.notify();
                }),
            ));
        }
        panel
    }
}
