use crate::{
    desktop::{After, Desktop, Pending},
    objects::{ObjectKey, entries, rect},
};
use gpui_kit::component::button::Button;
use gpui_kit::{prelude::*, *};
use joysheet_engine::model::Anchor;
use serde_json::{Value, json};
impl Desktop {
    pub fn object_selection(&self) -> Vec<(ObjectKey, Anchor)> {
        self.view()
            .map(entries)
            .unwrap_or_default()
            .into_iter()
            .filter(|(key, _)| self.selected_objects.contains(key) || self.selected_object.as_ref() == Some(key))
            .collect()
    }
    pub fn toggle_object(&mut self, key: ObjectKey, additive: bool, cx: &mut Context<Self>) {
        if self.draft.is_some() || self.confirm_close || self.trust_request.is_some() || self.validation_prompt.is_some() {
            return;
        }
        let existing = self.object_selection();
        self.selected_objects = if additive { existing.into_iter().map(|(k, _)| k).collect() } else { Vec::new() };
        if let Some(i) = self.selected_objects.iter().position(|k| *k == key) {
            self.selected_objects.remove(i);
        } else if self.selected_objects.len() < 256 {
            self.selected_objects.push(key);
        }
        self.selected_object = self.selected_objects.last().cloned();
        cx.notify();
    }
    pub fn object_selection_bounds(&self) -> Vec<(Bounds<Pixels>, Bounds<Pixels>)> {
        let (Some(view), Some(geo)) = (self.view(), self.geometry.as_ref()) else { return Vec::new() };
        self.object_selection()
            .into_iter()
            .filter_map(|(key, mut anchor)| {
                if let Some(d) = &self.object_drag {
                    if d.resize {
                        anchor.width = (anchor.width + d.delta.0).max(1.);
                        anchor.height = (anchor.height + d.delta.1).max(1.);
                    } else {
                        anchor.dx += d.delta.0;
                        anchor.dy += d.delta.1;
                    }
                }
                let freeze = view.sheet()?.freeze.unwrap_or((0, 0));
                let b = if self.selected_object.as_ref() == Some(&key) {
                    self.selected_object_bounds()?
                } else {
                    rect(&anchor, &self.layout, geo, freeze)
                };
                Some((b, crate::objects::object_clip(&anchor, &self.layout, geo, freeze, point(px(0.), px(0.)))))
            })
            .collect()
    }
    pub fn object_batch(&mut self, mut operations: Vec<Value>, revision: Option<u64>, cx: &mut Context<Self>) {
        if operations.is_empty()
            || self.draft.is_some()
            || self.preview.is_some()
            || self.confirm_close
            || self.trust_request.is_some()
            || self.validation_prompt.is_some()
        {
            return;
        }
        let Some(v) = self.view() else { return };
        if let Some(geo) = &self.geometry {
            let entries = entries(v);
            for operation in &mut operations {
                if operation["command"] != "object.move" {
                    continue;
                }
                let Some((_, a)) = entries.iter().find(|(key, _)| {
                    Some(key.id as u64) == operation["params"]["id"].as_u64() && Some(key.kind.as_str()) == operation["params"]["kind"].as_str()
                }) else {
                    continue;
                };
                let cell = operation["params"]["at"].as_str().and_then(joysheet_engine::core::CellRef::parse).unwrap_or(a.cell);
                let x = (geo.cols.start(cell.col) + operation["params"]["dx"].as_f64().unwrap_or(a.dx as f64)).max(0.);
                let y = (geo.rows.start(cell.row) + operation["params"]["dy"].as_f64().unwrap_or(a.dy as f64)).max(0.);
                let at = joysheet_engine::core::CellRef::new(geo.rows.at(y), geo.cols.at(x));
                operation["params"]["at"] = json!(at.a1());
                operation["params"]["dx"] = json!(x - geo.cols.start(at.col));
                operation["params"]["dy"] = json!(y - geo.rows.start(at.row));
            }
        }
        let p = json!({"sheet":v.sheet,"operations":operations});
        match self.engine.submit_guarded("__joysheet.objectBatch", p, self.target(), revision.or(Some(v.revision))) {
            Ok(ticket) => self.pending.push(Pending { ticket, after: After::None, commit: false, clipboard: false }),
            Err(error) => self.alert(error),
        }
        cx.notify();
    }
    pub fn nudge_objects(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        let operations = self
            .object_selection()
            .into_iter()
            .map(|(k, a)| json!({"command":"object.move","params":{"kind":k.kind,"id":k.id,"dx":a.dx+dx,"dy":a.dy+dy}}))
            .collect();
        self.object_batch(operations, None, cx);
    }
    pub fn align_objects(&mut self, how: &str, cx: &mut Context<Self>) {
        let objects = self.object_selection();
        if objects.len() < 2 {
            self.alert("请选择至少两个对象 / Select at least two objects");
            cx.notify();
            return;
        }
        let Some(g) = self.geometry.as_ref() else { return };
        let mut positions: Vec<_> = objects
            .into_iter()
            .map(|(k, a)| {
                let x = g.cols.start(a.cell.col) + a.dx as f64;
                let y = g.rows.start(a.cell.row) + a.dy as f64;
                (k, a, x, y)
            })
            .collect();
        let left = positions.iter().map(|p| p.2).fold(f64::INFINITY, f64::min);
        let right = positions.iter().map(|p| p.2 + p.1.width as f64).fold(f64::NEG_INFINITY, f64::max);
        let top = positions.iter().map(|p| p.3).fold(f64::INFINITY, f64::min);
        let bottom = positions.iter().map(|p| p.3 + p.1.height as f64).fold(f64::NEG_INFINITY, f64::max);
        let horizontal = how == "distributeH";
        if how.starts_with("distribute") {
            positions.sort_by(|a, b| if horizontal { a.2.total_cmp(&b.2) } else { a.3.total_cmp(&b.3) });
        }
        let total = positions.iter().map(|p| if horizontal { p.1.width as f64 } else { p.1.height as f64 }).sum::<f64>();
        let gap = ((if horizontal { right - left } else { bottom - top }) - total) / (positions.len() - 1) as f64;
        let mut cursor = if horizontal { left } else { top };
        let operations = positions
            .into_iter()
            .map(|(key, a, x, y)| {
                let (nx, ny) = match how {
                    "left" => (left, y),
                    "right" => (right - a.width as f64, y),
                    "center" => ((left + right - a.width as f64) / 2., y),
                    "top" => (x, top),
                    "bottom" => (x, bottom - a.height as f64),
                    "middle" => (x, (top + bottom - a.height as f64) / 2.),
                    "distributeH" => {
                        let p = (cursor, y);
                        cursor += a.width as f64 + gap;
                        p
                    }
                    "distributeV" => {
                        let p = (x, cursor);
                        cursor += a.height as f64 + gap;
                        p
                    }
                    _ => (x, y),
                };
                json!({"command":"object.move","params":{"kind":key.kind,"id":key.id,"dx":a.dx+(nx-x) as f32,"dy":a.dy+(ny-y) as f32}})
            })
            .collect();
        self.object_batch(operations, None, cx);
    }
    pub fn cycle_object(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let objects = self.view().map(entries).unwrap_or_default();
        if objects.is_empty() {
            return;
        }
        let i = objects.iter().position(|(k, _)| self.selected_object.as_ref() == Some(k)).unwrap_or(0);
        let next = if backwards { (i + objects.len() - 1) % objects.len() } else { (i + 1) % objects.len() };
        self.toggle_object(objects[next].0.clone(), false, cx);
    }
    pub fn object_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let selected = self.object_selection();
        let mut panel = div()
            .id("object-selection-panel")
            .absolute()
            .right_0()
            .top(px(54.))
            .bottom_0()
            .w(px(360.))
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x20332a))
            .occlude()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .role(Role::Dialog)
            .aria_label("Object selection and alignment")
            .child(Button::new("close-object-panel").label("对象选择与排列 · 关闭 ×").on_click(cx.listener(|this, _, w, cx| {
                this.daily_group = 0;
                this.focus_grid(w, cx);
                cx.notify();
            })))
            .child(format!("已选 {} 项；点击切换 / Click to select multiple", selected.len()));
        for (key, _) in self.view().map(entries).unwrap_or_default() {
            let checked = selected.iter().any(|(k, _)| *k == key);
            let id = format!("object-select-{}-{}", key.kind, key.id);
            panel = panel.child(
                Button::new(SharedString::from(id)).label(format!("{} {} {}", if checked { "☑" } else { "□" }, key.kind, key.id)).on_click(
                    cx.listener(move |this, _, w, cx| {
                        this.toggle_object(key.clone(), true, cx);
                        this.focus_grid(w, cx);
                    }),
                ),
            );
        }
        for (how, label) in [
            ("left", "左对齐 / Left"),
            ("center", "水平居中 / Center"),
            ("right", "右对齐 / Right"),
            ("top", "顶端对齐 / Top"),
            ("middle", "垂直居中 / Middle"),
            ("bottom", "底端对齐 / Bottom"),
            ("distributeH", "水平分布 / Distribute horizontally"),
            ("distributeV", "垂直分布 / Distribute vertically"),
        ] {
            panel = panel.child(
                Button::new(SharedString::from(format!("objects-{how}")))
                    .label(label)
                    .relative()
                    .child(self.advanced_probe(format!("objects-{how}"), cx))
                    .on_click(cx.listener(move |this, _, _, cx| this.align_objects(how, cx))),
            );
        }
        panel.child(
            Button::new("delete-selected-objects")
                .label("删除所选 / Delete selected")
                .on_click(cx.listener(|this, _, _, cx| this.delete_selected_object(cx))),
        )
    }
}
