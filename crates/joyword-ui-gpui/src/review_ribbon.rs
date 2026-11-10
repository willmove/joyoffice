//! Windows-style Review groups over JoyWord's existing review commands and panes.
use super::ribbon::Tool;
use super::*;
use gpui::component::{Icon, WindowExt, popover::Popover};
use joyoffice_ui::IconName as I;
use serde_json::{Value, json};

#[derive(Clone)]
enum Action {
    Command(&'static str, Value),
    Advanced(&'static str, &'static str),
    Pane(&'static str, &'static str),
    Proofing(&'static str),
    Language,
    Unavailable,
}

impl Action {
    fn command(&self) -> Option<&'static str> {
        match self {
            Self::Command(id, _)
            | Self::Advanced(_, id)
            | Self::Pane(_, id)
            | Self::Proofing(id) => Some(id),
            Self::Language => Some("review.language"),
            Self::Unavailable => None,
        }
    }
}

struct Entry {
    label: &'static str,
    icon: I,
    action: Action,
    checked: bool,
}

impl Entry {
    fn checked(mut self, value: bool) -> Self {
        self.checked = value;
        self
    }
}

impl JoyWord {
    fn review_ready(&self, action: &Action) -> bool {
        action.command().is_some_and(|id| self.format_ready(id))
    }

    fn review_button(
        &self,
        id: &'static str,
        tool: Tool,
        action: Action,
        cx: &Context<Self>,
    ) -> Button {
        self.ribbon_tool(id, tool, self.review_ready(&action), false)
            .on_click(cx.listener(move |v, _, w, cx| v.perform_review(&action, w, cx)))
    }

