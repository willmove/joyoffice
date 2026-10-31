//! The virtual grid exports only visible, clipped cells, sharing its production geometry.
use gpui_kit::{Role, accesskit};
use joysheet_engine::{Selection, core::CellRef};
use joysheet_app::{
    WindowSnapshot,
    geometry::{COL_HEADER, GridLayout, ROW_HEADER, Rect, SheetGeometry},
};
use serde_json::{Value, json};
use std::collections::HashSet;
pub struct Cell {
    pub at: CellRef,
    pub rect: Rect,
    pub value: String,
    pub description: String,
    pub selected: bool,
    pub readonly: bool,
    pub rows: usize,
    pub cols: usize,
}
impl Cell {
    pub fn node(&self, x: f64, y: f64, scale: f64) -> accesskit::Node {
        let mut n = accesskit::Node::new(Role::GridCell);
        n.set_label(self.at.a1());
        n.set_value(self.value.clone());
        n.set_description(self.description.clone());
        n.set_row_index(self.at.row as usize);
        n.set_column_index(self.at.col as usize);
        n.set_row_span(self.rows);
        n.set_column_span(self.cols);
        n.set_selected(self.selected);
        if self.readonly {
            n.set_read_only();
        }
        n.add_action(accesskit::Action::Focus);
        n.add_action(accesskit::Action::Click);
        n.set_bounds(accesskit::Rect {
            x0: x + self.rect.x as f64 * scale,
            y0: y + self.rect.y as f64 * scale,
            x1: x + (self.rect.x + self.rect.width) as f64 * scale,
            y1: y + (self.rect.y + self.rect.height) as f64 * scale,
        });
        n
    }
    pub fn inspect(&self) -> Value {
        json!({"cell":self.at.a1(),"value":self.value,"description":self.description,"readonly":self.readonly,"selected":self.selected,"row":self.at.row,"column":self.at.col,"rowSpan":self.rows,"columnSpan":self.cols,"bounds":{"x":self.rect.x,"y":self.rect.y,"width":self.rect.width,"height":self.rect.height}})
    }
}
pub fn cells(v: &WindowSnapshot, l: &GridLayout, g: &SheetGeometry, selection: &Selection) -> Vec<Cell> {
    let Some(s) = v.sheet() else { return Vec::new() };
    let freeze = s.freeze.unwrap_or((0, 0));
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for row in &l.rows {
        for col in &l.cols {
            let visible = CellRef::new(row.index, col.index);
            let merge = s.merge_at(visible);
            let at = merge.map_or(visible, |r| r.start);
            if !seen.insert(at) {
                continue;
            }
            let mut r = l.cell_rect(at, g, freeze);
            if let Some(m) = merge {
                r.width = ((g.cols.start(m.end.col + 1) - g.cols.start(m.start.col)) * l.zoom) as f32;
                r.height = ((g.rows.start(m.end.row + 1) - g.rows.start(m.start.row)) * l.zoom) as f32;
            }
            let left = ROW_HEADER + if at.col < freeze.1 { 0. } else { l.frozen_width };
            let top = COL_HEADER + if at.row < freeze.0 { 0. } else { l.frozen_height };
            let right = if at.col < freeze.1 { ROW_HEADER + l.frozen_width } else { l.width };
            let bottom = if at.row < freeze.0 { COL_HEADER + l.frozen_height } else { l.height };
            let x = r.x.max(left);
            let y = r.y.max(top);
            r = Rect { x, y, width: (r.x + r.width).min(right) - x, height: (r.y + r.height).min(bottom) - y };
            if r.width <= 0. || r.height <= 0. {
                continue;
            }
            let style = v.workbook.styles.get(s.style_id(at));
            let readonly = s.protection.is_some() && style.protection.locked;
            let mut description = format!("{} {}", s.name, at.a1());
            if let Some(f) = s.cell(at).and_then(|c| c.formula.as_ref()).map(|f| f.text.as_str())
                && !(s.protection.is_some() && style.protection.hidden)
            {
                description.push_str(&format!(" · ={f}"));
            }
            if readonly {
                description.push_str(" · 只读 / Read only");
            }
            let (rows, cols) = merge.map_or((1, 1), |m| (m.height() as usize, m.width() as usize));
            if rows > 1 || cols > 1 {
                description.push_str(&format!(" · 合并 / Merged {rows}×{cols}"));
            }
            result.push(Cell {
                at,
                rect: r,
                value: joysheet_engine::display::cell_text(&v.workbook, s, at).chars().take(4096).collect(),
                description: description.chars().take(4096).collect(),
                selected: selection.ranges.iter().any(|r| r.contains(at)),
                readonly,
                rows,
                cols,
            });
        }
    }
    result
}
#[cfg(test)]
mod tests {
    #[test]
    fn merged_accessible_bounds_are_physical_at_fractional_dpi() {
        let cell = super::Cell {
            at: joysheet_engine::core::CellRef::new(1, 1),
            rect: joysheet_app::geometry::Rect { x: 116., y: 48., width: 128., height: 40. },
            value: "中文🙂".into(),
            description: String::new(),
            selected: true,
            readonly: true,
            rows: 2,
            cols: 2,
        };
        let node = cell.node(0., 298., 1.75);
        let b = node.bounds().unwrap();
        assert_eq!((b.x0, b.y0, b.width(), b.height()), (203., 382., 224., 70.));
        assert!(node.is_read_only());
        assert_eq!(node.row_span(), Some(2));
    }
}
