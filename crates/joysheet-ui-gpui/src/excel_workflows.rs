//! Stateful desktop controls for Excel-style galleries and data wizards.
use crate::desktop::Desktop;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        popover::Popover,
    },
    prelude::*,
    *,
};
use joysheet_engine::{
    core::{CellRef, RangeRef, col_to_letters},
    model::Color,
};
use serde_json::{Value, json};
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
pub(crate) struct ExcelUi {
    pub fonts: Vec<String>,
    pub colors: HashMap<String, Color>,
    pub last_colors: HashMap<String, Value>,
    pub border_ops: Vec<Value>,
    pub sort: Vec<Value>,
    pub sort_changed: bool,
    pub sort_selected: usize,
    pub split_column: usize,
    pub formats: HashMap<usize, String>,
    pub import_preview: Value,
    pub return_panel: u8,
    pub manager_selected: usize,
    pub cf_draft: Option<Vec<joysheet_engine::model::CondFormat>>,
    pub cf_edit: Option<usize>,
    pub manager_confirm: bool,
    pub cf_target: Option<(u64, usize, u64)>,
    pub print_params: Value,
    pub popup_open: bool,
    pub cf_all: bool,
    pub color_source: String,
}
pub(crate) fn bound_probe(owner: WeakEntity<Desktop>, id: String) -> AnyElement {
    canvas(
        move |b, _, cx| {
            let _ = owner.update(cx, |v, _| {
                v.advanced_bounds.insert(id, b);
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
    .into_any_element()
}
type Choose = Rc<dyn Fn(String, &mut Window, &mut App)>;
pub(crate) fn choice(id: String, value: String, items: Vec<(String, String)>, choose: Choose, owner: WeakEntity<Desktop>) -> AnyElement {
    let label = items.iter().find(|(v, _)| v == &value).map(|(_, l)| l.clone()).unwrap_or(value);
    let watcher = owner.clone();
    Popover::new(SharedString::from(format!("choose-{id}")))
        .on_open_change(move |open, _, cx| {
            let _ = watcher.update(cx, |v, _| v.excel_ui.popup_open = *open);
        })
        .trigger(
            Button::new(SharedString::from(id.clone()))
                .relative()
                .child(bound_probe(owner.clone(), format!("choice:{id}")))
                .label(format!("{label} ▾")),
        )
        .content(move |_, _, cx| {
            let popup = cx.entity().downgrade();
            let choose = choose.clone();
            let owner = owner.clone();
            let choice_id = id.clone();
            div().id(SharedString::from(format!("choices-{id}"))).max_h(px(240.)).overflow_y_scroll().p_1().flex().flex_col().children(
                items.iter().enumerate().map(move |(i, (value, label))| {
                    let popup = popup.clone();
                    let choose = choose.clone();
                    let value = value.clone();
                    Button::new(("choice", i))
                        .relative()
                        .child(bound_probe(owner.clone(), format!("choice:{choice_id}:{value}")))
                        .ghost()
                        .justify_start()
                        .label(label.clone())
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                            choose(value.clone(), w, cx);
                        })
                }),
            )
        })
        .into_any_element()
}
impl Desktop {
    pub(crate) fn dialog_text(&self, key: &str, cx: &App) -> String {
        self.command_fields.iter().find(|f| f.key == key).map(|f| f.value.read(cx).value().to_string()).unwrap_or_default()
    }
    pub(crate) fn set_dialog_value(&mut self, key: &str, value: Value, w: &mut Window, cx: &mut Context<Self>) {
        if let Some(f) = self.command_fields.iter().find(|f| f.key == key) {
            let text = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string());
            if f.value.read(cx).value().as_ref() != text {
                f.value.update(cx, |s, cx| s.set_value(text, w, cx));
            }
        }
        cx.notify();
    }
    pub(crate) fn prepare_excel_dialog(&mut self, id: &str, p: &mut Value, cx: &App) {
        self.excel_ui.colors.clear();
        self.excel_ui.border_ops.clear();
        self.excel_ui.formats.clear();
        self.excel_ui.split_column = 0;
        self.excel_ui.import_preview = Value::Null;
        self.excel_ui.color_source = "color".into();
        if matches!(id, "home.fontName" | "home.formatCells") && self.excel_ui.fonts.is_empty() {
            self.excel_ui.fonts = cx.text_system().all_font_names();
            self.excel_ui.fonts.sort();
            self.excel_ui.fonts.dedup();
        }
        if id == "home.formatCells" {
            p["borderStyle"] = json!("Thin");
            p["borderColor"] = json!("#000000");
            p["borderInnerH"] = json!("None");
            p["borderInnerV"] = json!("None");
        }
        if matches!(id, "home.fillColor" | "home.fontColor" | "home.borderColor") {
            let color = self
                .view()
                .and_then(|v| {
                    let s = v.sheet()?;
                    let st = v.workbook.styles.get(s.style_id(self.selection().active));
                    match id {
                        "home.fillColor" => st.fill.fg.hex(&v.workbook.theme),
                        "home.fontColor" => st.font.color.hex(&v.workbook.theme),
                        _ => st.borders.bottom.color.hex(&v.workbook.theme),
                    }
                })
                .unwrap_or("#FFFFFF".into());
            p["color"] = json!(color);
            let rgb = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0xffffff);
            for (key, n) in [("red", (rgb >> 16) & 255), ("green", (rgb >> 8) & 255), ("blue", rgb & 255)] {
                p[key] = json!(n);
            }
        }
        if id == "data.sort" {
            let col = self.selection().current().start.col;
            p["column"] = json!(col_to_letters(col));
            p["matchCase"] = json!(false);
            p["orientation"] = json!("rows");
            self.excel_ui.sort = vec![json!({"column":col_to_letters(col),"order":"asc","by":"values"})];
            self.excel_ui.sort_changed = false;
            self.excel_ui.sort_selected = 0;
        }
        if id == "data.textToColumns" {
            p["mode"] = json!("delimited");
            p["tab"] = json!(true);
            p["comma"] = json!(false);
            p["semicolon"] = json!(false);
            p["space"] = json!(false);
            p["other"] = json!("");
            p["fixedBreaks"] = json!("");
            p["destination"] = json!(self.selection().current().start.a1());
            p["columnFormat"] = json!("general");
        }
        if id == "home.conditionalFormat" {
            crate::excel_managers::cf_defaults(p);
        }
    }
    pub(crate) fn set_dialog_color(&mut self, key: &str, color: Color, w: &mut Window, cx: &mut Context<Self>) {
        let theme = self.view().map(|v| v.workbook.theme.clone()).unwrap_or_default();
        let hex = color.hex(&theme).unwrap_or("#000000".into());
        self.excel_ui.colors.insert(key.into(), color);
        self.set_dialog_value(key, json!(hex), w, cx);
        if key == "color" {
            let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
            for (k, v) in [("red", (n >> 16) & 255), ("green", (n >> 8) & 255), ("blue", n & 255)] {
                self.set_dialog_value(k, json!(v), w, cx);
            }
        }
    }
    pub(crate) fn proposed_rgb(&self, cx: &App) -> Option<u32> {
        let rgb = ["red", "green", "blue"].map(|k| self.dialog_text(k, cx).parse::<u8>().ok());
        let changed = self.excel_ui.color_source != "color";
        if changed {
            Some(u32::from(rgb[0]?) << 16 | u32::from(rgb[1]?) << 8 | u32::from(rgb[2]?))
        } else {
            u32::from_str_radix(self.dialog_text("color", cx).trim_start_matches('#'), 16).ok()
        }
    }
    pub(crate) fn border_preset(&mut self, preset: &str, w: &mut Window, cx: &mut Context<Self>) {
        let style = self.dialog_text("borderStyle", cx);
        let color = self.dialog_text("borderColor", cx);
        self.excel_ui.border_ops.push(json!({"preset":preset,"style":style.to_ascii_lowercase(),"color":color}));
        for key in if preset == "inside" {
            vec!["borderInnerH", "borderInnerV"]
        } else if preset == "none" {
            vec!["borderLeft", "borderRight", "borderTop", "borderBottom", "borderDown", "borderUp", "borderInnerH", "borderInnerV"]
        } else {
            vec!["borderLeft", "borderRight", "borderTop", "borderBottom"]
        } {
            self.set_dialog_value(key, json!(if preset == "none" { "None" } else { &style }), w, cx);
            self.set_dialog_value(&format!("{key}Color"), json!(color), w, cx);
        }
    }
    pub(crate) fn toggle_border(&mut self, key: &str, w: &mut Window, cx: &mut Context<Self>) {
        let style = self.dialog_text("borderStyle", cx);
        let next = if self.dialog_text(key, cx) == style { "None" } else { &style };
        let color = self.dialog_text("borderColor", cx);
        let preset = match key {
            "borderLeft" => "left",
            "borderRight" => "right",
            "borderTop" => "top",
            "borderBottom" => "bottom",
            "borderDown" => "diagonalDown",
            "borderInnerH" => "insideHorizontal",
            "borderInnerV" => "insideVertical",
            _ => "diagonalUp",
        };
        self.excel_ui.border_ops.push(json!({"preset":preset,"style":next.to_ascii_lowercase(),"color":color}));
        self.set_dialog_value(key, json!(next), w, cx);
        self.set_dialog_value(&format!("{key}Color"), json!(color), w, cx);
    }
    pub(crate) fn excel_parameters(&self, id: &str, p: &mut Value, cx: &App) -> Result<(), String> {
        if matches!(id, "home.fillColor" | "home.fontColor" | "home.borderColor") {
            p["color"] = json!(format!("#{:06X}", self.proposed_rgb(cx).ok_or("RGB values must be between 0 and 255")?));
            for key in ["red", "green", "blue"] {
                p.as_object_mut().ok_or("Parameters must be an object")?.remove(key);
            }
        }
        if id == "home.formatCells" {
            p.as_object_mut().ok_or("Parameters must be an object")?.remove("borderStyle");
            p.as_object_mut().ok_or("Parameters must be an object")?.remove("borderColor");
            p["_colors"] = serde_json::to_value(&self.excel_ui.colors).map_err(|e| e.to_string())?;
        }
        if id == "home.conditionalFormat" {
            p["_cfInitial"] = self.command_initial.clone();
        }
        if id == "data.sort"
            && (self.excel_ui.sort_changed
                || p["keysText"].as_str().unwrap_or("").is_empty()
                    && p["column"] == self.command_initial["column"]
                    && p["order"] == self.command_initial["order"])
        {
            p["keys"] = json!(self.excel_ui.sort);
            if p["orientation"] == "columns" {
                for key in p["keys"].as_array_mut().into_iter().flatten() {
                    let n = key["column"].as_str().and_then(|s| s.parse::<u32>().ok()).unwrap_or(self.selection().current().start.row + 1);
                    key["row"] = json!(n);
                    key.as_object_mut().map(|k| k.remove("column"));
                }
            }
            for key in ["column", "order", "keysText"] {
                p.as_object_mut().ok_or("Parameters must be an object")?.remove(key);
            }
        }
        if id == "data.textToColumns" {
            let fixed = p["mode"] == "fixed";
            if let Some(r) = p["range"].as_str().and_then(RangeRef::parse)
                && r.width() != 1
            {
                return Err("一次只能对一列数据进行分列 / Select a single column of data".into());
            }
            let mut delimiters = Vec::new();
            for (k, v) in [("tab", "\t"), ("comma", ","), ("semicolon", ";"), ("space", " ")] {
                if p[k] == true {
                    delimiters.push(v);
                }
            }
            p["delimiters"] = json!(delimiters);
            if fixed {
                let breaks = p["fixedBreaks"]
                    .as_str()
                    .unwrap_or("")
                    .split([',', ' '])
                    .filter(|s| !s.is_empty())
                    .map(str::parse::<usize>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| "Use numeric break positions".to_string())?;
                p["fixedWidths"] = json!(breaks);
            } else {
                p.as_object_mut().ok_or("Parameters must be an object")?.remove("fixedWidths");
            }
            let mut formats = self.excel_ui.formats.clone();
            formats.insert(self.excel_ui.split_column, p["columnFormat"].as_str().unwrap_or("general").into());
            p["columnFormats"] = json!(formats);
            for key in ["mode", "tab", "comma", "semicolon", "space", "fixedBreaks", "columnFormat"] {
                p.as_object_mut().ok_or("Parameters must be an object")?.remove(key);
            }
        }
        Ok(())
    }
    pub(crate) fn field_choice(&self, key: &str, options: &[(&str, &str)], cx: &mut Context<Self>) -> AnyElement {
        let key = key.to_string();
        let owner = cx.weak_entity();
        let cbkey = key.clone();
        let pick: Choose = Rc::new(move |value, w, cx| {
            let _ = owner.update(cx, |v, cx| v.set_dialog_value(&cbkey, json!(value), w, cx));
        });
        choice(key.clone(), self.dialog_text(&key, cx), options.iter().map(|(v, l)| (v.to_string(), l.to_string())).collect(), pick, cx.weak_entity())
    }
    pub(crate) fn fields_body(&self, id: &str, keys: &[&str], cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .children(keys.iter().filter_map(|key| self.command_fields.iter().find(|f| f.key == *key)).map(|f| self.dialog_field(id, f, cx)))
            .into_any_element()
    }
    pub(crate) fn excel_dialog_body(&self, id: &str, page: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        Some(match (id, page) {
            ("home.fontName", _) | ("home.formatCells", 2) => self.font_dialog_body(id, cx),
            ("home.formatCells", 3) => self.border_dialog_body(cx),
            ("home.formatCells", 4) => div()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.color_picker_field("fillForeground", cx))
                .child(self.color_picker_field("fillBackground", cx))
                .child(self.fields_body(id, &["fillPattern"], cx))
                .into_any_element(),
            ("home.fillColor" | "home.fontColor" | "home.borderColor", _) => self.more_colors_body(cx),
            ("data.sort", _) => self.sort_body(cx),
            ("data.textToColumns", _) => self.split_body(cx),
            ("data.getData" | "data.fromTextCsv", _) => div()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.fields_body(id, &["path", "text", "delimiter", "sheet"], cx))
                .child(self.import_preview_body())
                .into_any_element(),
            ("home.formatAsTable", _) => {
                div().p_4().flex().flex_col().gap_3().child(self.fields_body(id, &["range", "header"], cx)).into_any_element()
            }
            ("home.conditionalFormat", _) => self.cf_rule_body(cx),
            _ => return self.visual_gallery_body(id, cx),
        })
    }
    fn sort_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div().p_4().flex().flex_col().gap_3();
        let sel = self.excel_ui.sort_selected;
        body = body.child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("sort-add")
                        .relative()
                        .child(self.advanced_probe("sort-add".into(), cx))
                        .label(self.label("添加条件", "Add Level"))
                        .disabled(self.excel_ui.sort.len() >= 64)
                        .on_click(cx.listener(|v, _, _, cx| {
                            v.excel_ui.sort.push(json!({"column":"A","order":"asc","by":"values"}));
                            v.excel_ui.sort_selected = v.excel_ui.sort.len() - 1;
                            v.excel_ui.sort_changed = true;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("sort-delete")
                        .relative()
                        .child(self.advanced_probe("sort-delete".into(), cx))
                        .label(self.label("删除条件", "Delete Level"))
                        .disabled(self.excel_ui.sort.len() < 2)
                        .on_click(cx.listener(move |v, _, _, cx| {
                            v.excel_ui.sort.remove(sel);
                            v.excel_ui.sort_selected = sel.saturating_sub(1);
                            v.excel_ui.sort_changed = true;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("sort-copy")
                        .relative()
                        .child(self.advanced_probe("sort-copy".into(), cx))
                        .label(self.label("复制条件", "Copy Level"))
                        .disabled(self.excel_ui.sort.len() >= 64)
                        .on_click(cx.listener(move |v, _, _, cx| {
                            let level = v.excel_ui.sort[sel].clone();
                            v.excel_ui.sort.insert(sel + 1, level);
                            v.excel_ui.sort_selected = sel + 1;
                            v.excel_ui.sort_changed = true;
                            cx.notify();
                        })),
                )
                .children([("sort-up", "↑", -1isize), ("sort-down", "↓", 1)].map(|(id, label, delta)| {
                    Button::new(id)
                        .relative()
                        .child(self.advanced_probe(id.into(), cx))
                        .label(label)
                        .disabled((sel as isize + delta) < 0 || (sel as isize + delta) >= self.excel_ui.sort.len() as isize)
                        .on_click(cx.listener(move |v, _, _, cx| {
                            let n = (sel as isize + delta) as usize;
                            v.excel_ui.sort.swap(sel, n);
                            v.excel_ui.sort_selected = n;
                            v.excel_ui.sort_changed = true;
                            cx.notify();
                        }))
                })),
        );
        body = body.child(self.fields_body("data.sort", &["range", "header", "matchCase"], cx));
        body = body.child(self.field_choice(
            "orientation",
            &[("rows", "按列排序 / Sort top to bottom"), ("columns", "按行排序 / Sort left to right")],
            cx,
        ));
        let range = RangeRef::parse(&self.dialog_text("range", cx)).unwrap_or(self.selection().current());
        let horizontal = self.dialog_text("orientation", cx) == "columns";
        for (i, key) in self.excel_ui.sort.iter().enumerate() {
            let mut row = div().relative().flex().gap_2().p_1().bg(rgb(if i == sel { 0xe4edf8 } else { self.palette().surface }));
            row = row.child(
                Button::new(("sort-select", i))
                    .ghost()
                    .label(if i == 0 { self.label("主要关键字", "Sort by") } else { self.label("次要关键字", "Then by") })
                    .on_click(cx.listener(move |v, _, _, cx| {
                        v.excel_ui.sort_selected = i;
                        cx.notify();
                    })),
            );
            for field in ["column", "by", "order"] {
                let value = key[field].as_str().unwrap_or("values").to_string();
                let items = match field {
                    "column" if horizontal => (range.start.row..=range.end.row.min(range.start.row + 1023))
                        .map(|r| ((r + 1).to_string(), format!("行 {} / Row {}", r + 1, r + 1)))
                        .collect(),
                    "column" => (range.start.col..=range.end.col.min(range.start.col + 1023))
                        .map(|c| {
                            let col = col_to_letters(c);
                            let header =
                                self.view().and_then(|v| v.sheet()).map(|s| s.value(CellRef::new(range.start.row, c)).display()).unwrap_or_default();
                            (
                                col.clone(),
                                if self.dialog_text("header", cx) == "true" && !header.is_empty() {
                                    header
                                } else {
                                    format!("列 {col} / Column {col}")
                                },
                            )
                        })
                        .collect(),
                    "by" => vec![
                        ("values".into(), self.label("单元格值", "Cell Values")),
                        ("cellColor".into(), self.label("单元格颜色", "Cell Color")),
                        ("fontColor".into(), self.label("字体颜色", "Font Color")),
                    ],
                    _ => vec![("asc".into(), self.label("升序", "A to Z")), ("desc".into(), self.label("降序", "Z to A"))],
                };
                let owner = cx.weak_entity();
                let f = field.to_string();
                row = row.child(choice(
                    format!("sort-{i}-{field}"),
                    value,
                    items,
                    Rc::new(move |value, _, cx| {
                        let _ = owner.update(cx, |v, cx| {
                            v.excel_ui.sort[i][&f] = json!(value);
                            v.excel_ui.sort_selected = i;
                            v.excel_ui.sort_changed = true;
                            cx.notify();
                        });
                    }),
                    cx.weak_entity(),
                ));
            }
            body = body.child(row);
        }
        body.into_any_element()
    }
    fn split_preview(&self, cx: &App) -> Vec<Vec<String>> {
        let Some(sh) = self.view().and_then(|v| v.sheet()) else { return vec![] };
        let range = RangeRef::parse(&self.dialog_text("range", cx)).unwrap_or(self.selection().current());
        let mut p = json!({"mode":self.dialog_text("mode",cx),"fixedBreaks":self.dialog_text("fixedBreaks",cx),"other":self.dialog_text("other",cx),"textQualifier":self.dialog_text("textQualifier",cx),"treatConsecutive":self.dialog_text("treatConsecutive",cx)=="true","columnFormat":self.dialog_text("columnFormat",cx)});
        for k in ["tab", "comma", "semicolon", "space"] {
            p[k] = json!(self.dialog_text(k, cx) == "true");
        }
        if self.excel_parameters("data.textToColumns", &mut p, cx).is_err() {
            return vec![];
        }
        (range.start.row..=range.end.row.min(range.start.row + 19))
            .map(|r| joysheet_engine::text_columns::split(&sh.value(CellRef::new(r, range.start.col)).display(), &p))
            .collect()
    }
    fn split_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.dialog_tab;
        let mut body = div().p_4().flex().flex_col().gap_3().child(format!(
            "{} {} / 3",
            self.label("文本分列向导 — 第", "Convert Text to Columns — Step"),
            page + 1
        ));
        if page == 0 {
            body = body.child(self.label("请选择最合适的文件类型：", "Choose the file type that best describes your data:"));
            body = body
                .child(self.field_choice("mode", &[("delimited", "分隔符号 / Delimited"), ("fixed", "固定宽度 / Fixed width")], cx))
                .child(self.fields_body("data.textToColumns", &["range"], cx));
        }
        if page == 1 {
            if self.dialog_text("mode", cx) == "fixed" {
                body = body.child(
                    self.label("单击标尺设置或删除分列线（字符位置）。", "Click the ruler to add or remove column breaks (character positions)."),
                );
                body = body.child(div().flex().children((1..=30).map(|i| {
                    Button::new(("split-break", i))
                        .relative()
                        .child(self.advanced_probe(format!("split-break:{i}"), cx))
                        .ghost()
                        .xsmall()
                        .w(px(18.))
                        .label(i.to_string())
                        .on_click(cx.listener(move |v, _, w, cx| {
                            let mut b =
                                v.dialog_text("fixedBreaks", cx).split(',').filter_map(|s| s.trim().parse::<usize>().ok()).collect::<Vec<_>>();
                            if b.contains(&i) {
                                b.retain(|n| *n != i);
                            } else {
                                b.push(i);
                                b.sort();
                            }
                            v.set_dialog_value("fixedBreaks", json!(b.iter().map(usize::to_string).collect::<Vec<_>>().join(",")), w, cx);
                        }))
                })));
                body = body.child(self.fields_body("data.textToColumns", &["fixedBreaks"], cx));
            } else {
                body = body.child(self.fields_body(
                    "data.textToColumns",
                    &["tab", "semicolon", "comma", "space", "other", "treatConsecutive", "textQualifier"],
                    cx,
                ));
            }
        }
        if page == 2 {
            body = body.child(self.label("单击预览中的列，设置该列的数据格式。", "Select a preview column to set its data format."));
            body = body
                .child(self.field_choice(
                    "columnFormat",
                    &[
                        ("general", "常规 / General"),
                        ("text", "文本 / Text"),
                        ("skip", "不导入此列 / Do not import column"),
                        ("dateMDY", "日期 MDY"),
                        ("dateDMY", "日期 DMY"),
                        ("dateYMD", "日期 YMD"),
                        ("dateMYD", "日期 MYD"),
                        ("dateDYM", "日期 DYM"),
                        ("dateYDM", "日期 YDM"),
                    ],
                    cx,
                ))
                .child(self.fields_body("data.textToColumns", &["destination"], cx));
        }
        let rows = self.split_preview(cx);
        let columns = rows.iter().map(Vec::len).max().unwrap_or(1).min(20);
        let mut table = div().id("split-preview").overflow_x_scroll().flex().flex_col().border_1().border_color(rgb(self.palette().border));
        if page == 2 {
            table = table.child(div().flex().children((0..columns).map(|c| {
                let label = self.excel_ui.formats.get(&c).cloned().unwrap_or("general".into());
                Button::new(("split-column", c))
                    .relative()
                    .child(self.advanced_probe(format!("split-column:{c}"), cx))
                    .ghost()
                    .w(px(100.))
                    .label(format!("{} · {label}", c + 1))
                    .on_click(cx.listener(move |v, _, w, cx| {
                        let old = v.dialog_text("columnFormat", cx);
                        v.excel_ui.formats.insert(v.excel_ui.split_column, old);
                        v.excel_ui.split_column = c;
                        let f = v.excel_ui.formats.get(&c).cloned().unwrap_or("general".into());
                        v.set_dialog_value("columnFormat", json!(f), w, cx);
                    }))
            })));
        }
        for row in rows {
            table = table.child(div().flex().children(row.into_iter().take(20).enumerate().map(|(i, text)| {
                div()
                    .w(px(100.))
                    .flex_shrink_0()
                    .p_1()
                    .border_r_1()
                    .border_b_1()
                    .border_color(rgb(self.palette().border))
                    .bg(rgb(if page == 2 && i == self.excel_ui.split_column { 0xe4edf8 } else { self.palette().surface }))
                    .child(text)
            })));
        }
        body.child(table).into_any_element()
    }
    fn import_preview_body(&self) -> AnyElement {
        let p = &self.excel_ui.import_preview;
        let mut body = div().id("import-preview").flex().flex_col().gap_1();
        if p.is_null() {
            return body
                .child(self.label("选择文件或粘贴数据，然后单击“预览”。", "Choose a file or paste data, then click Preview."))
                .into_any_element();
        }
        body = body.child(format!("{} × {}", p["rows"], p["columns"]));
        for row in p["cells"].as_array().into_iter().flatten() {
            body = body.child(div().flex().children(row.as_array().into_iter().flatten().map(|text| {
                div()
                    .w(px(100.))
                    .overflow_hidden()
                    .border_1()
                    .border_color(rgb(self.palette().border))
                    .p_1()
                    .child(text.as_str().unwrap_or("").to_string())
            })));
        }
        body.into_any_element()
    }
}
