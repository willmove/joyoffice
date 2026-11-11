//! Native grid ink capture; the engine owns the resulting editable/undoable shapes.
use crate::desktop::{Desktop, Pending};
use gpui_kit::{prelude::*, *};
use serde_json::json;

pub struct InkState {
    pub tool: &'static str,
    pub color: String,
    pub width: f32,
    pub points: Vec<[f64; 2]>,
    pub target: Option<(u64, usize, u64)>,
}
impl Default for InkState {
    fn default() -> Self {
        Self { tool: "", color: "#202020".into(), width: 2., points: Vec::new(), target: None }
    }
}
impl Desktop {
    fn ink_point(&self, x: f32, y: f32) -> Option<[f64; 2]> {
        let geo = self.geometry.as_ref()?;
        let cell = self.layout.hit(x, y)?;
        let freeze = self.view()?.sheet()?.freeze.unwrap_or((0, 0));
        let rect = self.layout.cell_rect(cell, geo, freeze);
        Some([geo.cols.start(cell.col) + (x - rect.x) as f64 / self.layout.zoom, geo.rows.start(cell.row) + (y - rect.y) as f64 / self.layout.zoom])
    }
    pub(crate) fn begin_ink(&mut self, x: f32, y: f32, w: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.ink.tool.is_empty() || self.draft.is_some() {
            return false;
        }
        let Some(point) = self.ink_point(x, y) else {
            return false;
        };
        if self.ink.tool == "eraser" {
            let hit = self.view().and_then(|v| {
                let s = v.sheet()?;
                let geo = self.geometry.as_ref()?;
                s.shapes
                    .iter()
                    .rev()
                    .filter(|s| s.kind == joysheet_engine::model::ShapeKind::Ink)
                    .find(|s| {
                        let rect = crate::objects::rect(&s.anchor, &self.layout, geo, sheet_freeze(v));
                        x >= f32::from(rect.origin.x)
                            && x <= f32::from(rect.bottom_right().x)
                            && y >= f32::from(rect.origin.y)
                            && y <= f32::from(rect.bottom_right().y)
                    })
                    .map(|s| s.id)
            });
            if let Some(id) = hit {
                self.issue("object.delete", json!({"kind":"shape","id":id}), Default::default(), cx);
            }
        } else if let Some(v) = self.view() {
            self.ink.target = Some((v.doc, v.sheet, v.revision));
            self.ink.points = vec![point];
        }
        self.focus_grid(w, cx);
        cx.notify();
        true
    }
    pub(crate) fn move_ink(&mut self, x: f32, y: f32, cx: &mut Context<Self>) -> bool {
        if self.ink.points.is_empty() {
            return false;
        }
        if let Some(point) = self.ink_point(x, y)
            && self.ink.points.len() < 20_000
            && self.ink.points.last().is_none_or(|p| (p[0] - point[0]).abs() + (p[1] - point[1]).abs() > 0.5)
        {
            self.ink.points.push(point);
            cx.notify();
        }
        true
    }
    pub(crate) fn finish_ink(&mut self, cx: &mut Context<Self>) {
        let points = std::mem::take(&mut self.ink.points);
        let Some((doc, sheet, revision)) = self.ink.target.take() else {
            return;
        };
        if points.len() < 2 || !self.view().is_some_and(|v| v.doc == doc && v.sheet == sheet) {
            return;
        }
        match self.engine.submit_guarded(
            "draw.stroke",
            json!({"points":points,"color":self.ink.color,"width":self.ink.width}),
            self.target(),
            Some(revision),
        ) {
            Ok(ticket) => self.pending.push(Pending { ticket, after: Default::default(), commit: false, clipboard: false }),
            Err(e) => self.alert(e),
        }
        cx.notify();
    }
}
fn sheet_freeze(v: &joysheet_app::WindowSnapshot) -> (u32, u32) {
    v.sheet().and_then(|s| s.freeze).unwrap_or((0, 0))
}
