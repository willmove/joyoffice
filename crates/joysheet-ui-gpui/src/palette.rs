use crate::desktop::{After, Desktop};
use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::{Input, InputState, Textarea},
};
use gpui_kit::{prelude::*, *};
use joysheet_engine::find_command;
use serde_json::{Value, json};

pub struct CommandField {
    pub key: String,
    pub label: String,
    pub value: Entity<InputState>,
    pub initial: Value,
}
fn field_label(key: &str, zh: bool) -> String {
    let labels = match key {
        "source" => ("源区域或表名", "Source range or table"),
        "destination" => ("目标地址或 new", "Destination or new"),
        "rowFields" => ("行字段，逗号分隔", "Row fields, comma separated"),
        "columnFields" => ("列字段，逗号分隔", "Column fields"),
        "valueFields" => ("值字段，逗号分隔", "Value fields"),
        "aggregate" => ("汇总方式", "Aggregation"),
        "tool" => ("分析工具", "Analysis tool"),
        "output" => ("输出起始地址", "Output cell"),
        "outputTo" => ("输出位置 range/newSheet", "Output range/newSheet"),
        "newSheet" => ("新工作表名", "New worksheet"),
        "objective" => ("目标公式单元格", "Objective formula cell"),
        "variables" => ("可变单元格区域", "Variable cells"),
        "goal" => ("目标", "Goal"),
        "method" => ("求解方法", "Method"),
        "constraintLines" => ("约束，每行一条", "Constraints, one per line"),
        "password" => ("密码", "Password"),
        "refersTo" => ("引用位置或公式", "Refers to"),
        "scope" => ("作用域 Workbook/工作表名", "Scope: Workbook or sheet"),
        "oldName" => ("编辑已有名称（新建时留空）", "Existing name, blank for new"),
        "oldScope" => ("原作用域（新建时留空）", "Existing scope"),
        "commandsText" => ("脚本步骤（JSON 命令数组）", "Script command array (JSON)"),
        "stop" => ("停止并保存录制", "Stop and save recording"),
        "trusted" => ("此会话信任", "Trust this session"),
        "range" => ("单元格区域", "Cell range"),
        "what" => ("查找内容", "Find"),
        "with" => ("替换为", "Replace with"),
        "column" => ("列字母", "Column letter"),
        "columns" => ("列字母，逗号分隔", "Columns, comma separated"),
        "order" => ("排序方向 asc/desc", "Order asc/desc"),
        "operator" => ("比较运算符", "Comparison operator"),
        "value" => ("比较值", "Comparison value"),
        "clear" => ("清除此列筛选", "Clear column filter"),
        "input" => ("填充内容或公式", "Fill value or formula"),
        "matchCase" => ("区分大小写", "Match case"),
        "wholeCell" => ("匹配整个单元格", "Match entire cell"),
        "lookIn" => ("查找位置 values/formulas", "Look in values/formulas"),
        "sheet" => ("工作表名称", "Worksheet name"),
        "by" => ("分组方向 rows/cols", "Group rows/cols"),
        "lines" => ("行或列范围，例如 2:4 / B:D", "Row or column range e.g. 2:4 / B:D"),
        "rows" => ("行层级（1–8）", "Row level (1–8)"),
        "cols" => ("列层级（1–8）", "Column level (1–8)"),
        "name" => ("名称", "Name"),
        "path" => ("本地文件路径", "Local file path"),
        "delimiter" => ("分隔符", "Delimiter"),
        "size" => ("字体大小", "Font size"),
        "fontName" => ("字体", "Font family"),
        "fontSize" => ("字号", "Font size"),
        "fontColor" => ("字体颜色 #RRGGBB", "Font color #RRGGBB"),
        "fontScript" => ("上下标 Baseline/Superscript/Subscript", "Baseline/Superscript/Subscript"),
        "underline" => ("下划线 None/Single/Double", "Underline None/Single/Double"),
        "fillPattern" => ("填充 Solid/Gray25/LightDown/LightGrid 等", "Fill Solid/Gray25/LightDown/LightGrid etc."),
        "fillForeground" => ("图案前景色 #RRGGBB", "Pattern foreground #RRGGBB"),
        "fillBackground" => ("图案背景色 #RRGGBB", "Pattern background #RRGGBB"),
        "horizontal" => ("水平 General/Left/Center/Right", "Horizontal alignment"),
        "vertical" => ("垂直 Top/Center/Bottom", "Vertical alignment"),
        "angle" => ("旋转角度 −90…90，255 为竖排", "Rotation −90…90; 255 stacked"),
        "indent" => ("缩进", "Indent"),
        "numberFormat" => ("数字格式", "Number format"),
        "keysText" => ("后续排序键，例如 B:desc,C:asc", "Additional sort keys, e.g. B:desc,C:asc"),
        "preset" => ("格式预设", "Format preset"),
        "set" => ("图标集", "Icon set"),
        "ignoreBlank" => ("忽略空值", "Ignore blanks"),
        "dropdown" => ("单元格下拉选项", "In-cell dropdown"),
        "errorMessage" => ("错误提示", "Error message"),
        "color" => ("颜色", "Color"),
        "code" => ("数字格式", "Number format"),
        "width" => ("宽度", "Width"),
        "height" => ("高度", "Height"),
        "text" => ("查找内容", "Find text"),
        "find" => ("查找内容", "Find text"),
        "replace" => ("替换为", "Replace with"),
        "all" => ("全部替换", "Replace all"),
        "header" => ("首行是标题", "First row contains headers"),
        "type" => ("类型", "Type"),
        "style" => ("样式", "Style"),
        "title" => ("标题", "Title"),
        "reference" => ("地址或名称", "Address or name"),
        "percent" => ("缩放百分比", "Zoom percent"),
        "formula1" => ("最小值或公式", "Minimum or formula"),
        "formula2" => ("最大值或公式", "Maximum or formula"),
        "to" => ("目标位置，从 0 开始", "Destination index, from 0"),
        "copy" => ("创建副本", "Create a copy"),
        _ => (key, key),
    };
    if zh { labels.0.into() } else { labels.1.into() }
}

