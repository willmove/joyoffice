//! Formula auditing is a view overlay. A reply is bound to the workbook revision queried.
use gpui_kit::*;
use joysheet_app::{
    WindowSnapshot,
    geometry::{COL_HEADER, GridLayout, ROW_HEADER, SheetGeometry},
};
use joysheet_engine::core::{CellRef, RangeRef};
use serde_json::Value;

#[derive(Clone)]
pub(crate) struct TraceRequest {
    pub doc: u64,
    pub sheet: usize,
    pub revision: u64,
    pub cell: CellRef,
    pub dependents: bool,
}
#[derive(Clone)]
pub(crate) struct TraceArrow {
    pub request: TraceRequest,
    pub sheet_name: String,
    pub range: RangeRef,
}
impl crate::desktop::Desktop {
    pub(crate) fn trace_selected(&mut self, command: &str, cx: &mut Context<Self>) {
        let Some(view) = self.view() else { return };
        let (doc, sheet, revision) = (view.doc, view.sheet, view.revision);
        let root = self.selection().active;
        let dependents = command == "formulas.traceDependents";
        let sheet_name = view.sheet().map(|s| s.name.clone()).unwrap_or_default();
        let mut connected = std::collections::BTreeSet::from([root]);
        for _ in 0..64 {
            let mut added = false;
            for a in self.trace_arrows.iter().filter(|a| {
                a.request.doc == doc
                    && a.request.sheet == sheet
                    && a.request.revision == revision
                    && a.request.dependents == dependents
                    && a.sheet_name == sheet_name
            }) {
                if connected.contains(&a.request.cell) {
                    if a.range.start == a.range.end {
                        added |= connected.insert(a.range.start);
                    } else if let Some(sh) = view.sheet() {
                        for (cell, _) in sh.cells.iter_formulas(a.range).take(2048) {
                            added |= connected.insert(cell);
                        }
                    }
                }
            }
            if !added || connected.len() > 2048 {
                break;
            }
        }
        let targets: Vec<_> =
            connected.into_iter().filter(|c| !self.trace_visited.contains(&(doc, sheet, revision, *c, dependents))).take(64).collect();
        if targets.is_empty() {
            self.message = self.label("未找到更多可展开的本表引用单元格", "No further references on this sheet");
        }
        for cell in targets {
            self.issue(command, serde_json::json!({"cell":cell.a1()}), crate::desktop::After::None, cx);
        }
    }

    pub(crate) fn refresh_traces(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.view() else { return };
        let (doc, sheet, revision) = (view.doc, view.sheet, view.revision);
        self.trace_visited.retain(|(d, s, r, _, _)| *d != doc || *s != sheet || *r == revision);
        let mut requests = std::collections::BTreeSet::new();
        for a in &self.trace_arrows {
            if a.request.doc == doc
                && a.request.sheet == sheet
                && a.request.revision != revision
                && !self.trace_requests.values().any(|r| {
                    r.doc == doc && r.sheet == sheet && r.revision == revision && r.cell == a.request.cell && r.dependents == a.request.dependents
                })
            {
                requests.insert((a.request.cell, a.request.dependents));
            }
        }
        for (cell, dependents) in requests.into_iter().take(256) {
            self.issue(
                if dependents { "formulas.traceDependents" } else { "formulas.tracePrecedents" },
                serde_json::json!({"cell":cell.a1()}),
                crate::desktop::After::None,
                cx,
            );
        }
        self.trace_arrows.retain(|a| self.snapshot.windows.iter().any(|v| v.doc == a.request.doc));
    }

