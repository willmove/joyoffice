use gpui_kit::*;
use joysheet_app::{
    WindowSnapshot,
    geometry::{COL_HEADER, GridLayout, ROW_HEADER, SheetGeometry},
};
use joysheet_engine::model::{Anchor, Color, ShapeKind};
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ObjectKey {
    pub kind: String,
    pub id: u32,
    pub doc: u64,
    pub sheet: usize,
}
pub struct ObjectDrag {
    pub members: Vec<(ObjectKey, Anchor)>,
    pub key: ObjectKey,
    pub start: (f32, f32),
    pub delta: (f32, f32),
    pub resize: bool,
    pub revision: u64,
}
pub fn entries(view: &WindowSnapshot) -> Vec<(ObjectKey, Anchor)> {
    let Some(s) = view.sheet() else { return Vec::new() };
    s.charts
        .iter()
        .map(|o| ("chart", o.id, o.anchor))
        .chain(s.images.iter().map(|o| ("image", o.id, o.anchor)))
        .chain(s.shapes.iter().map(|o| ("shape", o.id, o.anchor)))
        .map(|(kind, id, anchor)| (ObjectKey { kind: kind.into(), id, doc: view.doc, sheet: view.sheet }, anchor))
        .collect()
}
pub type Images = Rc<RefCell<HashMap<usize, (Arc<Vec<u8>>, Option<Arc<RenderImage>>)>>>;
fn shape_icon(name: &str) -> Option<gpui_kit::assets::IconName> {
    use gpui_kit::assets::IconName;
    let normalized = name.to_ascii_lowercase().replace(['-', ' ', '_'], "");
    let alias = match normalized.as_str() {
        "close" => "x",
        "chart" => "chartcolumn",
        "linechart" => "chartline",
        "piechart" => "chartpie",
        "scatter" => "chartscatter",
        "sum" => "sigma",
        "filter" => "funnel",
        "comment" => "messagesquare",
        "note" => "stickynote",
        "picture" => "image",
        "calc" => "calculator",
        "home" => "house",
        "theme" => "sunmoon",
        "text" => "type",
        "trace" => "route",
        "validation" => "listchecks",
        _ => &normalized,
    };
    IconName::ALL.iter().copied().find(|icon| format!("{icon:?}").eq_ignore_ascii_case(alias))
}
pub fn decode(data: &[u8]) -> Result<Arc<RenderImage>, String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(data)).with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let mut rgba = reader.decode().map_err(|e| e.to_string())?.thumbnail(1024, 1024).into_rgba8();
    for p in rgba.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
