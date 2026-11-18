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
pub(crate) fn field_label(key: &str, zh: bool) -> String {
    let labels = match key {
        "red" => ("红色 (R)", "Red (R)"),
        "colorLow" => ("最小值颜色", "Minimum color"),
        "colorMiddle" => ("中间值颜色", "Midpoint color"),
        "colorHigh" => ("最大值颜色", "Maximum color"),
        "green" => ("绿色 (G)", "Green (G)"),
        "blue" => ("蓝色 (B)", "Blue (B)"),
        "tab" => ("Tab 键", "Tab"),
        "comma" => ("逗号", "Comma"),
        "semicolon" => ("分号", "Semicolon"),
        "space" => ("空格", "Space"),
        "other" => ("其他分隔符", "Other delimiter"),
        "treatConsecutive" => ("连续分隔符号视为单个处理", "Treat consecutive delimiters as one"),
        "textQualifier" => ("文本识别符", "Text qualifier"),
        "fixedBreaks" => ("分列线（字符位置）", "Column break positions"),
        "columnFormat" => ("列数据格式", "Column data format"),
        "borderStyle" => ("线条样式", "Line style"),
        "borderColor" => ("颜色", "Color"),
        "rank" => ("排名数量", "Rank"),
        "below" => ("低于平均值", "Below average"),
        "gradient" => ("渐变填充", "Gradient fill"),
        "reverse" => ("反转图标次序", "Reverse icon order"),
        "value2" => ("第二个值", "Second value"),
        "at" => ("插入位置", "Insert at"),
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
        "scope" => ("作用域", "Scope"),
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
        "lookIn" => ("查找范围", "Look in"),
        "within" => ("范围", "Within"),
        "byColumns" => ("按列搜索", "Search by columns"),
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
        "bold" => ("加粗", "Bold"),
        "italic" => ("倾斜", "Italic"),
        "strike" => ("删除线", "Strikethrough"),
        "wrap" => ("自动换行", "Wrap text"),
        "shrink" => ("缩小字体填充", "Shrink to fit"),
        "locked" => ("锁定", "Locked"),
        "hidden" => ("隐藏", "Hidden"),
        "orientation" => ("方向", "Orientation"),
        "paper" => ("纸张大小", "Paper size"),
        "scale" => ("缩放比例 (%)", "Adjust to (%)"),
        "fitWidth" => ("页宽", "Pages wide"),
        "fitHeight" => ("页高", "Pages tall"),
        "fitMode" => ("调整为指定页数", "Fit to pages"),
        "left" => ("左", "Left"),
        "right" => ("右", "Right"),
        "top" => ("上", "Top"),
        "bottom" => ("下", "Bottom"),
        "headerMargin" => ("页眉", "Header"),
        "footerMargin" => ("页脚", "Footer"),
        "centerH" => ("水平居中", "Horizontally"),
        "centerV" => ("垂直居中", "Vertically"),
        "gridlines" => ("网格线", "Gridlines"),
        "headings" => ("行号列标", "Row and column headings"),
        "printArea" => ("打印区域", "Print area"),
        "titleRows" => ("顶端标题行", "Rows to repeat at top"),
        "titleCols" => ("左端标题列", "Columns to repeat at left"),
        "footer" => ("页脚", "Footer"),
        "fontColor" => ("字体颜色", "Font color"),
        "fontScript" => ("上下标", "Effects"),
        "underline" => ("下划线", "Underline"),
        "fillPattern" => ("图案样式", "Pattern style"),
        "fillForeground" => ("图案颜色", "Pattern color"),
        "fillBackground" => ("背景色", "Background color"),
        "horizontal" => ("水平对齐", "Horizontal alignment"),
        "vertical" => ("垂直对齐", "Vertical alignment"),
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
        "points" => ("行高 (磅)", "Row height (points)"),
        "chars" => ("列宽 (字符)", "Column width (characters)"),
        "window" => ("取消隐藏工作簿", "Unhide workbook"),
        "sourcesText" => ("源区域，每行或逗号分隔", "Source ranges, one per line or comma separated"),
        "columnsText" => ("汇总列，逗号分隔", "Subtotal columns, comma separated"),
        "word" => ("单词", "Word"),
        "function" => ("函数", "Function"),
        "shift" => ("移动方向 down/right/up/left", "Shift down/right/up/left"),
        "text" => ("文本", "Text"),
        "find" => ("查找内容", "Find text"),
        "replace" => ("替换为", "Replace with"),
        "all" => ("全部替换", "Replace all"),
        "header" => ("首行是标题", "First row contains headers"),
        "type" => ("类型", "Type"),
        "style" => ("样式", "Style"),
        "layout" => ("布局", "Layout"),
        "warp" => ("文本转换", "Text transform"),
        "shadow" => ("阴影", "Shadow"),
        "pivot" => ("数据透视表", "PivotTable"),
        "level" => ("时间级别", "Time level"),
        "start" => ("开始日期 YYYY-MM-DD", "Start date YYYY-MM-DD"),
        "end" => ("结束日期 YYYY-MM-DD", "End date YYYY-MM-DD"),
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
        "view.unhideWindow" => json!({"window":0}),
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
    pub fn preview_import(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        match self.form_params(cx) {
            Ok(values) => {
                self.import_draft = Some(values.clone());
                self.excel_ui.import_preview = Value::Null;
                self.issue("__joysheet.previewImport", values, After::None, cx);
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
        self.dialog_tab = 0;
        self.dialog_error.clear();
        self.daily_group = 0;
        self.palette_open = true;
        self.selected_command = Some(id.clone());
        let mut values = defaults(&id);
        if id == "home.formatCells"
            && let Some(view) = self.view()
            && let Some(sheet) = view.sheet()
        {
            let st = view.workbook.styles.get(sheet.style_id(self.selection().active));
            values = json!({"fontName":st.font.name,"fontSize":st.font.size,"bold":st.font.bold,"italic":st.font.italic,"strike":st.font.strike,"underline":st.font.underline,"fontScript":st.font.vert,"fontColor":st.font.color.hex(&view.workbook.theme).unwrap_or("#000000".into()),"fillPattern":st.fill.pattern,"fillForeground":st.fill.fg.hex(&view.workbook.theme).unwrap_or("#000000".into()),"fillBackground":st.fill.bg.hex(&view.workbook.theme).unwrap_or("#FFFFFF".into()),"horizontal":st.align.h,"vertical":st.align.v,"angle":st.align.rotation,"indent":st.align.indent,"wrap":st.align.wrap,"shrink":st.align.shrink,"numberFormat":st.num_fmt,"locked":st.protection.locked,"hidden":st.protection.hidden});
            for (key, line) in [
                ("borderLeft", st.borders.left),
                ("borderRight", st.borders.right),
                ("borderTop", st.borders.top),
                ("borderBottom", st.borders.bottom),
                ("borderDown", st.borders.diag_down),
                ("borderUp", st.borders.diag_up),
            ] {
                values[key] = json!(line.style);
                values[format!("{key}Color")] = json!(line.color.hex(&view.workbook.theme).unwrap_or("#000000".into()));
            }
        }
        if let Some(view) = self.view()
            && let Some(sheet) = view.sheet()
        {
            let style = view.workbook.styles.get(sheet.style_id(self.selection().active));
            match id.as_str() {
                "home.fontName" => values["name"] = json!(style.font.name),
                "home.fontSize" => values["size"] = json!(style.font.size),
                "home.numberFormat" => values["code"] = json!(style.num_fmt),
                "view.zoom" => values["percent"] = json!(sheet.zoom),
                "home.rowHeight" => values = json!({"points":f64::from(sheet.row_height(self.selection().active.row))*0.75}),
                "home.columnWidth" => {
                    values = json!({"chars":joysheet_engine::model::points_to_chars(f64::from(sheet.col_width(self.selection().active.col)))})
                }
                "sheet.rename" => values["name"] = json!(sheet.name),
                "pageLayout.pageSetup" => values = crate::dialogs::page_setup_values(&sheet.print, self.chinese()),
                "pageLayout.headerFooter" => {
                    values["header"] = json!(sheet.print.header);
                    values["footer"] = json!(sheet.print.footer);
                }
                "pageLayout.scaleToFit" => {
                    let (width, height) = sheet.print.fit_to.unwrap_or((0, 0));
                    values = json!({"width":width,"height":height,"scale":sheet.print.scale,"fitMode":sheet.print.fit_to.is_some()});
                }
                "data.validation" => {
                    if let Some(rule) = sheet.validations.iter().find(|r| r.ranges.iter().any(|r| r.contains(self.selection().active))) {
                        values = json!({"type":match rule.kind {joysheet_engine::model::ValidationKind::Any=>"any",joysheet_engine::model::ValidationKind::Whole=>"whole",joysheet_engine::model::ValidationKind::Decimal=>"decimal",joysheet_engine::model::ValidationKind::List=>"list",joysheet_engine::model::ValidationKind::Date=>"date",joysheet_engine::model::ValidationKind::Time=>"time",joysheet_engine::model::ValidationKind::TextLength=>"textLength",joysheet_engine::model::ValidationKind::Custom=>"custom"},
                            "operator":match rule.op{joysheet_engine::model::CfOperator::Between=>"between",joysheet_engine::model::CfOperator::NotBetween=>"notBetween",joysheet_engine::model::CfOperator::Equal=>"equal",joysheet_engine::model::CfOperator::NotEqual=>"notEqual",joysheet_engine::model::CfOperator::Greater=>"greaterThan",joysheet_engine::model::CfOperator::Less=>"lessThan",joysheet_engine::model::CfOperator::GreaterOrEqual=>"greaterOrEqual",joysheet_engine::model::CfOperator::LessOrEqual=>"lessOrEqual"},
                            "formula1":rule.f1,"formula2":rule.f2.clone().unwrap_or_default(),"ignoreBlank":rule.allow_blank,"dropdown":rule.in_cell_dropdown,"inputTitle":rule.input_title,"inputMessage":rule.input_message,"errorTitle":rule.error_title,"errorMessage":rule.error_message,"errorStyle":match rule.error_style{joysheet_engine::model::ErrorStyle::Stop=>"stop",joysheet_engine::model::ErrorStyle::Warning=>"warning",joysheet_engine::model::ErrorStyle::Information=>"information"}});
                    }
                }
                "pageLayout.printTitles" => {
                    values["rows"] = json!(sheet.print.title_rows.map(|(a, b)| format!("{}:{}", a + 1, b + 1)).unwrap_or_default());
                    values["cols"] = json!(
                        sheet
                            .print
                            .title_cols
                            .map(|(a, b)| format!("{}:{}", joysheet_engine::core::col_to_letters(a), joysheet_engine::core::col_to_letters(b)))
                            .unwrap_or_default()
                    );
                }
                _ => {}
            }
        }
        if matches!(id.as_str(), "object.edit" | "object.extract" | "timeline.filter" | "timeline.level")
            && let Some(shape) = self.selected_object.as_ref().and_then(|o| self.view()?.sheet()?.shapes.iter().find(|s| s.id == o.id))
        {
            use joysheet_engine::model::{DrawingObject, SmartArtLayout, TimelineLevel};
            match (&shape.format.object, id.as_str()) {
                (Some(DrawingObject::SmartArt { layout, .. }), "object.edit") => {
                    values = json!({"text":shape.text,"layout":match layout{SmartArtLayout::Process=>"process",SmartArtLayout::List=>"list",SmartArtLayout::Hierarchy=>"hierarchy",SmartArtLayout::Cycle=>"cycle"}})
                }
                (Some(DrawingObject::WordArt { warp, shadow }), "object.edit") => {
                    values = json!({"text":shape.text,"warp":warp,"shadow":shadow,"color":self.view().and_then(|v|shape.format.text_color.and_then(|c|c.hex(&v.workbook.theme))).unwrap_or("#4472C4".into())})
                }
                (Some(DrawingObject::Equation { .. }), "object.edit") => values = json!({"text":shape.text}),
                (Some(object @ DrawingObject::Timeline { selection, level, .. }), _) => {
                    if id == "timeline.filter" {
                        let wb = &self.view().map(|v| v.workbook.clone()).unwrap_or_default();
                        let periods = joysheet_engine::drawing_scene::timeline_periods(wb, self.view().map(|v| v.sheet).unwrap_or(0), object);
                        let (start, end) = selection.unwrap_or((periods.first().map(|p| p.0).unwrap_or(0), periods.last().map(|p| p.1).unwrap_or(0)));
                        values = json!({"start":crate::object_ui::iso_date(wb,start),"end":crate::object_ui::iso_date(wb,end),"clear":false});
                    } else {
                        values = json!({"level":match level{TimelineLevel::Years=>"years",TimelineLevel::Quarters=>"quarters",TimelineLevel::Months=>"months",TimelineLevel::Days=>"days"}});
                    }
                }
                _ => {}
            }
        }
        if id.starts_with("insert.") && values.get("at").is_some() {
            values["at"] = json!(self.selection().active.a1());
        }
        if id == "pageLayout.breaks" {
            values["cell"] = json!(self.selection().active.a1());
        }
        if id.starts_with("review.") && values.get("cell").is_some() {
            values["cell"] = json!(self.selection().active.a1());
        }
        if let Some((_, v)) = values.as_object().into_iter().flatten().find(|(k, _)| crate::advanced::multiline(&id, k)) {
            let text = v.as_str().unwrap_or("").to_string();
            self.import_text.update(cx, |s, cx| s.set_value(text, window, cx));
        }
        if id.starts_with("timeline.")
            || id.starts_with("object.")
            || id.starts_with("shape.")
            || id.starts_with("arrange.")
            || id.starts_with("chart.")
        {
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
            if ["data.sort", "data.filter", "data.removeDuplicates", "insert.table", "home.formatAsTable", "insert.chart"].contains(&id.as_str())
                && self.selection().current().width() == 1
                && self.selection().current().height() == 1
                && let Some(sheet) = self.view().and_then(|v| v.sheet())
            {
                values["range"] = json!(joysheet_engine::selection::current_region(sheet, self.selection().active).a1());
            }
        }
        self.prepare_excel_dialog(&id, &mut values, cx);
        self.advanced_params =
            values.as_object().is_some_and(|fields| fields.iter().any(|(k, v)| !k.starts_with('_') && (v.is_array() || v.is_object())));
        self.command_initial = values.clone();
        self.make_command_fields(&values, window, cx);
        self.params.update(cx, |input, cx| {
            input.set_value(values.to_string(), window, cx);
        });
        self.focus_command_input(window, cx);
        if id == "view.unhideWindow" {
            self.data_raw = Value::Null;
            self.issue("view.switchWindows", json!({}), After::None, cx);
        }
        cx.notify();
    }
    pub fn focus_command_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.advanced_params {
            self.params.update(cx, |input, cx| input.focus(window, cx));
            return;
        }
        let id = self.selected_command.as_deref().unwrap_or("");
        if let Some(field) = self.command_fields.iter().find(|f| {
            self.dialog_field_visible(&f.key)
                && !f.initial.is_boolean()
                && crate::dialogs::choices(id, &f.key).is_empty()
                && self.object_choices(id, &f.key, cx).is_empty()
                && !crate::advanced::multiline(id, &f.key)
        }) {
            field.value.update(cx, |input, cx| input.focus(window, cx));
        } else if self.command_fields.iter().any(|f| self.dialog_field_visible(&f.key) && crate::advanced::multiline(id, &f.key)) {
            self.import_text.update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.command_focus.focus(window, cx);
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
                    && !key.starts_with('_')
                    && !(key.as_str() == "id"
                        || key.as_str() == "kind"
                            && self
                                .selected_command
                                .as_deref()
                                .is_some_and(|s| s.starts_with("object.") || s.starts_with("arrange.") || s.starts_with("shape.")))
            })
            .map(|(key, value)| {
                let text = value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
                let input = cx.new(|cx| InputState::new(window, cx).default_value(text).masked(key.to_ascii_lowercase().contains("password")));
                let source = key.clone();
                cx.subscribe_in(&input, window, move |v, _, event: &gpui_kit::component::input::InputEvent, w, cx| {
                    if matches!(event, gpui_kit::component::input::InputEvent::Change)
                        && matches!(v.selected_command.as_deref(), Some("home.fillColor" | "home.fontColor" | "home.borderColor"))
                        && matches!(source.as_str(), "color" | "red" | "green" | "blue")
                    {
                        v.excel_ui.color_source = source.clone();
                        if source == "color" {
                            if let Ok(n) = u32::from_str_radix(v.dialog_text("color", cx).trim_start_matches('#'), 16)
                                && n <= 0xffffff
                            {
                                for (k, n) in [("red", (n >> 16) & 255), ("green", (n >> 8) & 255), ("blue", n & 255)] {
                                    v.set_dialog_value(k, json!(n), w, cx);
                                }
                            }
                        } else if let Some(n) = v.proposed_rgb(cx) {
                            v.set_dialog_value("color", json!(format!("#{n:06X}")), w, cx);
                        }
                    }
                    cx.notify();
                })
                .detach();
                CommandField {
                    key: key.clone(),
                    label: if self.selected_command.as_deref() == Some("insert.timeline") && key == "column" {
                        self.label("日期字段", "Date field")
                    } else {
                        field_label(key, self.chinese())
                    },
                    value: input,
                    initial: value.clone(),
                }
            })
            .collect();
    }
    pub(crate) fn form_params(&self, cx: &App) -> Result<Value, String> {
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
        let id = self.selected_command.as_deref().unwrap_or("");
        self.excel_parameters(id, &mut values, cx)?;
        if matches!(id, "home.formatCells" | "pageLayout.pageSetup") {
            values["_original"] = self.command_initial.clone();
        }
        crate::workflow::parameters(id, values)
    }
    pub fn run_selected_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_command.clone() else {
            return;
        };
        let id = dialog_command(&id).to_string();
        if id == "data.textToColumns" && self.dialog_tab < 2 {
            self.dialog_tab += 1;
            self.focus_command_input(window, cx);
            cx.notify();
            return;
        }
        let parameters = if self.advanced_params {
            serde_json::from_str::<Value>(&self.params.read(cx).value()).map_err(|e| e.to_string())
        } else {
            self.form_params(cx)
        };
        match parameters {
            Ok(params) if params.is_object() => {
                if id == "home.conditionalFormat" && self.excel_ui.return_panel == 23 {
                    match self.stage_cf_rule(&params) {
                        Ok(()) => {
                            self.close_command_dialog(window, cx);
                        }
                        Err(e) => {
                            self.dialog_error = e;
                            cx.notify();
                        }
                    }
                    return;
                }
                if matches!(id.as_str(), "home.fillColor" | "home.fontColor") {
                    self.excel_ui.last_colors.insert(if id == "home.fillColor" { "fill" } else { "font-color" }.into(), params["color"].clone());
                }
                if id == "view.unhideWindow"
                    && !self.data_raw.as_array().is_some_and(|items| items.iter().any(|i| i["hidden"] == true && i["window"] == params["window"]))
                {
                    self.dialog_error = self.label("没有可取消隐藏的工作簿。", "There are no hidden workbooks.");
                    cx.notify();
                    return;
                }
                if id == "pageLayout.pageSetup" && params.as_object().is_some_and(|m| m.is_empty()) {
                    self.close_command_dialog(window, cx);
                    return;
                }
                if id == "home.formatCells" && self.excel_ui.border_ops.is_empty() && params["style"].as_object().is_some_and(|m| m.is_empty()) {
                    self.close_command_dialog(window, cx);
                    return;
                }
                if id == "automate.runScript" && !self.view().is_some_and(|v| v.scripts_trusted) {
                    self.request_trust(params["name"].as_str().unwrap_or("").into(), true, window, cx);
                    return;
                }
                if id == "automate.trustScripts" && params["trusted"] == true {
                    self.request_trust(String::new(), false, window, cx);
                    return;
                }
                if (id.starts_with("object.") || id.starts_with("shape.") || id.starts_with("arrange.") || id.starts_with("timeline."))
                    && self.selected_anchor().is_none()
                {
                    self.alert("对象所在文档已切换，请重新选择 / Select the object again");
                    cx.notify();
                    return;
                }
                if ["data.getData", "data.fromTextCsv"].contains(&id.as_str()) {
                    self.import_draft = Some(params.clone());
                }
                let (id, params) = if id == "home.formatCells" && !self.excel_ui.border_ops.is_empty() {
                    let mut params = params;
                    params["style"].as_object_mut().map(|s| s.remove("borders"));
                    ("__joysheet.formatDialog".to_string(), json!({"format":params,"borders":self.excel_ui.border_ops}))
                } else {
                    (id, params)
                };
                let keep_open = matches!(id.as_str(), "edit.find" | "edit.replace");
                self.dialog_error.clear();
                if id == "view.arrangeAll" {
                    self.issue(&id, params, After::None, cx);
                } else {
                    self.commit(After::Command(id, params), window, cx);
                }
                self.palette_open = keep_open;
                if !keep_open && self.excel_ui.return_panel != 0 {
                    self.daily_group = self.excel_ui.return_panel;
                    self.excel_ui.return_panel = 0;
                }
                if keep_open {
                    self.focus_command_input(window, cx);
                } else if matches!(self.daily_group, 19 | 23) {
                    self.command_focus.focus(window, cx);
                } else {
                    self.focus_grid(window, cx);
                }
            }
            Ok(_) => {
                self.alert("参数必须是 JSON 对象 / Parameters must be a JSON object");
                cx.notify();
            }
            Err(error) => {
                self.dialog_error = error.clone();
                self.alert(error.to_string());
                cx.notify();
            }
        }
    }
    pub(crate) fn render_command_center(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.palette();
        let panel = p.surface;
        let muted = p.muted;
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
                    let choices = if field.initial.is_string() { self.object_choices(id, &field.key, cx) } else { Vec::new() };
                    if !choices.is_empty() {
                        let selected = field.value.read(cx).value().to_string();
                        let mut options = div().flex().flex_wrap().gap_1();
                        for (value, label) in choices {
                            let key = field.key.clone();
                            options = options.child(
                                Button::new(SharedString::from(format!("choice-{key}-{value}")))
                                    .label(label)
                                    .when(selected == value, |b| b.primary())
                                    .on_click(cx.listener(move |this, _, window, cx| this.advanced_choice(&key, &value, window, cx))),
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
            if let Some(preview) = self.object_preview(id, cx) {
                fields = div().flex().flex_col().gap_3().child(preview).child(fields);
            }
            body = body.child(
                div()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(p.border))
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
            if matches!(id.as_str(), "insert.object" | "object.extract" | "insert.picture") {
                body = body.child(
                    Button::new("pick-object-file")
                        .label(self.label("浏览…", "Browse…"))
                        .on_click(cx.listener(|this, _, w, cx| this.pick_object_file(w, cx))),
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
                        .hover(|style| style.bg(rgb(p.hover)))
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
        use gpui_kit::base::FocusTrapElement as _;
        let cancel = Button::new("cancel-close")
            .disabled(self.close_saving)
            .label(self.label("取消", "Cancel"))
            .on_click(cx.listener(|this, _, window, cx| this.cancel_close(window, cx)));
        let discard = Button::new("discard-close")
            .disabled(self.close_saving)
            .label(self.label("不保存", "Discard"))
            .on_click(cx.listener(|this, _, window, cx| this.after(After::Close(true), window, cx)));
        let save = Button::new("save-close")
            .primary()
            .disabled(self.close_saving)
            .label(if self.close_saving { self.label("保存中…", "Saving…") } else { self.label("保存", "Save") })
            .on_click(cx.listener(|this, _, window, cx| this.save_then_close(window, cx)));
        let status = self.error.then(|| {
            div().id("save-changes-status").role(Role::Status).text_sm().text_color(rgb(0xb42318)).child(self.message.clone()).into_any_element()
        });
        let panel = joyoffice_ui::save_changes_panel(
            self.palette(),
            self.label("保存更改？", "Save changes?"),
            self.label("关闭工作簿前可以先保存。", "Save this workbook before closing."),
            status,
            [
                self.close_button("cancel", cancel, cx).into_any_element(),
                self.close_button("discard", discard, cx).into_any_element(),
                self.close_button("save", save, cx).into_any_element(),
            ],
        )
        .key_context("JoysheetCloseDialog")
        .child(self.close_probe("panel", cx))
        .focus_trap("save-changes-focus", &self.close_focus);
        joyoffice_ui::save_changes_backdrop().child(panel)
    }
    pub fn save_then_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_saving || self.save_as_pending || !self.confirm_close {
            return;
        }
        let Some(view) = self.view() else { return };
        if view.path.is_none() {
            self.pick_save_for(true, window, cx);
        } else {
            let target = joysheet_app::SaveTarget::from(view);
            self.close_saving = true;
            self.error = false;
            self.submit_target_save(target, None, true, cx);
        }
    }
}
