//! Bounded, paginated local drafts shared by recipient and object-data editors.
use super::*;
use gpui::component::{
    WindowExt,
    input::{Input, InputState, Textarea, TextareaState},
};
use gpui::{Entity, EntityInputHandler};
use serde_json::Value;
use std::rc::Rc;

const PAGE_ROWS: usize = 8;
const MAX_CELLS: usize = 100_000;
const MAX_COLUMNS: usize = 256;
struct Draft {
    headers: Vec<Entity<InputState>>,
    rows: Vec<Vec<String>>,
    page: usize,
    editors: Vec<Vec<Entity<TextareaState>>>,
    error: String,
}
fn input(value: &str, w: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| {
        let mut s = InputState::new(w, cx);
        s.set_value(value, w, cx);
        s
    })
}
fn cell(value: &str, w: &mut Window, cx: &mut App) -> Entity<TextareaState> {
    cx.new(|cx| {
        let mut s = TextareaState::new(w, cx).auto_grow(1, 3);
        s.set_value(value, w, cx);
        s
    })
}
impl Draft {
    fn commit(&mut self, cx: &App) {
        for (offset, row) in self.editors.iter().enumerate() {
            if let Some(target) = self.rows.get_mut(self.page * PAGE_ROWS + offset) {
                *target = row.iter().map(|s| s.read(cx).value().to_string()).collect();
            }
        }
    }
    fn reload(&mut self, w: &mut Window, cx: &mut App) {
        self.page = self.page.min(self.rows.len().saturating_sub(1) / PAGE_ROWS);
        self.editors = self
            .rows
            .iter()
            .skip(self.page * PAGE_ROWS)
            .take(PAGE_ROWS)
            .map(|row| row.iter().map(|v| cell(v, w, cx)).collect())
            .collect();
    }
    fn composing(&self, w: &mut Window, cx: &mut App) -> bool {
        self.headers
            .iter()
            .any(|s| s.update(cx, |s, cx| s.marked_text_range(w, cx).is_some()))
            || self
                .editors
                .iter()
                .flatten()
                .any(|s| s.update(cx, |s, cx| s.marked_text_range(w, cx).is_some()))
    }
    fn add_row(&mut self, w: &mut Window, cx: &mut App) {
        if self.composing(w, cx) {
            return;
        }
        self.commit(cx);
        if (self.rows.len() + 1).saturating_mul(self.headers.len()) > MAX_CELLS {
            self.error = "数据表最多包含100000个单元格".into();
            return;
        }
        self.rows.push(vec![String::new(); self.headers.len()]);
        self.page = self.rows.len().saturating_sub(1) / PAGE_ROWS;
        self.reload(w, cx);
    }
    fn add_column(&mut self, w: &mut Window, cx: &mut App) {
        if self.composing(w, cx) {
            return;
        }
        self.commit(cx);
        if self.headers.len() >= MAX_COLUMNS
            || self.rows.len().saturating_mul(self.headers.len() + 1) > MAX_CELLS
        {
            self.error = "字段或数据量超过编辑器上限，现有数据未更改".into();
            return;
        }
        self.headers
            .push(input(&format!("字段{}", self.headers.len() + 1), w, cx));
        for row in &mut self.rows {
            row.push(String::new());
        }
        self.reload(w, cx);
    }
}
impl JoyWord {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn data_grid_form(
        &mut self,
        title: &'static str,
        command: &'static str,
        headers: Vec<String>,
        mut rows: Vec<Vec<String>>,
        editable_headers: bool,
        parse: impl Fn(&[String], &[Vec<String>]) -> Result<Value, String> + 'static,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.format_ready(command) {
            return;
        }
        if headers.is_empty()
            || rows.iter().any(|row| row.len() > headers.len())
            || headers.len() > MAX_COLUMNS
            || rows.len().saturating_mul(headers.len()) > MAX_CELLS
        {
            self.shell.notice = Some("数据量超过表格编辑器上限，现有数据未更改".into());
            cx.notify();
            return;
        }
        if rows.is_empty() {
            rows.push(vec![String::new(); headers.len()]);
        }
        for row in &mut rows {
            row.resize(headers.len(), String::new());
        }
        let draft = cx.new(|cx| {
            let mut draft = Draft {
                headers: headers.iter().map(|v| input(v, w, cx)).collect(),
                rows,
                page: 0,
                editors: Vec::new(),
                error: String::new(),
            };
            draft.reload(w, cx);
            draft
        });
        let target = self.ribbon_target();
        let owner = cx.weak_entity();
        let parse = Rc::new(parse);
        w.open_dialog(cx, move |d, w, cx| {
            let state = draft.read(cx);
            let start = state.page * PAGE_ROWS;
            let mut heading = div().flex().gap_1().child(div().w(px(48.)).child("行"));
            for (col, header) in state.headers.iter().enumerate() {
                let mut cell = div().flex().flex_col().w(px(142.)).flex_shrink_0().child(
                    Input::new(header)
                        .small()
                        .disabled(!editable_headers)
                        .aria_label(format!("第{}列名称", col + 1)),
                );
                if editable_headers {
                    let grid = draft.clone();
                    let grid2 = draft.clone();
                    cell = cell.child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                Button::new(("grid-column-left", col))
                                    .ghost()
                                    .xsmall()
                                    .label("左移")
                                    .disabled(col == 0 || (command == "chart.editData" && col == 1))
                                    .on_click(move |_, w, cx| {
                                        grid.update(cx, |s, cx| {
                                            if s.composing(w, cx) {
                                                return;
                                            }
                                            s.commit(cx);
                                            if col > 0 && col < s.headers.len() {
                                                s.headers.swap(col, col - 1);
                                                for row in &mut s.rows {
                                                    row.swap(col, col - 1);
                                                }
                                                s.reload(w, cx);
                                                cx.notify();
                                            }
                                        })
                                    }),
                            )
                            .child(
                                Button::new(("grid-column-remove", col))
                                    .ghost()
                                    .xsmall()
                                    .label("删除字段")
                                    .disabled(
                                        state.headers.len() == 1
                                            || (command == "chart.editData" && col == 0),
                                    )
                                    .on_click(move |_, w, cx| {
                                        grid2.update(cx, |s, cx| {
                                            if s.composing(w, cx) {
                                                return;
                                            }
                                            s.commit(cx);
                                            if s.headers.len() > 1 && col < s.headers.len() {
                                                s.headers.remove(col);
                                                for row in &mut s.rows {
                                                    row.remove(col);
                                                }
                                                s.reload(w, cx);
                                                cx.notify();
                                            }
                                        })
                                    }),
                            ),
                    );
                }
                heading = heading.child(cell);
            }
            let mut grid = div()
                .id("editable-data-grid")
                .flex()
                .flex_col()
                .gap_2()
                .max_h(px(
                    (f32::from(w.viewport_size().height) - 280.).clamp(100., 390.)
                ))
                .overflow_x_scroll()
                .overflow_y_scroll()
                .child(heading);
            for (offset, row) in state.editors.iter().enumerate() {
                let row_index = start + offset;
                let mut line = div().flex().gap_1().child(
                    div()
                        .w(px(48.))
                        .flex_shrink_0()
                        .child((row_index + 1).to_string()),
                );
                for (col, value) in row.iter().enumerate() {
                    let name = state.headers[col].read(cx).value().to_string();
                    line = line.child(div().w(px(142.)).flex_shrink_0().child(
                        Textarea::new(value).h(px(42.)).aria_label(format!(
                            "第{}行，{}",
                            row_index + 1,
                            name
                        )),
                    ));
                }
                let draft = draft.clone();
                grid = grid.child(
                    line.child(
                        Button::new(("grid-remove-row", row_index))
                            .ghost()
                            .xsmall()
                            .label("删除行")
                            .on_click(move |_, w, cx| {
                                draft.update(cx, |s, cx| {
                                    if s.composing(w, cx) {
                                        return;
                                    }
                                    s.commit(cx);
                                    if row_index < s.rows.len() {
                                        s.rows.remove(row_index);
                                        s.reload(w, cx);
                                        cx.notify();
                                    }
                                })
                            }),
                    ),
                );
            }
            let (previous, next, add_row, add_column, apply_grid) = (
                draft.clone(),
                draft.clone(),
                draft.clone(),
                draft.clone(),
                draft.clone(),
            );
            let (cancel, close, owner) = (owner.clone(), owner.clone(), owner.clone());
            let (target, parse) = (target.clone(), parse.clone());
            let rows_count = state.rows.len();
            let page = state.page;
            let buttons = div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    Button::new("grid-prev")
                        .ghost()
                        .small()
                        .label("上一页")
                        .disabled(page == 0)
                        .on_click(move |_, w, cx| {
                            previous.update(cx, |s, cx| {
                                if s.composing(w, cx) {
                                    return;
                                }
                                s.commit(cx);
                                s.page = s.page.saturating_sub(1);
                                s.reload(w, cx);
                                cx.notify();
                            })
                        }),
                )
                .child(div().child(format!("第{}页，共{}条", page + 1, rows_count)))
                .child(
                    Button::new("grid-next")
                        .ghost()
                        .small()
                        .label("下一页")
                        .disabled((page + 1) * PAGE_ROWS >= rows_count)
                        .on_click(move |_, w, cx| {
                            next.update(cx, |s, cx| {
                                if s.composing(w, cx) {
                                    return;
                                }
                                s.commit(cx);
                                s.page += 1;
                                s.reload(w, cx);
                                cx.notify();
                            })
                        }),
                )
                .child(
                    Button::new("grid-add-row")
                        .small()
                        .label("添加行")
                        .on_click(move |_, w, cx| {
                            add_row.update(cx, |s, cx| {
                                s.add_row(w, cx);
                                cx.notify();
                            })
                        }),
                )
                .when(editable_headers, |row| {
                    row.child(
                        Button::new("grid-add-column")
                            .small()
                            .label("添加字段")
                            .on_click(move |_, w, cx| {
                                add_column.update(cx, |s, cx| {
                                    s.add_column(w, cx);
                                    cx.notify();
                                })
                            }),
                    )
                });
            d.title(title)
                .width(px((f32::from(w.viewport_size().width) - 48.).min(860.)))
                .child(
                    div()
                        .text_sm()
                        .child("在表格中填写数据；确定后应用，取消保留原值。空数值表示缺失数据。"),
                )
                .child(grid)
                .child(buttons)
                .child(div().text_color(rgb(0xc42b1c)).child(state.error.clone()))
                .on_close(move |_, w, cx| {
                    let _ = close.update(cx, |v, cx| w.focus(&v.focus, cx));
                })
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("cancel-grid").ghost().label("取消").on_click(
                            move |_, w, cx| {
                                w.close_dialog(cx);
                                let _ = cancel.update(cx, |v, cx| w.focus(&v.focus, cx));
                            },
                        ))
                        .child(Button::new("apply-grid").primary().label("确定").on_click(
                            move |_, w, cx| {
                                if apply_grid.update(cx, |s, cx| s.composing(w, cx)) {
                                    return;
                                }
                                let values = apply_grid.update(cx, |s, cx| {
                                    s.commit(cx);
                                    (
                                        s.headers
                                            .iter()
                                            .map(|h| h.read(cx).value().to_string())
                                            .collect::<Vec<_>>(),
                                        s.rows.clone(),
                                    )
                                });
                                let _ = owner.update(cx, |v, cx| {
                                    let result = if v.ribbon_target() != target {
                                        Err("文档或选区已变化，请重新打开".into())
                                    } else {
                                        parse(&values.0, &values.1)
                                            .and_then(|args| v.try_edit(command, &args, w, cx))
                                    };
                                    match result {
                                        Ok(_) => {
                                            w.close_dialog(cx);
                                            w.focus(&v.focus, cx);
                                        }
                                        Err(e) => apply_grid.update(cx, |s, cx| {
                                            s.error = e;
                                            cx.notify();
                                        }),
                                    }
                                });
                            },
                        )),
                )
        });
    }
}