pub fn rect(a: &Anchor, layout: &GridLayout, geo: &SheetGeometry, freeze: (u32, u32)) -> Bounds<Pixels> {
    let p = layout.cell_rect(a.cell, geo, freeze);
    let k = layout.zoom as f32;
    Bounds::new(point(px(p.x + a.dx * k), px(p.y + a.dy * k)), size(px(a.width.max(1.) * k), px(a.height.max(1.) * k)))
}
pub fn object_clip(a: &Anchor, layout: &GridLayout, geo: &SheetGeometry, freeze: (u32, u32), origin: Point<Pixels>) -> Bounds<Pixels> {
    let col = geo.cols.at(geo.cols.start(a.cell.col) + a.dx as f64);
    let row = geo.rows.at(geo.rows.start(a.cell.row) + a.dy as f64);
    let x = ROW_HEADER + if col >= freeze.1 { layout.frozen_width } else { 0. };
    let y = COL_HEADER + if row >= freeze.0 { layout.frozen_height } else { 0. };
    Bounds::new(origin + point(px(x), px(y)), size(px((layout.width - x).max(0.)), px((layout.height - y).max(0.))))
}
pub fn paint(
    view: &WindowSnapshot,
    layout: &GridLayout,
    geo: &SheetGeometry,
    area: Bounds<Pixels>,
    images: &Images,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(sheet) = view.sheet() else { return };
    let freeze = sheet.freeze.unwrap_or((0, 0));
    let k = layout.zoom as f32;
    let clip = Bounds::new(
        area.origin + point(px(ROW_HEADER), px(COL_HEADER)),
        size(px((layout.width - ROW_HEADER).max(0.)), px((layout.height - COL_HEADER).max(0.))),
    );
    let rgba = |c: Color| {
        let [r, g, b] = c.resolve(&view.workbook.theme).unwrap_or([0, 0, 0]);
        [r, g, b, 255]
    };
    window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
        for object in &sheet.images {
            let bounds = rect(&object.anchor, layout, geo, freeze) + area.origin;
            let id = Arc::as_ptr(&object.data) as usize;
            if let Some((_, Some(image))) = images.borrow().get(&id) {
                window.with_content_mask(Some(ContentMask { bounds: object_clip(&object.anchor, layout, geo, freeze, area.origin) }), |window| {
                    let _ = window.paint_image(bounds, bounds, Corners::all(px(0.)), image.clone(), 0, false);
                });
            }
        }
        for (cell, picture) in &sheet.cell_pictures {
            let r = layout.cell_rect(*cell, geo, freeze);
            let bounds = Bounds::new(area.origin + point(px(r.x), px(r.y)), size(px(r.width), px(r.height)));
            let id = Arc::as_ptr(picture) as usize;
            if let Some((_, Some(image))) = images.borrow().get(&id) {
                window.with_content_mask(
                    Some(ContentMask { bounds: object_clip(&Anchor { cell: *cell, ..Default::default() }, layout, geo, freeze, area.origin) }),
                    |window| {
                        let _ = window.paint_image(bounds, bounds, Corners::all(px(0.)), image.clone(), 0, false);
                    },
                );
            }
        }
        for shape in &sheet.shapes {
            let b = rect(&shape.anchor, layout, geo, freeze) + area.origin;
            if b.intersect(&clip).size.width <= px(0.) || b.intersect(&clip).size.height <= px(0.) {
                continue;
            }
            window.with_content_mask(Some(ContentMask { bounds: object_clip(&shape.anchor, layout, geo, freeze, area.origin) }), |window| {
                let w = shape.anchor.width;
                let h = shape.anchor.height;
                let origin = b.origin;
                let stroke = if shape.format.no_line { None } else { Some((rgba(shape.line), 1.)) };
                let fill = shape.fill.resolve(&view.workbook.theme).map(|_| rgba(shape.fill));
                if shape.kind == ShapeKind::Icon {
                    if let Some(icon) = shape_icon(&shape.text) {
                        let path = icon.path();
                        if let Ok(Some(data)) = gpui_kit::assets::AllAssets.load(path.as_ref()) {
                            let _ = window.paint_svg(
                                b,
                                path,
                                Some(&data),
                                TransformationMatrix::unit(),
                                gpui_kit::rgba(u32::from_be_bytes(rgba(shape.fill))).into(),
                                cx,
                            );
                        }
                    }
                    return;
                }
                if shape.kind == ShapeKind::Ink {
                    let points = joysheet_engine::cmd::draw::ink_points(shape);
                    crate::charts::path(
                        &points,
                        false,
                        None,
                        Some((rgba(shape.line), joysheet_engine::cmd::draw::ink_width(shape))),
                        origin,
                        k,
                        window,
                    );
                    return;
                }
                let prim = match shape.kind {
                    ShapeKind::Ellipse => {
                        let pts = (0..64)
                            .map(|i| {
                                let a = i as f32 * std::f32::consts::TAU / 64.;
                                [w / 2. + w / 2. * a.cos(), h / 2. + h / 2. * a.sin()]
                            })
                            .collect();
                        joysheet_chart::Prim::Polygon { pts, fill: fill.unwrap_or([0; 4]), stroke }
                    }
                    ShapeKind::Triangle => {
                        joysheet_chart::Prim::Polygon { pts: vec![[w / 2., 0.], [w, h], [0., h]], fill: fill.unwrap_or([0; 4]), stroke }
                    }
                    ShapeKind::Line | ShapeKind::Arrow => {
                        joysheet_chart::Prim::Line { pts: vec![[0., 0.], [w, h]], color: rgba(shape.line), width: 1., dash: false }
                    }
                    _ => joysheet_chart::Prim::Rect {
                        x: 0.,
                        y: 0.,
                        w,
                        h,
                        fill,
                        stroke,
                        radius: if shape.kind == ShapeKind::RoundedRectangle { 8. } else { 0. },
                    },
                };
                crate::charts::paint_primitive(&prim, origin, k, window, cx);
                if shape.kind == ShapeKind::Arrow {
                    crate::charts::path(&[[w - 12., h - 3.], [w, h], [w - 3., h - 12.]], false, None, stroke, origin, k, window);
                }
                if !shape.text.is_empty() {
                    paint_shape_text(shape, b, k, view, window, cx);
                }
            });
        }
        for spark in &sheet.sparklines {
            if !layout.rows.iter().any(|r| r.index == spark.cell.row) || !layout.cols.iter().any(|c| c.index == spark.cell.col) {
                continue;
            }
            let values: Vec<Option<f64>> = match joysheet_engine::calc::evaluate(&view.workbook, view.sheet, spark.cell, &spark.source) {
                joysheet_engine::core::Value::Array(a) => a.iter().take(10000).map(|v| v.as_f64()).collect(),
                v => vec![v.as_f64()],
            };
            let r = layout.cell_rect(spark.cell, geo, freeze);
            let origin = area.origin + point(px(r.x), px(r.y));
            for prim in joysheet_chart::render_sparkline(spark.kind, &values, rgba(spark.color), spark.markers, r.width, r.height) {
                crate::charts::paint_primitive(&prim, origin, 1., window, cx);
            }
        }
    });
}
fn paint_shape_text(shape: &joysheet_engine::model::Shape, b: Bounds<Pixels>, scale: f32, view: &WindowSnapshot, window: &mut Window, cx: &mut App) {
    let font = Font {
        family: shape.format.font.clone().unwrap_or("Calibri".into()).into(),
        weight: if shape.format.bold { FontWeight::BOLD } else { FontWeight::NORMAL },
        style: if shape.format.italic { FontStyle::Italic } else { FontStyle::Normal },
        ..Default::default()
    };
    let sz = shape.format.font_size.unwrap_or(11.).clamp(1., 200.) * 4. / 3. * scale;
    let line_height = sz * 1.22;
    let margin = 6. * scale;
    let width = (f32::from(b.size.width) - margin * 2.).max(1.);
    let mut lines = Vec::new();
    let bounded_text: String = shape.text.chars().take(32767).collect();
    for paragraph in bounded_text.lines().take(256) {
        let mut line = String::new();
        let mut advance = 0.;
        for ch in paragraph.chars().take(32767) {
            let text = ch.to_string();
            let cw = f32::from(
                window
                    .text_system()
                    .shape_line(text.clone().into(), px(sz), &[TextRun { len: text.len(), font: font.clone(), ..Default::default() }], None)
                    .width(),
            );
            if !line.is_empty() && advance + cw > width {
                lines.push(std::mem::take(&mut line));
                advance = 0.;
            }
            line.push(ch);
            advance += cw;
            if lines.len() >= 256 {
                break;
            }
        }
        lines.push(line);
        if lines.len() >= 256 {
            break;
        }
    }
    let total = lines.len() as f32 * line_height;
    let offset = match shape.format.anchor {
        Some(joysheet_engine::model::ShapeTextAnchor::Bottom) => (f32::from(b.size.height) - total - margin).max(margin),
        Some(joysheet_engine::model::ShapeTextAnchor::Middle) => (f32::from(b.size.height) - total).max(0.) / 2.,
        _ if shape.kind != ShapeKind::TextBox => (f32::from(b.size.height) - total).max(0.) / 2.,
        _ => margin,
    };
    let color = shape.format.text_color.unwrap_or(Color::rgb(0, 0, 0)).resolve(&view.workbook.theme).unwrap_or([0, 0, 0]);
    window.with_content_mask(Some(ContentMask { bounds: b }), |window| {
        for (i, line) in lines.iter().enumerate() {
            let y = f32::from(b.origin.y) + offset + i as f32 * line_height;
            if y >= f32::from(b.bottom()) {
                break;
            }
            let text = if shape.format.rtl { format!("\u{202b}{line}\u{202c}") } else { line.clone() };
            let line = window.text_system().shape_line(
                text.clone().into(),
                px(sz),
                &[TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: rgb(u32::from_be_bytes([0, color[0], color[1], color[2]])).into(),
                    ..Default::default()
                }],
                None,
            );
            let natural = f32::from(line.width());
            let x = f32::from(b.origin.x)
                + match shape.format.align {
                    Some(joysheet_engine::model::ShapeTextAlign::Right) => width - natural + margin,
                    Some(joysheet_engine::model::ShapeTextAlign::Center) => (width - natural) / 2. + margin,
                    _ if shape.format.rtl => width - natural + margin,
                    _ if shape.kind != ShapeKind::TextBox => (width - natural) / 2. + margin,
                    _ => margin,
                };
            if let Some((outline, width)) = shape.format.text_outline {
                let ink = outline.resolve(&view.workbook.theme).unwrap_or([0, 0, 0]);
                let block_lines = vec![text.clone()];
                let block = crate::outline_text::TextBlock {
                    lines: &block_lines,
                    font: &font,
                    size: sz,
                    angle: 0.,
                    underline: false,
                    strike: false,
                    outline: (width * 4. / 3. * scale).clamp(0., 12.),
                };
                let baseline = (line_height - f32::from(line.ascent) - f32::from(line.descent)) / 2. + f32::from(line.ascent);
                block.paint(point(px(x), px(y + baseline - sz)), 0., 0., u32::from_be_bytes([0, ink[0], ink[1], ink[2]]), window, cx);
            }
            let _ = line.paint(point(px(x), px(y)), px(line_height), TextAlign::Left, None, window, cx);
        }
    });
}

