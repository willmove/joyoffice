//! Discoverable controls for selected native shapes, SmartArt and editable charts.
use super::*;
use forms::{Field, show};
use serde_json::{Value, json};

const CHART_TYPES: &[(&str, &str)] = &[
    ("column", "簇状柱形图"),
    ("columnStacked", "堆积柱形图"),
    ("bar", "簇状条形图"),
    ("barStacked", "堆积条形图"),
    ("line", "折线图"),
    ("area", "面积图"),
    ("pie", "饼图"),
    ("doughnut", "圆环图"),
    ("scatter", "散点图"),
];
const SMART_LAYOUTS: &[(&str, &str)] = &[
    ("basicList", "基本列表"),
    ("process", "流程"),
    ("cycle", "循环"),
    ("hierarchy", "层次结构"),
    ("pyramid", "棱锥图"),
    ("radial", "射线图"),
    ("matrix", "矩阵"),
    ("venn", "维恩图"),
];
impl JoyWord {
    pub(super) fn object_ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        let object = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.object_info())
            .unwrap_or_default();
        let kind = object["kind"].as_str().unwrap_or("");
        let mut editing = div().flex().items_center().gap_2();
        if kind == "shape" {
            editing=editing.child(Button::new("shape-edit-text").label("编辑文字").on_click(cx.listener(|v,_,w,cx|v.object_text_form(w,cx))))
                .child(Button::new("shape-fill").label("形状填充").on_click(cx.listener(|v,_,w,cx| {
                    let color=v.canvas.reader.as_ref().and_then(|r|r.object_info()).map(|o|show(&o["fill"])).unwrap_or_else(||"156082".into());
                    v.fields_form("形状填充","shape.fill",vec![Field::color("填充颜色",color)],false,|s|Ok(json!({"color":if s[0]=="auto" {Value::Null} else {json!(s[0])}})),w,cx);
                })));
        } else if kind == "smartArt" {
            editing = editing.child(
                Button::new("smartart-edit-items")
                    .label("编辑文字和层级")
                    .on_click(cx.listener(|v, _, w, cx| v.smartart_grid(w, cx))),
            );
            editing = editing.child(
                self.format_choices(
                    "smartart-layout",
                    Button::new("smartart-layout-trigger").label("更改布局"),
                    "smartArt.layout",
                    SMART_LAYOUTS
                        .iter()
                        .map(|(id, name)| ((*name).into(), json!({"layout":id})))
                        .collect(),
                    cx,
                ),
            );
        } else if kind == "chart" {
            editing =
                editing
                    .child(
                        Button::new("chart-edit-data")
                            .label("编辑数据")
                            .on_click(cx.listener(|v, _, w, cx| v.chart_grid(w, cx))),
                    )
                    .child(Button::new("chart-edit-title").label("图表标题").on_click(
                        cx.listener(|v, _, w, cx| {
                            let title = v
                                .canvas
                                .reader
                                .as_ref()
                                .and_then(|r| r.object_info())
                                .map(|o| show(&o["data"]["title"]))
                                .unwrap_or_default();
                            v.fields_form(
                                "图表标题",
                                "chart.title",
                                vec![Field::text("标题文字", title)],
                                false,
                                |s| {
                                    if s[0].chars().count() > 255 {
                                        Err("图表标题最多255个字符".into())
                                    } else {
                                        Ok(json!({"text":s[0]}))
                                    }
                                },
                                w,
                                cx,
                            );
                        }),
                    ));
            editing = editing.child(
                self.format_choices(
                    "chart-type",
                    Button::new("chart-type-trigger").label("更改图表类型"),
                    "chart.type",
                    CHART_TYPES
                        .iter()
                        .map(|(id, name)| ((*name).into(), json!({"type":id})))
                        .collect(),
                    cx,
                ),
            );
        }
        let size = Button::new("object-size")
            .label("大小")
            .on_click(cx.listener(|v, _, w, cx| v.object_size_form(w, cx)));
        div()
            .flex()
            .flex_shrink_0()
            .child(self.group_box("编辑", editing))
            .child(self.group_box("大小", size))
            .child(
                self.group_box(
                    "排列",
                    self.format_choices(
                        "object-wrap",
                        Button::new("object-wrap-trigger")
                            .label("环绕文字")
                            .disabled(!self.format_ready("arrange.wrap"))
                            .tooltip(if kind == "shape" {
                                "设置形状与正文的环绕方式"
                            } else {
                                "当前图表和SmartArt的文字环绕尚未提供"
                            }),
                        "arrange.wrap",
                        [
                            ("inline", "嵌入型"),
                            ("square", "四周型"),
                            ("tight", "紧密型"),
                            ("topAndBottom", "上下型"),
                            ("behindText", "衬于文字下方"),
                            ("inFrontOfText", "浮于文字上方"),
                        ]
                        .into_iter()
                        .map(|(id, name)| (name.into(), json!({"wrap":id})))
                        .collect(),
                        cx,
                    ),
                ),
            )
    }
    fn object_size_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(r) = self.canvas.reader.as_ref() else {
            return;
        };
        let Some(o) = r.object_info() else {
            return;
        };
        let unit = r.engine().units();
        self.fields_form(
            "对象大小",
            "picture.size",
            vec![
                Field::length("宽度", show(&o["width"]), unit),
                Field::length("高度", show(&o["height"]), unit),
                Field::check("锁定纵横比", "true", "true", "false"),
            ],
            true,
            |v| {
                let mut args = json!({"lockAspect":v[2]!="false"});
                for (i, key) in [(0, "width"), (1, "height")] {
                    if !v[i].is_empty() {
                        args[key] =
                            json!(joyword_shell::authoring::required_number(&v[i], 1., 4000.)?);
                    }
                }
                Ok(args)
            },
            w,
            cx,
        );
    }
    fn object_text_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let text = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.object_info())
            .map(|o| show(&o["text"]))
            .unwrap_or_default();
        self.fields_form(
            "形状文字",
            "shape.text",
            vec![Field::multiline("文字", text)],
            false,
            |v| Ok(json!({"text":v[0]})),
            w,
            cx,
        );
    }
    fn smartart_grid(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(o) = self.canvas.reader.as_ref().and_then(|r| r.object_info()) else {
            return;
        };
        let rows = o["data"]["items"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|i| vec![show(&i["text"]), show(&i["level"])])
                    .collect()
            })
            .unwrap_or_default();
        self.data_grid_form(
            "SmartArt文字和层级",
            "smartArt.items",
            vec!["文字".into(), "级别（0为顶层）".into()],
            rows,
            false,
            |_, rows| {
                if rows.len() > 100 {
                    return Err("SmartArt最多支持100项".into());
                }
                let mut previous = 0;
                let items = rows
                    .iter()
                    .enumerate()
                    .map(|(index, r)| {
                        let level = joyword_shell::authoring::integer(&r[1], 0, 7)?;
                        if (index == 0 && level != 0) || level > previous + 1 {
                            return Err("首项级别必须为0；后续每项最多增加一级".into());
                        }
                        previous = level;
                        if r[0].chars().count() > 255 {
                            return Err("单项文字过长".into());
                        }
                        Ok(json!({"text":r[0],"level":level}))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(json!({"items":items}))
            },
            w,
            cx,
        );
    }
    fn chart_grid(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(o) = self.canvas.reader.as_ref().and_then(|r| r.object_info()) else {
            return;
        };
        let c = &o["data"];
        let series = c["series"].as_array().cloned().unwrap_or_default();
        let mut headers = vec!["类别".into()];
        headers.extend(series.iter().map(|s| show(&s["name"])));
        let rows = c["categories"]
            .as_array()
            .map(|cats| {
                cats.iter()
                    .enumerate()
                    .map(|(i, name)| {
                        let mut row = vec![show(name)];
                        row.extend(series.iter().map(|s| show(&s["values"][i])));
                        row
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.data_grid_form(
            "编辑图表数据",
            "chart.editData",
            headers,
            rows,
            true,
            chart_data,
            w,
            cx,
        );
    }
}
fn chart_data(headers: &[String], rows: &[Vec<String>]) -> Result<Value, String> {
    if rows.iter().any(|r| r.len() != headers.len()) {
        return Err("数据行的列数必须与字段一致".into());
    }
    if headers.len() < 2 || rows.is_empty() {
        return Err("请至少保留一个类别和一个数据系列".into());
    }
    if headers.len() > 256
        || rows.len() > 4000
        || rows.len().saturating_mul(headers.len() - 1) > 100_000
    {
        return Err("图表数据超过支持范围".into());
    }
    if headers
        .iter()
        .skip(1)
        .any(|s| s.trim().is_empty() || s.chars().count() > 255)
        || rows.iter().any(|r| r[0].chars().count() > 255)
    {
        return Err("类别和系列名称最多255个字符，系列名称不能为空".into());
    }
    let series = (1..headers.len())
        .map(|col| {
            let values = rows
                .iter()
                .enumerate()
                .map(|(row, r)| {
                    let v = r[col].trim();
                    if v.is_empty() {
                        return Ok(Value::Null);
                    }
                    let n = v
                        .replace(',', ".")
                        .parse::<f64>()
                        .ok()
                        .filter(|n| n.is_finite())
                        .ok_or_else(|| {
                            format!("第{}行，{}：请输入有效数字", row + 1, headers[col])
                        })?;
                    Ok(json!(n))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(json!({"name":headers[col],"values":values}))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(
        json!({"data":{"categories":rows.iter().map(|r|r[0].clone()).collect::<Vec<_>>(),"series":series}}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chart_grid_rejects_bad_cells_without_converting_them_to_zero() {
        let heads = vec!["类别".into(), "收入".into()];
        assert_eq!(
            chart_data(
                &heads,
                &[
                    vec!["一月".into(), "9".into()],
                    vec!["二月".into(), "".into()]
                ]
            )
            .unwrap()["data"]["series"][0]["values"],
            json!([9., null])
        );
        assert!(chart_data(&heads, &[vec!["一月".into(), "NaN".into()]]).is_err());
    }
}
