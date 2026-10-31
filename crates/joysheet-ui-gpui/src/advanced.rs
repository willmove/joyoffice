//! Advanced workflows keep human input separate from the stable engine protocol.
use serde_json::{Value, json};
pub fn defaults(id: &str) -> Option<Value> {
    Some(match id {
        "file.open" => json!({"path":"","password":""}),
        "automate.newScript" => {
            json!({"name":"新脚本","commandsText":"[\n  {\"command\":\"cell.set\",\"params\":{\"cell\":\"G2\",\"input\":\"脚本结果\"}}\n]"})
        }
        "automate.recordActions" => json!({"stop":true,"name":"录制动作"}),
        "automate.runScript" | "automate.deleteScript" => json!({"name":"录制动作"}),
        "automate.trustScripts" => json!({"trusted":false}),
        "review.newComment" | "review.newNote" | "review.replyComment" => json!({"cell":"A1","text":"","author":"Joysheet"}),
        "review.resolveComment" => json!({"cell":"A1","resolved":true}),
        "review.deleteComment" | "review.showNote" => json!({"cell":"A1"}),
        "review.protectSheet" => json!({"password":"","formatCells":false,"formatColumns":false,"formatRows":false,"sort":false,"autofilter":false}),
        "review.unprotectSheet" => json!({"password":""}),
        "review.protectWorkbook" => json!({"on":true,"password":""}),
        "review.allowEditRanges" => json!({"action":"list","title":"可编辑区域","range":"B2:B4","password":""}),
        "formulas.defineName" => json!({"name":"Sales","refersTo":"=Sheet1!$B$2:$B$4","scope":"Workbook","comment":"","oldName":"","oldScope":""}),
        "formulas.deleteName" => json!({"name":"Sales","scope":"Workbook"}),
        "insert.pivotTable" => {
            json!({"source":"","destination":"new","name":"透视分析","rowFields":"地区","columnFields":"","valueFields":"金额","aggregate":"sum"})
        }
        "pivot.addField" => json!({"field":"","area":"rows","func":"sum"}),
        "pivot.removeField" => json!({"field":"","area":"rows"}),
        "pivot.moveField" => json!({"field":"","from":"rows","to":"columns","position":0}),
        "pivot.valueSettings" => json!({"field":"","func":"sum","showAs":"normal","numberFormat":"#,##0.00"}),
        "pivot.layout" => json!({"layout":"tabular","grandTotalsRows":true,"grandTotalsCols":true,"subtotals":false}),
        "pivot.filter" => json!({"field":"","selectedItems":""}),
        "pivot.sort" => json!({"field":"","order":"asc"}),
        "pivot.calculatedField" => json!({"name":"计算字段","formula":"=金额*0.1","values":true}),
        "pivot.changeSource" => json!({"source":""}),
        "data.analysisToolPak" => analysis_defaults("descriptive"),
        "data.goalSeek" => json!({"set":"D2","to":100,"changing":"B2"}),
        "data.solver" => {
            json!({"objective":"D2","goal":"max","value":100,"variables":"B2","constraintLines":"B2 <= 10","method":"simplex","nonNegative":true,"keep":true,"saveModel":true,"maxEvaluations":20000})
        }
        "data.scenarioAdd" => json!({"name":"方案一","changing":"B2","comment":""}),
        "data.scenarioShow" | "data.scenarioDelete" => json!({"name":"方案一"}),
        "data.scenarioSummary" => json!({"resultCells":"D2"}),
        _ => return None,
    })
}
pub fn analysis_defaults(tool: &str) -> Value {
    let mut p = json!({"tool":tool,"labels":false,"groupedBy":"columns","output":"J2","outputTo":"newSheet","newSheet":"分析结果","overwrite":false});
    let fields = match tool {
        "regression" => json!({"inputY":"B2:B10","inputX":"C2:C10","confidence":95,"residuals":true,"constantZero":false}),
        "tTestPaired" | "tTestEqual" | "tTestUnequal" | "fTest" => {
            json!({"range1":"B2:B10","range2":"C2:C10","alpha":0.05,"hypothesizedDifference":0})
        }
        "movingAverage" => json!({"range":"B2:B10","interval":3,"standardErrors":false}),
        "exponentialSmoothing" => json!({"range":"B2:B10","damping":0.3,"standardErrors":false}),
        "histogram" => json!({"range":"B2:B10","pareto":false,"cumulative":false}),
        _ => json!({"range":"B2:B10"}),
    };
    if let (Some(p), Some(f)) = (p.as_object_mut(), fields.as_object()) {
        p.extend(f.clone());
    }
    p
}
pub fn parameters(id: &str, p: &mut Value) -> Result<(), String> {
    let Some(m) = p.as_object_mut() else { return Err("Parameters must be an object".into()) };
    match id {
        "automate.newScript" => {
            if let Some(v) = m.remove("commandsText") {
                let commands: Value = serde_json::from_str(v.as_str().unwrap_or("")).map_err(|e| format!("脚本 JSON 无效: {e}"))?;
                let steps = commands.as_array().ok_or("脚本必须为命令数组")?;
                if steps.len() > 100000 {
                    return Err("脚本最多 100000 步".into());
                }
                for (i, st) in steps.iter().enumerate() {
                    if st.get("command").and_then(Value::as_str).and_then(joysheet_engine::find_command).is_none()
                        || !st.get("params").is_some_and(Value::is_object)
                    {
                        return Err(format!("第 {} 步必须包含已注册 command 和对象 params", i + 1));
                    }
                }
                m.insert("commands".into(), commands);
            }
        }
        "formulas.defineName" => {
            for key in ["oldName", "oldScope"] {
                if m.get(key).and_then(Value::as_str) == Some("") {
                    m.remove(key);
                }
            }
        }
        "insert.pivotTable" => {
            for (key, area) in [("rowFields", "rows"), ("columnFields", "columns"), ("valueFields", "values")] {
                if let Some(v) = m.remove(key) {
                    let fields = split(v.as_str().unwrap_or(""));
                    let list: Vec<_> = fields
                        .into_iter()
                        .map(|f| {
                            if area == "values" {
                                json!({"field":f,"func":m.get("aggregate").and_then(Value::as_str).unwrap_or("sum")})
                            } else {
                                json!(f)
                            }
                        })
                        .collect();
                    m.insert(area.into(), json!(list));
                }
            }
            m.remove("aggregate");
            if m.get("source").and_then(Value::as_str) == Some("") {
                m.remove("source");
            }
        }
        "pivot.filter" => {
            if let Some(v) = m.remove("selectedItems") {
                let items = split(v.as_str().unwrap_or(""));
                m.insert("selected".into(), if items.is_empty() { Value::Null } else { json!(items) });
            }
        }
        "data.solver" => {
            if let Some(v) = m.remove("constraintLines") {
                let lines: Vec<_> = v.as_str().unwrap_or("").lines().map(str::trim).filter(|s| !s.is_empty()).collect();
                m.insert("constraints".into(), json!(lines));
            }
        }
        _ => {}
    }
    Ok(())
}
fn split(s: &str) -> Vec<String> {
    s.split([',', '，']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}
pub fn multiline(id: &str, key: &str) -> bool {
    key == "constraintLines" || key == "commandsText" || (key == "text" && id.starts_with("review."))
}
pub fn choices(key: &str) -> Vec<(&'static str, &'static str)> {
    match key {
        "tool" => joysheet_engine::cmd::analysis::TOOLS.to_vec(),
        "aggregate" | "func" => vec![("sum", "求和"), ("count", "计数"), ("average", "平均"), ("min", "最小"), ("max", "最大"), ("stdDev", "标准差")],
        "method" => vec![("simplex", "Simplex LP"), ("grg", "非线性"), ("evolutionary", "进化算法")],
        "goal" => vec![("max", "最大值"), ("min", "最小值"), ("value", "目标值")],
        "area" | "from" | "to" => vec![("rows", "行"), ("columns", "列"), ("values", "值"), ("filters", "筛选")],
        "layout" => vec![("compact", "紧凑"), ("outline", "大纲"), ("tabular", "表格")],
        _ => Vec::new(),
    }
}
pub fn is_result(id: &str) -> bool {
    id.starts_with("automate.")
        || id == "__joysheet.scriptDetails"
        || id.starts_with("review.")
        || id == "formulas.nameManager"
        || id.starts_with("pivot.")
        || id.starts_with("data.scenario")
        || matches!(id, "insert.pivotTable" | "data.goalSeek" | "data.analysisToolPak" | "data.solver" | "data.solverReport")
}

use crate::desktop::{After, Desktop};
use gpui_kit::{prelude::*, *};
impl Desktop {
    pub fn control_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let runtime = cx.global::<crate::control::ControlRuntime>();
        let port = runtime._server.as_ref().map(|s| s.port);
        let exe = std::env::current_exe().ok().map(|p| p.with_file_name(if cfg!(windows) { "joysheet-cli.exe" } else { "joysheet-cli" }));
        let config = json!({"mcpServers":{"joysheet":{"command":exe,"args":["mcp","--connect",port.unwrap_or(7979).to_string()],"env":{"JOYSHEET_CONFIG_DIR":joysheet_platform::config_dir()}}}});
        let text = serde_json::to_string_pretty(&config).unwrap_or_default();
        let mut panel = div()
            .absolute()
            .right_0()
            .top(px(90.))
            .bottom_0()
            .w(px(430.))
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x20332a))
            .shadow_lg()
            .id("control-panel")
            .occlude()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().id("close-control").cursor_pointer().child("桌面控制与 MCP ×").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 0;
                cx.notify();
            })))
            .child(match port {
                Some(p) => format!("已启用：127.0.0.1:{p} · 令牌认证"),
                None => "未启用。保存后使用 joysheet --control 7979 启动。".into(),
            })
            .child("命令共用引擎队列。app.save / app.open 会先提交指定窗口草稿；ui.awaitIdle 与截图可等待修订。低层命令和 force 关闭保持原引擎语义。")
            .child(text.clone());
        if port.is_some() {
            panel = panel.child(div().id("copy-mcp-config").p_2().cursor_pointer().bg(rgb(0xe2eee5)).child("复制 MCP 配置").on_click(cx.listener(
                move |this, _, _, cx| {
                    if let Err(e) = this.clipboard.write(&text, None) {
                        this.alert(e);
                    } else {
                        this.message = "MCP 配置已复制".into();
                    }
                    cx.notify();
                },
            )));
        }
        for v in &self.snapshot.windows {
            let id = v.window;
            panel = panel.child(
                div()
                    .id(SharedString::from(format!("control-window-{id}")))
                    .p_2()
                    .cursor_pointer()
                    .child(format!("窗口 {id} · 文档 {} · {} · r{}", v.doc, v.title, v.revision))
                    .on_click(cx.listener(move |this, _, w, cx| this.commit(After::Command("window.activate".into(), json!({"window":id})), w, cx))),
            );
        }
        panel
    }
    pub fn open_prefilled(&mut self, id: &str, values: Value, window: &mut Window, cx: &mut Context<Self>) {
        self.open_command(id, window, cx);
        let mut p = defaults(id).unwrap_or_else(|| json!({}));
        if let (Some(base), Some(extra)) = (p.as_object_mut(), values.as_object()) {
            base.extend(extra.clone());
        }
        self.make_command_fields(&p, window, cx);
        self.params.update(cx, |s, cx| s.set_value(p.to_string(), window, cx));
        if let Some((_, v)) = p.as_object().into_iter().flatten().find(|(key, _)| multiline(id, key)) {
            let text = v.as_str().unwrap_or("").to_string();
            self.import_text.update(cx, |s, cx| s.set_value(text, window, cx));
        }
    }
    pub fn comments_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut list = div().flex().flex_col().gap_2();
        if let Some(sheet) = self.view().and_then(|v| v.sheet()) {
            for (index, (cell, c)) in sheet.comments.iter().take(200).enumerate() {
                let cell = cell.a1();
                let locate = cell.clone();
                let reply = cell.clone();
                let resolved = c.resolved;
                let resolve = cell.clone();
                let mut row =
                    div()
                        .p_2()
                        .border_1()
                        .border_color(rgb(0xdde5df))
                        .rounded_md()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .id(("comment-cell", index))
                                .cursor_pointer()
                                .child(format!("{cell} · {}{}", c.author, if c.resolved { " · 已解决" } else { "" }))
                                .on_click(cx.listener(move |this, _, w, cx| {
                                    this.commit(After::Command("edit.goTo".into(), json!({"reference":locate})), w, cx)
                                })),
                        )
                        .child(c.text.clone());
                for (author, text) in c.replies.iter().take(100) {
                    row = row.child(format!("↳ {author}: {text}"));
                }
                row =
                    row.child(
                        div()
                            .flex()
                            .gap_2()
                            .child(div().id(("reply", index)).cursor_pointer().child("回复").on_click(
                                cx.listener(move |this, _, w, cx| this.open_prefilled("review.replyComment", json!({"cell":reply}), w, cx)),
                            ))
                            .child(div().id(("resolve", index)).cursor_pointer().child(if resolved { "重新打开" } else { "解决" }).on_click(
                                cx.listener(move |this, _, w, cx| {
                                    this.commit(After::Command("review.resolveComment".into(), json!({"cell":resolve,"resolved":!resolved})), w, cx)
                                }),
                            ))
                            .child(div().id(("delete-comment", index)).cursor_pointer().child("删除").on_click(cx.listener(
                                move |this, _, w, cx| this.commit(After::Command("review.deleteComment".into(), json!({"cell":cell})), w, cx),
                            ))),
                    );
                list = list.child(row);
            }
        }
        list
    }
    pub fn names_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut panel = div()
            .absolute()
            .right_0()
            .top(px(90.))
            .bottom_0()
            .w(px(380.))
            .p_3()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x20332a))
            .shadow_lg()
            .id("names-panel")
            .occlude()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().id("close-names").cursor_pointer().child("名称管理 / Names ×").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 0;
                cx.notify();
            })));
        if let Some(view) = self.view() {
            for (index, n) in view.workbook.names.iter().filter(|n| !n.hidden).take(300).enumerate() {
                let scope = n.scope.and_then(|i| view.workbook.sheets.get(i)).map(|s| s.name.clone()).unwrap_or("Workbook".into());
                let name = n.name.clone();
                let values =
                    json!({"name":name,"oldName":name,"oldScope":scope,"scope":scope,"refersTo":format!("={}",n.formula),"comment":n.comment});
                let deletion = json!({"name":name,"scope":scope});
                panel =
                    panel.child(
                        div()
                            .p_2()
                            .border_b_1()
                            .border_color(rgb(0xdde5df))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(format!("{} · {scope}", n.name))
                            .child(format!("={}", n.formula))
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(div().id(("goto-name", index)).cursor_pointer().child("定位").on_click(cx.listener(
                                        move |this, _, w, cx| this.commit(After::Command("edit.goTo".into(), json!({"reference":name})), w, cx),
                                    )))
                                    .child(div().id(("edit-name", index)).cursor_pointer().child("编辑").on_click(
                                        cx.listener(move |this, _, w, cx| this.open_prefilled("formulas.defineName", values.clone(), w, cx)),
                                    ))
                                    .child(div().id(("delete-name", index)).cursor_pointer().child("删除").on_click(cx.listener(
                                        move |this, _, w, cx| this.commit(After::Command("formulas.deleteName".into(), deletion.clone()), w, cx),
                                    ))),
                            ),
                    );
            }
        }
        panel
    }
    pub fn advanced_probe(&self, id: String, cx: &Context<Self>) -> impl IntoElement + use<> {
        let view = cx.entity();
        canvas(
            move |b, _, cx| {
                view.update(cx, |this, _| {
                    this.advanced_bounds.insert(id, b);
                })
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
    pub fn advanced_choice(&mut self, key: &str, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if key == "tool" {
            let values = analysis_defaults(value);
            self.make_command_fields(&values, window, cx);
            self.params.update(cx, |s, cx| s.set_value(values.to_string(), window, cx));
        } else {
            for f in &self.command_fields {
                if f.key == key {
                    f.value.update(cx, |s, cx| s.set_value(value.to_string(), window, cx));
                }
            }
        }
        cx.notify();
    }
    pub fn pivot_fields_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut panel = div()
            .absolute()
            .right_0()
            .top(px(90.))
            .bottom_0()
            .w(px(360.))
            .bg(rgb(0xffffff))
            .text_color(rgb(0x20332a))
            .shadow_lg()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .id("pivot-fields")
            .occlude()
            .overflow_y_scroll()
            .child(div().id("close-pivot-fields").cursor_pointer().child("透视字段 / Pivot fields  ×").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 0;
                cx.notify();
            })));
        if let Some(fields) = self.data_raw.get("fields").and_then(Value::as_array) {
            let pivot = self.data_raw.get("id").cloned().unwrap_or(Value::Null);
            for (index, field) in fields.iter().enumerate() {
                let name = field["name"].as_str().unwrap_or("").to_string();
                let drag_name = name.clone();
                let drag_pivot = pivot.clone();
                let doc = self.view().map(|v| v.doc).unwrap_or(0);
                let mut row = div().relative().p_2().border_b_1().border_color(rgb(0xdde6df)).flex().flex_col().gap_1().child(
                    div()
                        .id(SharedString::from(format!("drag-field-{index}")))
                        .relative()
                        .cursor_pointer()
                        .child(name.clone())
                        .child(self.advanced_probe(format!("field:{name}"), cx))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.pivot_drag = Some((drag_name.clone(), doc, drag_pivot.clone()));
                                cx.notify();
                            }),
                        ),
                );
                let mut targets = div().flex().gap_1();
                for (area, label) in [("rows", "行"), ("columns", "列"), ("values", "值"), ("filters", "筛选")] {
                    let name = name.clone();
                    let pivot = pivot.clone();
                    let present = field["areas"].as_array().is_some_and(|a| a.iter().any(|v| v == area));
                    targets = targets.child(
                        div()
                            .id(SharedString::from(format!("pivot-{index}-{area}")))
                            .p_1()
                            .rounded_md()
                            .bg(rgb(if present { 0xdcefe1 } else { 0xf0f3f1 }))
                            .cursor_pointer()
                            .child(format!("{label} {}", if present { "−" } else { "+" }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.commit(
                                    After::Command(
                                        if present { "pivot.removeField" } else { "pivot.addField" }.into(),
                                        json!({"pivot":pivot,"field":name,"area":area}),
                                    ),
                                    window,
                                    cx,
                                );
                            })),
                    );
                }
                row = row.child(targets);
                panel = panel.child(row);
            }
        }
        for (area, label) in [("rows", "拖到这里：行"), ("columns", "拖到这里：列"), ("values", "拖到这里：值"), ("filters", "拖到这里：筛选")]
        {
            panel = panel.child(
                div()
                    .id(SharedString::from(format!("drop-{area}")))
                    .relative()
                    .p_3()
                    .bg(rgb(0xe8f2eb))
                    .child(label)
                    .child(self.advanced_probe(format!("area:{area}"), cx))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            if let Some((field, doc, pivot)) = this.pivot_drag.take()
                                && this.view().is_some_and(|v| v.doc == doc)
                            {
                                this.commit(After::Command("pivot.addField".into(), json!({"pivot":pivot,"field":field,"area":area})), window, cx);
                            }
                            cx.notify();
                        }),
                    ),
            );
        }
        panel.child(div().text_sm().child("可拖动字段到区域，或点击区域按钮添加/移除。布局与汇总方式在分析菜单设置。"))
    }
}
#[cfg(test)]
#[path = "tests/advanced_tests.rs"]
mod tests;
