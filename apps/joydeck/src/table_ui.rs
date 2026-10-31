//! Table interaction uses the same input handler and command history as text boxes.
use super::*;
use joydeck::{
    table::{self, Action},
    text::TextEdit,
};
#[derive(Clone)]
pub(super) struct Pick {
    pub id: String,
    pub anchor: (usize, usize),
    pub focus: (usize, usize),
}
#[derive(Clone)]
pub(super) struct Target {
    pub id: String,
    pub row: usize,
    pub col: usize,
}
pub(super) struct Resize {
    pub base: Object,
    pub row: bool,
    pub index: usize,
    pub start: f32,
    pub delta: f32,
    pub scale: f32,
}
impl Resize {
    pub fn result(&self) -> joydeck::document::Result<Object> {
        let mut o = self.base.clone();
        let t = o.table.as_mut().ok_or("调整大小的目标已不是表格")?;
        let sizes = if self.row {
            &mut t.rows
        } else {
            &mut t.columns
        };
        let size = sizes.get_mut(self.index).ok_or("调整大小的行列已不存在")?;
        *size = (*size + self.delta).clamp(24., 4096.);
        o.width = t.columns.iter().sum();
        o.height = t.rows.iter().sum();
        Ok(o)
    }
}
impl Editor {
    pub(super) fn paste_at_focus(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(w, cx);
        let e = KeyDownEvent {
            keystroke: Keystroke {
                key: "v".into(),
                modifiers: Modifiers {
                    platform: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            is_held: false,
            prefer_character_input: false,
        };
        if self.table_key(&e, w, cx) {
            return;
        }
        if self.text_edit.is_some() {
            self.text_key(&e, w, cx);
        } else {
            self.paste_media(w, cx);
        }
    }

    pub(super) fn cell_tab(&mut self, reverse: bool, w: &mut Window, cx: &mut Context<Self>) {
        let event = KeyDownEvent {
            keystroke: Keystroke {
                key: "tab".into(),
                modifiers: Modifiers {
                    shift: reverse,
                    ..Default::default()
                },
                ..Default::default()
            },
            is_held: false,
            prefer_character_input: false,
        };
        if !self.table_key(&event, w, cx) {
            cx.propagate();
        }
    }

    pub(super) fn table_target(&self) -> Option<(&Object, &table::Table)> {
        let p = self.table_pick.as_ref()?;
        let o = self
            .session
            .deck
            .slides
            .get(self.page)?
            .objects
            .iter()
            .find(|o| o.id == p.id)?;
        Some((o, o.table.as_ref()?))
    }
    pub(super) fn insert_table(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let (Ok(rows), Ok(cols)) = (
            self.table_fields[0].read(cx).value().parse::<usize>(),
            self.table_fields[1].read(cx).value().parse::<usize>(),
        ) else {
            self.message = "表格行列必须是整数".into();
            cx.notify();
            return;
        };
        match table::object(rows, cols) {
            Ok(mut o) => {
                o.x = self.session.deck.width * 0.075;
                o.y = self.session.deck.height * 0.2;
                if o.width > self.session.deck.width * 0.85 {
                    o.width = self.session.deck.width * 0.85;
                    table::resize(&mut o);
                }
                let before = self.session.deck.slides[self.page].objects.len();
                self.command(Command::AddObject(self.page, o), cx);
                if self.session.deck.slides[self.page].objects.len() == before {
                    return;
                }
                let Some(id) = self.session.deck.slides[self.page]
                    .objects
                    .last()
                    .map(|o| o.id.clone())
                else {
                    self.message = "插入表格后未找到目标对象".into();
                    cx.notify();
                    return;
                };
                if let Some(i) = self.session.deck.slides[self.page]
                    .objects
                    .iter()
                    .position(|o| o.id == id)
                {
                    self.select(i, w, cx);
                }
                self.table_pick = Some(Pick {
                    id,
                    anchor: (0, 0),
                    focus: (0, 0),
                });
                self.ribbon = "表格".into();
            }
            Err(e) => self.message = e,
        }
        cx.notify();
    }
    pub(super) fn begin_cell(
        &mut self,
        i: usize,
        r: usize,
        c: usize,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_text(cx);
        let o = &self.session.deck.slides[self.page].objects[i];
        if o.rotation != 0. {
            self.message = "旋转表格请先将旋转设为0后编辑文字".into();
            return;
        }
        let Some(t) = &o.table else { return };
        let Ok((r, c)) = t.owner(r, c) else { return };
        let Ok(virtual_o) = table::cell_object(&self.session.deck, self.page, o, r, c) else {
            return;
        };
        let mut edit = TextEdit::new(virtual_o);
        edit.typing = table::resolve(&self.session.deck, self.page, t, r, c).text;
        self.cell_edit = Some(Target {
            id: o.id.clone(),
            row: r,
            col: c,
        });
        self.table_pick = Some(Pick {
            id: o.id.clone(),
            anchor: (r, c),
            focus: (r, c),
        });
        self.gesture = None;
        self.text_edit = Some(edit);
        self.text_lines.clear();
        self.text_layout_key = None;
        self.visual_revision += 1;
        self.pending = None;
        self.scene_key = None;
        self.image = None;
        self.focus.focus(w, cx);
        cx.notify();
    }
    pub(super) fn table_down(
        &mut self,
        i: usize,
        p: (f32, f32),
        e: &MouseDownEvent,
        scale: f32,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if e.modifiers.alt {
            return false;
        }
        let o = self.session.deck.slides[self.page].objects[i].clone();
        let Some((r, c)) = table::hit(&o, p.0, p.1) else {
            return false;
        };
        if !self.selection.contains(&o.id) {
            self.select(i, w, cx);
        }
        let anchor = self
            .table_pick
            .as_ref()
            .filter(|s| s.id == o.id && e.modifiers.shift)
            .map_or((r, c), |s| s.anchor);
        self.table_pick = Some(Pick {
            id: o.id.clone(),
            anchor,
            focus: (r, c),
        });
        self.ribbon = "表格".into();
        if e.click_count >= 2 {
            self.begin_cell(i, r, c, w, cx);
        } else if o.rotation == 0. && !e.modifiers.shift {
            let Some(t) = o.table.as_ref() else {
                return false;
            };
            for row in [false, true] {
                let mut edge = if row { o.y } else { o.x };
                let sizes = if row { &t.rows } else { &t.columns };
                for (index, v) in sizes.iter().enumerate() {
                    edge += v;
                    let at = if row { p.1 } else { p.0 };
                    if (at - edge).abs() < 4. / scale {
                        self.table_resize = Some(Resize {
                            base: o.clone(),
                            row,
                            index,
                            start: at,
                            delta: 0.,
                            scale,
                        });
                        break;
                    }
                }
            }
            self.table_drag = self.table_resize.is_none();
        }
        self.focus.focus(w, cx);
        cx.notify();
        true
    }
    pub(super) fn table_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) -> bool {
        if let Some(g) = &mut self.table_resize {
            if e.dragging() {
                let at = if g.row {
                    f32::from(e.position.y) - self.canvas_origin.1
                } else {
                    f32::from(e.position.x) - self.canvas_origin.0
                };
                g.delta = at / g.scale - g.start;
                self.visual_revision += 1;
                self.pending = None;
                cx.notify();
            }
            return true;
        }
        if self.table_drag && e.dragging() {
            let scale = self.last_scale;
            let p = (
                (f32::from(e.position.x) - self.canvas_origin.0) / scale,
                (f32::from(e.position.y) - self.canvas_origin.1) / scale,
            );
            if let Some((o, _)) = self.table_target()
                && let Some(cell) = table::hit(o, p.0, p.1)
            {
                if let Some(pick) = &mut self.table_pick {
                    pick.focus = cell;
                }
                cx.notify();
            }
            return true;
        }
        false
    }
    pub(super) fn end_table_resize(&mut self, cx: &mut Context<Self>) {
        self.table_drag = false;
        if let Some(g) = self.table_resize.take() {
            if g.delta.abs() * g.scale >= 3. {
                let id = g.base.id.clone();
                let o = match g.result() {
                    Ok(o) => o,
                    Err(error) => {
                        self.message = error;
                        cx.notify();
                        return;
                    }
                };
                if let Some(i) = self.session.deck.slides[self.page]
                    .objects
                    .iter()
                    .position(|o| o.id == id)
                {
                    self.command(Command::ReplaceObject(self.page, i, o), cx);
                }
            }
            self.visual_revision += 1;
            self.pending = None;
            cx.notify();
        }
    }
    pub(super) fn table_action(&mut self, op: &str, cx: &mut Context<Self>) {
        self.finish_text(cx);
        let Some(p) = self.table_pick.clone() else {
            return;
        };
        let Some((_, t)) = self.table_target() else {
            return;
        };
        let (r, c) = p.focus;
        let range = match t.selection_range(p.anchor, p.focus) {
            Ok(range) => range,
            Err(error) => {
                self.message = error;
                cx.notify();
                return;
            }
        };
        let Some(cell) = t.index(r, c).ok().and_then(|i| t.cells.get(i)) else {
            self.message = "所选单元格已不存在，请重新选择".into();
            cx.notify();
            return;
        };
        let get = |i: usize| self.table_fields[i].read(cx).value().to_string();
        let mut style = t.style.clone();
        let a = match op {
            "row+" => Action::InsertRow(r + 1),
            "col+" => Action::InsertColumn(c + 1),
            "row-" => Action::DeleteRow(r),
            "col-" => Action::DeleteColumn(c),
            "merge" => Action::Merge(range),
            "split" => Action::Split(r, c),
            "rows=" => Action::DistributeRows,
            "cols=" => Action::DistributeColumns,
            "height" => Action::RowHeight(r, get(2).parse().unwrap_or(0.)),
            "width" => Action::ColumnWidth(c, get(3).parse().unwrap_or(0.)),
            "header" => {
                style.header = !style.header;
                Action::Style(style)
            }
            "banded" => {
                style.banded = !style.banded;
                Action::Style(style)
            }
            "style" => {
                style.padding = get(4).parse().unwrap_or(-1.);
                style.border_width = get(5).parse().unwrap_or(-1.);
                style.border = get(6);
                style.size = get(7).parse().unwrap_or(0.);
                Action::Style(style)
            }
            "left" | "center" | "right" => {
                let mut s = cell.style.clone();
                s.align = Some(op.into());
                Action::CellStyle(range, s)
            }
            "fill" => {
                let mut s = cell.style.clone();
                s.fill = Some(get(8));
                Action::CellStyle(range, s)
            }
            "inherit" => Action::CellStyle(range, table::CellStyle::default()),
            _ => return,
        };
        self.command(Command::Table(self.page, p.id, a), cx);
        if let Some((_, t)) = self.table_target() {
            let focus = t.owner(
                r.min(t.rows.len().saturating_sub(1)),
                c.min(t.columns.len().saturating_sub(1)),
            );
            if let (Ok(focus), Some(p)) = (focus, self.table_pick.as_mut()) {
                p.focus = focus;
                p.anchor = focus;
            } else {
                self.table_pick = None;
            }
        }
        cx.notify();
    }
    pub(super) fn table_key(
        &mut self,
        e: &KeyDownEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.focus.is_focused(w) || self.table_target().is_none() {
            return false;
        }
        let key = e.keystroke.key.as_str();
        let mods = e.keystroke.modifiers;
        if self.text_edit.as_ref().is_some_and(|t| t.marked.is_some()) {
            return false;
        }
        if (mods.platform || mods.control) && key == "v" {
            if cx
                .read_from_clipboard()
                .is_some_and(|i| i.metadata().is_some())
            {
                return false;
            }
            if let Some(s) = cx.read_from_clipboard().and_then(|i| i.text())
                && (s.contains('\t') || self.text_edit.is_none())
            {
                self.finish_text(cx);
                let Some(p) = self.table_pick.clone() else {
                    return false;
                };
                self.command(
                    Command::Table(self.page, p.id, Action::Paste(p.focus.0, p.focus.1, s)),
                    cx,
                );
                cx.stop_propagation();
                return true;
            }
            return false;
        }
        if key == "tab"
            || (self.text_edit.is_none()
                && matches!(key, "left" | "right" | "up" | "down" | "enter" | "f2"))
        {
            let editing = self.text_edit.is_some();
            self.finish_text(cx);
            let Some(p) = self.table_pick.clone() else {
                return false;
            };
            let (r, c) = p.focus;
            if matches!(key, "enter" | "f2") {
                let Some(i) = self.session.deck.slides[self.page]
                    .objects
                    .iter()
                    .position(|o| o.id == p.id)
                else {
                    self.table_pick = None;
                    return false;
                };
                self.begin_cell(i, r, c, w, cx);
            } else {
                let Some((_, t)) = self.table_target() else {
                    return false;
                };
                let Some(cell) = t.index(r, c).ok().and_then(|i| t.cells.get(i)) else {
                    self.table_pick = None;
                    return false;
                };
                let next = if key == "tab" {
                    t.next(r, c, mods.shift)
                } else {
                    let (nr, nc) = match key {
                        "left" => (r, c.saturating_sub(1)),
                        "right" => (r, (c + cell.span[1]).min(t.columns.len() - 1)),
                        "up" => (r.saturating_sub(1), c),
                        _ => ((r + cell.span[0]).min(t.rows.len() - 1), c),
                    };
                    t.owner(nr, nc).ok()
                };
                let next = if next.is_none() && !mods.shift && key == "tab" {
                    let n = t.rows.len();
                    self.command(
                        Command::Table(self.page, p.id.clone(), Action::InsertRow(n)),
                        cx,
                    );
                    self.table_target().and_then(|(_, t)| t.owner(n, 0).ok())
                } else {
                    next
                };
                if let Some(next) = next {
                    let Some(p) = self.table_pick.as_mut() else {
                        return false;
                    };
                    p.focus = next;
                    if !mods.shift || key == "tab" {
                        p.anchor = next;
                    }
                    if editing || key == "tab" {
                        let id = p.id.clone();
                        let Some(i) = self.session.deck.slides[self.page]
                            .objects
                            .iter()
                            .position(|o| o.id == id)
                        else {
                            self.table_pick = None;
                            return false;
                        };
                        self.begin_cell(i, next.0, next.1, w, cx);
                    }
                }
            }
            cx.stop_propagation();
            cx.notify();
            return true;
        }
        false
    }
    pub(super) fn suppress_active_text(&self, deck: &mut Deck) {
        if let Some(t) = &self.text_edit {
            for o in &mut deck.slides[self.page].objects {
                if let Some(target) = &self.cell_edit {
                    if o.id == target.id
                        && let Some(table) = &mut o.table
                        && let Ok(i) = table.index(target.row, target.col)
                    {
                        table.cells[i].text.clear();
                        table.cells[i].runs.clear();
                    }
                } else if o.id == t.object.id {
                    o.text.clear();
                    o.runs.clear();
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "table_ui_tests/tests.rs"]
mod tests;
