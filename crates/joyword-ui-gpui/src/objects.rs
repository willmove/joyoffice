//! Table and picture tools using the host's Kit forms, popovers and native file dialog.
use super::*;
use forms::show;
use gpui::component::{Disableable, Sizable, popover::Popover};
use joyword_shell::authoring::{insert_table_params, integer, picture_crop, required_number};
use joyword_shell::formatting::optional_number;
use serde_json::{Value, json};
fn yes(s: &str) -> Result<bool, String> {
    match s.trim() {
        "是" | "true" => Ok(true),
        "否" | "false" => Ok(false),
        _ => Err("请输入 是 或 否".into()),
    }
}
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
    pub(super) fn table_tools(&self, cx: &mut Context<Self>) -> gpui::Div {
        let table = self.canvas.reader.as_ref().and_then(|r| r.table_info());
        let mut first = div().flex().gap_1();
        for (id, label, cmd) in [
            ("select-cell", "选单元格", "table.selectCell"),
            ("select-row", "选行", "table.selectRow"),
            ("select-column", "选列", "table.selectColumn"),
            ("select-table", "全表", "table.selectTable"),
            ("row-above", "上插行", "table.insertRowAbove"),
            ("row-below", "下插行", "table.insertRowBelow"),
            ("column-left", "左插列", "table.insertColumnLeft"),
            ("column-right", "右插列", "table.insertColumnRight"),
            ("delete-row", "删除行", "table.deleteRow"),
            ("delete-column", "删除列", "table.deleteColumn"),
            ("delete-table", "删除表", "table.deleteTable"),
        ] {
            first = first.child(self.author_button(id, label, cmd, json!({}), cx));
        }
        let table_styles = self
            .style_catalog
            .iter()
            .filter(|s| s.kind == "table")
            .map(|s| (s.name.clone(), json!({"style":s.id})))
            .collect();
        let second=div().flex().gap_1()
            .child(self.author_button("merge-cells","合并","table.merge",json!({}),cx))
            .child(Button::new("split-cells").ghost().xsmall().label("拆分…").disabled(!self.format_ready("table.split")).on_click(cx.listener(|v,_,w,cx|v.fields_form("拆分单元格","table.split",vec![("列数","2".into()),("行数","1".into())],false,|s|Ok(json!({"columns":integer(&s[0],1,16)?,"rows":integer(&s[1],1,32)?,"mergeFirst":true})),w,cx))))
            .child(self.author_button("distribute-cols","均分列","table.distributeColumns",json!({}),cx))
            .child(self.author_button("distribute-rows","均分行","table.distributeRows",json!({}),cx))
            .child(self.choices("table-autofit","自适应 ▾".into(),"table.autofit",[("适应内容","contents"),("适应窗口","window"),("固定列宽","fixed")].into_iter().map(|(l,m)|(l.into(),json!({"mode":m}))).collect(),cx))
            .child(self.choices("table-style","表格样式 ▾".into(),"table.style",table_styles,cx))
            .child(self.choices("table-shading","底纹 ▾".into(),"table.shading",[("无底纹",None),("浅蓝",Some("DCE6F1")),("浅黄",Some("FFF2CC")),("浅绿",Some("E2F0D9"))].into_iter().map(|(l,c)|(l.into(),json!({"color":c}))).collect(),cx))
            .child(self.choices("table-borders","边框 ▾".into(),"table.borders",[("全部","all"),("外框","outside"),("内部","inside"),("无","none")].into_iter().map(|(l,k)|(l.into(),json!({"kind":k}))).collect(),cx))
            .child(self.choices("cell-align","单元格对齐 ▾".into(),"table.cellAlign",[("左上","topLeft"),("居中","center"),("右下","bottomRight")].into_iter().map(|(l,k)|(l.into(),json!({"value":k}))).collect(),cx))
            .child(Button::new("table-properties").ghost().xsmall().label("属性…").disabled(table.is_none() || !self.format_ready("table.properties")).on_click(cx.listener(|v,_,w,cx|v.table_form(w,cx))))
            .child(Button::new("cell-margins").ghost().xsmall().label("边距…").disabled(table.is_none() || !self.format_ready("table.properties")).on_click(cx.listener(|v,_,w,cx|v.fields_form("单元格边距","table.cellMargins",vec![("上（磅）",String::new()),("左（磅）",String::new()),("下（磅）",String::new()),("右（磅）",String::new())],true,|v|number_patch(v,&["top","left","bottom","right"],0.,1584.),w,cx))));
        div().flex().flex_col().px_3().gap_2().child(first).child(second).child(div().text_size(px(11.)).child(table.map(|t|format!("{} 行 × {} 列 · 当前第 {} 行 {} 列 · 拖动边界调整；Tab 在单元格间移动",t.rows,t.cols,t.row+1,t.col+1)).unwrap_or("点击表格单元格以编辑".into())))
    }
    fn table_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form(
            "表格属性",
            "table.properties",
            vec![
                ("表格宽度（磅）", String::new()),
                ("列宽（磅）", String::new()),
                ("行高（磅）", String::new()),
                ("表格缩进（磅）", String::new()),
                ("对齐（左/中/右）", String::new()),
                ("行可跨页（是/否）", String::new()),
                ("首行为标题（是/否）", String::new()),
            ],
            true,
            |v| {
                let mut p =
                    number_patch(&v[..3], &["width", "columnWidth", "rowHeight"], 1., 1584.)?;
                if let Some(n) = optional_number(&v[3], -1584., 1584.)? {
                    p["indent"] = json!(n);
                }
                if !v[4].trim().is_empty() {
                    p["align"] = json!(match v[4].trim() {
                        "左" => "left",
                        "中" => "center",
                        "右" => "right",
                        _ => return Err("对齐请输入 左、中、右".into()),
                    });
                }
                for (i, k) in [(5, "allowBreak"), (6, "headerRow")] {
                    if !v[i].trim().is_empty() {
                        p[k] = json!(yes(&v[i])?);
                    }
                }
                Ok(p)
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
    pub(super) fn picture_tools(&self, cx: &mut Context<Self>) -> gpui::Div {
        let pic = self.canvas.reader.as_ref().and_then(|r| r.picture_info());
        let disabled = pic.is_none() || !self.format_ready("picture.size");
        let first = div()
            .flex()
            .gap_1()
            .child(
                Button::new("insert-picture")
                    .ghost()
                    .xsmall()
                    .label("插入图片…")
                    .disabled(!self.format_ready("insert.picture"))
                    .on_click(cx.listener(|v, _, w, cx| v.pick_picture(false, w, cx))),
            )
            .child(
                Button::new("replace-picture")
                    .ghost()
                    .xsmall()
                    .label("替换…")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| v.pick_picture(true, w, cx))),
            )
            .child(
                Button::new("picture-size")
                    .ghost()
                    .xsmall()
                    .label("大小…")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| v.picture_size(w, cx))),
            )
            .child(
                Button::new("picture-crop")
                    .ghost()
                    .xsmall()
                    .label("裁剪…")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| v.picture_crop_form(w, cx))),
            )
            .child(
                self.choices(
                    "picture-wrap",
                    "环绕 ▾".into(),
                    "arrange.wrap",
                    [
                        ("嵌入型", "inline"),
                        ("四周型", "square"),
                        ("紧密型", "tight"),
                        ("上下型", "topAndBottom"),
                        ("衬于文字下方", "behindText"),
                        ("浮于文字上方", "inFrontOfText"),
                    ]
                    .into_iter()
                    .map(|(l, k)| (l.into(), json!({"wrap":k})))
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.choices(
                    "picture-rotate",
                    "旋转/翻转 ▾".into(),
                    "arrange.rotate",
                    [
                        ("向右 90°", "right"),
                        ("向左 90°", "left"),
                        ("水平翻转", "flipHorizontal"),
                        ("垂直翻转", "flipVertical"),
                    ]
                    .into_iter()
                    .map(|(l, k)| (l.into(), json!({"direction":k})))
                    .collect(),
                    cx,
                ),
            )
            .child(
                Button::new("picture-position")
                    .ghost()
                    .xsmall()
                    .label("位置…")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.fields_form(
                            "图片页面位置（设置后为浮动图片）",
                            "arrange.bounds",
                            vec![
                                ("横向位置（磅）", String::new()),
                                ("纵向位置（磅）", String::new()),
                            ],
                            true,
                            |v| number_patch(v, &["x", "y"], -1584., 1584.),
                            w,
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("picture-alt")
                    .ghost()
                    .xsmall()
                    .label("替代文字…")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| v.picture_alt(w, cx))),
            )
            .child(
                self.author_button("delete-picture", "删除图片", "text.delete", json!({}), cx)
                    .disabled(disabled),
            );
        let second = div()
            .flex()
            .gap_1()
            .child(
                Button::new("picture-adjust")
                    .ghost()
                    .xsmall()
                    .label("亮度/对比度…")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.fields_form(
                            "图片校正",
                            "picture.corrections",
                            vec![
                                ("亮度（-100–100）", "0".into()),
                                ("对比度（-100–100）", "0".into()),
                                ("锐化（-100–100）", "0".into()),
                            ],
                            false,
                            |v| {
                                number_patch(v, &["brightness", "contrast", "sharpen"], -100., 100.)
                            },
                            w,
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("picture-transparency")
                    .ghost()
                    .xsmall()
                    .label("透明度…")
                    .disabled(disabled)
                    .on_click(cx.listener(|v, _, w, cx| {
                        v.fields_form(
                            "图片透明度",
                            "picture.transparency",
                            vec![("透明度（0–100%）", "0".into())],
                            false,
                            |v| Ok(json!({"percent":required_number(&v[0],0.,100.)?})),
                            w,
                            cx,
                        )
                    })),
            )
            .child(
                self.choices(
                    "picture-recolor",
                    "颜色 ▾".into(),
                    "picture.color",
                    [
                        ("灰度", "grayscale"),
                        ("褐色", "sepia"),
                        ("冲蚀", "washout"),
                        ("黑白", "blackAndWhite"),
                    ]
                    .into_iter()
                    .map(|(l, k)| (l.into(), json!({"mode":k})))
                    .collect(),
                    cx,
                ),
            )
            .child(self.author_button("reset-picture", "重置图片", "picture.reset", json!({}), cx));
        div()
            .flex()
            .flex_col()
            .px_3()
            .gap_2()
            .child(first)
            .child(second)
            .child(
                div().text_size(px(11.)).child(
                    pic.map(|p| {
                        format!(
                            "{:.1} × {:.1} 磅 · 拖动移动或用右下角手柄缩放；Esc 取消拖动",
                            p.width, p.height
                        )
                    })
                    .unwrap_or("点击图片以选择并编辑".into()),
                ),
            )
    }
    fn picture_size(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(p) = self.canvas.reader.as_ref().and_then(|r| r.picture_info()) else {
            return;
        };
        self.fields_form(
            "图片大小",
            "picture.size",
            vec![
                ("宽（磅）", p.width.to_string()),
                ("高（磅）", p.height.to_string()),
                ("锁定比例（是/否）", "是".into()),
            ],
            true,
            |v| {
                let mut p = number_patch(&v[..2], &["width", "height"], 1., 4000.)?;
                p["lockAspect"] = json!(if v[2].trim().is_empty() {
                    true
                } else {
                    yes(&v[2])?
                });
                Ok(p)
            },
            w,
            cx,
        );
    }
    fn picture_crop_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
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
    fn picture_alt(&mut self, w: &mut Window, cx: &mut Context<Self>) {
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
                Ok(json!({"text":v[0],"title":v[1],"decorative":yes(&v[2])?}))
            },
            w,
            cx,
        );
    }
}