    fn review_menu(
        &self,
        id: &'static str,
        trigger: Button,
        entries: Vec<Entry>,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let owner = cx.weak_entity();
        let target = self.ribbon_target();
        let unavailable = self.shell.tr(
            "当前文档或版本不可用",
            "Unavailable in this document or build",
        );
        let items: Vec<_> = entries
            .into_iter()
            .map(|entry| {
                let ready = self.review_ready(&entry.action);
                (entry, ready)
            })
            .collect();
        Popover::new((id, 0usize))
            .trigger(trigger.disabled(!items.iter().any(|(_, ready)| *ready)))
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                div()
                    .flex()
                    .flex_col()
                    .w(px(244.))
                    .children(items.iter().enumerate().map(|(index, (entry, ready))| {
                        let (owner, popup, target, action) = (
                            owner.clone(),
                            popup.clone(),
                            target.clone(),
                            entry.action.clone(),
                        );
                        Button::new((id, index + 2))
                            .ghost()
                            .xsmall()
                            .h(px(25.))
                            .w_full()
                            .disabled(!ready)
                            .selected(entry.checked)
                            .accessibility_label(entry.label)
                            .tooltip(if *ready { entry.label } else { unavailable })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .gap_2()
                                    .child(Icon::new(entry.icon).size(px(15.)))
                                    .child(div().flex_1().child(entry.label))
                                    .when(entry.checked, |row| {
                                        row.child(Icon::new(I::Check).size(px(13.)))
                                    }),
                            )
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                let _ = owner.update(cx, |v, cx| {
                                    if v.ribbon_target() == target {
                                        v.perform_review(&action, w, cx);
                                    }
                                });
                            })
                    }))
            })
    }

    fn review_split(
        &self,
        id: &'static str,
        tool: Tool,
        main: Action,
        entries: Vec<Entry>,
        selected: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let menu_label = format!(
            "{} {}",
            tool.label(&self.shell),
            self.shell.tr("选项", "Options")
        );
        div()
            .flex()
            .flex_col()
            .child(
                self.review_button(id, tool.split().width(49.), main, cx)
                    .selected(selected),
            )
            .child(
                self.review_menu(
                    id,
                    Button::new((id, 1usize))
                        .ghost()
                        .xsmall()
                        .w(px(49.))
                        .h(px(16.))
                        .p_0()
                        .icon(I::ChevronDown)
                        .tooltip(menu_label.clone())
                        .accessibility_label(menu_label),
                    entries,
                    cx,
                ),
            )
    }

    pub(super) fn review_ribbon(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        use Action::{Advanced as A, Command as C, Pane, Proofing as P, Unavailable as U};
        let tr = |zh, en| self.shell.tr(zh, en);
        let chinese_ui = self.shell.locale == joyword_shell::l10n::Locale::ZhCn;
        let width = |zh, en| if chinese_ui { zh } else { en };
        let row = || div().flex().items_center().gap(px(2.));
        let stack = || div().flex().flex_col();
        let entry = |zh, en, icon, action| Entry {
            label: tr(zh, en),
            icon,
            action,
            checked: false,
        };
        let button = |id, tool, action| self.review_button(id, tool, action, cx);
        let menu = |id, tool, entries: Vec<Entry>| {
            let enabled = entries.iter().any(|entry| self.review_ready(&entry.action));
            self.review_menu(
                id,
                self.ribbon_tool((id, 1usize), tool, enabled, true),
                entries,
                cx,
            )
        };
        let engine = self.canvas.reader.as_ref().map(|r| r.engine());
        let tracking = engine.is_some_and(|e| e.doc.settings.track_changes);
        let markup = engine.is_some_and(|e| e.view.show_markup);
        let has_comments = engine.is_some_and(|e| !e.doc.comments.is_empty());

        let proofing = row()
            .child(button(
                "review-spelling",
                Tool::tile("拼写和语法", "Spelling & Grammar", I::SpellCheck)
                    .width(width(69., 75.)),
                P("review.spelling"),
            ))
            .child(
                stack()
                    .child(button(
                        "review-thesaurus",
                        Tool::compact("同义词库", "Thesaurus", I::BookOpen).width(width(79., 96.)),
                        P("review.thesaurus"),
                    ))
                    .child(button(
                        "review-count",
                        Tool::compact("字数统计", "Word Count", I::FileText).width(width(79., 96.)),
                        P("review.wordCount"),
                    )),
            );
        let speech = button(
            "review-read-aloud",
            Tool::tile("大声朗读", "Read Aloud", I::Volume2).width(58.),
            U,
        );
        let accessibility = button(
            "review-accessibility",
            Tool::tile("检查辅助功能", "Check Accessibility", I::Accessibility).width(83.),
            U,
        );
        let language = row()
            .child(button(
                "review-translate",
                Tool::tile("翻译", "Translate", I::Languages).width(58.),
                U,
            ))
            .child(menu(
                "review-language",
                Tool::tile("语言", "Language", I::Type).width(62.),
                vec![entry(
                    "设置校对语言…",
                    "Set Proofing Language…",
                    I::SpellCheck,
                    Action::Language,
                )],
            ));
        let chinese = stack()
            .child(button(
                "review-traditional",
                Tool::compact("简转繁", "To Traditional", I::Languages).width(74.),
                U,
            ))
            .child(button(
                "review-simplified",
                Tool::compact("繁转简", "To Simplified", I::Languages).width(74.),
                U,
            ));

        let comment_nav = |command| {
            if has_comments {
                C(command, json!({}))
            } else {
                U
            }
        };
        let comments = row()
            .child(
                stack()
                    .child(button(
                        "review-new",
                        Tool::compact("新建批注", "New Comment", I::MessageSquarePlus)
                            .width(width(84., 111.)),
                        A("review-new", "review.newComment"),
                    ))
                    .child(menu(
                        "review-delete-menu",
                        Tool::compact("删除", "Delete", I::MessageSquareX).width(width(84., 111.)),
                        vec![
                            entry(
                                "删除当前批注",
                                "Delete Current Comment",
                                I::MessageSquareX,
                                comment_nav("review.deleteComment"),
                            ),
                            entry(
                                "删除文档中的所有批注",
                                "Delete All Comments in Document",
                                I::Trash,
                                if has_comments {
                                    C("review.deleteComment", json!({"all":true}))
                                } else {
                                    U
                                },
                            ),
                        ],
                    ))
                    .child(button(
                        "review-prev-comment",
                        Tool::compact("上一条", "Previous", I::ArrowUp).width(width(84., 111.)),
                        comment_nav("review.previousComment"),
                    )),
            )
            .child(
                stack()
                    .child(button(
                        "review-next-comment",
                        Tool::compact("下一条", "Next", I::ArrowDown).width(width(88., 117.)),
                        comment_nav("review.nextComment"),
                    ))
                    .child(
                        button(
                            "review-comments",
                            Tool::compact("显示批注", "Show Comments", I::MessageSquareText)
                                .width(width(88., 117.)),
                            Pane("批注", "review.comments"),
                        )
                        .selected(self.advanced.pane == Some("批注")),
                    ),
            );

        let display_entries = vec![
            entry("简单标记", "Simple Markup", I::FileText, U),
            entry(
                "所有标记",
                "All Markup",
                I::FileText,
                C("review.markup", json!({"value":"allMarkup"})),
            )
            .checked(markup),
            entry(
                "无标记",
                "No Markup",
                I::File,
                C("review.markup", json!({"value":"noMarkup"})),
            )
            .checked(!markup),
            entry("原始版本", "Original", I::FileClock, U),
        ];
        let display_tool = if markup {
            Tool::compact("所有标记", "All Markup", I::FileText)
        } else {
            Tool::compact("无标记", "No Markup", I::File)
        };
        let display = self.review_menu(
            "review-display",
            self.ribbon_tool(
                "review-display-trigger",
                display_tool.width(width(105., 132.)),
                true,
                true,
            )
            .border_1()
            .border_color(rgb(self.palette().border))
            .pl(px(3.)),
            display_entries,
            cx,
        );
        let options = engine.map(|e| &e.prefs.markup);
        let filters = [
            (
                "批注",
                "Comments",
                "comments",
                I::MessageSquare,
                options.is_some_and(|m| m.comments),
            ),
            (
                "插入和删除",
                "Insertions and Deletions",
                "insertionsDeletions",
                I::FileDiff,
                options.is_some_and(|m| m.insertions_deletions),
            ),
            (
                "格式",
                "Formatting",
                "formatting",
                I::Type,
                options.is_some_and(|m| m.formatting),
            ),
            ("墨迹", "Ink", "ink", I::Pen, options.is_some_and(|m| m.ink)),
        ]
        .into_iter()
        .map(|(zh, en, key, icon, checked)| {
            entry(
                zh,
                en,
                icon,
                C("review.trackingOptions", json!({key:!checked})),
            )
            .checked(checked)
        })
        .collect();
        let mark = row()
            .child(button("review-all-markup", Tool::tile("重置筛选", "Reset Filters", I::ListFilter).width(68.),
                C("review.trackingOptions", json!({"comments":true,"insertionsDeletions":true,"formatting":true,"ink":true}))))
            .child(stack()
                .child(display)
                .child(menu("review-markup", Tool::compact("显示标记", "Show Markup", I::Eye).width(width(105., 132.)), filters))
                .child(button("review-changes", Tool::compact("审阅窗格", "Reviewing Pane", I::PanelRight).width(width(105., 132.)), Pane("修订", "review.changes"))
                    .selected(self.advanced.pane == Some("修订"))));

        let track = self.review_split(
            "review-track",
            Tool::tile("修订", "Track", I::FilePenLine),
            C("review.trackChanges", json!({})),
            vec![
                entry(
                    "启用修订",
                    "Track Changes On",
                    I::FilePenLine,
                    C("review.trackChanges", json!({"value":true})),
                )
                .checked(tracking),
                entry(
                    "关闭修订",
                    "Track Changes Off",
                    I::File,
                    C("review.trackChanges", json!({"value":false})),
                )
                .checked(!tracking),
            ],
            tracking,
            cx,
        );
        let accept = self.review_split(
            "review-accept",
            Tool::tile("接受", "Accept", I::FileCheck),
            C("review.accept", json!({"next":true})),
            vec![
                entry(
                    "接受并移到下一处",
                    "Accept and Move to Next",
                    I::FileCheck,
                    C("review.accept", json!({"next":true})),
                ),
                entry(
                    "接受此修订",
                    "Accept This Change",
                    I::FileCheck,
                    C("review.accept", json!({})),
                ),
                entry(
                    "接受所有修订",
                    "Accept All Changes",
                    I::CheckCheck,
                    C("review.acceptAll", json!({})),
                ),
                entry(
                    "接受所有修订并停止修订",
                    "Accept All and Stop Tracking",
                    I::CheckCheck,
                    C("review.acceptAll", json!({"stopTracking":true})),
                ),
            ],
            false,
            cx,
        );
        let reject = self.review_split(
            "review-reject",
            Tool::tile("拒绝", "Reject", I::FileX),
            C("review.reject", json!({"next":true})),
            vec![
                entry(
                    "拒绝并移到下一处",
                    "Reject and Move to Next",
                    I::FileX,
                    C("review.reject", json!({"next":true})),
                ),
                entry(
                    "拒绝此修订",
                    "Reject This Change",
                    I::FileX,
                    C("review.reject", json!({})),
                ),
                entry(
                    "拒绝所有修订",
                    "Reject All Changes",
                    I::X,
                    C("review.rejectAll", json!({})),
                ),
                entry(
                    "拒绝所有修订并停止修订",
                    "Reject All and Stop Tracking",
                    I::X,
                    C("review.rejectAll", json!({"stopTracking":true})),
                ),
            ],
            false,
            cx,
        );
        let changes = row().child(track).child(accept).child(reject).child(
            stack()
                .child(button(
                    "review-prev",
                    Tool::compact("上一处", "Previous", I::ArrowUp).width(width(65., 80.)),
                    C("review.previousChange", json!({})),
                ))
                .child(button(
                    "review-next",
                    Tool::compact("下一处", "Next", I::ArrowDown).width(width(65., 80.)),
                    C("review.nextChange", json!({})),
                )),
        );
        let compare = menu(
            "review-compare-options",
            Tool::tile("比较", "Compare", I::Files).width(63.),
            vec![
                entry(
                    "比较文档…",
                    "Compare Documents…",
                    I::FileDiff,
                    A("review-compare", "review.compare"),
                ),
                entry(
                    "合并修订…",
                    "Combine Revisions…",
                    I::Files,
                    A("review-combine", "review.combine"),
                ),
            ],
        );
        let protect = button(
            "review-protect",
            Tool::tile("保护", "Protect", I::ShieldLock).width(53.),
            U,
        );
        let ink = button(
            "review-hide-ink",
            Tool::tile("隐藏墨迹", "Hide Ink", I::PenOff).width(60.),
            C("review.hideInk", json!({})),
        )
        .selected(engine.is_some_and(|e| e.view.hide_ink));
        let ribbon = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .child(self.ribbon_centered_group(
                "review-group-proofing",
                tr("校对", "Proofing"),
                proofing,
            ))
            .child(self.ribbon_centered_group("review-group-speech", tr("语音", "Speech"), speech))
            .child(self.ribbon_centered_group(
                "review-group-accessibility",
                tr("辅助功能", "Accessibility"),
                accessibility,
            ))
            .child(self.ribbon_centered_group(
                "review-group-language",
                tr("语言", "Language"),
                language,
            ));
        ribbon
            .when(chinese_ui, |ribbon| {
                ribbon.child(self.ribbon_centered_group(
                    "review-group-chinese",
                    "中文简繁转换",
                    chinese,
                ))
            })
            .child(self.ribbon_centered_group(
                "review-group-comments",
                tr("批注", "Comments"),
                comments,
            ))
            .child(self.ribbon_centered_group("review-group-markup", tr("标记", "Markup"), mark))
            .child(self.ribbon_centered_group(
                "review-group-changes",
                tr("修订", "Changes"),
                changes,
            ))
            .child(self.ribbon_centered_group(
                "review-group-compare",
                tr("比较", "Compare"),
                compare,
            ))
            .child(self.ribbon_centered_group(
                "review-group-protect",
                tr("保护", "Protect"),
                protect,
            ))
            .child(self.ribbon_centered_group("review-group-ink", tr("墨迹", "Ink"), ink))
    }

    fn perform_review(&mut self, action: &Action, w: &mut Window, cx: &mut Context<Self>) {
        if !self.review_ready(action) {
            return;
        }
        match action {
            Action::Command(id, args) => self.run_format(id, args, w, cx),
            Action::Advanced(id, _) => self.advanced_action(id, w, cx),
            Action::Pane(pane, _) => {
                if self.advanced.pane == Some(pane) {
                    self.advanced.pane = None;
                    cx.notify();
                } else {
                    self.open_advanced(pane, cx);
                }
            }
            Action::Proofing(id) => self.review_proofing(id, w, cx),
            Action::Language => {
                let label = self.shell.tr(
                    "校对语言（例如 zh-CN 或 en-US）",
                    "Proofing language (e.g. zh-CN or en-US)",
                );
                let current = self.format_state().character["lang"]
                    .as_str()
                    .unwrap_or("")
                    .to_owned();
                self.fields_form(
                    self.shell.tr("设置校对语言", "Set Proofing Language"),
                    "review.language",
                    vec![(label, current)],
                    false,
                    |values| Ok(json!({"lang":values[0].trim()})),
                    w,
                    cx,
                );
            }
            Action::Unavailable => {}
        }
    }

    fn review_proofing(&mut self, id: &str, w: &mut Window, cx: &mut Context<Self>) {
        let Ok(result) = self.try_edit(id, &json!({}), w, cx) else {
            return;
        };
        let tr = |zh, en| self.shell.tr(zh, en);
        let (title, rows) = match id {
            "review.wordCount" => (
                tr("字数统计", "Word Count"),
                [
                    (tr("页数", "Pages"), "pages"),
                    (tr("字数", "Words"), "words"),
                    (
                        tr("字符数（不计空格）", "Characters (no spaces)"),
                        "characters",
                    ),
                    (
                        tr("字符数（计空格）", "Characters (with spaces)"),
                        "charactersWithSpaces",
                    ),
                    (tr("段落数", "Paragraphs"), "paragraphs"),
                    (tr("行数", "Lines"), "lines"),
                ]
                .into_iter()
                .map(|(label, key)| (label, result[key].to_string()))
                .collect::<Vec<_>>(),
            ),
            "review.thesaurus" => (
                tr("同义词库", "Thesaurus"),
                vec![
                    (
                        tr("单词", "Word"),
                        result["word"].as_str().unwrap_or("").to_owned(),
                    ),
                    (
                        tr("同义词", "Synonyms"),
                        review_suggestions(
                            &result["synonyms"],
                            tr("未找到同义词", "No synonyms found"),
                        ),
                    ),
                ],
            ),
            _ => (
                tr("拼写和语法", "Spelling & Grammar"),
                if result["done"] == true {
                    vec![("", tr("检查完毕", "The check is complete.").into())]
                } else {
                    vec![
                        (
                            tr("发现的问题", "Issue"),
                            result["text"].as_str().unwrap_or("").to_owned(),
                        ),
                        (
                            tr("建议", "Suggestions"),
                            review_suggestions(
                                &result["suggestions"],
                                tr("没有建议", "No suggestions"),
                            ),
                        ),
                    ]
                },
            ),
        };
        w.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title)
                .width(px(390.))
                .child(div().flex().flex_col().gap_2().children(rows.iter().map(
                    |(label, value)| {
                        div()
                            .flex()
                            .justify_between()
                            .gap_3()
                            .child(*label)
                            .child(value.clone())
                    },
                )))
        });
    }
}

fn review_suggestions(value: &Value, empty: &str) -> String {
    let words = value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if words.is_empty() {
        empty.to_owned()
    } else {
        words.join(", ")
    }
}
