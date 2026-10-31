//! Toolkit-free GridCraft primitives painted by GPUI; data/layout cached per document revision.
use crate::chart_dash::dashed;
use gpui_kit::*;
use joysheet_app::{
    WindowSnapshot,
    geometry::{GridLayout, SheetGeometry},
};
use joysheet_chart::{Prim, Rgba};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
pub type ChartCache = Rc<RefCell<HashMap<u32, Arc<Vec<Prim>>>>>;
struct Measure(Arc<WindowTextSystem>);
impl joysheet_chart::Measure for Measure {
    fn text_width(&self, text: &str, size: f32, bold: bool) -> f32 {
        let font = Font { family: "Calibri".into(), weight: if bold { FontWeight::BOLD } else { FontWeight::NORMAL }, ..Default::default() };
        f32::from(self.0.shape_line(text.to_string().into(), px(size), &[TextRun { len: text.len(), font, ..Default::default() }], None).width())
    }
}
fn color(value: Rgba) -> Hsla {
    rgba(u32::from_be_bytes(value)).into()
}
pub(crate) fn path(
    points: &[[f32; 2]],
    closed: bool,
    fill: Option<Rgba>,
    stroke: Option<(Rgba, f32)>,
    origin: Point<Pixels>,
    scale: f32,
    window: &mut Window,
) {
    let position = |p: [f32; 2]| origin + point(px(p[0] * scale), px(p[1] * scale));
    if let Some(first) = points.first().copied() {
        if let Some(fill) = fill {
            let mut builder = PathBuilder::fill();
            builder.move_to(position(first));
            for p in points.iter().skip(1) {
                builder.line_to(position(*p));
            }
            if closed {
                builder.close();
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, color(fill));
            }
        }
        if let Some((stroke, width)) = stroke {
            let mut builder = PathBuilder::stroke(px((width * scale).max(0.5)));
            builder.move_to(position(first));
            for p in points.iter().skip(1) {
                builder.line_to(position(*p));
            }
            if closed {
                builder.close();
            }
            if let Ok(path) = builder.build() {
                window.paint_path(path, color(stroke));
            }
        }
    }
}
pub fn paint(
    view: &WindowSnapshot,
    geo: &SheetGeometry,
    layout: &GridLayout,
    area: Bounds<Pixels>,
    cache: &ChartCache,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(sheet) = view.sheet() else {
        return;
    };
    let scale = layout.zoom as f32;
    for chart in &sheet.charts {
        let anchor = &chart.anchor;
        let clip = crate::objects::object_clip(anchor, layout, geo, sheet.freeze.unwrap_or((0, 0)), area.origin);
        if !anchor.width.is_finite() || !anchor.height.is_finite() || anchor.width <= 0. || anchor.height <= 0. {
            continue;
        }
        let rect = layout.cell_rect(anchor.cell, geo, sheet.freeze.unwrap_or((0, 0)));
        let origin = area.origin + point(px(rect.x + anchor.dx * scale), px(rect.y + anchor.dy * scale));
        let chart_bounds = Bounds::new(origin, size(px(anchor.width * scale), px(anchor.height * scale)));
        if chart_bounds.intersect(&clip).size.width <= px(0.) || chart_bounds.intersect(&clip).size.height <= px(0.) {
            continue;
        }
        let cached = cache.borrow().get(&chart.id).cloned();
        let primitives = cached.unwrap_or_else(|| {
            let data = joysheet_chart::resolve(&view.workbook, view.sheet, chart);
            let primitives = Arc::new(joysheet_chart::render(chart, &data, anchor.width, anchor.height, &Measure(window.text_system().clone())));
            cache.borrow_mut().insert(chart.id, primitives.clone());
            primitives
        });
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            for primitive in primitives.iter() {
                paint_primitive(primitive, origin, scale, window, cx);
            }
        });
    }
}
pub(crate) fn paint_primitive(primitive: &Prim, origin: Point<Pixels>, scale: f32, window: &mut Window, cx: &mut App) {
    match primitive {
        Prim::Rect { x, y, w, h, fill, stroke, radius } => {
            let bounds = Bounds::new(origin + point(px(x * scale), px(y * scale)), size(px(w * scale), px(h * scale)));
            let mut quad = gpui::fill(bounds, fill.map(color).unwrap_or_else(|| rgba(0).into()));
            quad.corner_radii = Corners::all(px(radius * scale));
            if let Some((value, width)) = stroke {
                quad.border_color = color(*value);
                quad.border_widths = Edges::all(px(width * scale));
            }
            window.paint_quad(quad);
        }
        Prim::Line { pts, color, width, dash } => {
            if *dash {
                for segment in dashed(pts, 5., 3.) {
                    path(&segment, false, None, Some((*color, *width)), origin, scale, window);
                }
            } else {
                path(pts, false, None, Some((*color, *width)), origin, scale, window);
            }
        }
        Prim::Polygon { pts, fill, stroke } => path(pts, true, Some(*fill), *stroke, origin, scale, window),
        Prim::Circle { cx, cy, r, fill, stroke } => {
            let points: Vec<_> = (0..64)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / 64.;
                    [cx + r * angle.cos(), cy + r * angle.sin()]
                })
                .collect();
            path(&points, true, *fill, *stroke, origin, scale, window);
        }
        Prim::Wedge { cx, cy, r_outer, r_inner, a0, a1, fill, stroke } => {
            let steps = (((a1 - a0).abs() * 32.).ceil() as usize).clamp(2, 256);
            let mut points: Vec<_> = (0..=steps)
                .map(|i| {
                    let angle = a0 + (a1 - a0) * i as f32 / steps as f32;
                    [cx + r_outer * angle.sin(), cy - r_outer * angle.cos()]
                })
                .collect();
            if *r_inner > 0. {
                points.extend((0..=steps).rev().map(|i| {
                    let angle = a0 + (a1 - a0) * i as f32 / steps as f32;
                    [cx + r_inner * angle.sin(), cy - r_inner * angle.cos()]
                }));
            } else {
                points.push([*cx, *cy]);
            }
            path(&points, true, Some(*fill), *stroke, origin, scale, window);
        }
        Prim::Text { x, y, text, size: font_size, color: ink, bold, align, valign, rotation } => {
            let font = Font { family: "Calibri".into(), weight: if *bold { FontWeight::BOLD } else { FontWeight::NORMAL }, ..Default::default() };
            let height = font_size * scale * 1.2;
            let line = window.text_system().shape_line(
                text.clone().into(),
                px(font_size * scale),
                &[TextRun { len: text.len(), font, color: color(*ink), ..Default::default() }],
                None,
            );
            let width = f32::from(line.width());
            let dx = match align {
                joysheet_chart::HAlign::Left => 0.,
                joysheet_chart::HAlign::Center => -width / 2.,
                joysheet_chart::HAlign::Right => -width,
            };
            let dy = match valign {
                joysheet_chart::VAlign::Top => 0.,
                joysheet_chart::VAlign::Middle => -height / 2.,
                joysheet_chart::VAlign::Bottom => -height,
            };
            if rotation.abs() > 0.0001 {
                let lines = vec![text.clone()];
                let block = crate::outline_text::TextBlock {
                    lines: &lines,
                    font: &Font { family: "Calibri".into(), weight: if *bold { FontWeight::BOLD } else { FontWeight::NORMAL }, ..Default::default() },
                    size: font_size * scale,
                    angle: *rotation,
                    underline: false,
                    strike: false,
                    outline: 0.,
                };
                let ink = u32::from_be_bytes([0, ink[0], ink[1], ink[2]]);
                if block.paint(origin + point(px(x * scale), px(y * scale)), dx, dy, ink, window, cx) {
                    return;
                }
            }
            let _ = line.paint(origin + point(px(x * scale + dx), px(y * scale + dy)), px(height), TextAlign::Left, None, window, cx);
        }
    }
}
