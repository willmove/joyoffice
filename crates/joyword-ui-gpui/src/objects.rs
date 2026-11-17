//! Table and picture tools using the host's Kit forms, popovers and native file dialog.
use super::*;
use forms::show;
use gpui::component::{Disableable, Sizable, popover::Popover};
use joyword_shell::authoring::{insert_table_params, picture_crop};
use joyword_shell::formatting::optional_number;
use serde_json::{Value, json};
fn number_patch(v: &[String], keys: &[&str], min: f64, max: f64) -> Result<Value, String> {
    let mut p = serde_json::Map::new();
    for (s, k) in v.iter().zip(keys) {
        if let Some(n) = optional_number(s, min, max)? {
            p.insert((*k).into(), json!(n));
        }
    }
    Ok(Value::Object(p))
}
impl JoyWord {
    pub(super) fn author_button(
        &self,
        id: &'static str,
        label: &'static str,
        command: &'static str,
        args: Value,
        cx: &mut Context<Self>,
    ) -> Button {
        let reason = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.formatting_disabled(command));
        Button::new(id)
            .ghost()
            .xsmall()
            .label(label)
            .tooltip(reason.unwrap_or(label))
            .disabled(!self.format_ready(command))
            .on_click(cx.listener(move |v, _, w, cx| {
                let _ = v.try_edit(command, &args, w, cx);
            }))
    }
    pub(super) fn insert_table_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form(
            "插入表格",
            "insert.table",
            vec![("行数（1–100）", "3".into()), ("列数（1–32）", "3".into())],
            false,
            insert_table_params,
            w,
            cx,
        );
    }
    pub(super) fn table_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let title = self.shell.tr("选择表格行列", "Insert Table");
        let more = self.shell.tr("更多行列…", "Insert Table…");
        let border = self.palette().muted;
        let target = self.canvas.reader.as_ref().map(|r| {
            (
                r.engine().document_id(),
                r.engine().rev(),
                r.engine().sel.clone(),
            )
        });
        Popover::new("table-insert-grid")
            .trigger(self.insert_table_trigger())
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut grid = div().flex().flex_col().gap_1().child(title);
                for r in 1usize..=8 {
                    let mut row = div().flex().gap_1();
                    for c in 1usize..=10 {
                        let view = view.clone();
                        let popup = popup.clone();
                        let target = target.clone();
                        row = row.child(
                            Button::new(("insert-grid-cell", r * 10 + c))
                                .ghost()
                                .xsmall()
                                .w(px(20.))
                                .h(px(20.))
                                .p_0()
                                .rounded(px(1.))
                                .border_1()
                                .border_color(rgb(border))
                                .tooltip(format!("{r} 行 × {c} 列"))
                                .accessibility_label(format!("插入 {r} 行 {c} 列表格"))
                                .on_click(move |_, w, cx| {
                                    let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                                    let _ = view.update(cx, |v, cx| {
                                        if v.canvas.reader.as_ref().map(|r| {
                                            (
                                                r.engine().document_id(),
                                                r.engine().rev(),
                                                r.engine().sel.clone(),
                                            )
                                        }) == target
                                        {
                                            let _ = v.try_edit(
                                                "insert.table",
                                                &json!({"rows":r,"cols":c}),
                                                w,
                                                cx,
                                            );
                                            v.tab = 6;
                                        }
                                    });
                                }),
                        );
                    }
                    grid = grid.child(row);
                }
                let view = view.clone();
                let popup = popup.clone();
                grid.child(
                    Button::new("insert-table-more")
                        .ghost()
                        .xsmall()
                        .label(more)
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = view.update(cx, |v, cx| v.insert_table_form(w, cx));
                        }),
                )
            })
    }
    pub(super) fn table_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        use forms::Field;
        let Some(reader) = self.canvas.reader.as_ref() else {
            return;
        };
        let Some(table) = reader.table_info() else {
            return;
        };
        let unit = reader.engine().units();
        let row = &table.formatting;
        let props = &table.props;
        let flag = |value: &Value| value.as_bool().map(|b| b.to_string()).unwrap_or_default();
        let mode = if props["widthPct"].is_number() {
            "percent"
        } else if props["width"].is_number() {
            "fixed"
        } else {
            "auto"
        };
        let row_mode = show(&row["rowHeightRule"]);
        let original_width = props["width"].clone();
        let original_pct = props["widthPct"].clone();
        let original_height = row["rowHeight"].clone();
        self.fields_form(
            "表格属性",
            "table.properties",
            vec![
                Field::choice(
                    "宽度方式",
                    mode,
                    &[
                        ("auto", "自动"),
                        ("fixed", "指定宽度"),
                        ("percent", "相对页面宽度（%）"),
                    ],
                )
                .in_tab("表格"),
                Field::length("表格宽度", show(&props["width"]), unit).in_tab("表格"),
                Field::text("相对宽度（%）", show(&props["widthPct"])).in_tab("表格"),
                Field::length(
                    "表格缩进",
                    show(
                        props
                            .get("indent")
                            .filter(|v| v.is_number())
                            .unwrap_or(&json!(0.)),
                    ),
                    unit,
                )
                .in_tab("表格"),
                Field::choice(
                    "对齐方式",
                    props["align"].as_str().unwrap_or("left"),
                    &[("left", "左对齐"), ("center", "居中"), ("right", "右对齐")],
                )
                .in_tab("表格"),
                Field::choice(
                    "行高方式",
                    row_mode,
                    &[("auto", "自动"), ("atLeast", "最小值"), ("exact", "固定值")],
                )
                .in_tab("行"),
                Field::length("指定行高", show(&row["rowHeight"]), unit).in_tab("行"),
                Field::check("允许跨页断行", flag(&row["allowBreak"]), "true", "false")
                    .in_tab("行"),
                Field::check(
                    "在各页顶端以标题行形式重复",
                    flag(&row["headerRow"]),
                    "true",
                    "false",
                )
                .in_tab("行"),
                Field::length("列宽", show(&row["columnWidth"]), unit).in_tab("列"),
                Field::length("单元格宽度", show(&row["cellWidth"]), unit).in_tab("单元格"),
                Field::choice(
                    "垂直对齐方式",
                    show(&row["valign"]),
                    &[("top", "顶端"), ("center", "居中"), ("bottom", "底端")],
                )
                .in_tab("单元格"),
            ],
            true,
            move |v| {
                let mut args = json!({});
                let numeric = |s: &str, min, max| optional_number(s, min, max);
                if v[0] == "auto" {
                    args["width"] = Value::Null;
                    args["widthPct"] = Value::Null;
                }
                if !v[1].is_empty() && !v[2].is_empty() {
                    return Err("请仅设置指定宽度或相对宽度中的一项".into());
                }
                if (!v[1].is_empty() || v[0] == "fixed") && v[0] != "auto" && v[0] != "percent" {
                    let text = if v[1].is_empty() {
                        show(&original_width)
                    } else {
                        v[1].clone()
                    };
                    args["width"] = json!(numeric(&text, 6., 1584.)?.ok_or("请输入表格宽度")?);
                }
                if (!v[2].is_empty() || v[0] == "percent") && v[0] != "auto" && v[0] != "fixed" {
                    let text = if v[2].is_empty() {
                        show(&original_pct)
                    } else {
                        v[2].clone()
                    };
                    args["widthPct"] = json!(numeric(&text, 1., 100.)?.ok_or("请输入相对宽度")?);
                }
                if let Some(n) = numeric(&v[3], -1584., 1584.)? {
                    args["indent"] = json!(n);
                }
                if !v[4].is_empty() {
                    args["align"] = json!(v[4]);
                }
                if v[5] == "auto" {
                    args["rowHeight"] = Value::Null;
                } else {
                    if let Some(n) = numeric(&v[6], 1., 1584.)? {
                        args["rowHeight"] = json!(n);
                    }
                    if !v[5].is_empty() {
                        if !args["rowHeight"].is_number() && !original_height.is_number() {
                            return Err("请输入指定行高".into());
                        }
                        args["rowHeightRule"] = json!(v[5]);
                    }
                }
                for (i, key) in [(7, "allowBreak"), (8, "headerRow")] {
                    if !v[i].is_empty() {
                        args[key] = json!(v[i] == "true");
                    }
                }
                for (i, key) in [(9, "columnWidth"), (10, "cellWidth")] {
                    if let Some(n) = numeric(&v[i], 6., 1584.)? {
                        args[key] = json!(n);
                    }
                }
                if !v[11].is_empty() {
                    args["valign"] = json!(v[11]);
                }
                Ok(args)
            },
            w,
            cx,
        );
    }
    pub(super) fn pick_picture(&mut self, replace: bool, w: &mut Window, cx: &mut Context<Self>) {
        let cmd = if replace {
            "picture.change"
        } else {
            "insert.picture"
        };
        if !self.format_ready(cmd) {
            return;
        }
        let Some(r) = self.canvas.reader.as_ref() else {
            return;
        };
        let target = (
            r.engine().document_id(),
            r.engine().rev(),
            r.engine().sel.clone(),
        );
        let Some(epoch) = self.asset_epoch.checked_add(1) else {
            self.shell.notice = Some("图片请求代次已耗尽".into());
            cx.notify();
            return;
        };
        self.asset_epoch = epoch;
        let chooser = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择图片".into()),
        });
        self.asset_task = Some(cx.spawn(async move |view, cx| {
            let path = match chooser.await {
                Ok(Ok(Some(p))) => p.into_iter().next(),
                _ => None,
            };
            let Some(path) = path else {
                return;
            };
            let result = cx
                .background_executor()
                .spawn(async move { joyword_shell::authoring::import_image(&path) })
                .await;
            let _ = view.update(cx, |v, cx| {
                if v.asset_epoch != epoch
                    || v.canvas.reader.as_ref().map(|r| {
                        (
                            r.engine().document_id(),
                            r.engine().rev(),
                            r.engine().sel.clone(),
                        )
                    }) != Some(target)
                {
                    return;
                }
                match result {
                    Err(e) => v.shell.notice = Some(e),
                    Ok(args) => {
                        if !v.format_ready(cmd) {
                            return;
                        }
                        let Some(r) = v.canvas.reader.as_mut() else {
                            return;
                        };
                        match r.execute(cmd, &args) {
                            Err(e) => v.shell.notice = Some(e),
                            Ok(_) => {
                                if !replace {
                                    let mut pos = r.engine().sel.focus.clone();
                                    pos.off = pos.off.saturating_sub('\u{fffc}'.len_utf8());
                                    let _ = r.select_object(pos);
                                }
                                v.format_state = None;
                                v.invalidate_document(cx);
                                v.reveal_input = true;
                                v.tab = 7;
                                v.shell.notice = None;
                            }
                        }
                    }
                }
                cx.notify();
            });
        }));
        let _ = w;
    }
    pub(super) fn picture_size(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.canvas.reader.as_ref().and_then(|r| r.picture_info()) else {
            return;
        };
        let unit = self
            .canvas
            .reader
            .as_ref()
            .map(|r| r.engine().units())
            .unwrap_or_default();
        self.fields_form(
            "图片大小",
            "picture.size",
            vec![
                forms::Field::length("宽度", p.width.to_string(), unit),
                forms::Field::length("高度", p.height.to_string(), unit),
                forms::Field::check("锁定纵横比", "true", "true", "false"),
            ],
            true,
            |v| {
                let mut p = number_patch(&v[..2], &["width", "height"], 1., 4000.)?;
                p["lockAspect"] = json!(if v[2].trim().is_empty() {
                    true
                } else {
                    v[2] == "true"
                });
                Ok(p)
            },
            w,
            cx,
        );
    }
    pub(super) fn picture_crop_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.canvas.reader.as_ref().and_then(|r| r.picture_info()) else {
            return;
        };
        self.fields_form(
            "裁剪图片",
            "picture.crop",
            [
                "左侧裁剪（%）",
                "顶部裁剪（%）",
                "右侧裁剪（%）",
                "底部裁剪（%）",
            ]
            .into_iter()
            .zip(p.crop)
            .map(|(l, n)| (l, format!("{:.2}", n * 100.)))
            .collect(),
            false,
            picture_crop,
            w,
            cx,
        );
    }
    pub(super) fn picture_alt(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.canvas.reader.as_ref().and_then(|r| r.picture_info()) else {
            return;
        };
        self.fields_form(
            "图片替代文字",
            "object.altText",
            vec![
                ("描述", p.alt),
                ("标题", show(&p.float["alt"]["title"])),
                (
                    "装饰性图片（是/否）",
                    if p.float["alt"]["decorative"].as_bool() == Some(true) {
                        "是"
                    } else {
                        "否"
                    }
                    .into(),
                ),
            ],
            false,
            |v| {
                if v[0].len() > 4096 || v[1].len() > 4096 {
                    return Err("替代文字最多 4096 字节".into());
                }
                Ok(json!({"text":v[0],"title":v[1],"decorative":v[2] == "true"}))
            },
            w,
            cx,
        );
    }
}
