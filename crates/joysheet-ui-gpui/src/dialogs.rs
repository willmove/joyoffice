//! Desktop dialogs arranged from the locally observed Excel controls.
//! The command palette remains a separate search surface.
use crate::{desktop::Desktop, palette::CommandField};
use gpui_base::FocusTrapElement as _;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        input::{Input, Textarea},
        popover::Popover,
        radio::Radio,
    },
    prelude::*,
    *,
};
use serde_json::{Value, json};

pub(crate) fn page_setup_values(p: &joysheet_engine::model::PrintSettings, centimeters: bool) -> Value {
    let (width, height) = p.fit_to.unwrap_or((1, 0));
    let margin = |i: usize| {
        let v = f64::from(p.margins[i]) * if centimeters { 2.54 } else { 1. };
        (v * 1000.).round() / 1000.
    };
    json!({"orientation":if p.orientation == joysheet_engine::model::Orientation::Landscape {"landscape"} else {"portrait"},
        "paper":p.paper,"scale":p.scale,"fitMode":p.fit_to.is_some(),"fitWidth":width,"fitHeight":height,
        "left":margin(0),"right":margin(1),"top":margin(2),"bottom":margin(3),
        "headerMargin":margin(4),"footerMargin":margin(5),"_marginUnit":if centimeters{"cm"}else{"in"},"centerH":p.center_h,"centerV":p.center_v,
        "header":p.header,"footer":p.footer,"gridlines":p.gridlines,"headings":p.headings,
        "printArea":p.print_area.map(|r|r.a1()).unwrap_or_default(),
        "titleRows":p.title_rows.map(|(a,b)|format!("{}:{}",a+1,b+1)).unwrap_or_default(),
        "titleCols":p.title_cols.map(|(a,b)|format!("{}:{}",joysheet_engine::core::col_to_letters(a),joysheet_engine::core::col_to_letters(b))).unwrap_or_default()})
}

