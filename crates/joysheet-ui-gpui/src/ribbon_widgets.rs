use crate::{
    desktop::{After, Desktop},
    ribbon_catalog::{self, Group, Tool},
    ribbon_menus,
};
use gpui_base::Selectable;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::Input,
        popover::Popover,
    },
    prelude::*,
    *,
};
use joyoffice_ui::IconName as I;
use serde_json::{Value, json};

impl Desktop {
    pub(crate) fn activate_ribbon_tool(&mut self, tool: Tool, w: &mut Window, cx: &mut Context<Self>) {
        if self.guard_composition(w, cx) {
            return;
        }
        let mut p: Value = serde_json::from_str(tool.params).unwrap_or_else(|_| json!({}));
        match tool.command {
            "@search" => {
                self.palette_open = true;
                self.search.update(cx, |s, cx| s.focus(w, cx));
            }
            "@help" => {
                self.help_open = true;
            }
            "@objects" => {
                self.daily_group = 22;
            }
            "@comments" => {
                self.daily_group = 15;
            }
            "@scripts" => {
                self.daily_group = 16;
                self.commit(After::Command("__joysheet.scriptDetails".into(), json!({})), w, cx);
            }
            "@control" => {
                self.daily_group = 17;
            }
            "@windows" => {
                self.daily_group = 3;
            }
            "@record" => {
                if self.view().is_some_and(|v| v.recording) {
                    self.open_command("automate.recordActions", w, cx);
                } else {
                    self.commit(After::Command("automate.recordActions".into(), json!({})), w, cx);
                }
            }
            "@formula-bar" => {
                self.preferences.formula_bar = !self.preferences.formula_bar;
                self.persist_preferences(cx);
            }
            "@format-painter" => {
                self.format_painter = !self.format_painter;
                if self.format_painter {
                    self.commit(After::Command("edit.formatPainter".into(), json!({})), w, cx);
                }
            }
            "@delete-sheet" => {
                self.daily_group = 2;
            }
            "@select" | "@pen" | "@eraser" => {
                self.ink.points.clear();
                self.ink.tool = match tool.command {
                    "@pen" => "pen",
                    "@eraser" => "eraser",
                    _ => "",
                };
                if let Some(c) = p["color"].as_str() {
                    self.ink.color = c.into();
                }
                if let Some(width) = p["width"].as_f64() {
                    self.ink.width = width as f32;
                }
                self.focus_grid(w, cx);
            }
            "@file-open" => {
                self.ribbon_tab = 1;
                self.pick_open(w, cx);
            }
            "@file-save" | "@file-save-as" => {
                self.ribbon_tab = 1;
                self.commit(After::Save(tool.command == "@file-save-as"), w, cx);
            }
            "@file-close" => {
                self.ribbon_tab = 1;
                self.commit(After::Close(false), w, cx);
            }
            "@settings" => {
                self.ribbon_tab = 1;
                self.daily_group = 20;
            }
            cmd if cmd.starts_with("@functions:") => {
                self.function_category = Some(cmd.trim_start_matches("@functions:").into());
                if cmd == "@functions:Recent" {
                    self.issue("formulas.recentlyUsed", json!({}), After::None, cx);
                }
                self.function_search.update(cx, |s, cx| {
                    s.set_value("", w, cx);
                    s.focus(w, cx);
                });
            }
            "formulas.tracePrecedents" | "formulas.traceDependents" => {
                self.trace_selected(tool.command, cx);
                self.focus_grid(w, cx);
            }
            _ if tool.dialog => {
                self.open_command(tool.command, w, cx);
                self.override_ribbon_form(p, w, cx);
            }
            cmd => {
                if cmd == "edit.paste" {
                    p = self.paste_parameters();
                }
                if matches!(cmd, "edit.copy" | "edit.cut") {
                    p = json!({"html":true});
                }
                if cmd == "view.freezePanes" && p["atSelection"] == true {
                    p = json!({"cell":self.selection().active.a1()});
                }
                if cmd.starts_with("file.") {
                    self.ribbon_tab = 1;
                }
                self.commit(After::Command(cmd.into(), p), w, cx);
                self.focus_grid(w, cx);
            }
        }
        cx.notify();
    }
    fn override_ribbon_form(&mut self, overrides: Value, w: &mut Window, cx: &mut Context<Self>) {
        if let (Ok(Value::Object(mut values)), Some(extra)) = (serde_json::from_str::<Value>(&self.params.read(cx).value()), overrides.as_object()) {
            values.extend(extra.clone());
            let values = Value::Object(values);
            self.make_command_fields(&values, w, cx);
            self.params.update(cx, |s, cx| s.set_value(values.to_string(), w, cx));
        }
    }
    fn ribbon_checked(&self, command: &str) -> bool {
        let Some(v) = self.view() else {
            return false;
        };
        let Some(s) = v.sheet() else {
            return false;
        };
        let style = v.workbook.styles.get(s.style_id(self.selection().active));
        use joysheet_engine::model::{HAlign, SheetViewMode, VAlign};
        match command {
            "home.bold" => style.font.bold,
            "home.italic" => style.font.italic,
            "home.wrapText" => style.align.wrap,
            "home.alignLeft" => style.align.h == HAlign::Left,
            "home.alignCenter" => style.align.h == HAlign::Center,
            "home.alignRight" => style.align.h == HAlign::Right,
            "home.alignTop" => style.align.v == VAlign::Top,
            "home.alignMiddle" => style.align.v == VAlign::Center,
            "home.alignBottom" => style.align.v == VAlign::Bottom,
            "view.gridlines" => s.show_gridlines,
            "view.headings" => s.show_headings,
            "pageLayout.printGridlines" => s.print.gridlines,
            "pageLayout.printHeadings" => s.print.headings,
            "formulas.showFormulas" => s.show_formulas,
            "view.normal" => s.view == SheetViewMode::Normal,
            "view.pageLayout" => s.view == SheetViewMode::PageLayout,
            "view.pageBreakPreview" => s.view == SheetViewMode::PageBreakPreview,
            "@formula-bar" => self.preferences.formula_bar,
            "@format-painter" => self.format_painter,
            "@pen" => self.ink.tool == "pen",
            "@eraser" => self.ink.tool == "eraser",
            "@select" => self.ink.tool.is_empty(),
            _ => false,
        }
    }
    fn ribbon_label(&self, tool: Tool) -> String {
        let style = self.view().and_then(|v| v.sheet().map(|s| v.workbook.styles.get(s.style_id(self.selection().active))));
        match tool.id {
            "font-name" => style.map(|s| s.font.name.clone()).unwrap_or_else(|| "Calibri".into()),
            "font-size" => style.map(|s| s.font.size.to_string()).unwrap_or_else(|| "11".into()),
            "number-format" => style
                .map(|s| if s.num_fmt.as_str() == "General" { self.label("常规", "General") } else { s.num_fmt.as_str().to_string() })
                .unwrap_or_else(|| self.label("常规", "General")),
            _ => self.label(tool.zh, tool.en),
        }
    }
    fn ribbon_button(&self, tool: Tool, large: bool, cx: &mut Context<Self>) -> AnyElement {
        if tool.id == "cell-styles" && self.ribbon_wide {
            return self.ribbon_style_gallery(cx);
        }
        let available = tool.menu || tool.command.starts_with('@') || joysheet_engine::find_command(tool.command).is_some();
        let enabled = available
            && self.view().is_some()
            && match tool.command {
                "edit.undo" => self.view().is_some_and(|v| !v.undo.is_empty()),
                "edit.redo" => self.view().is_some_and(|v| !v.redo.is_empty()),
                _ => true,
            };
        let label = self.label(tool.zh, tool.en);
        let mut checked = self.ribbon_checked(tool.command);
        let check =
            matches!(tool.command, "view.gridlines" | "view.headings" | "@formula-bar" | "pageLayout.printGridlines" | "pageLayout.printHeadings");
        let icon = if check { if checked { I::SquareCheck } else { I::Square } } else { tool.icon };
        let color = serde_json::from_str::<Value>(tool.params)
            .ok()
            .and_then(|p| p["color"].as_str().and_then(|s| u32::from_str_radix(s.trim_start_matches('#'), 16).ok()));
        if tool.command == "@pen"
            && let Some(color) = color
        {
            checked = checked && format!("#{color:06X}").eq_ignore_ascii_case(&self.ink.color);
        }
        let mut content = div().relative().flex().items_center().gap(px(4.)).child(self.advanced_probe(format!("ribbon:{}", tool.id), cx));
        if large {
            content = content.flex_col().justify_center().w_full().h_full().gap(px(5.));
        }
        let field = matches!(tool.id, "font-name" | "font-size" | "number-format");
        if !field {
            content = content.child(
                crate::ribbon_icons::icon(icon).size(px(if large { 26. } else { 16. })).text_color(rgb(color.unwrap_or(self.palette().accent))),
            );
        }
        if !tool.compact {
            content = content.child(
                div()
                    .max_w(px(if large { 78. } else { 170. }))
                    .overflow_hidden()
                    .text_center()
                    .when(large, |el| el.whitespace_normal().max_h(px(32.)))
                    .child(self.ribbon_label(tool)),
            );
        }
        if tool.menu {
            content = content.child(crate::ribbon_icons::icon(I::ChevronDown).size(px(9.)));
        }
        let tooltip = if available { label.clone() } else { format!("{} · {}", label, self.label("此功能尚未实现", "Not implemented yet")) };
        let mut button = Button::new(tool.id)
            .ghost()
            .xsmall()
            .rounded(px(3.))
            .px(px(4.))
            .h(px(if large { 72. } else { 24. }))
            .text_size(px(12.))
            .accessibility_label(label)
            .tooltip(tooltip)
            .disabled(!enabled)
            .selected(checked && !check)
            .child(content);
        if large {
            button = button.min_w(px(46.));
        }
        if tool.compact {
            button = button.w(px(if tool.menu { 32. } else { 24. })).px(px(2.));
        }
        if field {
            button = button
                .w(px(match tool.id {
                    "font-name" => 132.,
                    "font-size" => 44.,
                    _ => 151.,
                }))
                .border(px(0.5))
                .border_color(rgb(self.palette().border));
        }
        if tool.menu {
            let owner = cx.weak_entity();
            let choices = ribbon_menus::tools(tool.command);
            let chinese = self.chinese();
            let target = self.view().map(|v| (v.doc, v.sheet));
            Popover::new(ElementId::Name(format!("ribbon-menu-{}", tool.id).into()))
                .trigger(button)
                .content(move |_, _, cx| {
                    let popup = cx.entity().downgrade();
                    div().id("ribbon-menu-items").flex().flex_col().w(px(230.)).max_h(px(410.)).overflow_y_scroll().children(choices.iter().map(
                        |&item| {
                            let owner = owner.clone();
                            let popup = popup.clone();
                            Button::new(item.id)
                                .ghost()
                                .xsmall()
                                .h(px(28.))
                                .justify_start()
                                .label(if chinese { item.zh } else { item.en })
                                .icon(crate::ribbon_icons::icon(item.icon))
                                .accessibility_label(if chinese { item.zh } else { item.en })
                                .on_click(move |_, w, cx| {
                                    let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = owner.update(cx, |v, cx| {
                                        if v.view().map(|v| (v.doc, v.sheet)) == target {
                                            v.activate_ribbon_tool(item, w, cx);
                                        }
                                    });
                                })
                        },
                    ))
                })
                .into_any_element()
        } else {
            button.on_click(cx.listener(move |v, _, w, cx| v.activate_ribbon_tool(tool, w, cx))).into_any_element()
        }
    }
    fn render_ribbon_group(&self, group: Group, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div().flex().gap(px(3.)).h(px(73.)).items_center();
        if group.rows {
            body = body.flex_col().justify_center().items_start();
        }
        for row in group.tools {
            let large = !group.rows && row.len() == 1;
            let mut stack = div().flex().gap(px(1.));
            if !group.rows {
                stack = stack.flex_col().justify_center();
            }
            for tool in row {
                stack = stack.child(self.ribbon_button(tool, large, cx));
            }
            body = body.child(stack);
        }
        let mut footer = div()
            .relative()
            .h(px(18.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.))
            .text_color(rgb(self.palette().muted))
            .child(self.label(group.zh, group.en));
        if let Some(cmd) = group.launcher {
            footer = footer.child(
                Button::new(ElementId::Name(format!("launcher-{}-{}", group.en, cmd).into()))
                    .ghost()
                    .xsmall()
                    .absolute()
                    .right_0()
                    .size(px(15.))
                    .px_0()
                    .icon(crate::ribbon_icons::icon(I::ArrowDownRight))
                    .accessibility_label(self.label("打开格式设置", "Open formatting settings"))
                    .on_click(cx.listener(move |v, _, w, cx| v.open_command(cmd, w, cx))),
            );
        }
        div()
            .flex_shrink_0()
            .px(px(7.))
            .border_r(px(0.5))
            .border_color(rgb(self.palette().border))
            .flex()
            .flex_col()
            .child(body)
            .child(footer)
            .into_any_element()
    }
    pub(crate) fn excel_ribbon_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.ribbon_collapsed || self.ribbon_tab == 0 {
            return div().into_any_element();
        }
        div()
            .id("ribbon-tools")
            .tab_stop(true)
            .flex_shrink_0()
            .h(px(101.))
            .overflow_hidden()
            .bg(rgb(self.palette().surface))
            .border_b(px(0.5))
            .border_color(rgb(self.palette().border))
            .child(
                div()
                    .id("ribbon-scroll")
                    .flex()
                    .h_full()
                    .items_center()
                    .overflow_x_scroll()
                    .children(ribbon_catalog::groups(self.ribbon_tab).into_iter().map(|g| self.render_ribbon_group(g, cx))),
            )
            .into_any_element()
    }
    pub(crate) fn render_backstage(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = ribbon_catalog::tool;
        let navigation = [
            t("backstage-new", "新建", "New", I::FilePlus, "file.new"),
            t("file-open", "打开", "Open", I::FolderOpen, "@file-open"),
            t("file-info", "信息", "Info", I::Info, "file.properties"),
            t("file-save", "保存", "Save", I::Save, "@file-save"),
            t("file-save-as", "另存为", "Save As", I::Save, "@file-save-as"),
            t("system-print", "打印", "Print", I::Printer, "file.print"),
            t("export-pdf", "导出", "Export", I::FileOutput, "file.exportPdf").dialog(),
            t("file-close", "关闭", "Close", I::X, "@file-close"),
            t("file-options", "选项", "Options", I::Settings, "@settings"),
        ];
        let mut recent = div().flex().flex_col().gap_2().child(div().text_lg().child(self.label("最近", "Recent")));
        for (i, path) in self.preferences.recent.iter().take(12).enumerate() {
            let path = path.clone();
            let open = path.clone();
            recent = recent.child(
                Button::new(("backstage-recent", i))
                    .ghost()
                    .h(px(42.))
                    .justify_start()
                    .icon(crate::ribbon_icons::icon(I::FileSpreadsheet))
                    .label(path)
                    .on_click(cx.listener(move |v, _, w, cx| {
                        v.ribbon_tab = 1;
                        v.commit(After::OpenPaths(vec![open.clone()]), w, cx);
                    })),
            );
        }
        div()
            .id("sheet-backstage")
            .occlude()
            .absolute()
            .top(px(60.))
            .bottom_0()
            .left_0()
            .right_0()
            .bg(rgb(self.palette().chrome))
            .flex()
            .child(
                div()
                    .w(px(150.))
                    .h_full()
                    .p_3()
                    .bg(rgb(self.palette().header))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Button::new("backstage-back")
                            .ghost()
                            .icon(crate::ribbon_icons::icon(I::ArrowLeft))
                            .label(self.label("返回", "Back"))
                            .on_click(cx.listener(|v, _, w, cx| {
                                v.ribbon_tab = 1;
                                v.focus_grid(w, cx);
                                cx.notify();
                            })),
                    )
                    .children(navigation.into_iter().map(|tool| {
                        Button::new(tool.id)
                            .ghost()
                            .h(px(38.))
                            .justify_start()
                            .icon(crate::ribbon_icons::icon(tool.icon))
                            .label(self.label(tool.zh, tool.en))
                            .on_click(cx.listener(move |v, _, w, cx| v.activate_ribbon_tool(tool, w, cx)))
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(div().text_2xl().child(self.label("开始", "Home")))
                    .child(div().text_lg().child(self.label("新建", "New")))
                    .child(
                        Button::new("file-new")
                            .ghost()
                            .w(px(185.))
                            .h(px(110.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_3()
                                    .child(crate::ribbon_icons::icon(I::FileSpreadsheet).size(px(42.)))
                                    .child(self.label("空白工作簿", "Blank Workbook")),
                            )
                            .on_click(cx.listener(|v, _, w, cx| {
                                v.ribbon_tab = 1;
                                v.commit(After::Command("file.new".into(), json!({})), w, cx);
                            })),
                    )
                    .child(recent),
            )
            .into_any_element()
    }
    pub(crate) fn render_ribbon_help(&self, cx: &mut Context<Self>) -> AnyElement {
        div().id("sheet-help").occlude().absolute().right_0().top(px(180.)).bottom_0().w(px(350.)).p_5().bg(rgb(self.palette().surface)).border_l(px(0.5)).border_color(rgb(self.palette().border)).flex().flex_col().gap_4()
            .child(Button::new("close-ribbon-help").ghost().icon(crate::ribbon_icons::icon(I::X)).label(self.label("关闭帮助","Close Help")).on_click(cx.listener(|v,_,_,cx|{v.help_open=false;cx.notify();})))
            .child(div().text_lg().child(self.label("JoySheet 帮助","JoySheet Help")))
            .child(self.label("输入数据：选择单元格后键入；按 Enter 确认，Esc 取消。以 = 开始输入公式。","Select a cell and type. Enter commits, Esc cancels. Start formulas with =."))
            .child(self.label("格式刷：选择源区域，点击格式刷，再选择目标区域。绘图：选择笔后在表格中拖动，Esc 返回选择模式。","Format Painter: select the source, click the brush, then select the destination. Draw: choose a pen and drag on the sheet. Esc returns to selection."))
            .children(["Ctrl+S   保存 / Save","Ctrl+Z / Ctrl+Y   撤消 / Undo · 重做 / Redo","Ctrl+1   单元格格式 / Format Cells","Ctrl+F / Ctrl+H   查找 / Find · 替换 / Replace","Ctrl+K   搜索命令 / Search Commands","Ctrl+F1   折叠功能区 / Collapse Ribbon","F6   切换界面区域 / Switch Regions","F2   编辑单元格 / Edit Cell"].into_iter().map(|s|div().child(s)))
            .child(div().text_xs().text_color(rgb(self.palette().muted)).child(self.label("灰色命令尚未实现。在线协作、Office 加载项与云端服务不包含在本地版本中。","Disabled commands are not implemented. Online collaboration, Office add-ins and cloud services are not included in this local edition."))).into_any_element()
    }
    pub(crate) fn render_function_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let category = self.function_category.as_deref().unwrap_or("All");
        let query = self.function_search.read(cx).value().to_ascii_lowercase();
        div()
            .id("function-picker")
            .occlude()
            .absolute()
            .right_0()
            .top(px(180.))
            .bottom_0()
            .w(px(430.))
            .p_4()
            .bg(rgb(self.palette().surface))
            .flex()
            .flex_col()
            .gap_2()
            .child(
                Button::new("close-function-picker")
                    .ghost()
                    .icon(crate::ribbon_icons::icon(I::X))
                    .label(self.label("关闭函数库", "Close Function Library"))
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.function_category = None;
                        v.focus_grid(w, cx);
                        cx.notify();
                    })),
            )
            .child(Input::new(&self.function_search).id("function-search").aria_label(self.label("搜索函数", "Search functions")))
            .child(
                div().id("function-list").flex_1().overflow_y_scroll().flex().flex_col().gap_1().children(
                    joysheet_functions::all()
                        .iter()
                        .filter(|f| {
                            (category == "All"
                                || (category == "Recent" && self.recent_functions.iter().any(|n| n == f.name))
                                || format!("{:?}", f.category) == category)
                                && (f.name.to_ascii_lowercase().contains(&query) || f.description.to_ascii_lowercase().contains(&query))
                        })
                        .enumerate()
                        .map(|(i, f)| {
                            let name = f.name;
                            Button::new(("function", i))
                                .ghost()
                                .h(px(48.))
                                .w_full()
                                .min_w_0()
                                .justify_start()
                                .tooltip(format!("{} — {}", f.signature, f.description))
                                .child(
                                    div()
                                        .relative()
                                        .w_full()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .child(self.advanced_probe(format!("function:{}", f.name), cx))
                                        .child(div().w_full().truncate().child(f.signature))
                                        .child(div().w_full().truncate().text_xs().child(f.description)),
                                )
                                .on_click(cx.listener(move |v, _, w, cx| {
                                    v.function_category = None;
                                    if v.draft.is_some() {
                                        v.insert_reference(&format!("{name}("), w, cx);
                                    } else {
                                        v.begin_edit(Some(format!("={name}(")), true, w, cx);
                                    }
                                    cx.notify();
                                }))
                        }),
                ),
            )
            .into_any_element()
    }
}
