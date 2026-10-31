use crate::desktop::{Desktop, Pending};
use gpui_kit::{prelude::*, *};
use joysheet_app::geometry::{COL_HEADER, ROW_HEADER};
use serde_json::json;

#[derive(Clone, Copy)]
pub struct ScrollDrag {
    pub horizontal: bool,
    pub offset: f32,
    pub travel: f32,
    pub max: f64,
}
pub struct ScrollAxis {
    pub start: f32,
    pub thumb: f32,
    pub travel: f32,
    pub max: f64,
}
pub fn scroll_axis(viewport: f32, scroll: f64, used: f64, limit: f64) -> ScrollAxis {
    let viewport = viewport.max(1.);
    let extent = used.max(viewport as f64 * 2.).max(scroll + viewport as f64).min(limit).max(viewport as f64);
    let max = (extent - viewport as f64).max(0.);
    let thumb = (viewport as f64 * viewport as f64 / extent).clamp(24f64.min(viewport as f64), viewport as f64) as f32;
    let travel = (viewport - thumb).max(0.);
    ScrollAxis { start: if max > 0. { (scroll / max).clamp(0., 1.) as f32 * travel } else { 0. }, thumb, travel, max }
}
pub fn axes(layout: &joysheet_app::geometry::GridLayout, geo: &joysheet_app::geometry::SheetGeometry) -> (ScrollAxis, ScrollAxis) {
    let end = geo.used_end;
    (
        scroll_axis(
            layout.width - ROW_HEADER,
            layout.scroll_x,
            geo.cols.start(end.col.saturating_add(2).min(joysheet_engine::core::MAX_COLS)) * layout.zoom,
            geo.cols.start(joysheet_engine::core::MAX_COLS) * layout.zoom,
        ),
        scroll_axis(
            layout.height - COL_HEADER,
            layout.scroll_y,
            geo.rows.start(end.row.saturating_add(2).min(joysheet_engine::core::MAX_ROWS)) * layout.zoom,
            geo.rows.start(joysheet_engine::core::MAX_ROWS) * layout.zoom,
        ),
    )
}
pub struct DimensionDrag {
    pub column: bool,
    pub index: u32,
    pub origin: f32,
    pub size: f32,
    pub delta: f32,
    pub boundary: f32,
    pub doc: u64,
    pub sheet: usize,
    pub revision: u64,
}
impl Desktop {
    pub fn begin_dimension_drag(&mut self, x: f32, y: f32, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let line = if y < COL_HEADER && x >= ROW_HEADER {
            self.layout.cols.iter().find(|l| (x - l.start - l.size).abs() <= 4.).map(|l| (true, *l, x))
        } else if x < ROW_HEADER && y >= COL_HEADER {
            self.layout.rows.iter().find(|l| (y - l.start - l.size).abs() <= 4.).map(|l| (false, *l, y))
        } else {
            None
        };
        let Some((column, line, origin)) = line else { return false };
        let Some(v) = self.view() else { return false };
        self.resize_drag = Some(DimensionDrag {
            column,
            index: line.index,
            origin,
            size: line.size / self.layout.zoom as f32,
            delta: 0.,
            boundary: line.start + line.size,
            doc: v.doc,
            sheet: v.sheet,
            revision: v.revision,
        });
        self.focus_grid(window, cx);
        cx.notify();
        true
    }
    pub fn finish_dimension_drag(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.resize_drag.take() else { return };
        if d.delta.abs() < 1. || !self.view().is_some_and(|v| v.doc == d.doc && v.sheet == d.sheet) {
            return;
        }
        let pixels = (d.size + d.delta / self.layout.zoom as f32).clamp(8., if d.column { 1800. } else { 546. });
        let (id, p) = if d.column {
            let name = joysheet_engine::core::col_to_letters(d.index);
            ("home.columnWidth", json!({"cols":format!("{name}:{name}"),"width":pixels}))
        } else {
            ("home.rowHeight", json!({"rows":format!("{0}:{0}",d.index+1),"height":pixels}))
        };
        match self.engine.submit_guarded(id, p, self.target(), Some(d.revision)) {
            Ok(ticket) => self.pending.push(Pending { ticket, after: Default::default(), commit: false, clipboard: false }),
            Err(e) => self.alert(e),
        }
        cx.notify();
    }
    pub fn auto_scroll_drag(&mut self, cx: &mut Context<Self>) {
        if self.validation_prompt.is_some() || self.confirm_close || self.trust_request.is_some() || self.palette_open || self.preview.is_some() {
            self.edge_drag = None;
            return;
        }
        if self.drag_anchor.is_none() && self.reference_pick.is_none() {
            self.edge_drag = None;
            return;
        }
        let Some((x, y)) = self.edge_drag else { return };
        let dx = if x < ROW_HEADER + 16. {
            -12.
        } else if x > self.layout.width - 16. {
            12.
        } else {
            0.
        };
        let dy = if y < COL_HEADER + 16. {
            -16.
        } else if y > self.layout.height - 16. {
            16.
        } else {
            0.
        };
        if dx != 0. || dy != 0. {
            self.scroll.0 = (self.scroll.0 + dx).max(0.);
            self.scroll.1 = (self.scroll.1 + dy).max(0.);
            if self.reference_pick.is_none()
                && let (Some(anchor), Some(cell)) = (
                    self.drag_anchor,
                    self.layout.hit(x.clamp(ROW_HEADER + 1., self.layout.width - 1.), y.clamp(COL_HEADER + 1., self.layout.height - 1.)),
                )
            {
                let range = joysheet_engine::core::RangeRef::new(anchor, cell);
                if self.selection().current() != range {
                    self.local_selection = Some(joysheet_engine::Selection::range(range));
                    self.issue("selection.set", json!({"range":range.a1(),"active":anchor.a1()}), Default::default(), cx);
                }
            }
            cx.notify();
        }
    }
}