    pub(crate) fn accept_trace(&mut self, request: TraceRequest, result: &Value) {
        if !self.view().is_some_and(|v| v.doc == request.doc && v.sheet == request.sheet && v.revision == request.revision) {
            return;
        }
        self.trace_visited.insert((request.doc, request.sheet, request.revision, request.cell, request.dependents));
        self.trace_arrows.retain(|a| {
            !(a.request.doc == request.doc
                && a.request.sheet == request.sheet
                && a.request.cell == request.cell
                && a.request.dependents == request.dependents)
        });
        for item in result.as_array().into_iter().flatten().take(2048) {
            let Some(range) = item["range"].as_str().or_else(|| item["cell"].as_str()).and_then(RangeRef::parse) else { continue };
            let Some(sheet_name) = item["sheet"].as_str() else { continue };
            if !self.trace_arrows.iter().any(|a| {
                a.request.doc == request.doc
                    && a.request.sheet == request.sheet
                    && a.request.revision == request.revision
                    && a.request.cell == request.cell
                    && a.request.dependents == request.dependents
                    && a.range == range
                    && a.sheet_name == sheet_name
            }) {
                self.trace_arrows.push(TraceArrow { request: request.clone(), sheet_name: sheet_name.into(), range });
            }
        }
        self.trace_arrows.truncate(4096);
    }
}

pub(crate) fn paint(
    arrows: &[TraceArrow],
    view: &WindowSnapshot,
    layout: &GridLayout,
    geo: &SheetGeometry,
    area: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(sheet) = view.sheet() else { return };
    let freeze = sheet.freeze.unwrap_or((0, 0));
    let clip = Bounds::new(
        area.origin + point(px(ROW_HEADER), px(COL_HEADER)),
        size(px((layout.width - ROW_HEADER).max(0.)), px((layout.height - COL_HEADER).max(0.))),
    );
    window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
        for arrow in arrows.iter().filter(|a| a.request.doc == view.doc && a.request.sheet == view.sheet && a.request.revision == view.revision) {
            let selected = layout.cell_rect(arrow.request.cell, geo, freeze);
            let local = arrow.sheet_name == sheet.name;
            let other = layout.cell_rect(arrow.range.start, geo, freeze);
            let center = |r: joysheet_app::geometry::Rect| [r.x + r.width / 4., r.y + r.height / 2.];
            let a = center(selected);
            let b = if local { center(other) } else { [a[0] + 48., a[1] - 28.] };
            let (from, to) = if arrow.request.dependents { (a, b) } else { (b, a) };
            let color = if sheet.cell(arrow.request.cell).is_some_and(|c| matches!(c.value, joysheet_engine::core::Value::Error(_))) {
                [200, 35, 35, 255]
            } else {
                [0, 0, 255, 255]
            };
            crate::charts::paint_primitive(
                &joysheet_chart::Prim::Line { pts: vec![from, to], color, width: 1.2, dash: !local },
                area.origin,
                1.,
                window,
                cx,
            );
            if local {
                crate::charts::paint_primitive(
                    &joysheet_chart::Prim::Circle { cx: from[0], cy: from[1], r: 2., fill: Some(color), stroke: None },
                    area.origin,
                    1.,
                    window,
                    cx,
                );
            }
            let angle = (to[1] - from[1]).atan2(to[0] - from[0]);
            let point_at = |d: f32| [to[0] - 7. * (angle + d).cos(), to[1] - 7. * (angle + d).sin()];
            crate::charts::path(&[point_at(0.4), to, point_at(-0.4)], false, None, Some((color, 1.2)), area.origin, 1., window);
            if !local {
                let prim = joysheet_chart::Prim::Text {
                    x: b[0],
                    y: b[1] - 6.,
                    text: format!("▤ {}!{}", arrow.sheet_name, arrow.range.a1()),
                    size: 11.,
                    color,
                    bold: false,
                    align: joysheet_chart::HAlign::Left,
                    valign: joysheet_chart::VAlign::Bottom,
                    rotation: 0.,
                };
                crate::charts::paint_primitive(&prim, area.origin, 1., window, cx);
            } else if arrow.range.start != arrow.range.end {
                let end = layout.cell_rect(arrow.range.end, geo, freeze);
                crate::charts::path(
                    &[[other.x, other.y], [end.x + end.width, other.y], [end.x + end.width, end.y + end.height], [other.x, end.y + end.height]],
                    true,
                    None,
                    Some((color, 0.8)),
                    area.origin,
                    1.,
                    window,
                );
            }
        }
    });
}
