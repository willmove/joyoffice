//! Windows References groups and menus over the existing reference model.
use super::ribbon::Tool;
use super::*;
use joyoffice_ui::IconName as I;
use serde_json::{Value, json};

impl JoyWord {
    pub(super) fn ribbon_command(
        &self,
        id: &'static str,
        tool: Tool,
        command: &'static str,
        args: Value,
        cx: &Context<Self>,
    ) -> Button {
        self.ribbon_tool(id, tool, self.format_ready(command), false)
            .on_click(cx.listener(move |v, _, w, cx| v.run_format(command, &args, w, cx)))
    }

    pub(super) fn ribbon_advanced(
        &self,
        id: &'static str,
        tool: Tool,
        command: &'static str,
        cx: &Context<Self>,
    ) -> Button {
        self.ribbon_tool(id, tool, self.format_ready(command), false)
            .on_click(cx.listener(move |v, _, w, cx| v.advanced_action(id, w, cx)))
    }

    pub(super) fn references_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let stack = || div().flex().flex_col();
        let toc = self.home_menu(
            "refs-toc-menu",
            self.ribbon_tool(
                "refs-toc",
                Tool::new("目录", "Table of Contents", I::List),
                self.format_ready("references.toc"),
                true,
            ),
            vec![
                (
                    "toc-automatic-1",
                    tr("自动目录 1", "Automatic Table 1"),
                    self.format_ready("references.toc"),
                    |v, w, cx| {
                        v.run_format(
                            "references.toc",
                            &json!({"title":v.shell.tr("目录","Contents"),"levels":3}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "toc-automatic-2",
                    tr("自动目录 2", "Automatic Table 2"),
                    self.format_ready("references.toc"),
                    |v, w, cx| {
                        v.run_format(
                            "references.toc",
                            &json!({"title":v.shell.tr("内容","Table of Contents"),"levels":3}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "toc-custom",
                    tr("自定义目录…", "Custom Table of Contents…"),
                    self.format_ready("references.toc"),
                    |v, w, cx| v.advanced_action("refs-toc", w, cx),
                ),
                (
                    "toc-remove",
                    tr("删除目录", "Remove Table of Contents"),
                    self.format_ready("references.removeToc"),
                    |v, w, cx| v.run_format("references.removeToc", &json!({}), w, cx),
                ),
            ],
            cx,
        );
        let add_text = self.format_choices(
            "refs-level-menu",
            self.ribbon_tool(
                "refs-level",
                Tool::compact("添加文字", "Add Text", I::Plus).width(96.),
                self.format_ready("references.addText"),
                true,
            ),
            "references.addText",
            (0..=3)
                .map(|level| {
                    (
                        if level == 0 {
                            tr("不在目录中显示", "Do Not Show in Table").into()
                        } else {
                            format!("{} {level}", tr("级别", "Level"))
                        },
                        json!({"level":level}),
                    )
                })
                .collect(),
            cx,
        );
        let toc_body =
            div()
                .flex()
                .child(toc)
                .child(stack().child(add_text).child(self.ribbon_command(
                    "refs-update-toc",
                    Tool::compact("更新目录", "Update Table", I::RefreshCw).width(96.),
                    "references.updateToc",
                    json!({}),
                    cx,
                )));
        let notes = div()
            .flex()
            .child(self.ribbon_advanced(
                "refs-footnote",
                Tool::new("插入脚注", "Insert Footnote", I::Type).width(64.),
                "references.footnote",
                cx,
            ))
            .child(
                stack()
                    .child(self.ribbon_advanced(
                        "refs-endnote",
                        Tool::compact("插入尾注", "Insert Endnote", I::FileText).width(112.),
                        "references.endnote",
                        cx,
                    ))
                    .child(self.ribbon_command(
                        "refs-next-note",
                        Tool::compact("下一条脚注", "Next Footnote", I::ArrowDown).width(112.),
                        "references.nextFootnote",
                        json!({}),
                        cx,
                    ))
                    .child(self.ribbon_advanced(
                        "refs-notes",
                        Tool::compact("显示备注", "Show Notes", I::List).width(112.),
                        "references.notes",
                        cx,
                    )),
            );
        let launcher = Button::new("refs-note-options")
            .ghost()
            .xsmall()
            .p_0()
            .w(px(14.))
            .h(px(16.))
            .icon(I::ArrowDownRight)
            .accessibility_label(tr("脚注和尾注设置", "Footnote and Endnote Settings"))
            .disabled(!self.format_ready("references.noteOptions"))
            .on_click(cx.listener(|v, _, w, cx| v.note_options_form(w, cx)));
        let style = self
            .canvas
            .reader
            .as_ref()
            .map(|r| r.engine().bib_style.as_str())
            .unwrap_or("APA");
        let styles = self.format_choices(
            "refs-bib-style-menu",
            self.ribbon_tool(
                "refs-bib-style",
                Tool::compact("样式", "Style", I::BookOpen).width(112.),
                self.format_ready("references.citationStyle"),
                true,
            )
            .child(div().text_size(px(11.)).child(style.to_owned())),
            "references.citationStyle",
            joyword_shell::authoring::CITATION_STYLES
                .iter()
                .map(|style| (style.name.into(), json!({"style":style.name})))
                .collect(),
            cx,
        );
        let bibliography = self.home_menu(
            "refs-bibliography-menu",
            self.ribbon_tool(
                "refs-bibliography",
                Tool::compact("书目", "Bibliography", I::BookOpen).width(112.),
                self.format_ready("references.bibliography"),
                true,
            ),
            vec![
                (
                    "bib-insert",
                    tr("参考文献", "Bibliography"),
                    self.format_ready("references.bibliography"),
                    |v, w, cx| {
                        v.run_format(
                            "references.bibliography",
                            &json!({"title":v.shell.tr("参考文献","Bibliography")}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "bib-custom",
                    tr("自定义书目…", "Custom Bibliography…"),
                    self.format_ready("references.bibliography"),
                    |v, w, cx| v.advanced_action("refs-bibliography", w, cx),
                ),
                (
                    "bib-new-source",
                    tr("添加新源…", "Add New Source…"),
                    self.format_ready("references.sources"),
                    |v, w, cx| v.advanced_action("refs-new-source", w, cx),
                ),
            ],
            cx,
        );
        let citations = div()
            .flex()
            .child(self.ribbon_advanced(
                "refs-citation",
                Tool::new("插入引文", "Insert Citation", I::BookOpen).width(64.),
                "references.citation",
                cx,
            ))
            .child(
                stack()
                    .child(self.ribbon_advanced(
                        "refs-sources",
                        Tool::compact("管理源", "Manage Sources", I::Library).width(112.),
                        "references.sources",
                        cx,
                    ))
                    .child(styles)
                    .child(bibliography),
            );
        let captions = div()
            .flex()
            .child(self.ribbon_advanced(
                "refs-caption",
                Tool::new("插入题注", "Insert Caption", I::FileText).width(64.),
                "references.caption",
                cx,
            ))
            .child(
                stack()
                    .child(self.ribbon_advanced(
                        "refs-figures",
                        Tool::compact("插入表目录", "Insert Table of Figures", I::List).width(112.),
                        "references.tableOfFigures",
                        cx,
                    ))
                    .child(self.ribbon_command(
                        "refs-update-figures",
                        Tool::compact("更新表格", "Update Table", I::RefreshCw).width(112.),
                        "references.updateFigures",
                        json!({}),
                        cx,
                    ))
                    .child(self.ribbon_advanced(
                        "refs-crossref",
                        Tool::compact("交叉引用", "Cross-reference", I::Link).width(112.),
                        "insert.crossReference",
                        cx,
                    )),
            );
        let index = div()
            .flex()
            .child(self.ribbon_advanced(
                "refs-mark-entry",
                Tool::new("标记索引项", "Mark Entry", I::Bookmark).width(64.),
                "references.markEntry",
                cx,
            ))
            .child(
                stack()
                    .child(self.ribbon_command(
                        "refs-index",
                        Tool::compact("插入索引", "Insert Index", I::List).width(96.),
                        "references.index",
                        json!({}),
                        cx,
                    ))
                    .child(self.ribbon_command(
                        "refs-update-index",
                        Tool::compact("更新索引", "Update Index", I::RefreshCw).width(96.),
                        "references.updateIndex",
                        json!({}),
                        cx,
                    )),
            );
        let authorities = div()
            .flex()
            .child(self.ribbon_advanced(
                "refs-mark-authority",
                Tool::new("标记引文", "Mark Citation", I::Bookmark).width(64.),
                "references.markCitation",
                cx,
            ))
            .child(
                stack()
                    .child(
                        self.ribbon_command(
                            "refs-authorities",
                            Tool::compact("插入引文目录", "Insert Table of Authorities", I::List)
                                .width(120.),
                            "references.tableOfAuthorities",
                            json!({}),
                            cx,
                        ),
                    )
                    .child(self.ribbon_command(
                        "refs-update-authorities",
                        Tool::compact("更新引文目录", "Update Table", I::RefreshCw).width(120.),
                        "references.updateAuthorities",
                        json!({}),
                        cx,
                    )),
            );
        div()
            .flex()
            .flex_shrink_0()
            .child(self.ribbon_centered_group(
                "refs-toc-group",
                tr("目录", "Table of Contents"),
                toc_body,
            ))
            .child(self.ribbon_group_with_launcher(
                "refs-notes-group",
                tr("脚注", "Footnotes"),
                notes,
                Some(launcher),
            ))
            .child(self.ribbon_centered_group(
                "refs-citations-group",
                tr("引文与书目", "Citations & Bibliography"),
                citations,
            ))
            .child(self.ribbon_centered_group(
                "refs-captions-group",
                tr("题注", "Captions"),
                captions,
            ))
            .child(self.ribbon_centered_group("refs-index-group", tr("索引", "Index"), index))
            .child(self.ribbon_centered_group(
                "refs-authorities-group",
                tr("引文目录", "Table of Authorities"),
                authorities,
            ))
    }

    fn note_options_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form("脚注和尾注 / Footnotes and Endnotes","references.noteOptions",vec![
            ("类型 / Kind (footnote / endnote)","footnote".into()),
            ("起始编号 / Start at","1".into()),
            ("编号格式 / Format (decimal / lowerRoman / upperRoman / lowerLetter / upperLetter)","decimal".into())],false,|v|{
            if !["footnote","endnote"].contains(&v[0].trim()){return Err("类型应为 footnote 或 endnote".into());}
            if !["decimal","lowerRoman","upperRoman","lowerLetter","upperLetter"].contains(&v[2].trim()){return Err("无效编号格式 / Invalid numbering format".into());}
            Ok(json!({"kind":v[0].trim(),"numStart":joyword_shell::authoring::integer(&v[1],1,32767)?,"numFmt":v[2].trim()}))
        },w,cx);
    }
}