struct Page {
    zh: &'static str,
    en: &'static str,
    keys: &'static [&'static str],
}
fn pages(id: &str) -> Vec<Page> {
    match id {
        "home.fillColor" | "home.fontColor" | "home.borderColor" => {
            vec![Page { zh: "标准", en: "Standard", keys: &[] }, Page { zh: "自定义", en: "Custom", keys: &["red", "green", "blue", "color"] }]
        }
        "home.formatCells" => vec![
            Page { zh: "数字", en: "Number", keys: &["numberFormat"] },
            Page { zh: "对齐", en: "Alignment", keys: &["horizontal", "vertical", "indent", "angle", "wrap", "shrink"] },
            Page { zh: "字体", en: "Font", keys: &["fontName", "fontSize", "bold", "italic", "underline", "fontColor", "strike", "fontScript"] },
            Page { zh: "边框", en: "Border", keys: &["borderLeft", "borderRight", "borderTop", "borderBottom", "borderDown", "borderUp"] },
            Page { zh: "填充", en: "Fill", keys: &["fillBackground", "fillForeground", "fillPattern"] },
            Page { zh: "保护", en: "Protection", keys: &["locked", "hidden"] },
        ],
        "pageLayout.pageSetup" => vec![
            Page { zh: "页面", en: "Page", keys: &["orientation", "paper", "fitMode", "scale", "fitWidth", "fitHeight"] },
            Page { zh: "页边距", en: "Margins", keys: &["top", "bottom", "left", "right", "headerMargin", "footerMargin", "centerH", "centerV"] },
            Page { zh: "页眉/页脚", en: "Header/Footer", keys: &["header", "footer"] },
            Page { zh: "工作表", en: "Sheet", keys: &["printArea", "titleRows", "titleCols", "gridlines", "headings"] },
        ],
        "data.validation" => vec![
            Page { zh: "设置", en: "Settings", keys: &["type", "ignoreBlank", "operator", "formula1", "formula2", "dropdown"] },
            Page { zh: "输入信息", en: "Input Message", keys: &["inputTitle", "inputMessage"] },
            Page { zh: "出错警告", en: "Error Alert", keys: &["errorStyle", "errorTitle", "errorMessage"] },
        ],
        _ => Vec::new(),
    }
}
pub(crate) fn choices(id: &str, key: &str) -> Vec<(&'static str, &'static str)> {
    match (id, key) {
        ("edit.find" | "edit.replace", "lookIn") => vec![("values", "值"), ("formulas", "公式")],
        ("edit.find" | "edit.replace", "within") => vec![("sheet", "工作表"), ("workbook", "工作簿")],
        (_, "orientation") => vec![("portrait", "纵向"), ("landscape", "横向")],
        (_, "paper") => vec![("A4", "A4"), ("A3", "A3"), ("Letter", "Letter"), ("Legal", "Legal"), ("A5", "A5"), ("B5", "B5")],
        (_, "horizontal") => vec![
            ("General", "常规"),
            ("Left", "靠左"),
            ("Center", "居中"),
            ("Right", "靠右"),
            ("Fill", "填充"),
            ("Justify", "两端对齐"),
            ("CenterAcross", "跨列居中"),
            ("Distributed", "分散对齐"),
        ],
        (_, "vertical") => vec![("Top", "靠上"), ("Center", "居中"), ("Bottom", "靠下"), ("Justify", "两端对齐"), ("Distributed", "分散对齐")],
        (_, "fontScript") => vec![("Baseline", "常规"), ("Superscript", "上标"), ("Subscript", "下标")],
        (_, "underline") => vec![
            ("None", "无"),
            ("Single", "单下划线"),
            ("Double", "双下划线"),
            ("SingleAccounting", "会计用单下划线"),
            ("DoubleAccounting", "会计用双下划线"),
        ],
        (_, "fillPattern") => {
            vec![("None", "无填充"), ("Solid", "纯色"), ("Gray25", "25% 灰色"), ("Gray50", "50% 灰色"), ("LightDown", "斜线"), ("LightGrid", "网格")]
        }
        (_, k) if k.starts_with("border") && !k.ends_with("Color") => vec![
            ("None", "无"),
            ("Thin", "细实线"),
            ("Medium", "中实线"),
            ("Thick", "粗实线"),
            ("Dashed", "虚线"),
            ("Dotted", "点线"),
            ("Double", "双线"),
            ("Hair", "细点线"),
        ],
        ("home.insertCells", "shift") => vec![("right", "活动单元格右移"), ("down", "活动单元格下移")],
        ("home.deleteCells", "shift") => vec![("left", "右侧单元格左移"), ("up", "下方单元格上移")],
        ("edit.pasteSpecial", "what") => {
            vec![("all", "全部"), ("formulas", "公式"), ("values", "值"), ("formats", "格式"), ("comments", "批注"), ("validation", "验证")]
        }
        ("edit.pasteSpecial", "operation") => vec![("none", "无"), ("add", "加"), ("subtract", "减"), ("multiply", "乘"), ("divide", "除")],
        ("data.group" | "data.ungroup", "by") => vec![("rows", "行"), ("cols", "列")],
        ("data.sort", "order") => vec![("asc", "升序"), ("desc", "降序")],
        ("data.validation", "type") => vec![
            ("any", "任何值"),
            ("whole", "整数"),
            ("decimal", "小数"),
            ("list", "序列"),
            ("date", "日期"),
            ("time", "时间"),
            ("textLength", "文本长度"),
            ("custom", "自定义"),
        ],
        ("data.validation" | "home.conditionalFormat", "operator") => vec![
            ("between", "介于"),
            ("notBetween", "未介于"),
            ("equal", "等于"),
            ("notEqual", "不等于"),
            ("greaterThan", "大于"),
            ("lessThan", "小于"),
            ("greaterOrEqual", "大于或等于"),
            ("lessOrEqual", "小于或等于"),
        ],
        _ => Vec::new(),
    }
}
fn title(id: &str, zh: bool) -> String {
    let (a, b) = match id {
        "home.formatCells" => ("设置单元格格式", "Format Cells"),
        "pageLayout.pageSetup" => ("页面设置", "Page Setup"),
        "home.rowHeight" => ("行高", "Row Height"),
        "home.columnWidth" => ("列宽", "Column Width"),
        "home.insertCells" => ("插入单元格", "Insert Cells"),
        "home.deleteCells" => ("删除单元格", "Delete Cells"),
        "edit.pasteSpecial" => ("选择性粘贴", "Paste Special"),
        "edit.find" | "edit.replace" => ("查找和替换", "Find and Replace"),
        "edit.goTo" => ("定位", "Go To"),
        "edit.goToSpecial" => ("定位条件", "Go To Special"),
        "edit.fillSeries" => ("序列", "Series"),
        "data.sort" => ("排序", "Sort"),
        "data.validation" => ("数据验证", "Data Validation"),
        "data.group" => ("组合", "Group"),
        "data.ungroup" => ("取消组合", "Ungroup"),
        "data.goalSeek" => ("单变量求解", "Goal Seek"),
        "formulas.defineName" => ("新建名称", "New Name"),
        "formulas.createFromSelection" => ("以选定区域创建名称", "Create Names from Selection"),
        "review.protectSheet" => ("保护工作表", "Protect Sheet"),
        "review.protectWorkbook" => ("保护工作簿", "Protect Workbook"),
        "view.zoom" => ("显示比例", "Zoom"),
        "view.unhideWindow" => ("取消隐藏", "Unhide"),
        "insert.sparkline" => ("创建迷你图", "Create Sparklines"),
        "insert.table" | "home.formatAsTable" => ("创建表", "Create Table"),
        "home.fillColor" | "home.fontColor" | "home.borderColor" => ("颜色", "Colors"),
        "data.textToColumns" => ("文本分列向导", "Convert Text to Columns Wizard"),
        _ => {
            return (1..crate::ribbon_catalog::TABS.len())
                .flat_map(crate::ribbon_catalog::groups)
                .flat_map(|g| g.tools.into_iter().flatten())
                .find(|t| t.command == id)
                .map(|t| if zh { t.zh } else { t.en })
                .map(str::to_string)
                .or_else(|| joysheet_engine::find_command(id).map(|s| s.label.to_string()))
                .unwrap_or_else(|| id.into());
        }
    };
    if zh { a.into() } else { b.into() }
}

