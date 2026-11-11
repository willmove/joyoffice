//! Windows Mailings ribbon with real envelope, label and merge workflows.
use super::ribbon::Tool;
use super::*;
use gpui::component::WindowExt;
use joyoffice_ui::IconName as I;
use serde_json::json;

impl JoyWord {
    pub(super) fn mailings_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let stack = || div().flex().flex_col();
        let create = div()
            .flex()
            .when(
                self.shell.locale == joyword_shell::l10n::Locale::ZhCn,
                |r| {
                    r.child(
                        self.ribbon_tool(
                            "merge-chinese-envelope",
                            Tool::new("中文信封", "Chinese Envelope", I::Mail).width(60.),
                            self.format_ready("mailings.envelopes"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.mailing_form("envelope", w, cx))),
                    )
                },
            )
            .child(
                self.ribbon_tool(
                    "merge-envelope",
                    Tool::new("信封", "Envelopes", I::Mail).width(48.),
                    self.format_ready("mailings.envelopes"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.mailing_form("envelope", w, cx))),
            )
            .child(
                self.ribbon_tool(
                    "merge-labels",
                    Tool::new("标签", "Labels", I::Tags).width(48.),
                    self.format_ready("mailings.labels"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.mailing_form("labels", w, cx))),
            );
        let start = self.format_choices(
            "merge-start-menu",
            self.ribbon_tool(
                "merge-start",
                Tool::new("开始邮件合并", "Start Mail Merge", I::Files).width(70.),
                self.format_ready("mailings.start"),
                true,
            ),
            "mailings.start",
            [
                ("信函", "Letters", "letters"),
                ("电子邮件", "E-mail Messages", "emails"),
                ("信封", "Envelopes", "envelopes"),
                ("标签", "Labels", "labels"),
                ("目录", "Directory", "directory"),
                ("普通 Word 文档", "Normal Word Document", "normal"),
            ]
            .into_iter()
            .map(|(zh, en, kind)| (tr(zh, en).into(), json!({"kind":kind})))
            .collect(),
            cx,
        );
        let recipients = self.home_menu(
            "merge-recipients-menu",
            self.ribbon_tool(
                "merge-recipients",
                Tool::new("选择收件人", "Select Recipients", I::Users).width(68.),
                self.format_ready("mailings.recipients"),
                true,
            ),
            vec![
                (
                    "merge-use-list",
                    tr("使用现有列表…", "Use an Existing List…"),
                    self.format_ready("mailings.recipients"),
                    |v, w, cx| v.advanced_action("merge-recipients", w, cx),
                ),
                (
                    "merge-type-list",
                    tr("键入新列表…", "Type a New List…"),
                    self.format_ready("mailings.recipients"),
                    |v, w, cx| v.mailing_form("recipients", w, cx),
                ),
            ],
            cx,
        );
        let start_group = div()
            .flex()
            .child(start)
            .child(recipients)
            .child(self.ribbon_advanced(
                "merge-list",
                Tool::new("编辑收件人列表", "Edit Recipient List", I::Table).width(80.),
                "mailings.editRecipients",
                cx,
            ));
        let merge = self.canvas.reader.as_ref().map(|r| &r.engine().merge);
        let fields: Vec<_> = merge
            .map(|m| {
                m.headers
                    .iter()
                    .map(|field| (field.clone(), json!({"field":field})))
                    .collect()
            })
            .unwrap_or_default();
        let insert_field = self.format_choices(
            "merge-field-menu",
            self.ribbon_tool(
                "merge-field",
                Tool::new("插入合并域", "Insert Merge Field", I::FileText).width(66.),
                self.format_ready("mailings.insertField") && !fields.is_empty(),
                true,
            ),
            "mailings.insertField",
            fields,
            cx,
        );
        let rules = self.home_menu(
            "merge-rules-menu",
            self.ribbon_tool(
                "merge-rules",
                Tool::compact("规则", "Rules", I::FileSymlink).width(104.),
                self.format_ready("mailings.rules"),
                true,
            ),
            vec![
                (
                    "merge-rule-if",
                    tr("如果…那么…否则…", "If… Then… Else…"),
                    self.format_ready("mailings.rules"),
                    |v, w, cx| v.mailing_form("rule", w, cx),
                ),
                (
                    "merge-rule-next",
                    tr("下一记录", "Next Record"),
                    self.format_ready("mailings.rules"),
                    |v, w, cx| v.run_format("mailings.rules", &json!({"rule":"NEXT"}), w, cx),
                ),
                (
                    "merge-rule-number",
                    tr("合并记录序号", "Merge Record #"),
                    self.format_ready("mailings.rules"),
                    |v, w, cx| v.run_format("mailings.rules", &json!({"rule":"MERGEREC"}), w, cx),
                ),
            ],
            cx,
        );
        let write = div()
            .flex()
            .child(
                self.ribbon_command(
                    "merge-highlight",
                    Tool::new("突出显示合并域", "Highlight Merge Fields", I::Highlighter)
                        .width(74.),
                    "mailings.highlightFields",
                    json!({}),
                    cx,
                )
                .selected(merge.is_some_and(|m| m.highlight)),
            )
            .child(self.ribbon_command(
                "merge-address",
                Tool::new("地址块", "Address Block", I::Contact).width(56.),
                "mailings.addressBlock",
                json!({}),
                cx,
            ))
            .child(self.ribbon_command(
                "merge-greeting",
                Tool::new("问候语", "Greeting Line", I::FileText).width(56.),
                "mailings.greetingLine",
                json!({}),
                cx,
            ))
            .child(insert_field)
            .child(
                stack()
                    .child(rules)
                    .child(
                        self.ribbon_tool(
                            "merge-match",
                            Tool::compact("匹配域", "Match Fields", I::Link).width(104.),
                            self.format_ready("mailings.matchFields"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.mailing_report(false, w, cx))),
                    )
                    .child(self.ribbon_tool(
                        "merge-update-labels",
                        Tool::compact("更新标签", "Update Labels", I::RefreshCw).width(104.),
                        false,
                        false,
                    )),
            );
        let navigation = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(self.ribbon_command(
                "merge-first",
                Tool::icon("首条", "First Record", I::ChevronsLeft).width(24.),
                "mailings.first",
                json!({}),
                cx,
            ))
            .child(self.ribbon_command(
                "merge-prev",
                Tool::icon("上一条", "Previous Record", I::ChevronLeft).width(24.),
                "mailings.previous",
                json!({}),
                cx,
            ))
            .child(
                Button::new("merge-record")
                    .ghost()
                    .xsmall()
                    .w(px(46.))
                    .h(px(21.))
                    .border_1()
                    .border_color(rgb(self.palette().border))
                    .label(merge.map_or(1, |m| m.record + 1).to_string())
                    .accessibility_label(tr("当前记录", "Current Record"))
                    .disabled(!self.format_ready("mailings.goToRecord"))
                    .on_click(cx.listener(|v, _, w, cx| v.mailing_form("record", w, cx))),
            )
            .child(self.ribbon_command(
                "merge-next",
                Tool::icon("下一条", "Next Record", I::ChevronRight).width(24.),
                "mailings.next",
                json!({}),
                cx,
            ))
            .child(self.ribbon_command(
                "merge-last",
                Tool::icon("末条", "Last Record", I::ChevronsRight).width(24.),
                "mailings.last",
                json!({}),
                cx,
            ));
        let preview = div()
            .flex()
            .child(
                self.ribbon_command(
                    "merge-preview",
                    Tool::new("预览结果", "Preview Results", I::FileSearch).width(68.),
                    "mailings.preview",
                    json!({}),
                    cx,
                )
                .selected(merge.is_some_and(|m| m.preview)),
            )
            .child(
                stack()
                    .child(navigation)
                    .child(
                        self.ribbon_tool(
                            "merge-find",
                            Tool::compact("查找收件人", "Find Recipient", I::Search).width(144.),
                            self.format_ready("mailings.findRecipient"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.mailing_form("find", w, cx))),
                    )
                    .child(
                        self.ribbon_tool(
                            "merge-check",
                            Tool::compact("检查错误", "Check for Errors", I::CircleCheck)
                                .width(144.),
                            self.format_ready("mailings.checkErrors"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.mailing_report(true, w, cx))),
                    ),
            );
        let finish = self.home_menu(
            "merge-finish-menu",
            self.ribbon_tool(
                "merge-finish",
                Tool::new("完成并合并", "Finish & Merge", I::Files).width(74.),
                self.format_ready("mailings.finish"),
                true,
            ),
            vec![(
                "merge-export-all",
                tr("合并到新 DOCX…", "Merge to New DOCX…"),
                self.format_ready("mailings.finish"),
                |v, w, cx| v.advanced_action("merge-finish", w, cx),
            )],
            cx,
        );
        div()
            .flex()
            .flex_shrink_0()
            .child(self.ribbon_centered_group("merge-create-group", tr("创建", "Create"), create))
            .child(self.ribbon_centered_group(
                "merge-start-group",
                tr("开始邮件合并", "Start Mail Merge"),
                start_group,
            ))
            .child(self.ribbon_centered_group(
                "merge-write-group",
                tr("编写和插入域", "Write & Insert Fields"),
                write,
            ))
            .child(self.ribbon_centered_group(
                "merge-preview-group",
                tr("预览结果", "Preview Results"),
                preview,
            ))
            .child(self.ribbon_centered_group("merge-finish-group", tr("完成", "Finish"), finish))
    }

    fn output_labels(
        &mut self,
        args: serde_json::Value,
        _w: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<serde_json::Value, String> {
        let job = self
            .canvas
            .reader
            .as_ref()
            .ok_or("请先打开文档")?
            .labels_job(&args)?;
        self.output_job(job, "labels", "docx".into(), cx)?;
        Ok(json!({"saveDialog":true}))
    }

    fn mailing_report(&mut self, errors: bool, w: &mut Window, cx: &mut Context<Self>) {
        let command = if errors {
            "mailings.checkErrors"
        } else {
            "mailings.matchFields"
        };
        if let Ok(result) = self.try_edit(command, &json!({}), w, cx) {
            let text = if errors {
                if result["ok"] == true {
                    format!(
                        "{}: {}",
                        self.shell.tr("检查通过，记录数", "No errors. Records"),
                        result["records"]
                    )
                } else {
                    format!(
                        "{}: {}",
                        self.shell.tr("未匹配的合并域", "Unmatched merge fields"),
                        result["unknownFields"]
                    )
                }
            } else {
                format!(
                    "{}\n{}",
                    self.shell
                        .tr("字段名称 / 地址匹配", "Field names / address mapping"),
                    result
                )
            };
            w.open_dialog(cx, move |d, _, _| {
                d.title(if errors {
                    "检查错误 / Check for Errors"
                } else {
                    "匹配域 / Match Fields"
                })
                .width(px(580.))
                .child(text.clone())
            });
        }
    }

    fn mailing_form(&mut self, kind: &'static str, w: &mut Window, cx: &mut Context<Self>) {
        match kind {
            "envelope"=>self.fields_form("信封 / Envelopes","mailings.envelopes",vec![("收信人地址 / Delivery address",String::new()),("回信地址 / Return address",String::new()),("信封尺寸 / Size (DL / C4 / C5 / C6 / B5 / #10)","DL".into())],false,|v|{
                if v[0].trim().is_empty()||v[0].chars().count()>2000||v[1].chars().count()>2000{return Err("地址需要 1–2000 个字符 / Address text is limited to 2,000 characters".into());}
                // Add to this document; never replace an unsaved template implicitly.
                Ok(json!({"delivery":v[0],"return":v[1],"size":v[2].trim(),"add":true}))
            },w,cx),
            "labels"=>self.fields_action("标签 / Labels","mailings.labels",vec![("标签文字 / Label text",String::new()),("标签产品 / Product (a4-3x7 / a4-2x7 / letter-3x10)","a4-3x7".into())],false,|v|{
                if v[0].trim().is_empty()||v[0].chars().count()>2000{return Err("标签需要 1–2000 个字符 / Labels need 1–2000 characters".into());}
                Ok(json!({"text":v[0],"product":v[1].trim()}))
            },|v,args,w,cx|{
                // Labels create a document. Keep the current session intact and
                // publish a verified copy with the same output workflow as merge.
                v.output_labels(args.clone(),w,cx)
            },w,cx),
            "recipients"=>self.fields_form("新建收件人列表 / New Recipient List","mailings.recipients",vec![("字段名（逗号分隔） / Field names (comma-separated)",String::new()),("第一条记录（逗号分隔） / First record (CSV)",String::new())],false,|v|{
                if v.iter().any(|s|s.trim().is_empty()||s.len()>16384||s.contains(['\r','\n','\0'])){return Err("请填写字段名和第一条记录 / Enter field names and the first record".into());}
                Ok(json!({"csv":format!("{}\n{}",v[0],v[1])}))
            },w,cx),
            "rule"=>self.fields_form("如果…那么…否则… / If… Then… Else…","mailings.rules",vec![("字段 / Field",String::new()),("等于 / Equals",String::new()),("则插入 / Insert if true",String::new()),("否则插入 / Insert if false",String::new())],false,|v|{
                if v[0].trim().is_empty()||v.iter().any(|x|x.len()>2048||x.contains(['\0','\r','\n','"','{','}'])){return Err("字段和值不能含引号、花括号或换行 / Field values cannot contain quotes, braces or line breaks".into());}
                Ok(json!({"rule":"IF","field":v[0].trim(),"value":v[1],"then":v[2],"else":v[3]}))
            },w,cx),
            "record"=>self.fields_form("转到记录 / Go to Record","mailings.goToRecord",vec![("记录序号 / Record number",self.canvas.reader.as_ref().map_or(1,|r|r.engine().merge.record+1).to_string())],false,|v|Ok(json!({"record":joyword_shell::authoring::integer(&v[0],1,1_000_000)?})),w,cx),
            "find"=>self.fields_form("查找收件人 / Find Recipient","mailings.findRecipient",vec![("查找文字 / Find text",String::new())],false,|v|{
                if v[0].trim().is_empty()||v[0].len()>1024{return Err("查找文字需要 1–1024 字节 / Find text must be 1–1024 bytes".into());}
                Ok(json!({"text":v[0]}))
            },w,cx),
            _=>{}
        }
    }
}
