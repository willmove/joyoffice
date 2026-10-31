//! Familiar spreadsheet command hierarchy; all actions retain the existing edit/engine path.
use crate::desktop::{After, Desktop};
use gpui_kit::{prelude::*, *};
use serde_json::json;

impl Desktop {
    fn run_ribbon_command(&mut self, command: &str, w: &mut Window, cx: &mut Context<Self>) {
        let params = if command == "edit.paste" {
            self.clipboard.read_external().map(|text| json!({"text":text})).unwrap_or_else(|| json!({}))
        } else if command == "edit.copy" || command == "edit.cut" {
            json!({"html":true})
        } else {
            json!({})
        };
        self.commit(After::Command(command.into(), params), w, cx);
    }

    fn ribbon_action(
        &self,
        id: &'static str,
        zh: &'static str,
        en: &'static str,
        command: &'static str,
        dialog: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = self.label(zh, en);
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label.clone())
            .tab_stop(true)
            .h(px(28.))
            .px_2()
            .flex()
            .items_center()
            .rounded_sm()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(if self.preferences.dark { 0x35443d } else { 0xe7eeea })))
            .child(label)
            .on_key_down(cx.listener(move |this, e: &KeyDownEvent, w, cx| {
                if e.keystroke.key == "enter" || e.keystroke.key == "space" {
                    if dialog {
                        this.open_command(command, w, cx);
                    } else {
                        this.run_ribbon_command(command, w, cx);
                    }
                    cx.stop_propagation();
                }
            }))
            .on_click(cx.listener(move |this, _, w, cx| {
                if dialog {
                    this.open_command(command, w, cx);
                } else {
                    this.run_ribbon_command(command, w, cx);
                }
            }))
            .into_any_element()
    }
    fn ribbon_group(&self, zh: &'static str, en: &'static str, buttons: Vec<AnyElement>) -> AnyElement {
        div()
            .flex_shrink_0()
            .h_full()
            .px_3()
            .border_r_1()
            .border_color(rgb(if self.preferences.dark { 0x3b4942 } else { 0xdfe3e0 }))
            .flex()
            .flex_col()
            .justify_between()
            .child(div().flex().items_center().gap_1().children(buttons))
            .child(div().text_xs().text_center().text_color(rgb(if self.preferences.dark { 0xaebbb4 } else { 0x68756e })).child(self.label(zh, en)))
            .into_any_element()
    }
    pub fn render_document_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = self.view().map(|v| v.title.clone()).unwrap_or_else(|| self.label("工作簿", "Workbook"));
        div()
            .h(px(42.))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .gap_3()
            .bg(rgb(if self.preferences.dark { 0x1d3529 } else { 0xe9f0ec }))
            .child(div().font_weight(FontWeight::BOLD).text_color(rgb(if self.preferences.dark { 0x8cdbb3 } else { 0x146c43 })).child("Joysheet"))
            .child(self.toolbar_button("new", self.label("新建", "New"), "file.new", json!({}), cx))
            .child(div().id("open").cursor_pointer().child(self.label("打开", "Open")).on_click(cx.listener(|this, _, w, cx| this.pick_open(w, cx))))
            .child(
                div()
                    .id("save")
                    .role(Role::Button)
                    .aria_label(self.label("保存", "Save"))
                    .cursor_pointer()
                    .px_2()
                    .child(self.label("保存", "Save"))
                    .on_click(cx.listener(|this, _, w, cx| this.commit(After::Save(false), w, cx))),
            )
            .child(self.toolbar_button("undo", "↶", "edit.undo", json!({}), cx))
            .child(self.toolbar_button("redo", "↷", "edit.redo", json!({}), cx))
            .child(div().w(px(1.)).h(px(18.)).bg(rgb(0xbaccc0)))
            .child(div().max_w(px(280.)).overflow_hidden().font_weight(FontWeight::SEMIBOLD).child(title))
            .when(self.view().is_some_and(|v| v.dirty), |el| el.child(div().text_color(rgb(0x148256)).child("●")))
            .child(div().flex_1())
            .child(
                div()
                    .id("command-palette")
                    .role(Role::Button)
                    .aria_label(self.label("搜索命令", "Search commands"))
                    .w(px(236.))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(if self.preferences.dark { 0x293e32 } else { 0xf7faf8 }))
                    .cursor_pointer()
                    .text_color(rgb(if self.preferences.dark { 0xc1cfc6 } else { 0x617369 }))
                    .child(self.label("搜索命令   Ctrl+K", "Search commands   Ctrl+K"))
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.palette_open = true;
                        this.search.update(cx, |s, cx| s.focus(w, cx));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("autosave")
                    .cursor_pointer()
                    .text_xs()
                    .child(if self.preferences.autosave {
                        self.label("自动保存：开", "AutoSave: on")
                    } else {
                        self.label("自动保存：关", "AutoSave: off")
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.preferences.autosave = !this.preferences.autosave;
                        this.autosave_revision = None;
                        this.persist_preferences(cx);
                        cx.notify();
                    })),
            )
            .child(div().id("open-settings").cursor_pointer().child(self.label("设置", "Settings")).on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 20;
                cx.notify();
            })))
            .child(div().id("language").cursor_pointer().child(if self.chinese() { "EN" } else { "中文" }).on_click(cx.listener(|this, _, _, cx| {
                this.preferences.language = if this.chinese() { "en".into() } else { "zh".into() };
                this.persist_preferences(cx);
                cx.notify();
            })))
            .child(div().id("theme").cursor_pointer().child(if self.preferences.dark { "☀" } else { "◐" }).on_click(cx.listener(|this, _, w, cx| {
                this.preferences.dark = !this.preferences.dark;
                gpui_kit::component::Theme::change(
                    if this.preferences.dark { gpui_kit::component::ThemeMode::Dark } else { gpui_kit::component::ThemeMode::Light },
                    Some(w),
                    cx,
                );
                this.persist_preferences(cx);
                cx.notify();
            })))
            .into_any_element()
    }
    pub fn render_ribbon_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("ribbon-tabs")
            .h(px(34.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_3()
            .gap_1()
            .role(Role::TabList)
            .aria_label(self.label("功能区", "Ribbon"))
            .bg(rgb(if self.preferences.dark { 0x23332a } else { 0xf6f8f7 }))
            .children(
                [
                    ("ribbon-file", "文件", "File"),
                    ("ribbon-home", "开始", "Home"),
                    ("drawing-workflows", "插入", "Insert"),
                    ("print-workflows", "页面布局", "Page Layout"),
                    ("analysis-workflows", "公式", "Formulas"),
                    ("data-workflows", "数据", "Data"),
                    ("review-workflows", "审阅", "Review"),
                    ("window-panel", "视图", "View"),
                    ("automation-workflows", "自动化", "Automate"),
                ]
                .into_iter()
                .enumerate()
                .map(|(tab, (id, zh, en))| {
                    let active = self.ribbon_tab == tab;
                    div()
                        .id(id)
                        .role(Role::Tab)
                        .aria_label(self.label(zh, en))
                        .aria_selected(active)
                        .tab_stop(true)
                        .h_full()
                        .px_3()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .border_b_2()
                        .border_color(rgb(if active { 0x167347 } else { if self.preferences.dark { 0x23332a } else { 0xf6f8f7 } }))
                        .when(active, |el| {
                            el.font_weight(FontWeight::SEMIBOLD).text_color(rgb(if self.preferences.dark { 0x94dbb4 } else { 0x146c43 }))
                        })
                        .child(self.label(zh, en))
                        .on_key_down(cx.listener(move |this, e: &KeyDownEvent, _, cx| {
                            match e.keystroke.key.as_str() {
                                "enter" | "space" => this.ribbon_tab = tab,
                                "left" => this.ribbon_tab = (tab + 8) % 9,
                                "right" => this.ribbon_tab = (tab + 1) % 9,
                                _ => return,
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.ribbon_tab = tab;
                            this.daily_group = 0;
                            cx.notify();
                        }))
                }),
            )
            .into_any_element()
    }
    pub fn render_ribbon_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let action = |id, zh, en, cmd, dialog, cx: &mut Context<Self>| self.ribbon_action(id, zh, en, cmd, dialog, cx);
        let groups = match self.ribbon_tab {
            0 => vec![
                self.ribbon_group(
                    "工作簿",
                    "Workbook",
                    vec![
                        action("file-new", "新建工作簿", "New workbook", "file.new", false, cx),
                        div()
                            .id("file-open")
                            .px_2()
                            .cursor_pointer()
                            .child(self.label("打开…", "Open…"))
                            .on_click(cx.listener(|this, _, w, cx| this.pick_open(w, cx)))
                            .into_any_element(),
                    ],
                ),
                self.ribbon_group(
                    "保存",
                    "Save",
                    vec![
                        div()
                            .id("save-as")
                            .px_2()
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .child(self.label("另存为…", "Save as…"))
                            .on_click(cx.listener(|this, _, w, cx| this.commit(After::Save(true), w, cx)))
                            .into_any_element(),
                    ],
                ),
                self.ribbon_group(
                    "最近使用",
                    "Recent",
                    vec![
                        div()
                            .id("recent-files")
                            .px_2()
                            .cursor_pointer()
                            .child(self.label("最近文件", "Recent files"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.recent_open = !this.recent_open;
                                cx.notify();
                            }))
                            .into_any_element(),
                    ],
                ),
            ],
            1 => vec![
                self.ribbon_group(
                    "剪贴板",
                    "Clipboard",
                    vec![
                        action("paste", "粘贴", "Paste", "edit.paste", false, cx),
                        action("copy", "复制", "Copy", "edit.copy", false, cx),
                        action("cut", "剪切", "Cut", "edit.cut", false, cx),
                    ],
                ),
                self.ribbon_group(
                    "字体",
                    "Font",
                    vec![
                        div()
                            .flex()
                            .flex_col()
                            .child(div().flex().children([
                                action("font-name", "字体 ▾", "Font ▾", "home.fontName", true, cx),
                                action("font-size", "字号 ▾", "Size ▾", "home.fontSize", true, cx),
                            ]))
                            .child(div().flex().children([
                                self.toolbar_button("bold", "B", "home.bold", json!({}), cx).into_any_element(),
                                self.toolbar_button("italic", "I", "home.italic", json!({}), cx).into_any_element(),
                                self.toolbar_button("underline", "U", "home.underline", json!({}), cx).into_any_element(),
                                action("font-color", "字色 ▾", "Color ▾", "home.fontColor", true, cx),
                                action("fill", "填色 ▾", "Fill ▾", "home.fillColor", true, cx),
                            ]))
                            .into_any_element(),
                    ],
                ),
                self.ribbon_group(
                    "对齐方式",
                    "Alignment",
                    vec![
                        div()
                            .flex()
                            .flex_col()
                            .child(div().flex().children([
                                action("align-left", "左对齐", "Left", "home.alignLeft", false, cx),
                                action("align-center", "居中", "Center", "home.alignCenter", false, cx),
                                action("align-right", "右对齐", "Right", "home.alignRight", false, cx),
                            ]))
                            .child(div().flex().children([
                                action("wrap", "自动换行", "Wrap text", "home.wrapText", false, cx),
                                action("merge", "合并居中", "Merge & Center", "home.mergeCenter", false, cx),
                            ]))
                            .into_any_element(),
                    ],
                ),
                self.ribbon_group(
                    "数字",
                    "Number",
                    vec![
                        div()
                            .flex()
                            .flex_col()
                            .child(action("number-format", "数字格式 ▾", "Number format ▾", "home.numberFormat", true, cx))
                            .child(div().flex().children([
                                self.toolbar_button("currency", "¥", "home.numberFormat", json!({"code":"¥#,##0.00"}), cx).into_any_element(),
                                self.toolbar_button("percent", "%", "home.numberFormat", json!({"code":"0.00%"}), cx).into_any_element(),
                                self.toolbar_button("decimal", ".00", "home.numberFormat", json!({"code":"0.00"}), cx).into_any_element(),
                            ]))
                            .into_any_element(),
                    ],
                ),
                self.ribbon_group(
                    "单元格",
                    "Cells",
                    vec![
                        div()
                            .flex()
                            .flex_col()
                            .child(action("format-panel", "单元格格式…", "Format cells…", "home.formatCells", true, cx))
                            .child(div().flex().children([
                                action("borders", "边框 ▾", "Borders ▾", "home.borders", true, cx),
                                action("autofit", "自动列宽", "AutoFit", "home.autofitColumnWidth", false, cx),
                            ]))
                            .into_any_element(),
                    ],
                ),
            ],
            2 => vec![
                self.ribbon_group(
                    "表格",
                    "Tables",
                    vec![
                        action("insert-table", "表格", "Table", "insert.table", true, cx),
                        action("insert-pivot", "数据透视表", "PivotTable", "insert.pivotTable", true, cx),
                    ],
                ),
                self.ribbon_group(
                    "图表与插图",
                    "Charts & Illustrations",
                    vec![
                        action("insert-chart", "图表", "Chart", "insert.chart", true, cx),
                        action("insert-picture", "图片", "Picture", "insert.picture", true, cx),
                        action("insert-shape", "形状", "Shape", "insert.shape", true, cx),
                        action("insert-text", "文本框", "Text box", "insert.textBox", true, cx),
                    ],
                ),
            ],
            3 => vec![
                self.ribbon_group("页面设置", "Page Setup", vec![action("page-setup", "页面设置…", "Page setup…", "pageLayout.pageSetup", true, cx)]),
                self.ribbon_group(
                    "打印",
                    "Print",
                    vec![
                        action("print-preview", "打印预览", "Print preview", "file.printPreview", false, cx),
                        action("system-print", "打印…", "Print…", "file.print", false, cx),
                        action("export-pdf", "导出 PDF", "Export PDF", "file.exportPdf", true, cx),
                    ],
                ),
            ],
            4 => vec![
                self.ribbon_group(
                    "函数库",
                    "Function Library",
                    vec![
                        action("insert-function", "插入函数…", "Insert function…", "formulas.insertFunction", true, cx),
                        self.toolbar_button("autosum", "Σ 自动求和", "formulas.autoSum", json!({}), cx).into_any_element(),
                    ],
                ),
                self.ribbon_group(
                    "定义名称",
                    "Defined Names",
                    vec![
                        action("define-name", "定义名称", "Define name", "formulas.defineName", true, cx),
                        action("name-manager", "名称管理器", "Name manager", "formulas.nameManager", true, cx),
                    ],
                ),
                self.ribbon_group("计算", "Calculation", vec![action("calculate", "重新计算", "Calculate now", "formulas.calculateNow", false, cx)]),
            ],
            5 => vec![
                self.ribbon_group(
                    "排序和筛选",
                    "Sort & Filter",
                    vec![
                        action("sort-up", "升序", "Sort ascending", "data.sortAscending", false, cx),
                        action("sort-dialog", "排序…", "Sort…", "data.sort", true, cx),
                        action("filter", "筛选", "Filter", "data.filter", false, cx),
                    ],
                ),
                self.ribbon_group(
                    "数据工具",
                    "Data Tools",
                    vec![
                        action("validation", "数据验证", "Data validation", "data.validation", true, cx),
                        action("remove-duplicates", "删除重复项", "Remove duplicates", "data.removeDuplicates", true, cx),
                    ],
                ),
                self.ribbon_group(
                    "分析",
                    "Analysis",
                    vec![
                        action("analysis", "数据分析", "Data analysis", "data.analysisToolPak", true, cx),
                        action("solver", "规划求解", "Solver", "data.solver", true, cx),
                    ],
                ),
            ],
            6 => vec![
                self.ribbon_group(
                    "批注",
                    "Comments",
                    vec![
                        action("new-comment", "新建批注", "New comment", "review.newComment", true, cx),
                        action("show-notes", "显示笔记", "Show notes", "review.showAllNotes", false, cx),
                    ],
                ),
                self.ribbon_group(
                    "保护",
                    "Protect",
                    vec![
                        action("protect-sheet", "保护工作表", "Protect sheet", "review.protectSheet", true, cx),
                        action("unprotect-sheet", "撤销保护", "Unprotect sheet", "review.unprotectSheet", true, cx),
                    ],
                ),
            ],
            7 => vec![
                self.ribbon_group(
                    "窗口",
                    "Window",
                    vec![
                        action("new-window", "新建窗口", "New window", "view.newWindow", false, cx),
                        action("freeze", "冻结首行", "Freeze top row", "view.freezeTopRow", false, cx),
                        action("unfreeze", "取消冻结", "Unfreeze panes", "view.unfreezePanes", false, cx),
                    ],
                ),
                self.ribbon_group(
                    "显示",
                    "Display",
                    vec![
                        action("zoom", "缩放…", "Zoom…", "view.zoom", true, cx),
                        action("gridlines", "网格线", "Gridlines", "view.gridlines", false, cx),
                    ],
                ),
            ],
            _ => vec![self.ribbon_group(
                "脚本与控制",
                "Scripts & Control",
                vec![action("script-details", "动作与脚本…", "Actions & scripts…", "__joysheet.scriptDetails", false, cx)],
            )],
        };
        div()
            .id("ribbon-tools")
            .track_focus(&self.ribbon_focus)
            .tab_stop(true)
            .h(px(88.))
            .flex_shrink_0()
            .px_1()
            .py_2()
            .flex()
            .items_center()
            .bg(rgb(if self.preferences.dark { 0x20332a } else { 0xffffff }))
            .border_b_1()
            .border_color(rgb(if self.preferences.dark { 0x354c3e } else { 0xdce3de }))
            .overflow_x_scroll()
            .children(groups)
            .child(
                div().id("ribbon-more-options").flex_shrink_0().px_3().cursor_pointer().child(self.label("更多选项 ▾", "More options ▾")).on_click(
                    cx.listener(|this, _, w, cx| {
                        this.daily_group = match this.ribbon_tab {
                            0 => {
                                this.recent_open = !this.recent_open;
                                0
                            }
                            1 => 1,
                            2 => 10,
                            3 => 13,
                            4 => 14,
                            5 => 4,
                            6 => 15,
                            7 => 3,
                            _ => 16,
                        };
                        if this.daily_group == 16 {
                            this.commit(After::Command("__joysheet.scriptDetails".into(), json!({})), w, cx);
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("more")
                    .flex_shrink_0()
                    .px_3()
                    .cursor_pointer()
                    .text_color(rgb(if self.preferences.dark { 0x94dbb4 } else { 0x146c43 }))
                    .child(self.label("所有命令  ···", "All commands  ···"))
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.palette_open = true;
                        this.search.update(cx, |s, cx| s.focus(w, cx));
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