fn defaults(id: &str) -> Value {
    if let Some(values) = crate::workflow::defaults(id) {
        return values;
    }
    match id {
        "home.fontName" => json!({"name":"Microsoft YaHei UI"}),
        "home.fontSize" => json!({"size":12}),
        "table.style" => json!({"style":"TableStyleMedium2"}),
        "home.fillColor" => json!({"color":"#E3F2E7"}),
        "home.fontColor" => json!({"color":"#147D59"}),
        "home.borders" => json!({"preset":"all","style":"thin","color":"#27382E"}),
        "home.numberFormat" => json!({"code":"#,##0.00"}),
        "home.columnWidth" => json!({"width":100}),
        "home.rowHeight" => json!({"height":28}),
        "sheet.rename" => json!({"name":"报表"}),
        "home.insertSheet" => json!({"name":"新工作表"}),
        "sheet.unhide" => json!({"sheet":""}),
        "edit.find" => json!({"text":""}),
        "edit.replace" => json!({"find":"","replace":"","all":false}),
        "data.sort" => json!({"keys":[{"column":0,"order":"asc"}]}),
        "insert.chart" => json!({"type":"column"}),
        "insert.table" => json!({"header":true,"style":"TableStyleMedium2"}),
        "data.validation" => json!({"type":"whole","formula1":"0","formula2":"100"}),
        "sheet.move" => json!({"to":0,"copy":false}),
        "formulas.defineName" => json!({"name":"Total","refersTo":"=Sheet1!$A$1"}),
        "edit.goTo" => json!({"reference":"A1"}),
        "view.zoom" => json!({"percent":100}),
        _ => json!({}),
    }
}
fn dialog_command(name: &str) -> &str {
    match name {
        "analysisToolPak" => "data.analysisToolPak",
        "solver" => "data.solver",
        "dataValidation" => "data.validation",
        "pivotCalculatedField" => "pivot.calculatedField",
        "newFormattingRule" => "home.conditionalFormat",
        "insertLink" => "insert.link",
        "insertPicture" => "insert.picture",
        "unhideSheet" => "sheet.unhide",
        "formatCells" => "home.formatCells",
        "find" => "edit.find",
        "replace" => "edit.replace",
        "sort" => "data.sort",
        "chart" | "insertChart" => "insert.chart",
        "insertFunction" => "formulas.insertFunction",
        "pasteSpecial" => "edit.pasteSpecial",
        "defineName" => "formulas.defineName",
        "goTo" => "edit.goTo",
        "zoom" => "view.zoom",
        "columnWidth" => "home.columnWidth",
        "rowHeight" => "home.rowHeight",
        _ => name,
    }
}
impl Desktop {
    pub fn preview_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.form_params(cx) {
            Ok(values) => {
                self.import_draft = Some(values.clone());
                self.palette_open = false;
                self.commit(After::Command("__joysheet.previewImport".into(), values), window, cx);
            }
            Err(error) => self.alert(error),
        }
        cx.notify();
    }
    pub fn pick_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            if let Some(path) = joysheet_platform::pick_open().await {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.import_draft = Some(json!({"path":path.to_string_lossy(),"text":""}));
                    this.open_command("data.getData", window, cx);
                });
            }
        })
        .detach();
    }
    pub fn open_command(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let id = dialog_command(id).to_string();
        self.daily_group = 0;
        self.palette_open = true;
        self.selected_command = Some(id.clone());
        let mut values = defaults(&id);
        if id == "home.formatCells"
            && let Some(view) = self.view()
            && let Some(sheet) = view.sheet()
        {
            let st = view.workbook.styles.get(sheet.style_id(self.selection().active));
            values = json!({"fontName":st.font.name,"fontSize":st.font.size,"bold":st.font.bold,"italic":st.font.italic,"underline":st.font.underline,"fontScript":st.font.vert,"fontColor":st.font.color.hex(&view.workbook.theme).unwrap_or("#27382E".into()),"fillPattern":st.fill.pattern,"fillForeground":st.fill.fg.hex(&view.workbook.theme).unwrap_or("#147D59".into()),"fillBackground":st.fill.bg.hex(&view.workbook.theme).unwrap_or("#FFFFFF".into()),"horizontal":st.align.h,"vertical":st.align.v,"angle":st.align.rotation,"indent":st.align.indent,"wrap":st.align.wrap,"shrink":st.align.shrink,"numberFormat":st.num_fmt});
        }
        if id.starts_with("review.") && values.get("cell").is_some() {
            values["cell"] = json!(self.selection().active.a1());
        }
        if let Some((_, v)) = values.as_object().into_iter().flatten().find(|(k, _)| crate::advanced::multiline(&id, k)) {
            let text = v.as_str().unwrap_or("").to_string();
            self.import_text.update(cx, |s, cx| s.set_value(text, window, cx));
        }
        if id.starts_with("object.") || id.starts_with("shape.") || id.starts_with("arrange.") || id.starts_with("chart.") {
            if let Some(object) = self.selected_object.as_ref().filter(|_| self.selected_anchor().is_some()) {
                values["id"] = json!(object.id);
                if id.starts_with("chart.") && !id.starts_with("chart.shape") {
                    values["chart"] = json!(object.id);
                }
                if id.starts_with("object.") || id.starts_with("arrange.") || id.starts_with("chart.shape") {
                    values["kind"] = json!(object.kind);
                }
                if id == "object.move"
                    && let Some(a) = self.selected_anchor()
                {
                    values["at"] = json!(a.cell.a1());
                    values["dx"] = json!(a.dx);
                    values["dy"] = json!(a.dy);
                    values["width"] = json!(a.width);
                    values["height"] = json!(a.height);
                }
            } else if !id.starts_with("chart.") {
                self.palette_open = false;
                self.alert("请先在网格中选择一个对象 / Select an object first");
                return;
            }
        }
        if ["data.getData", "data.fromTextCsv"].contains(&id.as_str()) {
            if let Some(saved) = &self.import_draft
                && let (Some(a), Some(b)) = (values.as_object_mut(), saved.as_object())
            {
                a.extend(b.clone());
            }
            let text = values.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            self.import_text.update(cx, |input, cx| input.set_value(text, window, cx));
        }
        if id == "sheet.unhide"
            && let Some(name) = self
                .view()
                .and_then(|v| v.workbook.sheets.iter().find(|s| s.visibility != joysheet_engine::model::Visibility::Visible))
                .map(|s| s.name.clone())
        {
            values["sheet"] = json!(name);
        }
        if find_command(&id).is_some_and(|s| s.params.contains("range?"))
            && !matches!(id.as_str(), "file.print" | "file.printPreview" | "file.exportPdf")
        {
            values["range"] = json!(self.selection().a1());
            if ["data.sort", "data.filter", "data.removeDuplicates", "insert.table"].contains(&id.as_str())
                && self.selection().current().width() == 1
                && self.selection().current().height() == 1
                && let Some(sheet) = self.view().and_then(|v| v.sheet())
            {
                values["range"] = json!(joysheet_engine::selection::current_region(sheet, self.selection().active).a1());
            }
        }
        self.advanced_params = values.as_object().is_some_and(|fields| fields.values().any(|v| v.is_array() || v.is_object()));
        self.make_command_fields(&values, window, cx);
        self.params.update(cx, |input, cx| {
            input.set_value(values.to_string(), window, cx);
        });
        self.focus_command_input(window, cx);
        cx.notify();
    }
    pub fn focus_command_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.advanced_params {
            self.params.update(cx, |input, cx| input.focus(window, cx));
            return;
        }
        let id = self.selected_command.as_deref().unwrap_or("");
        if let Some(field) = self
            .command_fields
            .iter()
            .find(|f| !f.initial.is_boolean() && crate::advanced::choices(&f.key).is_empty() && !crate::advanced::multiline(id, &f.key))
        {
            field.value.update(cx, |input, cx| input.focus(window, cx));
        } else if self.command_fields.iter().any(|f| crate::advanced::multiline(id, &f.key)) {
            self.import_text.update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.search.update(cx, |input, cx| input.focus(window, cx));
        }
    }
    pub fn make_command_fields(&mut self, values: &Value, window: &mut Window, cx: &mut Context<Self>) {
        self.command_fields = values
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(key, v)| {
                !v.is_array()
                    && !v.is_object()
                    && !(key.as_str() == "id"
                        || key.as_str() == "kind"
                            && self
                                .selected_command
                                .as_deref()
                                .is_some_and(|s| s.starts_with("object.") || s.starts_with("arrange.") || s.starts_with("shape.")))
            })
            .map(|(key, value)| {
                let text = value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
                CommandField {
                    key: key.clone(),
                    label: field_label(key, self.chinese()),
                    value: cx.new(|cx| InputState::new(window, cx).default_value(text).masked(key.to_ascii_lowercase().contains("password"))),
                    initial: value.clone(),
                }
            })
            .collect();
    }
    fn form_params(&self, cx: &App) -> Result<Value, String> {
        let mut values = serde_json::from_str::<Value>(&self.params.read(cx).value()).map_err(|e| e.to_string())?;
        if !values.is_object() {
            return Err("Parameters must be an object".into());
        }
        for field in &self.command_fields {
            let text = if self.selected_command.as_deref().is_some_and(|id| {
                crate::advanced::multiline(id, &field.key) || field.key == "text" && ["data.getData", "data.fromTextCsv"].contains(&id)
            }) {
                self.import_text.read(cx).value().to_string()
            } else {
                field.value.read(cx).value().to_string()
            };
            let value = if field.initial.is_string() {
                Value::String(text)
            } else {
                let value = serde_json::from_str::<Value>(&text)
                    .map_err(|_| format!("{}: {}", field.label, self.label("请输入有效数字或布尔值", "Enter a valid number or boolean")))?;
                if (field.initial.is_number() && !value.is_number()) || (field.initial.is_boolean() && !value.is_boolean()) {
                    return Err(format!("{}: invalid value", field.label));
                }
                value
            };
            values[&field.key] = value;
        }
        crate::workflow::parameters(self.selected_command.as_deref().unwrap_or(""), values)
    }
    pub fn run_selected_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_command.clone() else {
            return;
        };
        let id = dialog_command(&id).to_string();
        let parameters = if self.advanced_params {
            serde_json::from_str::<Value>(&self.params.read(cx).value()).map_err(|e| e.to_string())
        } else {
            self.form_params(cx)
        };
        match parameters {
            Ok(params) if params.is_object() => {
                if id == "automate.runScript" && !self.view().is_some_and(|v| v.scripts_trusted) {
                    self.request_trust(params["name"].as_str().unwrap_or("").into(), true, window, cx);
                    return;
                }
                if id == "automate.trustScripts" && params["trusted"] == true {
                    self.request_trust(String::new(), false, window, cx);
                    return;
                }
                if (id.starts_with("object.") || id.starts_with("shape.") || id.starts_with("arrange.")) && self.selected_anchor().is_none() {
                    self.alert("对象所在文档已切换，请重新选择 / Select the object again");
                    cx.notify();
                    return;
                }
                if ["data.getData", "data.fromTextCsv"].contains(&id.as_str()) {
                    self.import_draft = Some(params.clone());
                }
                if id == "view.arrangeAll" {
                    self.issue(&id, params, After::None, cx);
                } else {
                    self.commit(After::Command(id, params), window, cx);
                }
                self.palette_open = false;
                self.focus_grid(window, cx);
            }
            Ok(_) => {
                self.alert("参数必须是 JSON 对象 / Parameters must be a JSON object");
                cx.notify();
            }
            Err(error) => {
                self.alert(error.to_string());
                cx.notify();
            }
        }
    }
    pub fn render_palette(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let dark = self.preferences.dark;
        let panel = if dark { 0x20332a } else { 0xffffff };
        let muted = if dark { 0xa6b7ac } else { 0x708477 };
        let query = self.search.read(cx).value().to_lowercase();
        let commands: Vec<_> = joysheet_engine::command_specs()
            .iter()
            .filter(|s| query.is_empty() || s.id.to_lowercase().contains(&query) || s.label.to_lowercase().contains(&query))
            .take(70)
            .map(|s| (s.id, s.label, s.menu.join(" / ")))
            .collect();
        let mut body = div()
            .id("command-panel")
            .absolute()
            .right_0()
            .top_0()
            .bottom_0()
            .w(px(460.))
            .flex()
            .flex_col()
            .bg(rgb(panel))
            .shadow_xl()
            .p_5()
            .gap_3()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(self.label("命令中心", "Command center")))
                    .child(Button::new("close-palette").label("×").on_click(cx.listener(|this, _, window, cx| {
                        this.palette_open = false;
                        this.focus_grid(window, cx);
                        cx.notify();
                    }))),
            )
            .child(Input::new(&self.search).id("command-search").aria_label("搜索命令 / Search commands"));
        if let Some(id) = self.selected_command.as_ref() {
            let command = find_command(dialog_command(id));
            let description = command.map(|c| c.params).unwrap_or("Enter explicit parameters for this command.");
            let mut fields = div().flex().flex_col().gap_3();
            if self.advanced_params {
                fields = fields
                    .child(div().text_xs().child(description.to_string()))
                    .child(Input::new(&self.params).id("command-params").aria_label("Advanced JSON parameters"));
            } else {
                for field in &self.command_fields {
                    let choices = if field.initial.is_string() { crate::advanced::choices(&field.key) } else { Vec::new() };
                    if !choices.is_empty() {
                        let selected = field.value.read(cx).value().to_string();
                        let mut options = div().flex().flex_wrap().gap_1();
                        for (value, label) in choices {
                            let key = field.key.clone();
                            options = options.child(
                                Button::new(SharedString::from(format!("choice-{key}-{value}")))
                                    .label(label)
                                    .when(selected == value, |b| b.primary())
                                    .on_click(cx.listener(move |this, _, window, cx| this.advanced_choice(&key, value, window, cx))),
                            );
                        }
                        fields = fields.child(div().text_sm().child(field.label.clone())).child(options);
                        continue;
                    }
                    if field.initial.is_boolean() {
                        let state = field.value.clone();
                        let enabled = state.read(cx).value().as_ref() == "true";
                        fields = fields.child(
                            Button::new(SharedString::from(format!("bool-{}", field.key)))
                                .label(format!("{}: {}", field.label, if enabled { "✓" } else { "—" }))
                                .on_click(move |_, window, cx| {
                                    state.update(cx, |input, cx| {
                                        let next = input.value().as_ref() != "true";
                                        input.set_value(next.to_string(), window, cx);
                                    });
                                }),
                        );
                        continue;
                    }
                    if crate::advanced::multiline(id, &field.key)
                        || field.key == "text" && ["data.getData", "data.fromTextCsv"].contains(&id.as_str())
                    {
                        fields = fields.child(div().text_sm().child(field.label.clone())).child(Textarea::new(&self.import_text).h(px(140.)));
                        continue;
                    }
                    fields =
                        fields.child(
                            div().flex().flex_col().gap_1().child(div().text_sm().child(field.label.clone())).child(
                                Input::new(&field.value).id(SharedString::from(format!("field-{}", field.key))).aria_label(field.label.clone()),
                            ),
                        );
                }
            }
            body = body.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(if dark { 0x3c5144 } else { 0xdfe8e0 }))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(command.map(|c| c.label.to_string()).unwrap_or_else(|| id.clone())))
                    .child(fields.id("workflow-fields").max_h(px(420.)).overflow_y_scroll())
                    .child(
                        Button::new("execute-command")
                            .primary()
                            .label(self.label("应用", "Apply"))
                            .on_click(cx.listener(|this, _, window, cx| this.run_selected_command(window, cx))),
                    ),
            );
            if ["data.getData", "data.fromTextCsv"].contains(&id.as_str()) {
                body = body
                    .child(
                        Button::new("pick-import-file")
                            .label("选择文件… / Choose file")
                            .on_click(cx.listener(|this, _, window, cx| this.pick_import(window, cx))),
                    )
                    .child(
                        Button::new("preview-import")
                            .label("预览（不修改工作簿） / Preview")
                            .on_click(cx.listener(|this, _, window, cx| this.preview_import(window, cx))),
                    );
            }
            body = body.child(Button::new("advanced-parameters").label(self.label("高级参数", "Advanced parameters")).on_click(cx.listener(
                |this, _, window, cx| {
                    if !this.advanced_params {
                        match this.form_params(cx) {
                            Ok(values) => this.params.update(cx, |input, cx| input.set_value(values.to_string(), window, cx)),
                            Err(error) => {
                                this.alert(error);
                                return;
                            }
                        }
                    } else if let Ok(values) = serde_json::from_str::<Value>(&this.params.read(cx).value()) {
                        this.make_command_fields(&values, window, cx);
                    }
                    this.advanced_params = !this.advanced_params;
                    cx.notify();
                },
            )));
        }
        body = body
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(muted))
                    .child(self.label("更改应用于当前工作表和所选区域。", "Changes apply to the current worksheet and selected range.")),
            )
            .child(div().id("command-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_1().children(commands.into_iter().map(
                |(id, label, path)| {
                    div()
                        .id(id)
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(if dark { 0x304a3a } else { 0xf0f5f0 })))
                        .child(div().font_weight(FontWeight::MEDIUM).child(label))
                        .child(div().text_xs().text_color(rgb(muted)).child(format!("{id}  ·  {path}")))
                        .on_click(cx.listener(move |this, _, window, cx| this.open_command(id, window, cx)))
                },
            )));
        div().absolute().inset_0().occlude().bg(rgba(0x0e1e1833)).child(body)
    }
    fn close_probe(&self, id: &'static str, cx: &Context<Self>) -> impl IntoElement + use<> {
        let view = cx.entity();
        canvas(
            move |bounds, _, cx| {
                view.update(cx, |this, _| {
                    this.close_button_bounds.insert(id, bounds);
                })
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
    fn close_button(&self, id: &'static str, button: Button, cx: &Context<Self>) -> impl IntoElement + use<> {
        div().relative().child(button).child(self.close_probe(id, cx))
    }
    pub fn cancel_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_saving {
            return;
        }
        self.confirm_close = false;
        self.focus_grid(window, cx);
        cx.notify();
    }
    pub fn render_close_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let panel = if self.preferences.dark { 0x20332a } else { 0xffffff };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .track_focus(&self.close_focus)
            .key_context("JoysheetCloseDialog")
            .bg(rgba(0x0e1e1855))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .relative()
                    .w(px(450.))
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(panel))
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(self.close_probe("panel", cx))
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(self.label("保存更改？", "Save changes?")))
                    .child(self.label("关闭工作簿前可以先保存。", "Save this workbook before closing."))
                    .when(self.error, |el| el.child(div().text_sm().text_color(rgb(0xb42318)).child(self.message.clone())))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                self.close_button(
                                    "cancel",
                                    Button::new("cancel-close")
                                        .disabled(self.close_saving)
                                        .label(self.label("取消", "Cancel"))
                                        .on_click(cx.listener(|this, _, window, cx| this.cancel_close(window, cx))),
                                    cx,
                                ),
                            )
                            .child(
                                self.close_button(
                                    "discard",
                                    Button::new("discard-close")
                                        .disabled(self.close_saving)
                                        .label(self.label("不保存", "Discard"))
                                        .on_click(cx.listener(|this, _, window, cx| this.after(After::Close(true), window, cx))),
                                    cx,
                                ),
                            )
                            .child(
                                self.close_button(
                                    "save",
                                    Button::new("save-close")
                                        .primary()
                                        .disabled(self.close_saving)
                                        .label(if self.close_saving { self.label("保存中…", "Saving…") } else { self.label("保存", "Save") })
                                        .on_click(cx.listener(|this, _, window, cx| this.save_then_close(window, cx))),
                                    cx,
                                ),
                            ),
                    ),
            )
    }
    fn submit_close_save(&mut self, doc: u64, revision: u64, path: Option<std::path::PathBuf>, cx: &mut Context<Self>) {
        if !self.view().is_some_and(|v| v.doc == doc) {
            self.close_saving = false;
            self.alert("保存目标文档已更改 / Save target changed");
            cx.notify();
            return;
        }
        let params = path.map(|p| json!({"path":p})).unwrap_or_else(|| json!({}));
        match self.engine.submit_guarded("file.save", params, self.target(), Some(revision)) {
            Ok(ticket) => self.pending.push(crate::desktop::Pending { ticket, after: After::CloseSaved, commit: false, clipboard: false }),
            Err(error) => {
                self.close_saving = false;
                self.alert(error);
            }
        }
        cx.notify();
    }
    pub fn save_then_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_saving || !self.confirm_close {
            return;
        }
        let Some(view) = self.view() else { return };
        let (doc, revision, has_path, name) = (view.doc, view.revision, view.path.is_some(), view.title.clone());
        self.close_saving = true;
        self.error = false;
        cx.notify();
        if has_path {
            self.submit_close_save(doc, revision, None, cx);
        } else {
            let name = if name.to_ascii_lowercase().ends_with(".xlsx") { name } else { format!("{name}.xlsx") };
            cx.spawn_in(window, async move |this, cx| {
                let path = joysheet_platform::pick_save(&name).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if let Some(path) = path {
                        this.submit_close_save(doc, revision, Some(path), cx);
                    } else {
                        this.close_saving = false;
                        window.focus(&this.close_focus, cx);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }
}
