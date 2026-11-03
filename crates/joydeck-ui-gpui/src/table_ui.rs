//! Table formatting uses bounded engine transactions and the existing public OOXML writer.
use super::*;
use gpui_kit::component::{Sizable, button::ButtonVariants, popover::Popover};
use joydeck_engine::{CellBorders, CellFormat, CellRange, Command, TableAction};
use joydeck_model::{
    ShapeKind, Table,
    text::{Anchor, TextDir},
};

impl Viewer {
    fn selected_table(&self) -> Option<(ShapeId, &Table, CellRange)> {
        let e = self.editor.as_ref()?;
        let [id] = e.session.selection().shapes.as_slice() else {
            return None;
        };
        let shape = e.session.model().slides.get(self.page)?.shape(*id)?;
        if shape.locked {
            return None;
        }
        let ShapeKind::Table(t) = &shape.kind else {
            return None;
        };
        let range = if let Some((_, row, col)) = self.cell.filter(|(shape, _, _)| shape == id) {
            CellRange {
                row,
                col,
                end_row: row,
                end_col: col,
            }
        } else if let Some((_, range)) = self.cell_range.filter(|(shape, _)| shape == id) {
            range
        } else {
            CellRange {
                row: 0,
                col: 0,
                end_row: t.n_rows().checked_sub(1)?,
                end_col: t.n_cols().checked_sub(1)?,
            }
        };
        Some((*id, t, range))
    }
    fn table_action(&mut self, action: TableAction, w: &mut Window, cx: &mut Context<Self>) {
        let structural = matches!(
            action,
            TableAction::InsertRow { .. }
                | TableAction::DeleteRow { .. }
                | TableAction::InsertCol { .. }
                | TableAction::DeleteCol { .. }
                | TableAction::Merge { .. }
                | TableAction::Split { .. }
        );
        self.dispatch(Command::Table { action }, w, cx);
        if structural {
            self.cell = None;
            self.cell_range = None;
        }
        self.focus.focus(w, cx);
    }
    fn table_menu(
        &self,
        id: &'static str,
        label: &'static str,
        items: Vec<(String, TableAction)>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().downgrade();
        Popover::new((id, 0usize))
            .trigger(
                Button::new(id)
                    .ghost()
                    .xsmall()
                    .label(format!("{label}⌄"))
                    .h(px(25.))
                    .accessibility_label(label)
                    .disabled(self.selected_table().is_none() || self.loading || self.composing()),
            )
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let mut list = div()
                    .id("table-choice-list")
                    .flex()
                    .flex_col()
                    .w(px(240.))
                    .max_h(px(330.))
                    .overflow_y_scroll()
                    .gap_1();
                for (index, (label, action)) in items.iter().enumerate() {
                    let view = view.clone();
                    let popup = popup.clone();
                    let action = action.clone();
                    list = list.child(
                        Button::new(("table-choice", index))
                            .ghost()
                            .xsmall()
                            .label(label.clone())
                            .on_click(move |_, w, cx| {
                                let _ = popup.update(cx, |s, cx| s.dismiss(w, cx));
                                let _ =
                                    view.update(cx, |s, cx| s.table_action(action.clone(), w, cx));
                            }),
                    );
                }
                list
            })
    }
    #[inline(never)]
    pub(super) fn table_controls(&self, cx: &mut Context<Self>) -> Div {
        let disabled = self.selected_table().is_none() || self.loading || self.composing();
        let range = self
            .selected_table()
            .map(|(_, _, range)| range)
            .unwrap_or(CellRange {
                row: 0,
                col: 0,
                end_row: 0,
                end_col: 0,
            });
        let mut bar = div().flex().items_start().h(px(66.));
        bar = bar.child(chrome::group(
            "编辑",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("table-edit-text")
                        .ghost()
                        .xsmall()
                        .label("编辑文字")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(Self::begin_inline)),
                )
                .child(
                    Button::new("table-copy-text")
                        .ghost()
                        .xsmall()
                        .label("复制文字／表格")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, _, cx| s.copy_grid(cx))),
                ),
        ));
        bar = bar.child(chrome::group(
            "选择",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("table-select-range")
                        .ghost()
                        .xsmall()
                        .label("选择区域…")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.table_range_dialog(w, cx))),
                )
                .child(
                    Button::new("table-select-all")
                        .ghost()
                        .xsmall()
                        .label("选择全表")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| {
                            s.end_inline(cx);
                            s.cell = None;
                            s.cell_range = None;
                            s.focus.focus(w, cx);
                            cx.notify();
                        })),
                ),
        ));
        let row_ops = vec![
            (
                "在上方插入行".into(),
                TableAction::InsertRow { at: range.row },
            ),
            (
                "在下方插入行".into(),
                TableAction::InsertRow {
                    at: range.end_row + 1,
                },
            ),
            (
                "删除当前行".into(),
                TableAction::DeleteRow { at: range.row },
            ),
        ];
        let col_ops = vec![
            (
                "在左侧插入列".into(),
                TableAction::InsertCol { at: range.col },
            ),
            (
                "在右侧插入列".into(),
                TableAction::InsertCol {
                    at: range.end_col + 1,
                },
            ),
            (
                "删除当前列".into(),
                TableAction::DeleteCol { at: range.col },
            ),
        ];
        bar = bar.child(chrome::group(
            "行和列",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.table_menu("table-rows", "行", row_ops, cx))
                .child(self.table_menu("table-columns", "列", col_ops, cx)),
        ));
        let styles = joydeck_model::table::table_styles()
            .into_iter()
            .map(|(name, label)| {
                (
                    label,
                    TableAction::Style {
                        name,
                        first_row: true,
                        band_row: true,
                    },
                )
            })
            .collect();
        let colors = [
            ("浅蓝", 0xdce6f1),
            ("浅橙", 0xfce4d6),
            ("浅绿", 0xe2efda),
            ("浅黄", 0xfff2cc),
            ("白色", 0xffffff),
            ("蓝色", 0x4472c4),
            ("橙色", 0xed7d31),
            ("灰色", 0xa5a5a5),
        ];
        let mut fills: Vec<_> = colors
            .into_iter()
            .map(|(label, rgb)| {
                (
                    label.into(),
                    TableAction::Format {
                        range,
                        format: CellFormat {
                            fill_rgb: Some(rgb),
                            ..Default::default()
                        },
                    },
                )
            })
            .collect();
        fills.push((
            "恢复样式底色".into(),
            TableAction::Format {
                range,
                format: CellFormat {
                    clear_fill: true,
                    ..Default::default()
                },
            },
        ));
        bar = bar.child(chrome::group(
            "表格样式",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.table_menu("table-styles", "表格样式", styles, cx))
                .child(self.table_menu("table-shading", "单元格底色", fills, cx)),
        ));
        let borders = [
            ("所有边框", [true; 4], false),
            ("无边框", [true; 4], true),
            ("下边框", [false, false, false, true], false),
            ("上边框", [false, false, true, false], false),
            ("左边框", [true, false, false, false], false),
            ("右边框", [false, true, false, false], false),
        ]
        .into_iter()
        .map(|(label, sides, remove)| {
            (
                label.into(),
                TableAction::Format {
                    range,
                    format: CellFormat {
                        borders: Some(CellBorders {
                            sides,
                            rgb: 0x404040,
                            width: 1.,
                            remove,
                        }),
                        ..Default::default()
                    },
                },
            )
        })
        .collect();
        bar = bar.child(chrome::group(
            "边框",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.table_menu("table-borders", "边框", borders, cx))
                .child(
                    Button::new("table-border-settings")
                        .ghost()
                        .xsmall()
                        .label("边框设置…")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.table_border_dialog(w, cx))),
                ),
        ));
        bar = bar.child(chrome::group(
            "合并",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("merge-cells")
                        .ghost()
                        .xsmall()
                        .label("合并选区")
                        .h(px(25.))
                        .disabled(
                            disabled || (range.row == range.end_row && range.col == range.end_col),
                        )
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.table_action(
                                TableAction::Merge {
                                    row: range.row,
                                    col: range.col,
                                    end_row: range.end_row,
                                    end_col: range.end_col,
                                },
                                w,
                                cx,
                            )
                        })),
                )
                .child(
                    Button::new("split-cell")
                        .ghost()
                        .xsmall()
                        .label("拆分单元格")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(move |s, _, w, cx| {
                            s.table_action(
                                TableAction::Split {
                                    row: range.row,
                                    col: range.col,
                                },
                                w,
                                cx,
                            )
                        })),
                ),
        ));
        let anchors = [
            ("顶端对齐", Anchor::Top),
            ("垂直居中", Anchor::Middle),
            ("底端对齐", Anchor::Bottom),
        ]
        .into_iter()
        .map(|(label, anchor)| {
            (
                label.into(),
                TableAction::Format {
                    range,
                    format: CellFormat {
                        anchor: Some(anchor),
                        ..Default::default()
                    },
                },
            )
        })
        .collect();
        let directions = [
            ("水平文字", TextDir::Horizontal),
            ("旋转 90°", TextDir::Vertical),
            ("旋转 270°", TextDir::Vertical270),
            ("中文竖排", TextDir::EaVertical),
        ]
        .into_iter()
        .map(|(label, vertical)| {
            (
                label.into(),
                TableAction::Format {
                    range,
                    format: CellFormat {
                        vertical: Some(vertical),
                        ..Default::default()
                    },
                },
            )
        })
        .collect();
        bar = bar.child(chrome::group(
            "对齐",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(self.table_menu("cell-anchor", "垂直对齐", anchors, cx))
                .child(self.table_menu("cell-direction", "文字方向", directions, cx)),
        ));
        bar.child(chrome::group(
            "大小",
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    Button::new("table-cell-size")
                        .ghost()
                        .xsmall()
                        .label("行高和列宽…")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.table_size_dialog(w, cx))),
                )
                .child(
                    Button::new("table-cell-margins")
                        .ghost()
                        .xsmall()
                        .label("单元格边距…")
                        .h(px(25.))
                        .disabled(disabled)
                        .on_click(cx.listener(|s, _, w, cx| s.table_margins_dialog(w, cx))),
                ),
        ))
    }
    fn table_range_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some((id, t, range)) = self.selected_table() else {
            return;
        };
        let (rows, cols) = (t.n_rows(), t.n_cols());
        self.fields_dialog(
            "选择单元格区域",
            vec![
                ("起始行", (range.row + 1).to_string()),
                ("起始列", (range.col + 1).to_string()),
                ("结束行", (range.end_row + 1).to_string()),
                ("结束列", (range.end_col + 1).to_string()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                let parse = |i: usize| {
                    v[i].trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .ok_or("行列号从 1 开始")
                };
                let range = CellRange {
                    row: parse(0)?,
                    col: parse(1)?,
                    end_row: parse(2)?,
                    end_col: parse(3)?,
                };
                if range.row > range.end_row
                    || range.col > range.end_col
                    || range.end_row >= rows
                    || range.end_col >= cols
                {
                    return Err("选区超出表格或起止顺序错误".into());
                }
                s.end_inline(cx);
                s.cell = None;
                s.cell_range = Some((id, range));
                s.focus.focus(w, cx);
                cx.notify();
                Ok(())
            },
        );
    }
    fn table_border_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some((_, _, range)) = self.selected_table() else {
            return;
        };
        self.fields_dialog(
            "边框设置",
            vec![
                ("颜色（六位 RGB）", "404040".into()),
                ("线宽（磅）", "1".into()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                let c = format_ui::parse_rgb(&v[0])?;
                let c = c.resolve(&joydeck_model::Theme::default().colors, None);
                let width: f64 = v[1].trim().parse().map_err(|_| "请输入有效线宽")?;
                if !width.is_finite() || !(0.1..=100.).contains(&width) {
                    return Err("线宽范围 0.1–100 磅".into());
                }
                s.table_action(
                    TableAction::Format {
                        range,
                        format: CellFormat {
                            borders: Some(CellBorders {
                                sides: [true; 4],
                                rgb: ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32,
                                width,
                                remove: false,
                            }),
                            ..Default::default()
                        },
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
    fn table_size_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some((_, t, range)) = self.selected_table() else {
            return;
        };
        let heights: Vec<_> = t
            .rows
            .iter()
            .take(range.end_row + 1)
            .skip(range.row)
            .map(|r| r.height)
            .collect();
        let widths: Vec<_> = t
            .cols
            .iter()
            .copied()
            .take(range.end_col + 1)
            .skip(range.col)
            .collect();
        let common = |values: &[f64]| {
            values
                .first()
                .filter(|first| values.iter().all(|n| n == *first))
                .map(|n| n.to_string())
                .unwrap_or_default()
        };
        let original = [common(&heights), common(&widths)];
        let initial = original.clone();
        self.fields_dialog(
            "行高和列宽（留空保持原值）",
            vec![
                ("行高（磅）", initial[0].clone()),
                ("列宽（磅）", initial[1].clone()),
            ],
            w,
            cx,
            move |s, v, w, cx| {
                let height = if v[0] == original[0] {
                    None
                } else {
                    paragraph_ui::optional_number(&v[0], 4., 4000.)?
                };
                let width = if v[1] == original[1] {
                    None
                } else {
                    paragraph_ui::optional_number(&v[1], 4., 4000.)?
                };
                let mut commands = vec![];
                if let Some(height) = height {
                    for row in range.row..=range.end_row {
                        commands.push(Command::Table {
                            action: TableAction::RowHeight { row, height },
                        });
                    }
                }
                if let Some(width) = width {
                    for col in range.col..=range.end_col {
                        commands.push(Command::Table {
                            action: TableAction::ColWidth { col, width },
                        });
                    }
                }
                if !commands.is_empty() {
                    s.dispatch(Command::Batch { commands }, w, cx);
                }
                Ok(())
            },
        );
    }
    fn table_margins_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some((_, t, range)) = self.selected_table() else {
            return;
        };
        let cells: Vec<_> = t
            .rows
            .iter()
            .take(range.end_row + 1)
            .skip(range.row)
            .flat_map(|r| r.cells.iter().take(range.end_col + 1).skip(range.col))
            .filter(|c| !c.is_covered())
            .map(|c| c.margins.unwrap_or([7.2, 7.2, 3.6, 3.6]))
            .collect();
        let mut original = [String::new(), String::new(), String::new(), String::new()];
        for (i, value) in original.iter_mut().enumerate() {
            if let Some(first) = cells.first()
                && cells.iter().all(|c| c[i] == first[i])
            {
                *value = first[i].to_string();
            }
        }
        let initial = original.clone();
        self.fields_dialog(
            "单元格边距（留空保持原值）",
            [
                "左边距（磅）",
                "右边距（磅）",
                "上边距（磅）",
                "下边距（磅）",
            ]
            .into_iter()
            .zip(initial)
            .collect(),
            w,
            cx,
            move |s, v, w, cx| {
                let mut patch = [None; 4];
                for i in 0..4 {
                    if v[i] != original[i] {
                        patch[i] = paragraph_ui::optional_number(&v[i], 0., 720.)?;
                    }
                }
                if patch.iter().any(Option::is_some) {
                    s.table_action(
                        TableAction::Format {
                            range,
                            format: CellFormat {
                                margin_patch: Some(patch),
                                ..Default::default()
                            },
                        },
                        w,
                        cx,
                    );
                }
                Ok(())
            },
        );
    }
}
