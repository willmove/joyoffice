//! Word-style ribbon groups over the single Session. Native pickers follow JoyOffice's
//! epoch/version-bound import and copy-export pattern; no VBA or network execution.
use super::*;
use gpui::component::{Icon, WindowExt};
use joyoffice_ui::IconName;
use joyword_shell::advanced as core;
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct Advanced {
    pub(super) pane: Option<&'static str>,
    items: Vec<Value>,
    pick: Option<usize>,
    pub(super) key: Option<(u64, u64, u64, &'static str)>,
    task: Option<gpui::Task<()>>,
    epoch: u64,
    pub(super) checked_export: super::export::CheckedExport,
}

fn text(v: &Value, k: &str) -> String {
    v[k].as_str().map(str::to_owned).unwrap_or_else(|| {
        if v[k].is_null() {
            String::new()
        } else {
            v[k].to_string()
        }
    })
}
fn required(v: &[String], i: usize) -> Result<String, String> {
    let s = core::bounded_text(v.get(i).map(String::as_str).unwrap_or(""), 16_384)?;
    if s.trim().is_empty() {
        Err("请填写必填项".into())
    } else {
        Ok(s)
    }
}
impl JoyWord {
    fn advanced_button(
        &self,
        id: &'static str,
        label: &'static str,
        icon: IconName,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .ghost()
            .xsmall()
            .icon(icon)
            .label(label)
            .disabled(self.canvas.reader.is_none() || self.canvas.loading || self.file_page)
            .on_click(cx.listener(move |v, _, w, cx| v.advanced_action(id, w, cx)))
    }
    fn large_advanced(
        &self,
        id: &'static str,
        label: &'static str,
        icon: IconName,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = self.palette();
        div()
            .id(id)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .px_2()
            .h(px(56.))
            .rounded(px(4.))
            .hover(move |s| s.bg(rgb(p.hover)))
            .cursor_pointer()
            .role(gpui::Role::Button)
            .aria_label(label)
            .focusable()
            .child(Icon::new(icon).size(px(26.)).text_color(rgb(p.accent)))
            .child(div().text_size(px(12.)).child(label))
            .on_click(cx.listener(move |v, _, w, cx| v.advanced_action(id, w, cx)))
            .on_key_down(cx.listener(move |v, e: &gpui::KeyDownEvent, w, cx| {
                if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                    v.advanced_action(id, w, cx);
                    cx.stop_propagation();
                }
            }))
    }
    pub(super) fn references_ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        let stack = |a, b| div().flex().flex_col().gap_1().child(a).child(b);
        div()
            .flex()
            .child(
                self.group_box(
                    "目录",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced("refs-toc", "目录 ▾", IconName::List, cx))
                        .child(stack(
                            self.advanced_button("refs-level", "添加文字", IconName::Plus, cx),
                            self.author_button(
                                "refs-update-toc",
                                "更新目录",
                                "references.updateToc",
                                json!({}),
                                cx,
                            ),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "脚注",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced(
                            "refs-footnote",
                            "插入脚注",
                            IconName::FileText,
                            cx,
                        ))
                        .child(stack(
                            self.advanced_button(
                                "refs-endnote",
                                "插入尾注",
                                IconName::FileText,
                                cx,
                            ),
                            self.advanced_button("refs-notes", "显示注释", IconName::List, cx),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "引文与书目",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced(
                            "refs-citation",
                            "插入引文",
                            IconName::BookOpen,
                            cx,
                        ))
                        .child(stack(
                            self.advanced_button("refs-sources", "管理源", IconName::BookOpen, cx),
                            self.advanced_button(
                                "refs-bib-style",
                                "样式 ▾",
                                IconName::Settings,
                                cx,
                            ),
                        ))
                        .child(stack(
                            self.advanced_button(
                                "refs-bibliography",
                                "参考文献",
                                IconName::BookOpen,
                                cx,
                            ),
                            self.advanced_button("refs-new-source", "添加新源", IconName::Plus, cx),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "题注",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced(
                            "refs-caption",
                            "插入题注",
                            IconName::FileText,
                            cx,
                        ))
                        .child(stack(
                            self.advanced_button("refs-figures", "插入表目录", IconName::List, cx),
                            self.advanced_button("refs-crossref", "交叉引用", IconName::Link, cx),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "索引",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.advanced_button(
                            "refs-mark-entry",
                            "标记索引项",
                            IconName::Bookmark,
                            cx,
                        ))
                        .child(self.author_button(
                            "refs-index",
                            "插入索引",
                            "references.index",
                            json!({}),
                            cx,
                        ))
                        .child(self.author_button(
                            "refs-update-index",
                            "更新索引",
                            "references.updateIndex",
                            json!({}),
                            cx,
                        )),
                ),
            )
            .child(
                self.group_box(
                    "引文目录",
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(self.advanced_button(
                            "refs-mark-authority",
                            "标记引文",
                            IconName::Bookmark,
                            cx,
                        ))
                        .child(self.author_button(
                            "refs-authorities",
                            "插入引文目录",
                            "references.tableOfAuthorities",
                            json!({}),
                            cx,
                        )),
                ),
            )
            .child(self.group_box(
                "链接与域",
                stack(
                    self.advanced_button("refs-bookmark", "书签", IconName::Bookmark, cx),
                    self.author_button(
                        "refs-update-all",
                        "更新全部域",
                        "references.updateFields",
                        json!({"all":true}),
                        cx,
                    ),
                ),
            ))
    }
    pub(super) fn review_ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        let stack = |a, b| div().flex().flex_col().gap_1().child(a).child(b);
        let tracking = self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.engine().doc.settings.track_changes);
        div()
            .flex()
            .child(self.group_box(
                "校对",
                self.advanced_button("review-count", "字数统计", IconName::FileText, cx),
            ))
            .child(
                self.group_box(
                    "批注",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced(
                            "review-new",
                            "新建批注",
                            IconName::MessageSquare,
                            cx,
                        ))
                        .child(stack(
                            self.advanced_button(
                                "review-comments",
                                "显示批注",
                                IconName::MessageSquare,
                                cx,
                            ),
                            self.author_button(
                                "review-next-comment",
                                "下一条",
                                "review.nextComment",
                                json!({}),
                                cx,
                            ),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "修订",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.author_button(
                            "review-track",
                            if tracking { "✓ 修订" } else { "修订" },
                            "review.trackChanges",
                            json!({}),
                            cx,
                        ))
                        .child(stack(
                            self.advanced_button("review-markup", "显示标记 ▾", IconName::Eye, cx),
                            self.advanced_button("review-changes", "审阅窗格", IconName::List, cx),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "更改",
                    div()
                        .flex()
                        .gap_1()
                        .child(stack(
                            self.author_button(
                                "review-accept",
                                "接受",
                                "review.accept",
                                json!({"next":true}),
                                cx,
                            ),
                            self.author_button(
                                "review-reject",
                                "拒绝",
                                "review.reject",
                                json!({"next":true}),
                                cx,
                            ),
                        ))
                        .child(stack(
                            self.author_button(
                                "review-prev",
                                "上一处",
                                "review.previousChange",
                                json!({}),
                                cx,
                            ),
                            self.author_button(
                                "review-next",
                                "下一处",
                                "review.nextChange",
                                json!({}),
                                cx,
                            ),
                        ))
                        .child(stack(
                            self.author_button(
                                "review-accept-all",
                                "接受所有修订",
                                "review.acceptAll",
                                json!({}),
                                cx,
                            ),
                            self.author_button(
                                "review-reject-all",
                                "拒绝所有修订",
                                "review.rejectAll",
                                json!({}),
                                cx,
                            ),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "比较",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced(
                            "review-compare",
                            "比较文档",
                            IconName::Files,
                            cx,
                        ))
                        .child(self.advanced_button(
                            "review-combine",
                            "合并修订",
                            IconName::Files,
                            cx,
                        )),
                ),
            )
    }
    pub(super) fn mailings_ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        let stack = |a, b| div().flex().flex_col().gap_1().child(a).child(b);
        div()
            .flex()
            .child(
                self.group_box(
                    "开始邮件合并",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced(
                            "merge-start",
                            "开始邮件合并",
                            IconName::Mail,
                            cx,
                        ))
                        .child(stack(
                            self.advanced_button(
                                "merge-recipients",
                                "选择收件人",
                                IconName::FolderOpen,
                                cx,
                            ),
                            self.advanced_button(
                                "merge-list",
                                "编辑收件人列表",
                                IconName::List,
                                cx,
                            ),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "编写和插入域",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced(
                            "merge-field",
                            "插入合并域",
                            IconName::FileText,
                            cx,
                        ))
                        .child(stack(
                            self.author_button(
                                "merge-address",
                                "地址块",
                                "mailings.addressBlock",
                                json!({}),
                                cx,
                            ),
                            self.author_button(
                                "merge-greeting",
                                "问候语",
                                "mailings.greetingLine",
                                json!({}),
                                cx,
                            ),
                        ))
                        .child(stack(
                            self.advanced_button("merge-filter", "筛选收件人", IconName::List, cx),
                            self.advanced_button("merge-sort", "排序收件人", IconName::List, cx),
                        )),
                ),
            )
            .child(
                self.group_box(
                    "预览结果",
                    div()
                        .flex()
                        .gap_1()
                        .child(self.large_advanced("merge-preview", "预览结果", IconName::Eye, cx))
                        .child(stack(
                            self.author_button(
                                "merge-prev",
                                "上一条",
                                "mailings.previous",
                                json!({}),
                                cx,
                            ),
                            self.author_button(
                                "merge-next",
                                "下一条",
                                "mailings.next",
                                json!({}),
                                cx,
                            ),
                        )),
                ),
            )
            .child(self.group_box(
                "完成",
                self.large_advanced("merge-finish", "完成并合并 ▾", IconName::Check, cx),
            ))
    }
    pub(super) fn developer_ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .child(
                self.group_box(
                    "宏",
                    div()
                        .flex()
                        .gap_2()
                        .child(self.large_advanced("macro-list", "宏", IconName::List, cx))
                        .child(self.advanced_button("macro-record", "录制宏", IconName::Circle, cx))
                        .child(
                            self.advanced_button("macro-stop", "停止录制", IconName::Square, cx)
                                .disabled(
                                    !self
                                        .canvas
                                        .reader
                                        .as_ref()
                                        .is_some_and(|r| r.engine().recording.is_some()),
                                ),
                        ),
                ),
            )
            .child(self.group_box(
                "选项",
                self.advanced_button("general-options", "用户信息与单位", IconName::Settings, cx),
            ))
            .child(self.group_box(
                "自动化",
                self.advanced_button("automation-info", "MCP 自动化说明", IconName::Settings, cx),
            ))
            .child(self.group_box(
                "格式交换",
                self.large_advanced("exchange-export", "导出副本", IconName::FileText, cx),
            ))
    }
    pub(super) fn open_advanced(&mut self, pane: &'static str, cx: &mut Context<Self>) {
        self.advanced.pane = Some(pane);
        self.advanced.key = None;
        self.advanced.pick = None;
        self.styles_open = false;
        cx.notify();
    }
    pub(super) fn sync_advanced(&mut self) {
        let Some(pane) = self.advanced.pane else {
            return;
        };
        let Some(r) = self.canvas.reader.as_mut() else {
            return;
        };
        let key = (
            r.engine().document_id(),
            r.engine().rev(),
            r.presentation_revision(),
            pane,
        );
        if self.advanced.key == Some(key) {
            return;
        }
        let result = match pane {
            "来源" => Ok(r.sources()),
            "收件人" => Ok(r.merge_info()["rows"].clone()),
            "批注" => r.advanced_query("review.comments"),
            "修订" => r.advanced_query("review.changes"),
            "注释" => r.advanced_query("references.notes"),
            "对象" => r.advanced_query("arrange.selectionPane"),
            _ => r.advanced_query("tools.macros"),
        };
        match result {
            Ok(v) => {
                self.advanced.items = if let Some(a) = v.as_array() {
                    a.clone()
                } else if let Some(o) = v.as_object() {
                    o.iter()
                        .map(|(k, v)| json!({"tag":k,"name":k,"value":v}))
                        .collect()
                } else {
                    vec![]
                };
                self.advanced.key = Some(key);
                if self
                    .advanced
                    .pick
                    .is_some_and(|i| i >= self.advanced.items.len())
                {
                    self.advanced.pick = None;
                }
            }
            Err(e) => self.shell.notice = Some(e),
        }
    }
    pub(super) fn advanced_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette();
        let pane = self.advanced.pane.unwrap_or("");
        let count = self.advanced.items.len();
        let rows = gpui::uniform_list(
            "advanced-list",
            count,
            cx.processor(move |v, range: std::ops::Range<usize>, _, cx| {
                range
                    .filter_map(|i| {
                        let item = v.advanced.items.get(i)?;
                        let label = match pane {
                            "批注" => format!(
                                "{} {} · {}",
                                text(item, "id"),
                                if item["resolved"] == true {
                                    "✓ 已解决"
                                } else {
                                    ""
                                },
                                text(item, "author")
                            ),
                            "修订" => {
                                format!("{} · {}", text(item, "kind"), text(item, "author"))
                            }
                            "来源" => format!("{} {}", text(item, "tag"), text(item, "title")),
                            "收件人" => format!(
                                "{} · {}",
                                i + 1,
                                item.as_array()
                                    .map(|a| a
                                        .iter()
                                        .map(|v| v.as_str().unwrap_or(""))
                                        .collect::<Vec<_>>()
                                        .join(" · "))
                                    .unwrap_or_default()
                            ),
                            "注释" => format!("{} {}", text(item, "kind"), text(item, "id")),
                            _ => item
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| text(item, "name")),
                        };
                        Some(
                            joyoffice_ui::control(p, true, v.advanced.pick == Some(i))
                                .id(("advanced-row", i))
                                .role(gpui::Role::Button)
                                .aria_label(format!("{pane} {label}"))
                                .focusable()
                                .h(px(76.))
                                .w_full()
                                .justify_start()
                                .cursor_pointer()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .overflow_hidden()
                                        .child(div().text_sm().child(label))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .line_clamp(2)
                                                .child(text(item, "text")),
                                        ),
                                )
                                .on_click(
                                    cx.listener(move |v, _, w, cx| {
                                        v.choose_advanced(i, pane, w, cx)
                                    }),
                                )
                                .on_key_down(cx.listener(
                                    move |v, e: &gpui::KeyDownEvent, w, cx| {
                                        if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                                            v.choose_advanced(i, pane, w, cx);
                                            cx.stop_propagation();
                                        }
                                    },
                                )),
                        )
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .flex_1()
        .min_h(px(80.));
        let mut panel = joyoffice_ui::card(p)
            .w(px(320.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(format!("{pane} · {count}"))
                    .child(
                        Button::new("advanced-close")
                            .ghost()
                            .xsmall()
                            .label("关闭")
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.advanced.pane = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(rows);
        let mut actions = div().flex().flex_wrap().gap_1();
        for (id, label) in match pane {
            "批注" => vec![
                ("review-new", "新建"),
                ("review-reply", "回复"),
                ("review-edit", "编辑"),
                ("review-resolve", "解决 / 重新打开"),
                ("review-delete", "删除"),
            ],
            "来源" => vec![
                ("refs-new-source", "添加 / 修改源"),
                ("refs-citation", "插入引文"),
            ],
            "注释" => vec![("refs-body", "返回正文")],
            "宏" => vec![("macro-run", "运行"), ("macro-record", "录制")],
            "收件人" => vec![
                ("merge-filter", "筛选"),
                ("merge-sort", "排序"),
                ("merge-finish", "完成合并"),
            ],
            _ => vec![],
        } {
            actions = actions.child(self.advanced_button(id, label, IconName::ChevronRight, cx));
        }
        if pane == "宏" {
            panel = panel.child(
                div()
                    .text_size(px(11.))
                    .child("宏仅在本次会话中保存；支持文档命令，不执行 VBA、文件或网络操作。"),
            );
        }
        if pane == "收件人" {
            let info = self
                .canvas
                .reader
                .as_ref()
                .map(|r| r.merge_info())
                .unwrap_or_default();
            panel = panel.child(div().text_size(px(11.)).child(format!(
                "第 {} 条 / {} 条 · 预览 {}",
                info["record"], info["records"], info["preview"]
            )));
        }
        if pane == "收件人" {
            let fields = self
                .canvas
                .reader
                .as_ref()
                .map(|r| {
                    r.merge_info()["fields"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join(" · ")
                        })
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            panel = panel.child(div().text_size(px(12.)).child(format!("字段：{fields}")));
        }
        panel.child(actions)
    }
    fn choose_advanced(
        &mut self,
        index: usize,
        pane: &str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if pane == "对象" {
            let key = self.canvas.reader.as_ref().map(|r| {
                (
                    r.engine().document_id(),
                    r.engine().rev(),
                    r.presentation_revision(),
                    "对象",
                )
            });
            if key != self.advanced.key {
                self.advanced.key = None;
                cx.notify();
                return;
            }
            if let Some(item) = self.advanced.items.get(index).cloned()
                && let Err(error) =
                    self.try_edit("select.objects", &json!({"index":item["index"]}), w, cx)
            {
                self.shell.notice = Some(error);
            }
            self.advanced.pick = Some(index);
            cx.notify();
            return;
        }
        self.advanced.pick = Some(index);
        if let Some(s) = self.advanced.items.get(index).cloned() {
            if s.get("start").is_some() {
                self.apply_input(w, cx, |r| r.select_json(&s["start"], &s["end"]));
            } else if !s["anchor"].is_null() {
                self.apply_input(w, cx, |r| r.select_json(&s["anchor"], &s["anchor"]));
            } else if pane == "收件人" {
                let _ = self.try_edit("mailings.goToRecord", &json!({"record":index+1}), w, cx);
            } else if pane == "注释"
                && let Some(id) = s["id"].as_u64().and_then(|n| u32::try_from(n).ok())
            {
                self.apply_input(w, cx, |r| r.visit_note(id));
            }
        }
        cx.notify();
    }
    fn selected_advanced(&self) -> Option<Value> {
        self.advanced
            .pick
            .and_then(|i| self.advanced.items.get(i))
            .cloned()
    }
    pub(super) fn advanced_action(
        &mut self,
        id: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.canvas.reader.is_none() || self.canvas.loading || self.file_page {
            return;
        }
        match id {
            "refs-sources" => {
                self.open_advanced("来源", cx);
                return;
            }
            "refs-notes" => {
                self.open_advanced("注释", cx);
                return;
            }
            "review-comments" => {
                self.open_advanced("批注", cx);
                return;
            }
            "review-changes" => {
                self.open_advanced("修订", cx);
                return;
            }
            "merge-list" => {
                self.open_advanced("收件人", cx);
                return;
            }
            "macro-list" => {
                self.open_advanced("宏", cx);
                return;
            }
            "macro-stop" => {
                if self
                    .canvas
                    .reader
                    .as_ref()
                    .is_some_and(|r| r.engine().recording.is_some())
                {
                    let _ = self.try_edit("tools.recordMacro", &json!({}), w, cx);
                }
                return;
            }
            "refs-body" => {
                self.apply_input(w, cx, |r| r.return_to_body());
                return;
            }
            "review-compare" | "review-combine" | "merge-recipients" => {
                self.advanced_file(id, w, cx);
                return;
            }
            "merge-preview" => {
                let _ = self.try_edit("mailings.preview", &json!({}), w, cx);
                return;
            }
            "review-resolve" | "review-delete" => {
                if let Some(item) = self.selected_advanced() {
                    let command = if id == "review-resolve" {
                        "review.resolveComment"
                    } else {
                        "review.deleteComment"
                    };
                    let _ = self.try_edit(command, &json!({"id":item["id"]}), w, cx);
                }
                return;
            }
            "review-count" => {
                if let Some(r) = self.canvas.reader.as_mut() {
                    match r.execute("review.wordCount", &json!({})) {
                        Ok(v) => self.shell.notice = Some(format!("字数统计：{v}")),
                        Err(e) => self.shell.notice = Some(e),
                    }
                }
                cx.notify();
                return;
            }
            "automation-info" => {
                w.open_dialog(cx,|d,_,_|d.title("MCP 自动化").width(px(580.)).child("在终端运行：joyword-cli mcp --automation-read-root <输入目录> --automation-write-root <输出目录>。通过标准输入输出使用 MCP JSON-RPC；范围外文件访问被拒绝。此开发包提供独立内核自动化，尚未连接当前桌面文档。默认不监听网络、不执行 VBA。"));
                return;
            }
            _ => {}
        }
        let picked = self.selected_advanced().unwrap_or_default();
        let (title, cmd, fields): (&'static str, &'static str, Vec<(&'static str, String)>) =
            match id {
                "general-options" => (
                    "Word 选项 · 常规",
                    "file.options",
                    vec![
                        (
                            "用户名",
                            self.canvas
                                .reader
                                .as_ref()
                                .map(|r| r.engine().author.clone())
                                .unwrap_or_default(),
                        ),
                        (
                            "单位（centimeters / millimeters / inches / points）",
                            self.canvas
                                .reader
                                .as_ref()
                                .map(|r| super::forms::show(&json!(r.engine().units())))
                                .unwrap_or_default(),
                        ),
                    ],
                ),
                "refs-mark-entry" => (
                    "标记索引项",
                    "references.markEntry",
                    vec![("主索引项", String::new()), ("子项", String::new())],
                ),
                "refs-mark-authority" => (
                    "标记引文",
                    "references.markCitation",
                    vec![("引文文字", String::new())],
                ),
                "refs-toc" => (
                    "目录",
                    "references.toc",
                    vec![("目录标题", "目录".into()), ("显示级别（1–9）", "3".into())],
                ),
                "refs-level" => (
                    "添加到目录",
                    "references.addText",
                    vec![("大纲级别（0 移除，1–9）", "1".into())],
                ),
                "refs-footnote" => (
                    "插入脚注",
                    "references.footnote",
                    vec![("脚注文字", String::new())],
                ),
                "refs-endnote" => (
                    "插入尾注",
                    "references.endnote",
                    vec![("尾注文字", String::new())],
                ),
                "refs-new-source" => (
                    "添加或修改来源",
                    "references.sources",
                    vec![
                        ("唯一标记", text(&picked, "tag")),
                        (
                            "类型（book / article / website / report）",
                            if picked["kind"].is_string() {
                                text(&picked, "kind")
                            } else {
                                "book".into()
                            },
                        ),
                        ("标题", text(&picked, "title")),
                        ("作者", text(&picked, "author")),
                        ("年份", text(&picked, "year")),
                        ("出版者", text(&picked, "publisher")),
                        ("网址", text(&picked, "url")),
                    ],
                ),
                "refs-citation" => (
                    "插入引文",
                    "references.citation",
                    vec![("来源标记", text(&picked, "tag")), ("页码", String::new())],
                ),
                "refs-bib-style" => (
                    "引文样式",
                    "references.citationStyle",
                    vec![("样式（APA / MLA / Chicago / IEEE 等）", "APA".into())],
                ),
                "refs-bibliography" => (
                    "参考文献",
                    "references.bibliography",
                    vec![("标题", "参考文献".into())],
                ),
                "refs-caption" => (
                    "插入题注",
                    "references.caption",
                    vec![("标签", "图".into()), ("题注文字", String::new())],
                ),
                "refs-figures" => (
                    "表目录",
                    "references.tableOfFigures",
                    vec![("标签", "图".into()), ("标题", "图目录".into())],
                ),
                "refs-crossref" => (
                    "交叉引用",
                    "insert.crossReference",
                    vec![
                        (
                            "类型（heading / bookmark / figure / table / label:图）",
                            "heading".into(),
                        ),
                        ("目标文字 / 名称 / 编号", String::new()),
                        ("内容（text / page / number / aboveBelow）", "text".into()),
                    ],
                ),
                "refs-bookmark" => (
                    "插入书签",
                    "insert.bookmark",
                    vec![("书签名称", String::new())],
                ),
                "review-new" => (
                    "新建批注",
                    "review.newComment",
                    vec![("批注内容", String::new())],
                ),
                "review-reply" => (
                    "回复批注",
                    "review.reply",
                    vec![("回复内容", String::new())],
                ),
                "review-edit" => (
                    "编辑批注",
                    "review.editComment",
                    vec![("批注内容", text(&picked, "text"))],
                ),
                "review-markup" => (
                    "显示标记",
                    "review.markup",
                    vec![("显示（所有标记 / 无标记）", "所有标记".into())],
                ),
                "merge-start" => (
                    "开始邮件合并",
                    "mailings.start",
                    vec![(
                        "文档类型（letters 信函 / directory 目录 / labels 标签）",
                        "letters".into(),
                    )],
                ),
                "merge-field" => (
                    "插入合并域",
                    "mailings.insertField",
                    vec![(
                        "收件人字段名称",
                        self.canvas
                            .reader
                            .as_ref()
                            .and_then(|r| {
                                r.merge_info()["fields"]
                                    .as_array()
                                    .and_then(|a| a.first())
                                    .and_then(Value::as_str)
                                    .map(str::to_owned)
                            })
                            .unwrap_or_default(),
                    )],
                ),
                "merge-filter" => (
                    "筛选收件人",
                    "mailings.filterRecipients",
                    vec![
                        ("字段（留空清除筛选）", String::new()),
                        ("包含文字", String::new()),
                    ],
                ),
                "merge-sort" => (
                    "排序收件人",
                    "mailings.sortRecipients",
                    vec![("字段（留空还原顺序）", String::new())],
                ),
                "merge-finish" => (
                    "完成并合并为新 DOCX",
                    "mailings.finish",
                    vec![("记录（all 全部 / current 当前）", "all".into())],
                ),
                "exchange-export" => (
                    "导出副本 · 交换格式可能丢失版式与审阅信息",
                    "file.saveAs",
                    vec![
                        (
                            "格式（docx / pdf / txt / md / html / rtf / odt / tex / xml）",
                            "pdf".into(),
                        ),
                        ("保留标记与批注（是 / 否，仅适用支持的格式）", "否".into()),
                    ],
                ),
                "macro-record" => (
                    "录制宏 · 仅本次会话有效",
                    "tools.recordMacro",
                    vec![("宏名称", String::new())],
                ),
                "macro-run" => (
                    "运行宏",
                    "tools.macros",
                    vec![(
                        "宏名称",
                        picked
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| text(&picked, "name")),
                    )],
                ),
                _ => return,
            };
        if matches!(id, "review-reply" | "review-edit") && picked["id"].is_null() {
            self.shell.notice = Some("请先在批注窗格选择一条批注".into());
            cx.notify();
            return;
        }
        self.fields_action(
            title,
            cmd,
            fields,
            false,
            move |v| {
                for s in v {
                    core::bounded_text(s, 16_384)?;
                }
                Ok(match id {
                    "general-options" => {
                        if joyword_shell::prefs::Units::parse(v[1].trim()).is_none() {
                            return Err("请选择 centimeters / millimeters / inches / points".into());
                        }
                        json!({"author":required(v,0)?,"units":v[1].trim()})
                    }
                    "refs-mark-entry" => json!({"entry":required(v,0)?,"subentry":v[1]}),
                    "refs-mark-authority" => json!({"entry":required(v,0)?}),
                    "refs-toc" => {
                        let levels = v[1].trim().parse::<u8>().map_err(|_| "请输入 1–9")?;
                        if !(1..=9).contains(&levels) {
                            return Err("请输入 1–9".into());
                        }
                        json!({"title":v[0],"levels":levels,"pageNumbers":true,"hyperlinks":true})
                    }
                    "refs-level" => {
                        let n = v[0].trim().parse::<u8>().map_err(|_| "请输入 0–9")?;
                        if n > 9 {
                            return Err("请输入 0–9".into());
                        }
                        json!({"level":n})
                    }
                    "refs-footnote" | "refs-endnote" | "review-new" => {
                        json!({"text":required(v,0)?})
                    }
                    "refs-new-source" => core::source_params(v)?,
                    "refs-citation" => json!({"tag":required(v,0)?,"pages":v[1]}),
                    "refs-bib-style" => json!({"style":required(v,0)?}),
                    "refs-bibliography" => json!({"title":v[0]}),
                    "refs-caption" => json!({"label":required(v,0)?,"text":v[1]}),
                    "refs-figures" => json!({"label":required(v,0)?,"title":v[1]}),
                    "refs-crossref" => {
                        json!({"to":required(v,0)?,"target":required(v,1)?,"show":v[2]})
                    }
                    "refs-bookmark" => json!({"name":required(v,0)?}),
                    "review-reply" | "review-edit" => {
                        json!({"id":picked["id"],"text":required(v,0)?})
                    }
                    "review-markup" => {
                        let value = match v[0].trim() {
                            "所有标记" | "allMarkup" => "allMarkup",
                            "无标记" | "noMarkup" => "noMarkup",
                            _ => return Err("请选择 所有标记 或 无标记".into()),
                        };
                        json!({"value":value})
                    }
                    "merge-start" => {
                        let kind = required(v, 0)?;
                        if !["letters", "directory", "labels"].contains(&kind.trim()) {
                            return Err("请选择 letters / directory / labels".into());
                        }
                        json!({"kind":kind})
                    }
                    "merge-field" => json!({"field":required(v,0)?}),
                    "merge-filter" => {
                        if v[0].trim().is_empty() {
                            json!({})
                        } else {
                            json!({"rules":[{"field":v[0],"op":"contains","value":v[1]}]})
                        }
                    }
                    "merge-sort" => {
                        if v[0].trim().is_empty() {
                            json!({})
                        } else {
                            json!({"field":v[0]})
                        }
                    }
                    "merge-finish" => {
                        if !["all", "current"].contains(&v[0].trim()) {
                            return Err("请选择 all / current".into());
                        }
                        json!({"current":v[0].trim()=="current"})
                    }
                    "exchange-export" => {
                        let ext = v[0].trim().to_ascii_lowercase();
                        if !core::EXPORTS.contains(&ext.as_str()) {
                            return Err("不支持此导出格式".into());
                        }
                        let markup = match v[1].trim() {
                            "是" => true,
                            "否" => false,
                            _ => return Err("请输入 是 或 否".into()),
                        };
                        json!({"ext":ext,"markup":markup})
                    }
                    "macro-record" => json!({"name":required(v,0)?}),
                    "macro-run" => json!({"run":required(v,0)?}),
                    _ => json!({}),
                })
            },
            move |v, args, w, cx| {
                if id == "general-options" {
                    let author = args["author"].as_str().ok_or("请输入用户名")?;
                    let units = args["units"]
                        .as_str()
                        .and_then(joyword_shell::prefs::Units::parse)
                        .ok_or("请选择有效的单位")?;
                    let reader = v.canvas.reader.as_mut().ok_or("文档当前不可用")?;
                    v.shell.save_general_preferences(reader, author, units)?;
                    return Ok(json!({}));
                }
                if id == "exchange-export" || id == "merge-finish" {
                    v.output_copy(args, id == "merge-finish", cx)?;
                    Ok(json!({"queued":true}))
                } else {
                    let mut params = args.clone();
                    if cmd == "references.sources"
                        && let Some(existing) = v.canvas.reader.as_ref().and_then(|r| {
                            r.sources()
                                .as_array()
                                .and_then(|a| a.iter().find(|s| s["tag"] == params["add"]["tag"]))
                                .cloned()
                        })
                    {
                        let mut source = existing;
                        if let (Some(base), Some(changed)) =
                            (source.as_object_mut(), params["add"].as_object())
                        {
                            base.extend(changed.clone());
                        }
                        params["add"] = source;
                    }
                    let result = v.try_edit(cmd, &params, w, cx)?;
                    match id {
                        "review-new" | "review-reply" | "review-edit" => {
                            v.open_advanced("批注", cx)
                        }
                        "refs-new-source" => v.open_advanced("来源", cx),
                        "macro-record" => v.open_advanced("宏", cx),
                        _ => {}
                    }
                    Ok(result)
                }
            },
            w,
            cx,
        );
    }
    fn advanced_file(&mut self, id: &'static str, w: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.canvas.reader.as_ref() else {
            return;
        };
        if r.is_composing() {
            return;
        }
        let target = (
            r.engine().document_id(),
            r.engine().rev(),
            r.engine().sel.clone(),
        );
        self.advanced.epoch = self.advanced.epoch.saturating_add(1);
        let epoch = self.advanced.epoch;
        let chooser = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                if matches!(id, "review-compare" | "review-combine") {
                    "选择修订后的 DOCX"
                } else {
                    "选择 CSV、TSV 或电子表格收件人"
                }
                .into(),
            ),
        });
        self.advanced.task = Some(cx.spawn_in(w, async move |view, cx| {
            let path = match chooser.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let Some(path) = path else { return };
            let result = cx
                .background_executor()
                .spawn(async move {
                    if matches!(id, "review-compare" | "review-combine") {
                        core::comparison(&path)
                    } else {
                        core::recipients(&path)
                    }
                })
                .await;
            let _ = view.update_in(cx, |v, w, cx| {
                if v.advanced.epoch != epoch
                    || v.canvas.reader.as_ref().map(|r| {
                        (
                            r.engine().document_id(),
                            r.engine().rev(),
                            r.engine().sel.clone(),
                        )
                    }) != Some(target)
                {
                    v.shell.notice = Some("文档或选区已变化，请重新选择文件".into());
                    cx.notify();
                    return;
                }
                match result {
                    Err(e) => v.shell.notice = Some(e),
                    Ok(args) => {
                        let cmd = if matches!(id, "review-compare" | "review-combine") {
                            if id == "review-combine" {
                                "review.combine"
                            } else {
                                "review.compare"
                            }
                        } else {
                            "mailings.recipients"
                        };
                        match v.try_edit(cmd, &args, w, cx) {
                            Err(e) => v.shell.notice = Some(e),
                            Ok(result) => {
                                if result["chooseSheet"] == true {
                                    let data = args.clone();
                                    let names = result["sheets"].to_string();
                                    v.fields_form(
                                        "选择收件人工作表",
                                        "mailings.recipients",
                                        vec![("工作表名称", String::new()), ("可选工作表", names)],
                                        false,
                                        move |s| {
                                            let mut args = data.clone();
                                            args["sheet"] = json!(required(s, 0)?);
                                            Ok(args)
                                        },
                                        w,
                                        cx,
                                    );
                                } else {
                                    v.open_advanced(
                                        if matches!(id, "review-compare" | "review-combine") {
                                            "修订"
                                        } else {
                                            "收件人"
                                        },
                                        cx,
                                    );
                                }
                            }
                        }
                    }
                }
                cx.notify();
            });
        }));
    }
    fn output_copy(
        &mut self,
        args: &Value,
        merge: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let r = self.canvas.reader.as_ref().ok_or("请先打开文档")?;
        let ext = if merge {
            "docx"
        } else {
            args["ext"].as_str().ok_or("缺少格式")?
        }
        .to_string();
        let job = if merge {
            r.merge_job(args["current"] == true)?
        } else {
            r.export_job(&ext, args["markup"] == true)?
        };
        let directory = r
            .source_path()
            .and_then(|p| p.parent())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| ".".into())
            });
        let target = (
            r.engine().document_id(),
            r.engine().rev(),
            r.presentation_revision(),
        );
        let suggestion = format!(
            "{}-{}.{}",
            std::path::Path::new(&r.summary.name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled"),
            if merge { "merged" } else { "export" },
            ext
        );
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggestion));
        self.advanced.task = Some(cx.spawn(async move |view, cx| {
            let path = match prompt.await {
                Ok(Ok(Some(p))) => p,
                _ => return,
            };
            let valid = view
                .update(cx, |v, _| {
                    v.canvas.reader.as_ref().is_some_and(|r| {
                        (
                            r.engine().document_id(),
                            r.engine().rev(),
                            r.presentation_revision(),
                        ) == target
                            && !r.is_composing()
                    })
                })
                .unwrap_or(false);
            if !valid {
                let _ = view.update(cx, |v, cx| {
                    v.shell.notice = Some("文档已变化，请重新导出".into());
                    cx.notify();
                });
                return;
            }
            let result = cx
                .background_executor()
                .spawn(async move { job.publish(path) })
                .await;
            let _ = view.update(cx, |v, cx| {
                v.shell.notice = Some(match result {
                    Ok(p) => format!("已输出副本：{}", p.display()),
                    Err(e) => format!("输出失败：{e}"),
                });
                cx.notify();
            });
        }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::sync::atomic::AtomicBool;
    #[gpui::test]
    fn real_kit_root_routes_review_and_refreshes_pure_macro_queries(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut editor = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let v = cx.new(|cx| JoyWord::new(w, cx));
            editor = Some(v.clone());
            gpui::base::Root::new(v, w, cx)
        });
        let view = editor.unwrap();
        view.update(cx, |v, cx| {
            let r = joyword_shell::reader::Reader::blank(&AtomicBool::new(false)).unwrap();
            let id = r.engine().document_id();
            v.canvas.reader = Some(r);
            v.install_input(id, cx);
        });
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                v.try_edit("text.insert", &json!({"text":"R5"}), w, cx)
                    .unwrap();
                v.try_edit("review.newComment", &json!({"text":"thread"}), w, cx)
                    .unwrap();
                v.open_advanced("批注", cx);
                v.sync_advanced();
                assert_eq!(v.advanced.items.len(), 1);
                v.try_edit("review.reply", &json!({"id":0,"text":"reply"}), w, cx)
                    .unwrap();
                v.sync_advanced();
                assert_eq!(v.advanced.items.len(), 2);
                let revision = v.canvas.reader.as_ref().unwrap().engine().rev();
                v.try_edit("tools.recordMacro", &json!({"name":"one"}), w, cx)
                    .unwrap();
                v.advanced_action("macro-stop", w, cx);
                v.open_advanced("宏", cx);
                v.sync_advanced();
                assert_eq!(v.advanced.items[0]["name"], "one");
                assert_eq!(v.canvas.reader.as_ref().unwrap().engine().rev(), revision);
                v.advanced_action("macro-stop", w, cx);
                assert!(
                    v.canvas
                        .reader
                        .as_ref()
                        .unwrap()
                        .engine()
                        .recording
                        .is_none()
                );
                v.advanced_action("review-new", w, cx);
            })
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        let revision = view.read_with(cx, |v, _| v.canvas.reader.as_ref().unwrap().engine().rev());
        cx.simulate_keystrokes("tab");
        assert_eq!(
            view.read_with(cx, |v, _| v.canvas.reader.as_ref().unwrap().engine().rev()),
            revision
        );
    }
}
