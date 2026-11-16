//! Human-facing data forms translated to the unchanged engine protocol.
use serde_json::{Value, json};
pub fn defaults(id: &str) -> Option<Value> {
    if let Some(p) = crate::ribbon_defaults::defaults(id) {
        return Some(p);
    }
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
        "edit.find" | "edit.replace" => {
            json!({"what":"","with":"","all":false,"matchCase":false,"wholeCell":false,"lookIn":"values","within":"sheet","byColumns":false})
        }
        "range.fill" => json!({"input":""}),
        "home.conditionalFormat" => {
            json!({"type":"cellIs","operator":"greaterThan","value":15,"preset":"greenFill","color":"#638EC6","set":"3Arrows"})
        }
        "home.manageRules" => json!({}),
        "data.group" | "data.ungroup" | "data.hideDetail" | "data.showDetail" => json!({"by":"rows","lines":"2:4"}),
        "data.showLevels" => json!({"rows":1,"cols":1}),
        "data.getData" | "data.fromTextCsv" => json!({"path":"","text":"","delimiter":"","sheet":"导入数据"}),
        "data.validation" => {
            json!({"type":"any","operator":"between","formula1":"","formula2":"","ignoreBlank":true,"dropdown":true,"inputTitle":"","inputMessage":"","errorStyle":"stop","errorTitle":"","errorMessage":""})
        }
        _ => return None,
    })
}
pub fn parameters(id: &str, mut p: Value) -> Result<Value, String> {
    crate::advanced::parameters(id, &mut p)?;
    let m = p.as_object_mut().ok_or("Parameters must be an object")?;
    match id {
        "pageLayout.pageSetup" | "pageLayout.scaleToFit" => {
            let centimeters = m.remove("_marginUnit").and_then(|v| v.as_str().map(str::to_string)).as_deref() == Some("cm");
            if let Some(original) = m.remove("_original") {
                let mode_changed = m.get("fitMode") != original.get("fitMode");
                let fitting = m.get("fitMode").and_then(Value::as_bool).unwrap_or(false);
                m.retain(|key, value| {
                    original.get(key) != Some(value)
                        || mode_changed && if fitting { matches!(key.as_str(), "fitWidth" | "fitHeight") } else { key == "scale" }
                });
            }
            if let Some(fit) = m.remove("fitMode").and_then(|v| v.as_bool()) {
                if fit {
                    m.remove("scale");
                } else {
                    for key in ["fitWidth", "fitHeight", "width", "height"] {
                        m.remove(key);
                    }
                }
            }
            if id == "pageLayout.pageSetup" {
                let mut margins = serde_json::Map::new();
                for (key, target) in [
                    ("left", "left"),
                    ("right", "right"),
                    ("top", "top"),
                    ("bottom", "bottom"),
                    ("headerMargin", "header"),
                    ("footerMargin", "footer"),
                ] {
                    if let Some(v) = m.remove(key) {
                        margins.insert(target.into(), if centimeters { json!(v.as_f64().ok_or("Margin must be a number")? / 2.54) } else { v });
                    }
                }
                if !margins.is_empty() {
                    m.insert("margins".into(), Value::Object(margins));
                }
            }
        }
        "data.consolidate" | "data.subtotal" => {
            let (source, target) = if id == "data.consolidate" { ("sourcesText", "sources") } else { ("columnsText", "columns") };
            if let Some(text) = m.remove(source).and_then(|v| v.as_str().map(str::to_owned)) {
                let parts: Vec<_> = text.split([',', ';', '\n']).map(str::trim).filter(|s| !s.is_empty()).collect();
                if parts.is_empty() {
                    return Err("请输入至少一个区域或列 / Enter at least one range or column".into());
                }
                m.insert(target.into(), json!(parts));
            }
        }
        "home.formatCells" if m.contains_key("fontName") => {
            // Send only edited properties. Applying a font change must not replace
            // theme colours or flatten mixed formatting in the selected range.
            let original = m.remove("_original");
            let colors = m.remove("_colors").unwrap_or_default();
            let mut style = json!({});
            for (key, group, property) in [
                ("fontName", "font", "name"),
                ("fontSize", "font", "size"),
                ("bold", "font", "bold"),
                ("italic", "font", "italic"),
                ("strike", "font", "strike"),
                ("underline", "font", "underline"),
                ("fontScript", "font", "vert"),
                ("fontColor", "font", "color"),
                ("fillPattern", "fill", "pattern"),
                ("fillForeground", "fill", "fg"),
                ("fillBackground", "fill", "bg"),
                ("horizontal", "align", "h"),
                ("vertical", "align", "v"),
                ("angle", "align", "rotation"),
                ("indent", "align", "indent"),
                ("wrap", "align", "wrap"),
                ("shrink", "align", "shrink"),
                ("locked", "protection", "locked"),
                ("hidden", "protection", "hidden"),
                ("numberFormat", "", "num_fmt"),
            ] {
                let Some(v) = m.get(key) else { continue };
                if original.as_ref().and_then(|o| o.get(key)) == Some(v) {
                    continue;
                }
                let mut value = v.clone();
                if matches!(key, "fontColor" | "fillForeground" | "fillBackground") {
                    let c = joysheet_engine::model::Color::from_hex(v.as_str().unwrap_or("")).ok_or("Color must be #RRGGBB")?;
                    value = colors.get(key).cloned().unwrap_or(serde_json::to_value(c).map_err(|e| e.to_string())?);
                }
                if group.is_empty() {
                    style[property] = value;
                } else {
                    if style.get(group).is_none() {
                        style[group] = json!({});
                    }
                    style[group][property] = value;
                }
            }
            for (key, side) in [
                ("borderLeft", "left"),
                ("borderRight", "right"),
                ("borderTop", "top"),
                ("borderBottom", "bottom"),
                ("borderDown", "diag_down"),
                ("borderUp", "diag_up"),
            ] {
                let color_key = format!("{key}Color");
                for (source, target) in [(key, "style"), (color_key.as_str(), "color")] {
                    if let Some(v) = m.get(source) {
                        if original.as_ref().and_then(|o| o.get(source)) == Some(v) {
                            continue;
                        }
                        if style.get("borders").is_none() {
                            style["borders"] = json!({});
                        }
                        if style["borders"].get(side).is_none() {
                            style["borders"][side] = json!({});
                        }
                        style["borders"][side][target] = if target == "color" {
                            serde_json::to_value(joysheet_engine::model::Color::from_hex(v.as_str().unwrap_or("")).ok_or("Color must be #RRGGBB")?)
                                .map_err(|e| e.to_string())?
                        } else {
                            v.clone()
                        };
                    }
                }
            }
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
            let initial = m.remove("_cfInitial").unwrap_or_default();
            let model = m.remove("_modelRule");
            if !matches!(m.get("operator").and_then(Value::as_str), Some("between" | "notBetween")) {
                m.remove("value2");
            }
            let scale_keys = if m.get("scaleStops").and_then(Value::as_u64) == Some(3) {
                vec!["colorLow", "colorMiddle", "colorHigh"]
            } else {
                vec!["colorLow", "colorHigh"]
            };
            let colors_changed = scale_keys.iter().any(|k| m.get(*k) != initial.get(*k)) || m.get("scaleStops") != initial.get("scaleStops");
            let scale_colors = scale_keys.iter().filter_map(|k| m.get(*k).cloned()).collect::<Vec<_>>();
            for key in ["scaleStops", "colorLow", "colorMiddle", "colorHigh"] {
                m.remove(key);
            }
            let mut rule = serde_json::Map::new();
            for key in [
                "type", "operator", "value", "value2", "formula", "text", "rank", "bottom", "percent", "below", "period", "preset", "color", "set",
                "gradient", "reverse",
            ] {
                if let Some(v) = m.remove(key) {
                    rule.insert(key.into(), v);
                }
            }
            if rule.get("type").and_then(Value::as_str) == Some("colorScale") && !scale_colors.is_empty() {
                rule.insert("colors".into(), json!(scale_colors));
            }
            if initial["preset"] == rule.get("preset").cloned().unwrap_or_default()
                && let Some(mut style) = m.remove("_cfStyle")
            {
                if let Some(fields) = style.as_object_mut() {
                    fields.retain(|k, _| matches!(k.as_str(), "font" | "fill" | "borders" | "align" | "num_fmt" | "protection"));
                }
                rule.insert("style".into(), style);
            } else {
                m.remove("_cfStyle");
            }
            if let Some(extra) = m.remove("_cfExtras").and_then(|v| v.as_object().cloned()) {
                rule.extend(extra);
            }
            if let Some(model) = model {
                let mut parsed =
                    serde_json::to_value(joysheet_engine::cmd::insert::parse_cf_rule(&Value::Object(rule.clone())).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if let (Some(old), Some(new)) = (model.as_object(), parsed.as_object_mut())
                    && let (Some((kind, fields)), Some((new_kind, new_fields))) = (old.iter().next(), new.iter_mut().next())
                    && kind == new_kind
                    && let (Some(fields), Some(new_fields)) = (fields.as_object(), new_fields.as_object_mut())
                {
                    for (k, value) in fields {
                        if k == "stops" && colors_changed {
                            if let (Some(old), Some(new)) = (value.as_array(), new_fields.get_mut(k).and_then(Value::as_array_mut))
                                && old.len() == new.len()
                            {
                                for (old, new) in old.iter().zip(new) {
                                    let color = new[1].clone();
                                    *new = old.clone();
                                    new[1] = color;
                                }
                            }
                            continue;
                        }
                        let key = match k.as_str() {
                            "a" => "value",
                            "b" => "value2",
                            "op" => "operator",
                            "style" => "preset",
                            other => other,
                        };
                        if !rule.contains_key(key) || initial.get(key) == rule.get(key) {
                            new_fields.insert(k.clone(), value.clone());
                        }
                    }
                }
                rule.clear();
                rule.insert("model".into(), parsed);
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
#[path = "tests/workflow_tests.rs"]
mod tests;
