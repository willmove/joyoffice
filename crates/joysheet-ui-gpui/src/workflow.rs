//! Human-facing data forms translated to the unchanged engine protocol.
use serde_json::{Value, json};
pub fn defaults(id: &str) -> Option<Value> {
    if let Some(p) = crate::advanced::defaults(id) {
        return Some(p);
    }
    match id {
        "view.arrangeAll" => return Some(json!({"arrange":"tiled","activeWorkbook":false})),
        "home.orientation" => return Some(json!({"angle":45})),
        "home.formatCells" => {
            return Some(
                json!({"fontName":"Calibri","fontSize":11,"bold":false,"italic":false,"underline":"None","fontScript":"Baseline","fontColor":"#27382E","fillPattern":"None","fillForeground":"#147D59","fillBackground":"#FFFFFF","horizontal":"General","vertical":"Bottom","angle":0,"indent":0,"wrap":false,"shrink":false,"numberFormat":"General"}),
            );
        }
        "pageLayout.pageSetup" => {
            return Some(
                json!({"orientation":"portrait","paper":"A4","printArea":"","fitWidth":1,"fitHeight":0,"gridlines":false,"headings":false,"centerH":false,"titleRows":"","titleCols":"","header":"&L&A&R&P / &N","footer":"","left":0.3,"right":0.3,"top":0.5,"bottom":0.5}),
            );
        }
        "file.printPreview" | "file.print" => return Some(json!({"sheets":"active","fitToPage":false})),
        "file.exportPdf" => return Some(json!({"path":"Joysheet.pdf","sheets":"active","fitToPage":false})),
        "object.move" => return Some(json!({"at":"D2","dx":0,"dy":0,"width":180,"height":80})),
        "object.setAnchorMode" => return Some(json!({"mode":"moveOnly"})),
        "shape.setText" => return Some(json!({"text":"文本框"})),
        "chart.shapeFill" | "chart.shapeOutline" => return Some(json!({"color":"#147D59"})),
        "insert.picture" => return Some(json!({"path":"","at":"K2","width":160,"height":100})),
        "insert.shape" => {
            return Some(json!({"kind":"rectangle","at":"D17","width":180,"height":80,"text":"形状","fill":"#DDEFE4","line":"#147D59"}));
        }
        "insert.textBox" => return Some(json!({"at":"H17","width":180,"height":80,"text":"文本框"})),
        "insert.sparkline" => return Some(json!({"range":"B2:B4","location":"C2","type":"line"})),
        "chart.changeType" => return Some(json!({"type":"line"})),
        "chart.set" => return Some(json!({"title":"图表标题"})),
        _ => {}
    }
    if id == "insert.chart" {
        return Some(json!({"range":"A1:B4","type":"column","title":"图表","at":"D2","width":400,"height":260}));
    }
    Some(match id {
        "data.sort" => json!({"column":"B","order":"asc","keysText":"","header":true}),
        "data.filterBy" => json!({"column":"B","operator":">","value":"10","clear":false}),
        "data.removeDuplicates" => json!({"columns":"A,B","header":true}),
        "edit.find" => json!({"what":"","all":false,"matchCase":false,"wholeCell":false,"lookIn":"values"}),
        "edit.replace" => json!({"what":"","with":"","all":true,"matchCase":false}),
        "range.fill" => json!({"input":""}),
        "home.conditionalFormat" => {
            json!({"type":"cellIs","operator":"greaterThan","value":15,"preset":"greenFill","color":"#638EC6","set":"3Arrows"})
        }
        "home.manageRules" => json!({}),
        "data.group" | "data.ungroup" | "data.hideDetail" | "data.showDetail" => json!({"by":"rows","lines":"2:4"}),
        "data.showLevels" => json!({"rows":1,"cols":1}),
        "data.getData" | "data.fromTextCsv" => json!({"path":"","text":"","delimiter":"","sheet":"导入数据"}),
        "data.validation" => {
            json!({"type":"whole","operator":"between","formula1":"0","formula2":"100","ignoreBlank":true,"dropdown":true,"errorMessage":"输入值不符合规则 / Value does not satisfy validation"})
        }
        _ => return None,
    })
}
pub fn parameters(id: &str, mut p: Value) -> Result<Value, String> {
    crate::advanced::parameters(id, &mut p)?;
    let m = p.as_object_mut().ok_or("Parameters must be an object")?;
    match id {
        "home.formatCells" if m.contains_key("fontName") => {
            let value = |k: &str| m.get(k).cloned().unwrap_or(Value::Null);
            let color = |k: &str| -> Result<Value, String> {
                let s = m.get(k).and_then(Value::as_str).ok_or("Color must be #RRGGBB")?;
                let c = joysheet_engine::model::Color::from_hex(s).ok_or("Color must be #RRGGBB")?;
                serde_json::to_value(c).map_err(|e| e.to_string())
            };
            let style = json!({"font":{"name":value("fontName"),"size":value("fontSize"),"bold":value("bold"),"italic":value("italic"),"underline":value("underline"),"vert":value("fontScript"),"color":color("fontColor")?},"fill":{"pattern":value("fillPattern"),"fg":color("fillForeground")?,"bg":color("fillBackground")?},"align":{"h":value("horizontal"),"v":value("vertical"),"rotation":value("angle"),"indent":value("indent"),"wrap":value("wrap"),"shrink":value("shrink")},"num_fmt":value("numberFormat")});
            let range = m.get("range").cloned();
            m.clear();
            m.insert("style".into(), style);
            if let Some(range) = range {
                m.insert("range".into(), range);
            }
        }
        "data.getData" | "data.fromTextCsv" => {
            if m.get("delimiter").and_then(Value::as_str) == Some("") {
                m.remove("delimiter");
            }
            for key in ["path", "text"] {
                if m.get(key).and_then(Value::as_str).is_some_and(|s| s.trim().is_empty()) {
                    m.remove(key);
                }
            }
            if m.contains_key("path") == m.contains_key("text") {
                return Err("请选择文件，或粘贴数据，二者只能选择一个 / Choose a file or paste text".into());
            }
        }
        "data.group" | "data.ungroup" | "data.hideDetail" | "data.showDetail" if m.contains_key("lines") => {
            let lines = m.remove("lines").unwrap_or(json!("2:4"));
            let direction = m.remove("by").unwrap_or(json!("rows"));
            let key = match direction.as_str() {
                Some("rows") => "rows",
                Some("cols") => "cols",
                _ => return Err("分组方向应为 rows 或 cols".into()),
            };
            m.insert(key.into(), lines);
        }
        "home.conditionalFormat" if !m.contains_key("rule") => {
            let mut rule = serde_json::Map::new();
            for key in ["type", "operator", "value", "preset", "color", "set"] {
                if let Some(v) = m.remove(key) {
                    rule.insert(key.into(), v);
                }
            }
            m.insert("rule".into(), Value::Object(rule));
        }
        "data.sort" if !m.contains_key("keys") => {
            let column = m.remove("column").unwrap_or(json!("B"));
            let order = m.remove("order").unwrap_or(json!("asc"));
            if !["asc", "desc"].contains(&order.as_str().unwrap_or("")) {
                return Err("Order must be asc or desc".into());
            }
            let mut keys = vec![json!({"column":column,"order":order})];
            if let Some(text) = m.remove("keysText").and_then(|v| v.as_str().map(str::to_string)) {
                for part in text.split([',', ';', '\n']).map(str::trim).filter(|s| !s.is_empty()) {
                    let (column, order) = part.split_once(':').ok_or("后续排序键格式为 B:desc,C:asc / Use B:desc,C:asc")?;
                    let order = order.trim().to_ascii_lowercase();
                    if !["asc", "desc"].contains(&order.as_str()) || column.trim().is_empty() {
                        return Err("Sort order must be asc or desc".into());
                    }
                    keys.push(json!({"column":column.trim(),"order":order}));
                    if keys.len() > 16 {
                        return Err("At most 16 sort keys".into());
                    }
                }
            }
            m.insert("keys".into(), json!(keys));
        }
        "data.filterBy" if m.contains_key("operator") => {
            let op = m.remove("operator").unwrap_or(json!("="));
            let value = m.remove("value").unwrap_or(json!(""));
            m.insert("custom".into(), json!({"op":op,"value":value}));
        }
        "data.removeDuplicates" => {
            if let Some(text) = m.get("columns").and_then(Value::as_str) {
                let columns: Vec<_> = text.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
                m.insert("columns".into(), json!(columns));
            }
        }
        _ => {}
    }
    Ok(p)
}
pub fn result_rows(_kind: &str, value: &Value) -> Vec<(String, Option<String>)> {
    if let Some(array) = value
        .as_array()
        .or_else(|| value.get("results").and_then(Value::as_array))
        .or_else(|| value.get("invalid").and_then(Value::as_array))
        .or_else(|| value.get("rules").and_then(Value::as_array))
    {
        return array
            .iter()
            .take(200)
            .map(|row| {
                if let Some(ranges) = row.get("ranges").and_then(Value::as_array) {
                    let range = ranges.first().and_then(Value::as_str).map(str::to_string);
                    let kind = row.get("rule").and_then(Value::as_object).and_then(|o| o.keys().next()).map(String::as_str).unwrap_or("规则");
                    return (format!("{} · {}", kind, ranges.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")), range);
                }
                if let Some(cell) = row.as_str() {
                    return (cell.into(), Some(cell.into()));
                }
                let sheet = row.get("sheet").and_then(Value::as_str).unwrap_or("");
                let cell = row.get("cell").or_else(|| row.get("range")).and_then(Value::as_str).unwrap_or("");
                let reference = if sheet.is_empty() { cell.to_string() } else { format!("'{}'!{}", sheet.replace('\'', "''"), cell) };
                let label = format!(
                    "{} · {} · {}",
                    row.get("name").and_then(Value::as_str).unwrap_or(""),
                    reference,
                    row.get("value").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default()
                );
                (label, if cell.is_empty() { None } else { Some(reference) })
            })
            .collect();
    }
    value
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, v)| !v.is_object() && !v.is_array())
        .map(|(k, v)| (format!("{k}: {}", v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())), None))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sort_retains_region_and_rejects_bad_order() {
        let p = parameters("data.sort", json!({"column":"C","order":"desc","range":"A1:C8"})).unwrap();
        assert_eq!(p["keys"][0]["column"], "C");
        assert_eq!(p["range"], "A1:C8");
        assert!(parameters("data.sort", json!({"order":"bad"})).is_err());
    }
    #[test]
    fn multiple_sort_keys_keep_priority_and_reject_invalid_directions() {
        let p = parameters("data.sort", json!({"column":"A","order":"asc","keysText":"B:desc, C:asc","range":"A1:C8","header":true})).unwrap();
        assert_eq!(p["keys"], json!([{"column":"A","order":"asc"},{"column":"B","order":"desc"},{"column":"C","order":"asc"}]));
        assert_eq!(p["range"], "A1:C8");
        assert!(parameters("data.sort", json!({"keysText":"B:sideways"})).is_err());
        assert!(p.get("keysText").is_none());
    }
    #[test]
    fn columns_trim_and_keep_engine_arrays() {
        assert_eq!(parameters("data.removeDuplicates", json!({"columns":" A, C "})).unwrap()["columns"], json!(["A", "C"]));
        assert_eq!(parameters("data.removeDuplicates", json!({"columns":["B"]})).unwrap()["columns"], json!(["B"]));
    }
}