impl crate::desktop::Desktop {
    pub fn delete_selected_object(&mut self, cx: &mut Context<Self>) {
        if self.draft.is_none() && self.preview.is_none() && !self.confirm_close && self.selected_anchor().is_some() && !self.palette_open {
            let operations = self
                .object_selection()
                .iter()
                .map(|(o, _)| serde_json::json!({"command":"object.delete","params":{"kind":o.kind,"id":o.id}}))
                .collect();
            self.object_batch(operations, None, cx);
            self.selected_object = None;
            self.selected_objects.clear();
            cx.stop_propagation();
        }
    }
    pub fn selected_anchor(&self) -> Option<Anchor> {
        let key = self.selected_object.as_ref()?;
        let view = self.view()?;
        entries(view).into_iter().find(|(o, _)| o.doc == key.doc && o.sheet == key.sheet && o.kind == key.kind && o.id == key.id).map(|(_, a)| a)
    }
    pub fn selected_object_bounds(&self) -> Option<Bounds<Pixels>> {
        let mut anchor = self.selected_anchor()?;
        if let Some(drag) = &self.object_drag {
            apply_delta(&mut anchor, drag.delta, drag.resize);
        }
        Some(rect(&anchor, &self.layout, self.geometry.as_ref()?, self.view()?.sheet()?.freeze.unwrap_or((0, 0))))
    }
    pub fn object_mouse_down(&mut self, x: f32, y: f32, additive: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.draft.is_some() || x < ROW_HEADER || y < COL_HEADER {
            return false;
        }
        let Some(view) = self.view() else { return false };
        let Some(geo) = &self.geometry else { return false };
        let freeze = view.sheet().and_then(|s| s.freeze).unwrap_or((0, 0));
        let hit = entries(view).into_iter().rev().find(|(_, a)| {
            let mask = object_clip(a, &self.layout, geo, freeze, point(px(0.), px(0.)));
            if !mask.contains(&point(px(x), px(y))) {
                return false;
            }
            let b = rect(a, &self.layout, geo, freeze).intersect(&mask);
            if b.size.width <= px(0.) || b.size.height <= px(0.) {
                return false;
            }
            x >= f32::from(b.origin.x) - 5. && x <= f32::from(b.right()) + 5. && y >= f32::from(b.origin.y) - 5. && y <= f32::from(b.bottom()) + 5.
        });
        let Some((key, anchor)) = hit else {
            self.selected_object = None;
            self.selected_objects.clear();
            return false;
        };
        let b = rect(&anchor, &self.layout, geo, freeze);
        let revision = view.revision;
        let corner = (x - f32::from(b.right())).abs() < 10. && (y - f32::from(b.bottom())).abs() < 10.;
        if additive {
            self.toggle_object(key, true, cx);
            self.focus_grid(window, cx);
            return true;
        }
        if !self.selected_objects.contains(&key) {
            self.selected_objects = vec![key.clone()];
        }
        self.selected_object = Some(key.clone());
        let members = self.object_selection();
        let resize = corner && members.len() == 1;
        self.object_drag = Some(ObjectDrag { members, key, start: (x, y), delta: (0., 0.), resize, revision });
        self.focus_grid(window, cx);
        cx.notify();
        true
    }
    pub fn finish_object_drag(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.object_drag.take() else { return };
        if drag.delta.0.abs() + drag.delta.1.abs() < 1. {
            return;
        }
        if !self.view().is_some_and(|v| v.doc == drag.key.doc && v.sheet == drag.key.sheet) {
            return;
        }
        let operations=drag.members.into_iter().map(|(key,mut anchor)| {
            apply_delta(&mut anchor,drag.delta,drag.resize);
            serde_json::json!({"command":"object.move","params":{"kind":key.kind,"id":key.id,"at":anchor.cell.a1(),"dx":anchor.dx,"dy":anchor.dy,"width":anchor.width,"height":anchor.height}})
        }).collect();
        self.object_batch(operations, Some(drag.revision), cx);
        cx.notify();
    }
    pub fn prepare_images(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.view() else { return };
        let Some(sheet) = view.sheet() else { return };
        let visible = Bounds::new(point(px(ROW_HEADER), px(COL_HEADER)), size(px(self.layout.width), px(self.layout.height)));
        let Some(geo) = &self.geometry else { return };
        let data: Vec<_> = sheet
            .images
            .iter()
            .filter(|o| {
                let r = rect(&o.anchor, &self.layout, geo, sheet.freeze.unwrap_or((0, 0))).intersect(&visible);
                r.size.width > px(0.) && r.size.height > px(0.)
            })
            .map(|o| (Arc::as_ptr(&o.data) as usize, o.data.clone()))
            .chain(
                sheet
                    .cell_pictures
                    .iter()
                    .filter(|(cell, _)| self.layout.rows.iter().any(|r| r.index == cell.row) && self.layout.cols.iter().any(|c| c.index == cell.col))
                    .map(|(_, p)| (Arc::as_ptr(p) as usize, Arc::new(p.data.clone()))),
            )
            .take(16)
            .collect();
        self.object_images.borrow_mut().retain(|id, _| data.iter().any(|(i, _)| i == id));
        for (id, bytes) in data {
            if self.object_images.borrow().contains_key(&id) {
                continue;
            }
            if self.image_decoding >= 2 {
                break;
            }
            self.image_decoding += 1;
            self.object_images.borrow_mut().insert(id, (bytes.clone(), None));
            let task = cx.background_executor().spawn(async move { decode(&bytes) });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    this.image_decoding = this.image_decoding.saturating_sub(1);
                    match result {
                        Ok(image) => {
                            if let Some(entry) = this.object_images.borrow_mut().get_mut(&id) {
                                entry.1 = Some(image);
                            }
                        }
                        Err(e) => {
                            if this.object_images.borrow().contains_key(&id) {
                                this.alert(format!("图片无法解码 / Image decode failed: {e}"));
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
}
fn apply_delta(anchor: &mut Anchor, delta: (f32, f32), resize: bool) {
    if resize {
        anchor.width = (anchor.width + delta.0).clamp(1., 10000.);
        anchor.height = (anchor.height + delta.1).clamp(1., 10000.);
    } else {
        anchor.dx += delta.0;
        anchor.dy += delta.1;
    }
}
#[cfg(test)]
#[path = "tests/objects_tests.rs"]
mod tests;
