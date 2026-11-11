//! Structured object properties and the timeline's native canvas gestures.
use crate::{
    desktop::{After, Desktop, Pending},
    objects::ObjectKey,
};
use gpui_kit::*;
use joysheet_engine::model::{Anchor, DrawingObject};
use serde_json::json;

pub(crate) fn iso_date(wb: &joysheet_engine::model::Workbook, serial: i32) -> String {
    let d = joysheet_engine::core::date::datetime_from_serial(wb.date_system, serial as f64).unwrap_or_default();
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

pub(crate) fn summary(object: Option<&DrawingObject>) -> serde_json::Value {
    match object {
        Some(DrawingObject::Embedded { name, mime, data }) => json!({"Embedded":{"name":name,"mime":mime,"bytes":data.len()}}),
        other => serde_json::to_value(other).unwrap_or(serde_json::Value::Null),
    }
}

pub(crate) fn choices(id: &str, key: &str) -> Vec<(&'static str, &'static str)> {
    match key {
        "layout" if matches!(id, "insert.smartArt" | "object.edit") => {
            vec![("process", "流程"), ("list", "列表"), ("hierarchy", "层次结构"), ("cycle", "循环")]
        }
        "warp" => vec![("textNoShape", "无转换"), ("textArchUp", "上弧形"), ("textWave1", "波形")],
        "level" => vec![("years", "年"), ("quarters", "季度"), ("months", "月"), ("days", "日")],
        _ => crate::advanced::choices(key),
    }
}
pub(crate) struct TimelineDrag {
    pub key: ObjectKey,
    pub first: (i32, i32),
    pub last: (i32, i32),
    pub revision: u64,
}
impl Desktop {
    pub(crate) fn object_choices(&self, id: &str, key: &str, cx: &App) -> Vec<(String, String)> {
        if id == "insert.timeline"
            && let Some(view) = self.view()
        {
            if key == "pivot" {
                return view.workbook.sheets.iter().flat_map(|s| s.pivots.iter()).map(|p| (p.name.clone(), p.name.clone())).collect();
            }
            if key == "column" {
                let selected =
                    self.command_fields.iter().find(|f| f.key == "pivot").map(|f| f.value.read(cx).value().to_string()).unwrap_or_default();
                let mut fields = Vec::new();
                for pt in view.workbook.sheets.iter().flat_map(|s| s.pivots.iter()).filter(|p| selected.is_empty() || p.name == selected) {
                    if let Ok(src) = joysheet_engine::pivot::read_source(&view.workbook, &pt.source, view.sheet) {
                        for (i, h) in src.headers.iter().enumerate() {
                            if src.dates.get(i) == Some(&true) && !fields.iter().any(|(name, _)| name == h) {
                                fields.push((h.clone(), h.clone()));
                            }
                        }
                    }
                }
                return fields;
            }
        }
        choices(id, key).into_iter().map(|(a, b)| (a.into(), b.into())).collect()
    }
    pub(crate) fn pick_object_file(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = self.selected_command.clone() else { return };
        let target = self.view().map(|v| (v.doc, v.sheet));
        let name = self
            .selected_object
            .as_ref()
            .and_then(|o| self.view()?.sheet()?.shapes.iter().find(|s| s.id == o.id))
            .map(|s| s.text.clone())
            .unwrap_or("Object.bin".into());
        let values = self.form_params(cx).unwrap_or_else(|_| json!({}));
        cx.spawn_in(w, async move |this, cx| {
            let path = match command.as_str() {
                "object.extract" => joysheet_platform::pick_extracted(&name).await,
                "insert.picture" => joysheet_platform::pick_picture().await,
                _ => joysheet_platform::pick_embedded().await,
            };
            if let Some(path) = path {
                let _ = this.update_in(cx, |this, w, cx| {
                    if this.selected_command.as_deref() != Some(&command) || this.view().map(|v| (v.doc, v.sheet)) != target {
                        return;
                    }
                    let mut values = values;
                    values["path"] = json!(path.to_string_lossy());
                    this.make_command_fields(&values, w, cx);
                    this.params.update(cx, |s, cx| s.set_value(values.to_string(), w, cx));
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(crate) fn click_timeline(
        &mut self,
        key: &ObjectKey,
        anchor: &Anchor,
        x: f32,
        y: f32,
        extend: bool,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(view) = self.view() else { return false };
        let Some(object) = view.sheet().and_then(|s| s.shapes.iter().find(|s| s.id == key.id)).and_then(|s| s.format.object.as_ref()) else {
            return false;
        };
        let DrawingObject::Timeline { selection, .. } = object else { return false };
        let Some(geo) = self.geometry.as_ref() else { return false };
        let b = crate::objects::rect(anchor, &self.layout, geo, view.sheet().and_then(|s| s.freeze).unwrap_or((0, 0)));
        let k = self.layout.zoom as f32;
        let lx = (x - f32::from(b.origin.x)) / k;
        let ly = (y - f32::from(b.origin.y)) / k;
        if ly < 32. && lx > anchor.width - 32. {
            self.selected_object = Some(key.clone());
            self.issue("timeline.filter", json!({"id":key.id,"clear":true}), After::None, cx);
            return true;
        }
        if (28. ..58.).contains(&ly) && lx > anchor.width - 80. {
            self.selected_object = Some(key.clone());
            self.open_command("timeline.level", w, cx);
            return true;
        }
        if ly > anchor.height - 25. && (lx < 32. || lx > anchor.width - 32.) {
            self.issue("timeline.scroll", json!({"id":key.id,"delta":if lx<32.{-6}else{6}}), After::None, cx);
            return true;
        }
        if ly < anchor.height - 52. || ly > anchor.height - 27. || lx < 8. || lx > anchor.width - 8. {
            return false;
        }
        let periods = joysheet_engine::drawing_scene::timeline_periods(&view.workbook, view.sheet, object);
        let i = ((lx - 8.) / (anchor.width - 16.) * periods.len() as f32).floor() as usize;
        let Some(&(a, b, _)) = periods.get(i) else { return false };
        let first = if extend { selection.unwrap_or((a, b)) } else { (a, b) };
        self.timeline_drag = Some(TimelineDrag { key: key.clone(), first, last: (a, b), revision: view.revision });
        self.focus_grid(w, cx);
        true
    }
    pub(crate) fn move_timeline(&mut self, x: f32, cx: &mut Context<Self>) -> bool {
        let Some(d) = &self.timeline_drag else { return false };
        let Some(view) = self.view() else { return true };
        let Some(geo) = &self.geometry else { return true };
        let Some(shape) = view.sheet().and_then(|s| s.shapes.iter().find(|s| s.id == d.key.id)) else { return true };
        let Some(object) = &shape.format.object else { return true };
        let periods = joysheet_engine::drawing_scene::timeline_periods(&view.workbook, view.sheet, object);
        let b = crate::objects::rect(&shape.anchor, &self.layout, geo, (0, 0));
        let lx = (x - f32::from(b.origin.x)) / self.layout.zoom as f32;
        let i = (((lx - 8.) / (shape.anchor.width - 16.)).clamp(0., 0.9999) * periods.len() as f32) as usize;
        if let Some(&(a, b, _)) = periods.get(i)
            && let Some(d) = &mut self.timeline_drag
        {
            d.last = (a, b);
            cx.notify();
        }
        true
    }
    pub(crate) fn finish_timeline(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.timeline_drag.take() else { return };
        if !self.view().is_some_and(|v| v.doc == d.key.doc && v.sheet == d.key.sheet) {
            return;
        }
        let p = json!({"id":d.key.id,"start":d.first.0.min(d.last.0),"end":d.first.1.max(d.last.1)});
        match self.engine.submit_guarded("timeline.filter", p, self.target(), Some(d.revision)) {
            Ok(ticket) => self.pending.push(Pending { ticket, after: After::None, commit: false, clipboard: false }),
            Err(e) => self.alert(e),
        }
        cx.notify();
    }
}