impl Desktop {
    pub(crate) fn dialog_field_visible(&self, key: &str) -> bool {
        if self.selected_command.as_deref() == Some("data.sort") && matches!(key, "column" | "order" | "keysText") {
            return false;
        }
        if self.selected_command.as_deref() == Some("data.textToColumns") {
            return match self.dialog_tab {
                0 => key == "range",
                1 => matches!(key, "tab" | "comma" | "semicolon" | "space" | "other" | "treatConsecutive" | "textQualifier" | "fixedBreaks"),
                _ => matches!(key, "destination" | "columnFormat"),
            };
        }
        if matches!(self.selected_command.as_deref(), Some("edit.find" | "edit.replace"))
            && (key == "all" || key == "with" && self.selected_command.as_deref() == Some("edit.find"))
        {
            return false;
        }
        let tabs = pages(self.selected_command.as_deref().unwrap_or(""));
        if let Some(p) = tabs.get(self.dialog_tab.min(tabs.len().saturating_sub(1))) { p.keys.contains(&key) } else { true }
    }
    pub(crate) fn close_command_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = false;
        self.selected_command = None;
        self.dialog_error.clear();
        if self.excel_ui.return_panel != 0 {
            self.daily_group = self.excel_ui.return_panel;
            self.excel_ui.return_panel = 0;
        }
        if matches!(self.daily_group, 19 | 23) {
            self.command_focus.focus(w, cx);
        } else {
            self.focus_grid(w, cx);
        }
        cx.notify();
    }
    pub(crate) fn show_command_search(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.selected_command = None;
        self.palette_open = true;
        self.search.update(cx, |s, cx| s.focus(w, cx));
        cx.notify();
    }
    pub(crate) fn dialog_field(&self, id: &str, f: &CommandField, cx: &mut Context<Self>) -> AnyElement {
        let key = f.key.clone();
        let state = f.value.clone();
        let value = state.read(cx).value().to_string();
        let mut label = f.label.clone();
        if key.starts_with("border") {
            label = self.label(
                match key.as_str() {
                    "borderLeft" => "左边框",
                    "borderRight" => "右边框",
                    "borderTop" => "上边框",
                    "borderBottom" => "下边框",
                    "borderDown" => "下斜线",
                    _ => "上斜线",
                },
                key.trim_start_matches("border"),
            );
        }
        if id == "data.validation" {
            label = self.label(
                match key.as_str() {
                    "type" => "允许",
                    "operator" => "数据",
                    "formula1" => "最小值 / 来源 / 公式",
                    "formula2" => "最大值",
                    "inputTitle" | "errorTitle" => "标题",
                    "inputMessage" => "输入信息",
                    "errorStyle" => "样式",
                    _ => f.label.as_str(),
                },
                f.label.as_str(),
            );
        }
        label = match (id, key.as_str()) {
            ("pageLayout.pageSetup" | "pageLayout.headerFooter", "header") => self.label("页眉", "Header"),
            ("data.goalSeek", "set") => self.label("目标单元格", "Set cell"),
            ("data.goalSeek", "to") => self.label("目标值", "To value"),
            ("data.goalSeek", "changing") => self.label("可变单元格", "By changing cell"),
            ("review.protectSheet", "formatCells") => self.label("设置单元格格式", "Format cells"),
            ("review.protectSheet", "formatColumns") => self.label("设置列格式", "Format columns"),
            ("review.protectSheet", "formatRows") => self.label("设置行格式", "Format rows"),
            ("review.protectSheet", "sort") => self.label("排序", "Sort"),
            ("review.protectSheet", "autofilter") => self.label("使用自动筛选", "Use AutoFilter"),
            _ => label,
        };
        let probe = self.advanced_probe(format!("dialog-field:{key}"), cx);
        if f.initial.is_boolean() {
            return div()
                .relative()
                .child(probe)
                .child(Checkbox::new(SharedString::from(format!("bool-{key}"))).label(label).checked(value == "true").on_change(move |v, w, cx| {
                    state.update(cx, |s, cx| s.set_value(v.to_string(), w, cx));
                }))
                .into_any_element();
        }
        let mut opts = choices(id, &key)
            .into_iter()
            .map(|(a, b)| (a.to_string(), if self.chinese() { b.to_string() } else { a.to_string() }))
            .collect::<Vec<_>>();
        if opts.is_empty() {
            opts = self.object_choices(id, &key, cx);
        }
        if key == "errorStyle" {
            opts = vec![
                ("stop".into(), self.label("停止", "Stop")),
                ("warning".into(), self.label("警告", "Warning")),
                ("information".into(), self.label("信息", "Information")),
            ];
        }
        if !opts.is_empty() {
            let caption = opts.iter().find(|(v, _)| *v == value).map(|(_, s)| s.clone()).unwrap_or(value);
            let radio = matches!(
                (id, key.as_str()),
                ("home.insertCells" | "home.deleteCells", "shift")
                    | ("edit.pasteSpecial", "what" | "operation")
                    | ("data.group" | "data.ungroup", "by")
                    | ("view.unhideWindow", "window")
            );
            if radio {
                let mut options = div().flex().flex_col().gap_2();
                for (v, caption) in opts {
                    let state = state.clone();
                    let selected = state.read(cx).value().as_ref() == v;
                    options = options.child(
                        div().relative().child(self.advanced_probe(format!("dialog-choice:{key}:{v}"), cx)).child(
                            Radio::new(SharedString::from(format!("choice-{key}-{v}")))
                                .checked(selected)
                                .label(caption)
                                .on_change(move |_, w, cx| state.update(cx, |s, cx| s.set_value(v.clone(), w, cx))),
                        ),
                    );
                }
                return div().relative().child(probe).flex().flex_col().gap_2().child(label).child(options).into_any_element();
            }
            let owner = cx.weak_entity();
            let watcher = owner.clone();
            let select = Popover::new(SharedString::from(format!("select-{key}")))
                .on_open_change(move |open, _, cx| {
                    let _ = watcher.update(cx, |v, _| v.excel_ui.popup_open = *open);
                })
                .trigger(
                    Button::new(SharedString::from(format!("field-{key}")))
                        .w_full()
                        .justify_between()
                        .label(format!("{caption}  ▾"))
                        .accessibility_label(label.clone()),
                )
                .content(move |_, _, cx| {
                    let popup = cx.entity().downgrade();
                    div().id("dialog-choices").max_h(px(280.)).overflow_y_scroll().flex().flex_col().min_w(px(200.)).children(opts.iter().map(
                        |(v, label)| {
                            let (v, label) = (v.clone(), label.clone());
                            let state = state.clone();
                            let popup = popup.clone();
                            let owner = owner.clone();
                            Button::new(SharedString::from(format!("choice-{key}-{v}"))).ghost().justify_start().label(label).on_click(
                                move |_, w, cx| {
                                    state.update(cx, |s, cx| s.set_value(v.clone(), w, cx));
                                    let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = owner.update(cx, |_, cx| cx.notify());
                                },
                            )
                        },
                    ))
                });
            return div()
                .relative()
                .child(probe)
                .flex()
                .items_center()
                .gap_3()
                .child(div().w(px(145.)).child(label))
                .child(div().flex_1().child(select))
                .into_any_element();
        }
        if crate::advanced::multiline(id, &key) || key == "text" && matches!(id, "data.getData" | "data.fromTextCsv") {
            return div()
                .relative()
                .child(probe)
                .flex()
                .flex_col()
                .gap_2()
                .child(label)
                .child(Textarea::new(&self.import_text).h(px(130.)))
                .into_any_element();
        }
        div()
            .relative()
            .child(probe)
            .flex()
            .items_center()
            .gap_3()
            .child(div().w(px(145.)).child(label))
            .child(Input::new(&state).id(SharedString::from(format!("field-{key}"))).aria_label(f.label.clone()).flex_1())
            .into_any_element()
    }
    pub(crate) fn render_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(id) = self.selected_command.clone() else { return self.render_command_center(window, cx).into_any_element() };
        let p = self.palette();
        let tabs = pages(&id);
        let page = if id == "data.textToColumns" { self.dialog_tab.min(2) } else { self.dialog_tab.min(tabs.len().saturating_sub(1)) };
        let mut fields = div().id("workflow-fields").flex().flex_col().gap_3().p_4();
        if self.advanced_params {
            fields = fields.child(Input::new(&self.params).id("command-params").aria_label("Advanced JSON parameters"));
        } else {
            let keys = tabs.get(page).map(|p| p.keys.to_vec()).unwrap_or_else(|| self.command_fields.iter().map(|f| f.key.as_str()).collect());
            for key in keys {
                if !self.dialog_field_visible(key) {
                    continue;
                }
                if let Some(f) = self.command_fields.iter().find(|f| f.key == key) {
                    fields = fields.child(self.dialog_field(&id, f, cx));
                }
            }
        }
        if let Some(preview) = self.object_preview(&id, cx) {
            fields = fields.child(preview);
        }
        let unhide_empty = id == "view.unhideWindow" && self.object_choices(&id, "window", cx).is_empty();
        if unhide_empty {
            fields = div().id("workflow-fields").p_4().child(self.label("没有可取消隐藏的工作簿。", "There are no hidden workbooks."));
        }
        if id == "home.formatCells"
            && page == 0
            && let Some(field) = self.command_fields.iter().find(|f| f.key == "numberFormat")
        {
            let state = field.value.clone();
            let code = state.read(cx).value().to_string();
            let mut categories = div().flex().flex_wrap().gap_1();
            for (zh, en, value) in [
                ("常规", "General", "General"),
                ("数值", "Number", "0.00"),
                ("货币", "Currency", "¥#,##0.00"),
                ("会计专用", "Accounting", "_ ¥* #,##0.00_ ;_ ¥* -#,##0.00_ ;_ ¥* \"-\"??_ ;_ @_ "),
                ("日期", "Date", "yyyy/m/d"),
                ("时间", "Time", "h:mm:ss"),
                ("百分比", "Percentage", "0.00%"),
                ("分数", "Fraction", "# ?/?"),
                ("科学记数", "Scientific", "0.00E+00"),
                ("文本", "Text", "@"),
            ] {
                let state = state.clone();
                categories = categories.child(
                    Button::new(SharedString::from(format!("number-category-{en}")))
                        .small()
                        .label(self.label(zh, en))
                        .on_click(move |_, w, cx| state.update(cx, |s, cx| s.set_value(value, w, cx))),
                );
            }
            fields = div().id("workflow-fields").p_4().flex().flex_col().gap_3().child(categories).child(fields);
            if let Some(view) = self.view()
                && let Some(sheet) = view.sheet()
            {
                let sample = joysheet_engine::display::format(&sheet.value(self.selection().active), &code, &view.workbook).text;
                fields = fields.child(
                    div().border_1().border_color(rgb(p.border)).p_3().child(self.label("示例", "Sample")).child(div().text_right().child(sample)),
                );
            }
        }
        if id == "view.zoom"
            && let Some(f) = self.command_fields.iter().find(|f| f.key == "percent")
        {
            let mut presets = div().flex().flex_col().gap_2();
            for percent in [200, 100, 75, 50, 25] {
                let state = f.value.clone();
                let selected = state.read(cx).value().as_ref() == percent.to_string();
                presets = presets.child(
                    Button::new(SharedString::from(format!("zoom-preset-{percent}")))
                        .ghost()
                        .justify_start()
                        .label(format!("{} {percent}%", if selected { "◉" } else { "○" }))
                        .on_click(move |_, w, cx| state.update(cx, |s, cx| s.set_value(percent.to_string(), w, cx))),
                );
            }
            fields = div().id("workflow-fields").p_4().flex().flex_col().gap_2().child(presets).child(fields);
        }
        if id == "pageLayout.pageSetup" && page == 1 {
            fields = fields.child(div().text_xs().text_color(rgb(p.muted)).child(self.label("单位：厘米", "Units: inches")));
        }
        if id == "home.formatCells" && page == 5 {
            fields = fields.child(div().text_sm().whitespace_normal().child(self.label(
                "只有保护工作表后，锁定单元格或隐藏公式才会生效。",
                "Locking cells or hiding formulas takes effect only when the worksheet is protected.",
            )));
        }
        if !self.dialog_error.is_empty() {
            fields = fields
                .child(div().id("dialog-error").role(Role::Alert).text_color(rgb(0xc00000)).whitespace_normal().child(self.dialog_error.clone()));
        }
        let mut tabbar = div().id("dialog-tabs").role(Role::TabList).flex().px_3().gap_1().border_b_1().border_color(rgb(p.border));
        let finding = matches!(id.as_str(), "edit.find" | "edit.replace");
        if finding && self.data_result_kind == id && self.data_raw.get("results").is_some_and(Value::is_array) {
            fields = fields.child(div().text_sm().child(self.label("查找结果", "Results")));
            for (i, (label, reference)) in self.data_results.iter().cloned().enumerate() {
                fields =
                    fields.child(Button::new(("find-result", i)).ghost().justify_start().label(label).on_click(cx.listener(move |v, _, w, cx| {
                        if let Some(reference) = &reference {
                            v.commit(crate::desktop::After::Command("edit.goTo".into(), json!({"reference":reference})), w, cx);
                        }
                    })));
            }
        }
        if id == "edit.replace"
            && self.data_result_kind == id
            && let Some(count) = self.data_raw.get("replaced").and_then(Value::as_u64)
        {
            fields = fields.child(div().text_sm().child(if self.chinese() {
                format!("已替换 {count} 处。")
            } else {
                format!("Replaced {count} occurrences.")
            }));
        }
        if finding {
            for (command, zh, en) in [("edit.find", "查找", "Find"), ("edit.replace", "替换", "Replace")] {
                tabbar = tabbar.child(Button::new(command).ghost().label(self.label(zh, en)).on_click(cx.listener(move |v, _, w, cx| {
                    v.selected_command = Some(command.into());
                    v.focus_command_input(w, cx);
                    cx.notify();
                })));
            }
        }
        for (i, t) in tabs.iter().enumerate() {
            tabbar = tabbar.child(
                div()
                    .id(SharedString::from(format!("dialog-tab-{i}")))
                    .relative()
                    .role(Role::Tab)
                    .aria_selected(i == page)
                    .tab_stop(true)
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .border_b_2()
                    .border_color(rgb(if i == page { p.accent } else { p.surface }))
                    .child(self.advanced_probe(format!("dialog-tab:{i}"), cx))
                    .child(self.label(t.zh, t.en))
                    .on_click(cx.listener(move |v, _, w, cx| {
                        v.dialog_tab = i;
                        v.focus_command_input(w, cx);
                        cx.notify();
                    })),
            );
        }
        let mut footer = div().flex().items_center().justify_end().gap_2().p_3().border_t_1().border_color(rgb(p.border));
        let splitting = id == "data.textToColumns";
        if splitting {
            footer = footer.child(
                Button::new("wizard-back")
                    .relative()
                    .child(self.advanced_probe("wizard-back".into(), cx))
                    .disabled(page == 0)
                    .label(self.label("< 上一步", "< Back"))
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.dialog_tab = v.dialog_tab.saturating_sub(1);
                        v.focus_command_input(w, cx);
                        cx.notify();
                    })),
            );
            if page < 2 {
                footer = footer.child(
                    Button::new("wizard-next")
                        .relative()
                        .child(self.advanced_probe("wizard-next".into(), cx))
                        .primary()
                        .label(self.label("下一步 >", "Next >"))
                        .on_click(cx.listener(|v, _, w, cx| {
                            v.dialog_tab = (v.dialog_tab + 1).min(2);
                            v.focus_command_input(w, cx);
                            cx.notify();
                        })),
                );
            }
        }
        if matches!(id.as_str(), "data.getData" | "data.fromTextCsv") {
            footer = footer.child(
                Button::new("pick-import-file")
                    .relative()
                    .child(self.advanced_probe("pick-import-file".into(), cx))
                    .label(self.label("浏览…", "Browse…"))
                    .on_click(cx.listener(|v, _, w, cx| v.pick_import(w, cx))),
            );
            footer = footer.child(
                Button::new("preview-import")
                    .relative()
                    .child(self.advanced_probe("preview-import".into(), cx))
                    .label(self.label("预览", "Preview"))
                    .on_click(cx.listener(|v, _, w, cx| v.preview_import(w, cx))),
            );
        }
        if matches!(id.as_str(), "insert.object" | "object.extract" | "insert.picture") {
            footer = footer.child(
                Button::new("pick-object-file")
                    .relative()
                    .child(self.advanced_probe("pick-object-file".into(), cx))
                    .label(self.label("浏览…", "Browse…"))
                    .on_click(cx.listener(|v, _, w, cx| v.pick_object_file(w, cx))),
            );
        }
        if finding {
            for (all, zh, en) in [
                (false, if id == "edit.find" { "查找下一个" } else { "替换" }, if id == "edit.find" { "Find Next" } else { "Replace" }),
                (true, if id == "edit.find" { "查找全部" } else { "全部替换" }, if id == "edit.find" { "Find All" } else { "Replace All" }),
            ] {
                footer = footer.child(Button::new(if all { "find-all" } else { "find-next" }).label(self.label(zh, en)).on_click(cx.listener(
                    move |v, _, w, cx| {
                        if let Some(f) = v.command_fields.iter().find(|f| f.key == "all") {
                            f.value.update(cx, |s, cx| s.set_value(all.to_string(), w, cx));
                        }
                        v.run_selected_command(w, cx);
                    },
                )));
            }
        }
        footer = footer
            .when(!finding && (!splitting || page == 2), |el| {
                el.child(
                    div().relative().child(self.advanced_probe("dialog-ok".into(), cx)).child(
                        Button::new("execute-command")
                            .relative()
                            .child(self.advanced_probe("execute-command".into(), cx))
                            .primary()
                            .disabled(unhide_empty)
                            .min_w(px(82.))
                            .label(if splitting { self.label("完成", "Finish") } else { self.label("确定", "OK") })
                            .on_click(cx.listener(|v, _, w, cx| v.run_selected_command(w, cx))),
                    ),
                )
            })
            .child(
                div().relative().child(self.advanced_probe("dialog-cancel".into(), cx)).child(
                    Button::new("cancel-command")
                        .relative()
                        .child(self.advanced_probe("cancel-command".into(), cx))
                        .min_w(px(82.))
                        .label(if finding { self.label("关闭", "Close") } else { self.label("取消", "Cancel") })
                        .on_click(cx.listener(|v, _, w, cx| v.close_command_dialog(w, cx))),
                ),
            );
        let width: f32 = if matches!(id.as_str(), "data.sort" | "data.textToColumns" | "data.getData" | "data.fromTextCsv") {
            700.
        } else if tabs.is_empty() {
            480.
        } else {
            620.
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000024))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("command-dialog")
                    .track_focus(&self.command_focus)
                    .role(Role::Dialog)
                    .aria_label(title(&id, self.chinese()))
                    .w(px(width.min(f32::from(window.viewport_size().width) - 32.)))
                    .max_h(px((f32::from(window.viewport_size().height) - 48.).max(180.)))
                    .flex()
                    .flex_col()
                    .bg(rgb(p.surface))
                    .border_1()
                    .border_color(rgb(p.border))
                    .rounded_md()
                    .shadow_xl()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_4()
                            .py_2()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(title(&id, self.chinese())))
                            .child(
                                Button::new("close-palette")
                                    .relative()
                                    .child(self.advanced_probe("close-palette".into(), cx))
                                    .ghost()
                                    .xsmall()
                                    .label("×")
                                    .accessibility_label(self.label("关闭", "Close"))
                                    .on_click(cx.listener(|v, _, w, cx| v.close_command_dialog(w, cx))),
                            ),
                    )
                    .when(!tabs.is_empty() || finding, |el| el.child(tabbar))
                    .child(
                        div()
                            .id("dialog-scroll")
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(self.excel_dialog_body(&id, page, cx).unwrap_or_else(|| fields.into_any_element())),
                    )
                    .child(footer)
                    .when(!self.dialog_error.is_empty(), |el| {
                        el.child(
                            div()
                                .id("special-dialog-error")
                                .p_3()
                                .role(Role::Alert)
                                .text_color(rgb(0xc00000))
                                .whitespace_normal()
                                .child(self.dialog_error.clone()),
                        )
                    })
                    .focus_trap("command-dialog-focus", &self.command_focus),
            )
            .into_any_element()
    }
}
