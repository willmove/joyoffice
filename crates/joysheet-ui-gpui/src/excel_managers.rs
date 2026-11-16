//! Name and conditional-format managers, with cancellable rule drafts.
use crate::desktop::{After, Desktop, Pending};
use gpui_base::FocusTrapElement as _;
use gpui_kit::{
    component::{
        Disableable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
    },
    prelude::*,
    *,
};
use joysheet_engine::{
    core::RangeRef,
    model::{CfRule, CondFormat},
};
use serde_json::{Value, json};

const RULE_TYPES: &[(&str, &str)] = &[
    ("cellIs", "只为包含以下内容的单元格设置格式"),
    ("expression", "使用公式确定要设置格式的单元格"),
    ("colorScale", "基于各自值设置所有单元格的格式 — 色阶"),
    ("dataBar", "数据条"),
    ("iconSet", "图标集"),
    ("top10", "排名靠前或靠后的值"),
    ("aboveAverage", "高于或低于平均值"),
    ("duplicate", "重复值"),
    ("unique", "唯一值"),
    ("containsText", "包含文本"),
    ("notContainsText", "不包含文本"),
    ("beginsWith", "开头是"),
    ("endsWith", "结尾是"),
    ("blanks", "空值"),
    ("noBlanks", "非空值"),
    ("errors", "错误值"),
    ("noErrors", "非错误值"),
    ("timePeriod", "发生日期"),
];
pub(crate) fn cf_defaults(p: &mut Value) {
    for (k, v) in [
        ("value", json!("15")),
        ("value2", json!("")),
        ("formula", json!("=A1>0")),
        ("text", json!("")),
        ("rank", json!(10)),
        ("bottom", json!(false)),
        ("percent", json!(false)),
        ("below", json!(false)),
        ("period", json!("today")),
        ("gradient", json!(true)),
        ("reverse", json!(false)),
        ("scaleStops", json!(2)),
        ("colorLow", json!("#F8696B")),
        ("colorMiddle", json!("#FFEB84")),
        ("colorHigh", json!("#63BE7B")),
    ] {
        if p.get(k).is_none() {
            p[k] = v;
        }
    }
    // Formula and literal comparisons share Excel's edit box.
    if p["value"].is_number() {
        p["value"] = json!(p["value"].to_string());
    }
}
fn editor_values(cf: &CondFormat, theme: &joysheet_engine::model::Theme) -> Value {
    let mut p = json!({"range":cf.ranges.iter().map(|r|r.a1()).collect::<Vec<_>>().join(","),"preset":"greenFill"});
    let model = serde_json::to_value(&cf.rule).unwrap_or_default();
    let record = model.as_object().and_then(|m| m.values().next()).cloned().unwrap_or_default();
    let typ = match &cf.rule {
        CfRule::CellIs { op, a, b, .. } => {
            p["operator"] = json!(match op {
                joysheet_engine::model::CfOperator::Between => "between",
                joysheet_engine::model::CfOperator::NotBetween => "notBetween",
                joysheet_engine::model::CfOperator::Equal => "equal",
                joysheet_engine::model::CfOperator::NotEqual => "notEqual",
                joysheet_engine::model::CfOperator::Greater => "greaterThan",
                joysheet_engine::model::CfOperator::Less => "lessThan",
                joysheet_engine::model::CfOperator::GreaterOrEqual => "greaterOrEqual",
                joysheet_engine::model::CfOperator::LessOrEqual => "lessOrEqual",
            });
            p["value"] = json!(format!("={a}"));
            p["value2"] = json!(b.as_ref().map(|s| format!("={s}")).unwrap_or_default());
            "cellIs"
        }
        CfRule::Expression { formula, .. } => {
            p["formula"] = json!(format!("={formula}"));
            "expression"
        }
        CfRule::ContainsText { .. } => "containsText",
        CfRule::NotContainsText { .. } => "notContainsText",
        CfRule::BeginsWith { .. } => "beginsWith",
        CfRule::EndsWith { .. } => "endsWith",
        CfRule::Blanks { .. } => "blanks",
        CfRule::NoBlanks { .. } => "noBlanks",
        CfRule::Errors { .. } => "errors",
        CfRule::NoErrors { .. } => "noErrors",
        CfRule::Duplicate { .. } => "duplicate",
        CfRule::Unique { .. } => "unique",
        CfRule::Top10 { .. } => "top10",
        CfRule::AboveAverage { .. } => "aboveAverage",
        CfRule::TimePeriod { .. } => "timePeriod",
        CfRule::ColorScale { stops } => {
            p["scaleStops"] = json!(stops.len());
            for (key, stop) in [("colorLow", stops.first()), ("colorMiddle", stops.get(1)), ("colorHigh", stops.last())] {
                if let Some((_, color)) = stop {
                    p[key] = json!(color.hex(theme).unwrap_or("#000000".into()));
                }
            }
            "colorScale"
        }
        CfRule::DataBar { color, .. } => {
            p["color"] = json!(color.hex(theme).unwrap_or("#638EC6".into()));
            "dataBar"
        }
        CfRule::IconSet { .. } => "iconSet",
    };
    p["type"] = json!(typ);
    for k in ["text", "rank", "bottom", "percent", "below", "period", "gradient", "reverse", "set"] {
        if let Some(v) = record.get(k) {
            p[k] = v.clone();
        }
    }
    if let Some(style) = record.get("style") {
        p["_cfStyle"] = style.clone();
    }
    // Keep all model fields that have no desktop edit control. Editing a rule must
    // never silently reset imported thresholds, icon settings or average options.
    p["_modelRule"] = model;
    p
}
impl Desktop {
    fn manager_shell(&self, id: &'static str, title: &str, body: AnyElement, footer: AnyElement, cx: &Context<Self>) -> AnyElement {
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
                    .id(id)
                    .track_focus(&self.command_focus)
                    .role(Role::Dialog)
                    .aria_label(title.to_string())
                    .w(px(840.))
                    .max_h(px(660.))
                    .bg(rgb(self.palette().surface))
                    .border_1()
                    .border_color(rgb(self.palette().border))
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .child(div().p_3().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
                    .child(div().id("manager-scroll").overflow_y_scroll().p_3().child(body))
                    .child(footer)
                    .child(self.advanced_probe(format!("manager:{id}"), cx))
                    .focus_trap(id, &self.command_focus),
            )
            .into_any_element()
    }
    pub(crate) fn name_manager(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div().flex().flex_col().gap_2();
        body = body.child(
            Button::new("name-new").relative().child(self.advanced_probe("name-new".into(), cx)).label(self.label("新建…", "New…")).on_click(
                cx.listener(|v, _, w, cx| {
                    v.open_command("formulas.defineName", w, cx);
                    v.excel_ui.return_panel = 19;
                }),
            ),
        );
        let mut table = div().border_1().border_color(rgb(self.palette().border)).flex().flex_col().child(
            div().flex().bg(rgb(0xeeeeee)).children(
                [("名称 / Name", 130.), ("值 / Value", 140.), ("引用位置 / Refers To", 240.), ("作用域 / Scope", 110.), ("注释 / Comment", 130.)]
                    .map(|(s, w)| div().w(px(w)).p_1().child(s)),
            ),
        );
        if let Some(view) = self.view() {
            for (i, n) in view.workbook.names.iter().filter(|n| !n.hidden).take(300).enumerate() {
                let scope = n.scope.and_then(|i| view.workbook.sheets.get(i)).map(|s| s.name.clone()).unwrap_or("Workbook".into());
                let selected = self.excel_ui.manager_selected == i;
                let value =
                    joysheet_engine::calc::evaluate(&view.workbook, n.scope.unwrap_or(view.sheet), self.selection().active, &n.formula).display();
                table = table.child(
                    div()
                        .id(("name-row", i))
                        .relative()
                        .flex()
                        .bg(rgb(if selected { 0xdce9f9 } else { self.palette().surface }))
                        .cursor_pointer()
                        .child(self.advanced_probe(format!("name-row:{i}"), cx))
                        .on_click(cx.listener(move |v, _, _, cx| {
                            v.excel_ui.manager_selected = i;
                            v.excel_ui.manager_confirm = false;
                            cx.notify();
                        }))
                        .children(
                            [(n.name.clone(), 130.), (value, 140.), (format!("={}", n.formula), 240.), (scope, 110.), (n.comment.clone(), 130.)]
                                .map(|(s, w)| div().w(px(w)).overflow_hidden().p_1().child(s)),
                        ),
                );
            }
        }
        body = body.child(table);
        if let Some((n, scope)) = self.view().and_then(|v| {
            v.workbook
                .names
                .iter()
                .filter(|n| !n.hidden)
                .nth(self.excel_ui.manager_selected)
                .map(|n| (n.clone(), n.scope.and_then(|s| v.workbook.sheets.get(s)).map(|s| s.name.clone()).unwrap_or("Workbook".into())))
        }) {
            let edit = json!({"name":n.name,"oldName":n.name,"scope":scope,"oldScope":scope,"refersTo":format!("={}",n.formula),"comment":n.comment});
            let deletion = json!({"name":n.name,"scope":scope});
            body = body.child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("name-edit")
                            .relative()
                            .child(self.advanced_probe("name-edit".into(), cx))
                            .label(self.label("编辑…", "Edit…"))
                            .on_click(cx.listener(move |v, _, w, cx| {
                                v.open_prefilled("formulas.defineName", edit.clone(), w, cx);
                                v.excel_ui.return_panel = 19;
                            })),
                    )
                    .child(
                        Button::new("name-delete")
                            .relative()
                            .child(self.advanced_probe("name-delete".into(), cx))
                            .label(self.label("删除", "Delete"))
                            .on_click(cx.listener(|v, _, _, cx| {
                                v.excel_ui.manager_confirm = true;
                                cx.notify();
                            })),
                    ),
            );
            if self.excel_ui.manager_confirm {
                body = body.child(
                    div()
                        .p_3()
                        .border_1()
                        .border_color(rgb(self.palette().border))
                        .child(self.label("确实要删除选定的名称吗？", "Are you sure you want to delete the selected name?"))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new("name-delete-confirm")
                                        .relative()
                                        .child(self.advanced_probe("name-delete-confirm".into(), cx))
                                        .label(self.label("确定", "OK"))
                                        .on_click(cx.listener(move |v, _, w, cx| {
                                            v.excel_ui.manager_confirm = false;
                                            v.commit(After::Command("formulas.deleteName".into(), deletion.clone()), w, cx);
                                        })),
                                )
                                .child(
                                    Button::new("name-delete-cancel")
                                        .relative()
                                        .child(self.advanced_probe("name-delete-cancel".into(), cx))
                                        .label(self.label("取消", "Cancel"))
                                        .on_click(cx.listener(|v, _, _, cx| {
                                            v.excel_ui.manager_confirm = false;
                                            cx.notify();
                                        })),
                                ),
                        ),
                );
            }
        }
        let footer = div()
            .p_3()
            .flex()
            .justify_end()
            .child(
                Button::new("manager-close")
                    .relative()
                    .child(self.advanced_probe("manager-close".into(), cx))
                    .label(self.label("关闭", "Close"))
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.daily_group = 0;
                        v.focus_grid(w, cx);
                        cx.notify();
                    })),
            )
            .into_any_element();
        self.manager_shell("names-panel", &self.label("名称管理器", "Name Manager"), body.into_any_element(), footer, cx)
    }
    pub(crate) fn begin_cf_manager(&mut self) {
        if let Some((target, rules)) = self.view().map(|v| ((v.doc, v.sheet, v.revision), v.sheet().map(|s| s.cond_formats.clone()))) {
            self.excel_ui.cf_target = Some(target);
            self.excel_ui.cf_draft = rules;
        }
        self.excel_ui.manager_selected = 0;
        self.excel_ui.cf_all = false;
        self.daily_group = 23;
    }
    fn edit_cf(&mut self, new: bool, w: &mut Window, cx: &mut Context<Self>) {
        let index = self.excel_ui.manager_selected;
        let theme = self.view().map(|v| v.workbook.theme.clone()).unwrap_or_default();
        let rule = self.excel_ui.cf_draft.as_ref().and_then(|r| r.get(index)).map(|cf| editor_values(cf, &theme));
        if new {
            self.open_command("home.conditionalFormat", w, cx);
        } else if let Some(p) = rule {
            self.open_prefilled("home.conditionalFormat", p, w, cx);
        } else {
            return;
        }
        self.excel_ui.cf_edit = if new { None } else { Some(index) };
        self.excel_ui.return_panel = 23;
    }
    pub(crate) fn stage_cf_rule(&mut self, p: &Value) -> Result<(), String> {
        let rule = joysheet_engine::cmd::insert::parse_cf_rule(&p["rule"]).map_err(|e| e.to_string())?;
        let ranges = p["range"]
            .as_str()
            .unwrap_or("")
            .split(',')
            .map(|s| RangeRef::parse(s.trim()).ok_or("Invalid applies-to range".to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let draft = self.excel_ui.cf_draft.as_mut().ok_or("Rules manager closed")?;
        if let Some(i) = self.excel_ui.cf_edit {
            if let Some(cf) = draft.get_mut(i) {
                cf.rule = rule;
                cf.ranges = ranges;
            }
        } else {
            draft.insert(0, CondFormat { rule, ranges, priority: 1, stop_if_true: false });
        }
        Ok(())
    }
    fn apply_cf(&mut self, close: bool, w: &mut Window, cx: &mut Context<Self>) {
        if self.view().map(|v| (v.doc, v.sheet, v.revision)) != self.excel_ui.cf_target {
            self.alert("规则编辑期间文档已更改，请重新打开管理器 / Reopen the manager: document changed");
            cx.notify();
            return;
        }
        let rules = self.excel_ui.cf_draft.clone().unwrap_or_default();
        if self.view().and_then(|v| v.sheet()).is_some_and(|s| s.cond_formats == rules) {
            if close {
                self.daily_group = 0;
                self.excel_ui.cf_draft = None;
                self.focus_grid(w, cx);
            }
            cx.notify();
            return;
        }
        let revision = self.excel_ui.cf_target.map(|t| t.2);
        match self.engine.submit_guarded("home.manageRules", json!({"replaceAll":rules}), self.target(), revision) {
            Ok(ticket) => self.pending.push(Pending { ticket, after: After::RulesApplied, commit: false, clipboard: false }),
            Err(e) => self.alert(e),
        }
        // Keep the draft visible while the queued edit completes. The next snapshot
        // updates the revision guard after Apply.
        self.excel_ui.cf_target = None;
        if close {
            self.daily_group = 0;
            self.excel_ui.cf_draft = None;
            self.focus_grid(w, cx);
        }
        cx.notify();
    }
    pub(crate) fn cf_manager(&self, cx: &mut Context<Self>) -> AnyElement {
        let rules = self.excel_ui.cf_draft.clone().unwrap_or_default();
        let selected = self.excel_ui.manager_selected.min(rules.len().saturating_sub(1));
        let owner = cx.weak_entity();
        let scope = crate::excel_workflows::choice(
            "cf-scope".into(),
            if self.excel_ui.cf_all { "sheet" } else { "selection" }.into(),
            vec![("selection".into(), self.label("当前选择", "Current Selection")), ("sheet".into(), self.label("此工作表", "This Worksheet"))],
            std::rc::Rc::new(move |value, _, cx| {
                let _ = owner.update(cx, |v, cx| {
                    v.excel_ui.cf_all = value == "sheet";
                    cx.notify();
                });
            }),
            cx.weak_entity(),
        );
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().flex().items_center().gap_2().child(self.label("显示其格式规则：", "Show formatting rules for:")).child(scope));
        body = body.child(
            div()
                .flex()
                .gap_2()
                .child(
                    Button::new("cf-new")
                        .relative()
                        .child(self.advanced_probe("cf-new".into(), cx))
                        .label(self.label("新建规则…", "New Rule…"))
                        .on_click(cx.listener(|v, _, w, cx| v.edit_cf(true, w, cx))),
                )
                .child(
                    Button::new("cf-edit")
                        .relative()
                        .child(self.advanced_probe("cf-edit".into(), cx))
                        .disabled(rules.is_empty())
                        .label(self.label("编辑规则…", "Edit Rule…"))
                        .on_click(cx.listener(|v, _, w, cx| v.edit_cf(false, w, cx))),
                )
                .child(
                    Button::new("cf-delete")
                        .relative()
                        .child(self.advanced_probe("cf-delete".into(), cx))
                        .disabled(rules.is_empty())
                        .label(self.label("删除规则", "Delete Rule"))
                        .on_click(cx.listener(move |v, _, _, cx| {
                            if let Some(r) = v.excel_ui.cf_draft.as_mut() {
                                r.remove(selected);
                            }
                            v.excel_ui.manager_selected = selected.saturating_sub(1);
                            cx.notify();
                        })),
                )
                .children([("cf-up", "↑", -1isize), ("cf-down", "↓", 1)].map(|(id, label, delta)| {
                    Button::new(id)
                        .relative()
                        .child(self.advanced_probe(id.into(), cx))
                        .label(label)
                        .disabled(rules.is_empty() || (selected as isize + delta) < 0 || (selected as isize + delta) >= rules.len() as isize)
                        .on_click(cx.listener(move |v, _, _, cx| {
                            let n = (selected as isize + delta) as usize;
                            if let Some(r) = v.excel_ui.cf_draft.as_mut() {
                                r.swap(selected, n);
                            }
                            v.excel_ui.manager_selected = n;
                            cx.notify();
                        }))
                })),
        );
        body = body.child(
            div()
                .flex()
                .bg(rgb(0xeeeeee))
                .child(div().w(px(350.)).p_2().child(self.label("规则（按所示顺序应用）", "Rule (applied in order shown)")))
                .child(div().w(px(230.)).p_2().child(self.label("应用于", "Applies to")))
                .child(self.label("如果为真则停止", "Stop If True")),
        );
        for (i, cf) in rules.iter().enumerate() {
            if !self.excel_ui.cf_all && !cf.ranges.iter().any(|r| r.intersects(&self.selection().current())) {
                continue;
            }
            let model = serde_json::to_value(&cf.rule).unwrap_or_default();
            let text = model.as_object().and_then(|m| m.keys().next()).cloned().unwrap_or_default();
            let ranges = cf.ranges.iter().map(|r| r.a1()).collect::<Vec<_>>().join(", ");
            body = body.child(
                div()
                    .flex()
                    .bg(rgb(if selected == i { 0xdce9f9 } else { self.palette().surface }))
                    .child(Button::new(("cf-row", i)).ghost().w(px(350.)).justify_start().label(text).on_click(cx.listener(move |v, _, _, cx| {
                        v.excel_ui.manager_selected = i;
                        cx.notify();
                    })))
                    .child(div().w(px(230.)).p_2().child(ranges))
                    .child(Checkbox::new(("cf-stop", i)).checked(cf.stop_if_true).on_click(cx.listener(move |v, checked, _, cx| {
                        if let Some(cf) = v.excel_ui.cf_draft.as_mut().and_then(|r| r.get_mut(i)) {
                            cf.stop_if_true = *checked;
                        }
                        cx.notify();
                    }))),
            );
        }
        let footer = div()
            .p_3()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("cf-ok")
                    .relative()
                    .child(self.advanced_probe("cf-ok".into(), cx))
                    .primary()
                    .label(self.label("确定", "OK"))
                    .on_click(cx.listener(|v, _, w, cx| v.apply_cf(true, w, cx))),
            )
            .child(
                Button::new("cf-cancel").relative().child(self.advanced_probe("cf-cancel".into(), cx)).label(self.label("取消", "Cancel")).on_click(
                    cx.listener(|v, _, w, cx| {
                        v.excel_ui.cf_draft = None;
                        v.daily_group = 0;
                        v.focus_grid(w, cx);
                        cx.notify();
                    }),
                ),
            )
            .child(
                Button::new("cf-apply")
                    .relative()
                    .child(self.advanced_probe("cf-apply".into(), cx))
                    .label(self.label("应用", "Apply"))
                    .on_click(cx.listener(|v, _, w, cx| v.apply_cf(false, w, cx))),
            )
            .into_any_element();
        self.manager_shell(
            "cf-manager",
            &self.label("条件格式规则管理器", "Conditional Formatting Rules Manager"),
            body.into_any_element(),
            footer,
            cx,
        )
    }
    pub(crate) fn cf_rule_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let typ = self.dialog_text("type", cx);
        let category = match typ.as_str() {
            "colorScale" | "dataBar" | "iconSet" => "colorScale",
            "top10" => "top10",
            "aboveAverage" => "aboveAverage",
            "duplicate" | "unique" => "duplicate",
            "expression" => "expression",
            _ => "cellIs",
        };
        let mut list = div().border_1().border_color(rgb(self.palette().border)).flex().flex_col();
        for (value, zh, en) in [
            ("colorScale", "基于各自值设置所有单元格的格式", "Format all cells based on their values"),
            ("cellIs", "只为包含以下内容的单元格设置格式", "Format only cells that contain"),
            ("top10", "仅对排名靠前或靠后的数值设置格式", "Format only top or bottom ranked values"),
            ("aboveAverage", "仅对高于或低于平均值的数值设置格式", "Format only values above or below average"),
            ("duplicate", "仅对唯一值或重复值设置格式", "Format only unique or duplicate values"),
            ("expression", "使用公式确定要设置格式的单元格", "Use a formula to determine which cells to format"),
        ] {
            list = list.child(
                Button::new(value)
                    .relative()
                    .child(self.advanced_probe(format!("rule-category:{value}"), cx))
                    .ghost()
                    .justify_start()
                    .bg(rgb(if value == category { 0xdce9f9 } else { self.palette().surface }))
                    .label(self.label(zh, en))
                    .on_click(cx.listener(move |v, _, w, cx| v.set_dialog_value("type", json!(value), w, cx))),
            );
        }
        let mut body = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.label("选择规则类型：", "Select a Rule Type:"))
            .child(list)
            .child(self.label("编辑规则说明：", "Edit the Rule Description:"));
        if matches!(category, "colorScale" | "cellIs" | "duplicate") {
            let subtypes = RULE_TYPES
                .iter()
                .copied()
                .filter(|(v, _)| match category {
                    "colorScale" => matches!(*v, "colorScale" | "dataBar" | "iconSet"),
                    "duplicate" => matches!(*v, "duplicate" | "unique"),
                    _ => matches!(
                        *v,
                        "cellIs"
                            | "containsText"
                            | "notContainsText"
                            | "beginsWith"
                            | "endsWith"
                            | "blanks"
                            | "noBlanks"
                            | "errors"
                            | "noErrors"
                            | "timePeriod"
                    ),
                })
                .collect::<Vec<_>>();
            body = body.child(self.field_choice("type", &subtypes, cx));
        }
        let mut keys = vec!["range"];
        match typ.as_str() {
            "cellIs" => {
                keys.extend(["operator", "value"]);
                if matches!(self.dialog_text("operator", cx).as_str(), "between" | "notBetween") {
                    keys.push("value2");
                }
            }
            "expression" => keys.push("formula"),
            "containsText" | "notContainsText" | "beginsWith" | "endsWith" => keys.push("text"),
            "top10" => keys.extend(["rank", "bottom", "percent"]),
            "aboveAverage" => keys.push("below"),
            "timePeriod" => keys.push("period"),
            "dataBar" => keys.extend(["color", "gradient"]),
            "iconSet" => keys.extend(["reverse"]),
            _ => {}
        }
        body = body.child(self.fields_body("home.conditionalFormat", &keys, cx));
        if typ == "colorScale" {
            body = body.child(self.field_choice("scaleStops", &[("2", "双色刻度 / 2-Color Scale"), ("3", "三色刻度 / 3-Color Scale")], cx));
            body = body.child(self.color_picker_field("colorLow", cx));
            if self.dialog_text("scaleStops", cx) == "3" {
                body = body.child(self.color_picker_field("colorMiddle", cx));
            }
            body = body.child(self.color_picker_field("colorHigh", cx));
            let a = u32::from_str_radix(self.dialog_text("colorLow", cx).trim_start_matches('#'), 16).unwrap_or(0);
            let b = u32::from_str_radix(self.dialog_text("colorHigh", cx).trim_start_matches('#'), 16).unwrap_or(0xffffff);
            let mid = u32::from_str_radix(self.dialog_text("colorMiddle", cx).trim_start_matches('#'), 16).unwrap_or(0xffff00);
            let three = self.dialog_text("scaleStops", cx) == "3";
            body = body.child(div().h(px(30.)).flex().children((0..64).map(|i| {
                let (a, b, t) =
                    if three { if i < 32 { (a, mid, i as f32 / 31.) } else { (mid, b, (i - 32) as f32 / 31.) } } else { (a, b, i as f32 / 63.) };
                let channel = |shift: u32| (((a >> shift) & 255u32) as f32 * (1. - t) + ((b >> shift) & 255u32) as f32 * t) as u32;
                div().flex_1().h_full().bg(rgb(channel(16) << 16 | channel(8) << 8 | channel(0)))
            })));
        }
        if typ == "iconSet" {
            body = body.child(self.field_choice(
                "set",
                &[
                    ("3Arrows", "3 个箭头 / 3 Arrows"),
                    ("3TrafficLights1", "3 个交通灯 / 3 Traffic Lights"),
                    ("4Rating", "4 个等级 / 4 Ratings"),
                    ("5Quarters", "5 个象限 / 5 Quarters"),
                ],
                cx,
            ));
        }
        if !matches!(typ.as_str(), "colorScale" | "dataBar" | "iconSet") {
            body = body.child(self.field_choice(
                "preset",
                &[
                    ("greenFill", "绿填充色深绿色文本"),
                    ("yellowFill", "黄填充色深黄色文本"),
                    ("lightRedFillDarkRedText", "浅红填充色深红色文本"),
                    ("redText", "红色文本"),
                    ("redBorder", "红色边框"),
                ],
                cx,
            ));
        }
        body.into_any_element()
    }
}
