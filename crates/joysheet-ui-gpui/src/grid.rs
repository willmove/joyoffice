use crate::desktop::{After, Desktop};
use gpui_kit::component::input::Input;
use gpui_kit::{prelude::*, *};
use joysheet_app::{
    WindowSnapshot,
    geometry::{COL_HEADER, GridLayout, ROW_HEADER, SheetGeometry},
};
use joysheet_engine::{
    core::{CellRef, MAX_COLS, RangeRef, Value as CellValue},
    model::{Color, HAlign, PatternType, Style as CellStyle},
};
use serde_json::json;
use std::{collections::HashSet, time::Instant};

struct SheetElement {
    view: Entity<Desktop>,
    a11y_actions: std::rc::Rc<std::cell::RefCell<Vec<(accesskit::NodeId, CellRef)>>>,
}
struct PaintState {
    prepaint_started: Instant,
    frame_timing: crate::performance::FrameTiming,
    scale: f32,
    references: Vec<crate::formula_feedback::Reference>,
    sequence: u64,
    view: WindowSnapshot,
    geo: SheetGeometry,
    layout: GridLayout,
    selection: joysheet_engine::Selection,
    cf_cache: std::rc::Rc<std::cell::RefCell<joysheet_engine::cf::CfCache>>,
    chart_cache: crate::charts::ChartCache,
    images: crate::objects::Images,
}
impl IntoElement for SheetElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for SheetElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<PaintState>;
    fn id(&self) -> Option<ElementId> {
        Some("worksheet-canvas".into())
    }
    fn a11y_role(&self) -> Option<Role> {
        Some(Role::Grid)
    }
    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        node.set_label("工作表 / Worksheet");
    }
    fn a11y_synthetic_children(&mut self, state: &mut Option<PaintState>, builder: &mut A11ySubtreeBuilder) {
        let Some(state) = state.as_ref() else { return };
        builder.parent_node().set_row_count(joysheet_engine::core::MAX_ROWS as usize);
        builder.parent_node().set_column_count(MAX_COLS as usize);
        let parent = builder.parent_node().bounds().unwrap_or(accesskit::Rect::ZERO);
        for cell in crate::accessibility::cells(&state.view, &state.layout, &state.geo, &state.selection) {
            let id = builder.synthetic_node_id((state.view.doc, state.view.sheet, cell.at.row, cell.at.col));
            builder.push_child(id, cell.node(parent.x0, parent.y0, state.scale as f64));
            self.a11y_actions.borrow_mut().push((id, cell.at));
        }
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<PaintState> {
        let prepaint_started = Instant::now();
        self.view.update(cx, |view, cx| {
            let snapshot = view.view()?.clone();
            let sheet = snapshot.sheet()?;
            let key = (snapshot.doc, snapshot.sheet, snapshot.revision);
            let geometry_started = Instant::now();
            let mut extent_ms = 0.;
            let mut dimensions_ms = 0.;
            if view.geometry_key != Some(key) {
                let first = view.geometry.is_none();
                let extent_started = Instant::now();
                let used_end = sheet.used_range().map(|r| r.end).unwrap_or_default();
                extent_ms = extent_started.elapsed().as_secs_f64() * 1000.;
                let dimensions_started = Instant::now();
                view.geometry = Some(SheetGeometry::with_extent(sheet, used_end));
                dimensions_ms = dimensions_started.elapsed().as_secs_f64() * 1000.;
                view.geometry_key = Some(key);
                *view.cf_cache.borrow_mut() = joysheet_engine::cf::CfCache::new();
                view.chart_cache.borrow_mut().clear();
                if first {
                    cx.notify();
                }
            }
            let geometry_ms = geometry_started.elapsed().as_secs_f64() * 1000.;
            let geo = view.geometry.clone()?;
            let layout_started = Instant::now();
            let layout =
                GridLayout::new(sheet, &geo, view.scroll.0, view.scroll.1, f32::from(bounds.size.width) - 14., f32::from(bounds.size.height) - 14.);
            let layout_ms = layout_started.elapsed().as_secs_f64() * 1000.;
            view.scroll = (layout.scroll_x, layout.scroll_y);
            view.layout = layout.clone();
            view.grid_bounds = bounds;
            view.prepare_images(cx);
            Some(PaintState {
                prepaint_started,
                frame_timing: crate::performance::FrameTiming {
                    prepaint_ms: prepaint_started.elapsed().as_secs_f64() * 1000.,
                    geometry_ms,
                    extent_ms,
                    dimensions_ms,
                    layout_ms,
                    ..Default::default()
                },
                scale: window.scale_factor(),
                references: view
                    .draft
                    .as_ref()
                    .filter(|d| d.doc == snapshot.doc)
                    .map(|d| {
                        crate::formula_feedback::references(&d.text)
                            .into_iter()
                            .filter(|r| snapshot.sheet().is_some_and(|s| r.sheet.as_deref().unwrap_or(&d.sheet_name).eq_ignore_ascii_case(&s.name)))
                            .collect()
                    })
                    .unwrap_or_default(),
                sequence: view.snapshot.sequence,
                view: snapshot,
                geo,
                layout,
                selection: view.selection(),
                cf_cache: view.cf_cache.clone(),
                chart_cache: view.chart_cache.clone(),
                images: view.object_images.clone(),
            })
        })
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        state: &mut Option<PaintState>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(state) = state.as_ref() else {
            return;
        };
        for (id, at) in self.a11y_actions.borrow().iter().copied() {
            for action in [accesskit::Action::Focus, accesskit::Action::Click] {
                let view = self.view.clone();
                window.on_a11y_action(id, action, move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        if this.palette_open
                            || this.preview.is_some()
                            || this.confirm_close
                            || this.trust_request.is_some()
                            || this.validation_prompt.is_some()
                        {
                            return;
                        }
                        this.commit(After::Select(at, false), window, cx);
                        this.focus_grid(window, cx);
                    })
                });
            }
        }
        let started = Instant::now();
        let mut frame = state.frame_timing.clone();
        let count = paint_sheet(state, bounds, window, cx, &mut frame);
        let elapsed = started.elapsed().as_secs_f64() * 1000.;
        frame.paint_ms = elapsed;
        self.view.update(cx, |view, _| {
            view.performance.paint(
                state.view.window,
                state.view.doc,
                state.view.sheet,
                state.sequence,
                state.view.revision,
                state.prepaint_started,
                frame,
            );
            view.painted_cells = count;
            view.frame_number = view.frame_number.saturating_add(1);
            view.painted_sequence = state.sequence;
            view.paint_ms = elapsed;
            if let Some((start, doc)) = view.edit_clock
                && doc == state.view.doc
                && view.edit_fence.is_some_and(|r| state.view.revision >= r)
            {
                view.edit_samples.push(start.elapsed().as_secs_f64() * 1000.);
                view.edit_clock = None;
                view.edit_fence = None;
                if view.edit_samples.len() > 2000 {
                    view.edit_samples.drain(..1000);
                }
            }
            view.frame_samples.push(elapsed);
            if view.frame_samples.len() > 2000 {
                view.frame_samples.drain(..1000);
            }
        });
    }
}

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w.max(0.)), px(h.max(0.))))
}
fn filter_header(sheet: &joysheet_engine::model::Sheet, cell: CellRef) -> bool {
    sheet.autofilter.as_ref().is_some_and(|f| cell.row == f.range.start.row && cell.col >= f.range.start.col && cell.col <= f.range.end.col)
        || sheet
            .tables
            .iter()
            .any(|t| t.header_row && t.filter_button && cell.row == t.range.start.row && cell.col >= t.range.start.col && cell.col <= t.range.end.col)
}
fn fill(rect: Bounds<Pixels>, color: u32, window: &mut Window) {
    let rect = if rect.size.width <= px(1.5) || rect.size.height <= px(1.5) { window.pixel_snap_bounds(rect) } else { rect };
    window.paint_quad(gpui::fill(rect, rgb(color)));
}
fn rgb_color(color: Color, view: &WindowSnapshot, default: u32) -> u32 {
    color.resolve(&view.workbook.theme).map(|[r, g, b]| ((r as u32) << 16) | ((g as u32) << 8) | b as u32).unwrap_or(default)
}
fn paint_text(
    text: &str,
    font: Font,
    font_size: f32,
    color: u32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    align: HAlign,
    window: &mut Window,
    cx: &mut App,
) {
    if text.is_empty() || width <= 0. || height <= 0. {
        return;
    }
    // Cell input can contain line breaks; each line is shaped independently.
    for (index, text) in text.lines().take(32).enumerate() {
        let line = window.text_system().shape_line(
            text.to_string().into(),
            px(font_size),
            &[TextRun { len: text.len(), font: font.clone(), color: rgb(color).into(), ..Default::default() }],
            None,
        );
        let line_height = font_size * 1.22;
        let py = y + (height - line_height).max(0.) / 2. + index as f32 * line_height;
        if py >= y + height {
            break;
        }
        let line_width = f32::from(line.width());
        let px_x = match align {
            HAlign::Right => x + width - line_width - 6.,
            HAlign::Center | HAlign::CenterAcross => x + (width - line_width) / 2.,
            _ => x + 6.,
        };
        if let Err(error) = line.paint(point(px(px_x.max(x + 2.)), px(py)), px(line_height), TextAlign::Left, None, window, cx) {
            log::debug!("cell text: {error}");
        }
    }
}
struct PaintedCell {
    rect: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    style: CellStyle,
    text: String,
    color: u32,
    align: HAlign,
    font_size: f32,
    text_width: f32,
}
fn paint_sheet(state: &PaintState, area: Bounds<Pixels>, window: &mut Window, cx: &mut App, timing: &mut crate::performance::FrameTiming) -> usize {
    let cells_started = Instant::now();
    let Some(sheet) = state.view.sheet() else {
        return 0;
    };
    let ox = f32::from(area.origin.x);
    let oy = f32::from(area.origin.y);
    let layout = &state.layout;
    fill(area, 0xffffff, window);
    let mut cells = Vec::new();
    let mut seen = HashSet::new();
    let mut cf = state.cf_cache.borrow_mut();
    for row in &layout.rows {
        for col in &layout.cols {
            let cell = CellRef::new(row.index, col.index);
            let merge = sheet.merge_at(cell);
            let anchor = merge.map(|m| m.start).unwrap_or(cell);
            if !seen.insert((anchor, row.frozen, col.frozen)) {
                continue;
            }
            let end = merge.map(|m| m.end).unwrap_or(anchor);
            let x = ROW_HEADER + (state.geo.cols.start(anchor.col) * layout.zoom - if col.frozen { 0. } else { layout.scroll_x }) as f32;
            let y = COL_HEADER + (state.geo.rows.start(anchor.row) * layout.zoom - if row.frozen { 0. } else { layout.scroll_y }) as f32;
            let width = ((state.geo.cols.start(end.col.saturating_add(1)) - state.geo.cols.start(anchor.col)) * layout.zoom) as f32;
            let height = ((state.geo.rows.start(end.row.saturating_add(1)) - state.geo.rows.start(anchor.row)) * layout.zoom) as f32;
            let clip_x = if col.frozen { ROW_HEADER } else { ROW_HEADER + layout.frozen_width };
            let clip_y = if row.frozen { COL_HEADER } else { COL_HEADER + layout.frozen_height };
            let clip_right = if col.frozen { (ROW_HEADER + layout.frozen_width).min(layout.width) } else { layout.width };
            let clip_bottom = if row.frozen { (COL_HEADER + layout.frozen_height).min(layout.height) } else { layout.height };
            let clip = bounds(ox + clip_x, oy + clip_y, clip_right - clip_x, clip_bottom - clip_y);
            if clip.size.width <= px(0.) || clip.size.height <= px(0.) {
                continue;
            }
            let rect = bounds(ox + x, oy + y, width, height);
            let mut style = state.view.workbook.styles.get(sheet.style_id(anchor)).clone();
            if let Some(table) = sheet.table_at(anchor) {
                style = joysheet_engine::tables::table_cell_style(&state.view.workbook, state.view.sheet, table, anchor);
            }
            let look = cf.look(&state.view.workbook, state.view.sheet, anchor);
            let mut background =
                rgb_color(if style.fill.pattern == PatternType::Solid { style.fill.fg } else { style.fill.bg }, &state.view, 0xffffff);
            let mut color = rgb_color(style.font.color, &state.view, 0x27382e);
            if let Some(look) = &look {
                if let Some([r, g, b]) = look.scale_fill {
                    background = ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                }
                if let Some(dxf) = &look.style {
                    if dxf.fill.pattern != PatternType::None {
                        style.fill = dxf.fill;
                        background =
                            rgb_color(if dxf.fill.pattern == PatternType::Solid { dxf.fill.fg } else { dxf.fill.bg }, &state.view, background);
                    }
                    color = rgb_color(dxf.font.color, &state.view, color);
                    style.font.bold |= dxf.font.bold;
                    style.font.italic |= dxf.font.italic;
                    style.font.strike |= dxf.font.strike;
                    if dxf.font.underline != joysheet_engine::model::Underline::None {
                        style.font.underline = dxf.font.underline;
                    }
                    for (dst, src) in [
                        (&mut style.borders.left, dxf.borders.left),
                        (&mut style.borders.right, dxf.borders.right),
                        (&mut style.borders.top, dxf.borders.top),
                        (&mut style.borders.bottom, dxf.borders.bottom),
                        (&mut style.borders.diag_up, dxf.borders.diag_up),
                        (&mut style.borders.diag_down, dxf.borders.diag_down),
                    ] {
                        if !src.is_none() {
                            *dst = src;
                        }
                    }
                    if dxf.num_fmt.as_str() != "General" {
                        style.num_fmt = dxf.num_fmt.clone();
                    }
                }
            }
            let selected = state.selection.contains(cell) || merge.is_some_and(|m| state.selection.current().intersects(&m));
            window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                fill(rect, background, window);
                crate::complex_format::pattern(
                    rect,
                    clip,
                    style.fill.pattern,
                    rgb_color(style.fill.fg, &state.view, 0x27382e),
                    layout.zoom as f32,
                    window,
                );
                if selected && anchor != state.selection.active {
                    window.paint_quad(gpui::fill(rect, rgba(0x14825616)));
                }
                if let Some(look) = &look
                    && let Some((fraction, [r, g, b], gradient)) = look.bar
                {
                    let bar = bounds(ox + x + 2., oy + y + 3., (width - 4.) * fraction, height - 6.);
                    if gradient {
                        for i in 0..16 {
                            let mut segment = bar;
                            segment.origin.x += bar.size.width * (i as f32 / 16.);
                            segment.size.width = bar.size.width / 16. + px(0.1);
                            window.paint_quad(gpui::fill(segment, rgba(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (180 - i * 8))));
                        }
                    } else {
                        window.paint_quad(gpui::fill(bar, rgba(((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | 0x77)));
                    }
                }
                if sheet.show_gridlines {
                    fill(bounds(ox + x + width - 0.5, oy + y, 0.5, height), 0xe3e9e4, window);
                    fill(bounds(ox + x, oy + y + height - 0.5, width, 0.5), 0xe3e9e4, window);
                }
                if style.num_fmt.as_str() == "checkbox" && matches!(sheet.value(anchor), CellValue::Bool(_) | CellValue::Empty) {
                    crate::complex_format::checkbox(
                        sheet.value(anchor) == CellValue::Bool(true),
                        sheet.cell(anchor).is_some_and(|c| c.formula.is_some()),
                        rect,
                        layout.zoom as f32,
                        window,
                    );
                }
                let (l, t, r, b) = (ox + x + 1., oy + y + 1., ox + x + width - 1., oy + y + height - 1.);
                for (line, a, z) in [
                    (style.borders.left, [l, t], [l, b]),
                    (style.borders.top, [l, t], [r, t]),
                    (style.borders.right, [r, t], [r, b]),
                    (style.borders.bottom, [l, b], [r, b]),
                    (style.borders.diag_down, [l, t], [r, b]),
                    (style.borders.diag_up, [l, b], [r, t]),
                ] {
                    crate::complex_format::border(a, z, line.style, rgb_color(line.color, &state.view, 0x3c5144), layout.zoom as f32, clip, window);
                }
            });
            let formatted = joysheet_engine::display::format(&sheet.value(anchor), style.num_fmt.as_str(), &state.view.workbook);
            if !look.as_ref().and_then(|l| l.style.as_ref()).is_some_and(|s| s.font.color != Color::Auto)
                && let Some([r, g, b]) = formatted.color.and_then(|c| c.rgb())
            {
                color = u32::from_be_bytes([0, r, g, b]);
            }
            let mut text = if look.as_ref().is_some_and(|l| l.hide_value)
                || (style.num_fmt.as_str() == "checkbox" && matches!(sheet.value(anchor), CellValue::Bool(_) | CellValue::Empty))
            {
                String::new()
            } else if !sheet.show_formulas {
                formatted.text
            } else {
                joysheet_engine::display::cell_text(&state.view.workbook, sheet, anchor)
            };
            let align = if style.align.h == HAlign::General {
                if matches!(sheet.value(anchor), CellValue::Number(_)) { HAlign::Right } else { HAlign::Left }
            } else {
                style.align.h
            };
            let font_size = style.font.size * 4. / 3. * layout.zoom as f32;
            let mut text_width = width;
            if align == HAlign::CenterAcross && merge.is_none() {
                for next in anchor.col.saturating_add(1)..MAX_COLS.min(anchor.col.saturating_add(128)) {
                    let at = CellRef::new(anchor.row, next);
                    if !sheet.value(at).is_empty()
                        || sheet.merge_at(at).is_some()
                        || state.view.workbook.styles.get(sheet.style_id(at)).align.h != HAlign::CenterAcross
                    {
                        break;
                    }
                    text_width = ((state.geo.cols.start(next + 1) - state.geo.cols.start(anchor.col)) * layout.zoom) as f32;
                    if x + text_width >= clip_right {
                        break;
                    }
                }
            }
            if !text.is_empty() && !sheet.show_formulas && !style.align.wrap && !style.align.shrink && style.align.rotation == 0 {
                let font = Font {
                    family: style.font.name.clone().into(),
                    weight: if style.font.bold { FontWeight::BOLD } else { FontWeight::NORMAL },
                    style: if style.font.italic { FontStyle::Italic } else { FontStyle::Normal },
                    ..Default::default()
                };
                let measure = |s: &str| {
                    f32::from(
                        window
                            .text_system()
                            .shape_line(
                                s.to_string().into(),
                                px(font_size),
                                &[TextRun { len: s.len(), font: font.clone(), ..Default::default() }],
                                None,
                            )
                            .width(),
                    )
                };
                if let Some((ch, pos)) = formatted.fill
                    && text.is_char_boundary(pos)
                {
                    let count = ((text_width - 12. - measure(&text)) / measure(&ch.to_string()).max(1.)).floor().clamp(0., 4096.) as usize;
                    text.insert_str(pos, &ch.to_string().repeat(count));
                }
                if formatted.numeric && style.num_fmt.as_str() != "General" && measure(&text) > text_width - 12. {
                    text = "#".repeat(((text_width - 12.) / measure("#").max(1.)).floor().clamp(1., 128.) as usize);
                }
            }
            if merge.is_none() && !style.align.wrap && matches!(sheet.value(anchor), CellValue::Text(_)) && matches!(align, HAlign::Left) {
                for next in anchor.col.saturating_add(1)..MAX_COLS.min(anchor.col.saturating_add(128)) {
                    let next_cell = CellRef::new(anchor.row, next);
                    if sheet.cell(next_cell).is_some_and(|cell| cell.formula.is_some() || !matches!(cell.value, CellValue::Empty))
                        || sheet.merge_at(next_cell).is_some()
                    {
                        break;
                    }
                    text_width = ((state.geo.cols.start(next + 1) - state.geo.cols.start(anchor.col)) * layout.zoom) as f32;
                    if x + text_width >= clip_right {
                        break;
                    }
                }
            }
            if let Some((set, index)) = look.as_ref().and_then(|v| v.icon.as_ref()) {
                crate::cf_icons::paint(set, *index, bounds(ox + x + width - 18., oy + y, 18., height), window, cx);
                text_width = (width - 18.).max(0.);
            }
            cells.push(PaintedCell { rect, clip, style, text, color, align, font_size, text_width });
            if let Some(comment) = sheet.comments.get(&anchor) {
                crate::charts::path(
                    &[[width - 7., 0.], [width, 0.], [width, 7.]],
                    true,
                    Some(if comment.resolved { [120, 130, 125, 255] } else { [142, 64, 177, 255] }),
                    None,
                    point(px(ox + x), px(oy + y)),
                    1.,
                    window,
                );
            }
        }
    }
    timing.cells_ms = cells_started.elapsed().as_secs_f64() * 1000.;
    let text_started = Instant::now();
    for cell in &cells {
        let font = Font {
            family: cell.style.font.name.clone().into(),
            weight: if cell.style.font.bold { FontWeight::BOLD } else { FontWeight::NORMAL },
            style: if cell.style.font.italic { FontStyle::Italic } else { FontStyle::Normal },
            ..Default::default()
        };
        let text_bounds = Bounds::new(cell.rect.origin, size(px(cell.text_width), cell.rect.size.height));
        let clip = cell.clip.intersect(&text_bounds);
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            paint_cell_text(
                &cell.text,
                font,
                cell.font_size,
                cell.color,
                f32::from(cell.rect.origin.x),
                f32::from(cell.rect.origin.y),
                cell.text_width,
                f32::from(cell.rect.size.height),
                cell.align,
                &cell.style,
                window,
                cx,
            )
        });
    }
    timing.text_ms = text_started.elapsed().as_secs_f64() * 1000.;
    crate::charts::paint(&state.view, &state.geo, layout, area, &state.chart_cache, window, cx);
    crate::objects::paint(&state.view, layout, &state.geo, area, &state.images, window, cx);
    for reference in &state.references {
        let r = reference.range;
        for frozen_row in [false, true] {
            for frozen_col in [false, true] {
                let left = ROW_HEADER + (state.geo.cols.start(r.start.col) * layout.zoom - if frozen_col { 0. } else { layout.scroll_x }) as f32;
                let right = ROW_HEADER
                    + (state.geo.cols.start(r.end.col.saturating_add(1)) * layout.zoom - if frozen_col { 0. } else { layout.scroll_x }) as f32;
                let top = COL_HEADER + (state.geo.rows.start(r.start.row) * layout.zoom - if frozen_row { 0. } else { layout.scroll_y }) as f32;
                let bottom = COL_HEADER
                    + (state.geo.rows.start(r.end.row.saturating_add(1)) * layout.zoom - if frozen_row { 0. } else { layout.scroll_y }) as f32;
                let x = if frozen_col { ROW_HEADER } else { ROW_HEADER + layout.frozen_width };
                let y = if frozen_row { COL_HEADER } else { COL_HEADER + layout.frozen_height };
                let w = if frozen_col { layout.frozen_width } else { layout.width - x };
                let h = if frozen_row { layout.frozen_height } else { layout.height - y };
                let clip = bounds(ox + x, oy + y, w, h);
                window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                    for b in [
                        bounds(ox + left, oy + top, right - left, 2.),
                        bounds(ox + left, oy + top, 2., bottom - top),
                        bounds(ox + right - 2., oy + top, 2., bottom - top),
                        bounds(ox + left, oy + bottom - 2., right - left, 2.),
                    ] {
                        fill(b, crate::formula_feedback::COLORS[reference.color], window);
                    }
                });
            }
        }
    }
    for row in &layout.rows {
        for col in &layout.cols {
            let cell = CellRef::new(row.index, col.index);
            if filter_header(sheet, cell) {
                paint_text(
                    "▾",
                    Font::default(),
                    12.,
                    0x147d59,
                    ox + col.start + col.size - 16.,
                    oy + row.start,
                    16.,
                    row.size,
                    HAlign::Center,
                    window,
                    cx,
                );
            }
        }
    }
    // The selection outline is clipped per frozen quadrant, matching the geometry used by cells.
    let selection = state.selection.current();
    for frozen_row in [false, true] {
        for frozen_col in [false, true] {
            let sx = ROW_HEADER + (state.geo.cols.start(selection.start.col) * layout.zoom - if frozen_col { 0. } else { layout.scroll_x }) as f32;
            let sy = COL_HEADER + (state.geo.rows.start(selection.start.row) * layout.zoom - if frozen_row { 0. } else { layout.scroll_y }) as f32;
            let sw = ((state.geo.cols.start(selection.end.col.saturating_add(1)) - state.geo.cols.start(selection.start.col)) * layout.zoom) as f32;
            let sh = ((state.geo.rows.start(selection.end.row.saturating_add(1)) - state.geo.rows.start(selection.start.row)) * layout.zoom) as f32;
            let x = if frozen_col { ROW_HEADER } else { ROW_HEADER + layout.frozen_width };
            let y = if frozen_row { COL_HEADER } else { COL_HEADER + layout.frozen_height };
            let right = if frozen_col { ROW_HEADER + layout.frozen_width } else { layout.width };
            let bottom = if frozen_row { COL_HEADER + layout.frozen_height } else { layout.height };
            let clip = bounds(ox + x, oy + y, (right - x).max(0.), (bottom - y).max(0.));
            window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                for rect in [
                    bounds(ox + sx, oy + sy, sw, 2.),
                    bounds(ox + sx, oy + sy, 2., sh),
                    bounds(ox + sx + sw - 2., oy + sy, 2., sh),
                    bounds(ox + sx, oy + sy + sh - 2., sw, 2.),
                ] {
                    fill(rect, 0x148256, window);
                }
            });
        }
    }
    // Row and column headers cover the scrolled content and have their own stable hit geometry.
    fill(bounds(ox, oy, layout.width, COL_HEADER), 0xf4f7f4, window);
    fill(bounds(ox, oy, ROW_HEADER, layout.height), 0xf4f7f4, window);
    for col in &layout.cols {
        let selected = col.index >= selection.start.col && col.index <= selection.end.col;
        let clip = if col.frozen {
            bounds(ox + ROW_HEADER, oy, layout.frozen_width, COL_HEADER)
        } else {
            bounds(ox + ROW_HEADER + layout.frozen_width, oy, layout.width - ROW_HEADER - layout.frozen_width, COL_HEADER)
        };
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            if selected {
                fill(bounds(ox + col.start, oy, col.size, COL_HEADER), 0xe2efe5, window);
            }
            let text = joysheet_engine::core::col_to_letters(col.index);
            paint_text(
                &text,
                Font::default(),
                12.,
                if selected { 0x147d59 } else { 0x718277 },
                ox + col.start,
                oy,
                col.size,
                COL_HEADER,
                HAlign::Center,
                window,
                cx,
            );
            fill(bounds(ox + col.start + col.size - 0.5, oy, 0.5, COL_HEADER), 0xdce4dd, window);
        });
    }
    for row in &layout.rows {
        let selected = row.index >= selection.start.row && row.index <= selection.end.row;
        let clip = if row.frozen {
            bounds(ox, oy + COL_HEADER, ROW_HEADER, layout.frozen_height)
        } else {
            bounds(ox, oy + COL_HEADER + layout.frozen_height, ROW_HEADER, layout.height - COL_HEADER - layout.frozen_height)
        };
        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
            if selected {
                fill(bounds(ox, oy + row.start, ROW_HEADER, row.size), 0xe2efe5, window);
            }
            paint_text(
                &sheet
                    .rows
                    .get(&row.index)
                    .filter(|r| r.outline > 0 || r.collapsed)
                    .map(|r| format!("{}{}", if r.collapsed { "+" } else { "−" }, row.index + 1))
                    .unwrap_or_else(|| (row.index + 1).to_string()),
                Font::default(),
                12.,
                if selected { 0x147d59 } else { 0x718277 },
                ox,
                oy + row.start,
                ROW_HEADER - 5.,
                row.size,
                HAlign::Right,
                window,
                cx,
            );
            fill(bounds(ox, oy + row.start + row.size - 0.5, ROW_HEADER, 0.5), 0xdce4dd, window);
        });
    }
    fill(bounds(ox, oy, ROW_HEADER, COL_HEADER), 0xeaf0eb, window);
    fill(bounds(ox + ROW_HEADER - 1., oy, 1., layout.height), 0xd0dcd2, window);
    fill(bounds(ox, oy + COL_HEADER - 1., layout.width, 1.), 0xd0dcd2, window);
    if layout.frozen_width > 0. {
        fill(bounds(ox + ROW_HEADER + layout.frozen_width - 1., oy, 1.5, layout.height), 0x9eb5a5, window);
    }
    if layout.frozen_height > 0. {
        fill(bounds(ox, oy + COL_HEADER + layout.frozen_height - 1., layout.width, 1.5), 0x9eb5a5, window);
    }
    let scrollbar_started = Instant::now();
    let (horizontal, vertical) = crate::grid_tools::axes(layout, &state.geo);
    timing.scrollbar_ms = scrollbar_started.elapsed().as_secs_f64() * 1000.;
    fill(bounds(ox + layout.width, oy, 14., layout.height + 14.), 0xf5f7f5, window);
    fill(bounds(ox, oy + layout.height, layout.width, 14.), 0xf5f7f5, window);
    fill(bounds(ox + layout.width + 4., oy + COL_HEADER + vertical.start, 6., vertical.thumb), 0xbccac0, window);
    fill(bounds(ox + ROW_HEADER + horizontal.start, oy + layout.height + 4., horizontal.thumb, 6.), 0xbccac0, window);
    cells.len()
}

fn paint_cell_text(
    text: &str,
    font: Font,
    mut font_size: f32,
    color: u32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    align: HAlign,
    style: &CellStyle,
    window: &mut Window,
    cx: &mut App,
) {
    let indent = style.align.indent as f32 * font_size * 0.75;
    let base_size = font_size;
    if style.font.vert != joysheet_engine::model::VertAlign::Baseline {
        font_size *= 0.65;
    }
    let x = if align == HAlign::Right { x } else { x + indent };
    let width = (width - indent).max(0.);
    let measure = |text: &str, sz: f32, window: &Window| {
        f32::from(
            window
                .text_system()
                .shape_line(text.to_string().into(), px(sz), &[TextRun { len: text.len(), font: font.clone(), ..Default::default() }], None)
                .width(),
        )
    };
    if style.align.shrink && !style.align.wrap {
        let longest = text.lines().map(|s| measure(s, font_size, window)).fold(0f32, f32::max);
        if longest > width - 12. {
            font_size = (font_size * (width - 12.).max(1.) / longest).max(3.);
        }
    }
    let mut lines = Vec::new();
    let mut budget = 8192usize;
    for original in text.lines().take(256) {
        if lines.len() >= 256 || budget == 0 {
            break;
        }
        if !style.align.wrap {
            let line: String = original.chars().take(budget.min(4096)).collect();
            budget = budget.saturating_sub(line.chars().count());
            lines.push(line);
            continue;
        }
        let mut line = String::new();
        let mut advance = 0.;
        for ch in original.chars().take(4096) {
            if budget == 0 || lines.len() >= 256 {
                break;
            }
            budget -= 1;
            let cw = measure(&ch.to_string(), font_size, window);
            if style.align.wrap && advance + cw > width - 12. && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                advance = 0.;
            }
            line.push(ch);
            advance += cw;
        }
        lines.push(line);
    }
    if style.align.rotation == 255 {
        lines = text.chars().filter(|c| !c.is_control()).take(4096).map(|c| c.to_string()).collect();
    }
    if style.align.rotation != 0 && style.align.rotation != 255 {
        let block = crate::outline_text::TextBlock {
            lines: &lines,
            font: &font,
            size: font_size,
            angle: -(style.align.rotation.clamp(-90, 90) as f32).to_radians(),
            underline: style.font.underline != joysheet_engine::model::Underline::None,
            strike: style.font.strike,
            outline: 0.,
        };
        let (w, h) = block.dimensions(window);
        let [left, top, rw, rh] = crate::outline_text::rotated_box(0., 0., w, h, block.angle);
        let dx = match align {
            HAlign::Right => width - rw - 6.,
            HAlign::Center | HAlign::CenterAcross => (width - rw) / 2.,
            _ => 6.,
        };
        let dy = match style.align.v {
            joysheet_engine::model::VAlign::Top => 0.,
            joysheet_engine::model::VAlign::Center => (height - rh) / 2.,
            _ => height - rh,
        };
        if block.paint(point(px(x + dx - left), px(y + dy - top)), 0., 0., color, window, cx) {
            return;
        }
    }
    if align == HAlign::Fill && lines.len() == 1 {
        let line = &lines[0];
        let w = measure(line, font_size, window);
        if w > 0. {
            lines[0] = line.repeat((((width - 12.) / w).ceil().clamp(1., 128.) as usize).min(8192 / line.len().max(1)));
        }
    }
    let line_height = font_size * 1.22;
    let total = line_height * lines.len() as f32;
    let spread_v = matches!(style.align.v, joysheet_engine::model::VAlign::Distributed | joysheet_engine::model::VAlign::Justify) && lines.len() > 1;
    let gap = if spread_v { (height - total).max(0.) / (lines.len() - 1) as f32 } else { 0. };
    let top = match style.align.v {
        joysheet_engine::model::VAlign::Top => y,
        joysheet_engine::model::VAlign::Center => y + (height - total).max(0.) / 2.,
        _ => y + (height - total).max(0.),
    };
    for (i, line) in lines.iter().enumerate() {
        let yy = (if spread_v { y } else { top }) + i as f32 * (line_height + gap)
            - if style.font.vert == joysheet_engine::model::VertAlign::Superscript { base_size * 0.35 } else { 0. };
        if yy >= y + height {
            break;
        }
        if align == HAlign::Distributed || (align == HAlign::Justify && i + 1 < lines.len()) {
            let words: Vec<_> = if align == HAlign::Distributed {
                line.chars().map(|c| c.to_string()).collect()
            } else {
                line.split_whitespace().map(str::to_string).collect()
            };
            let total = words.iter().map(|w| measure(w, font_size, window)).sum::<f32>();
            let space = (width - 12. - total).max(0.) / words.len().saturating_sub(1).max(1) as f32;
            let mut at = x;
            for word in words {
                let w = measure(&word, font_size, window);
                paint_text(&word, font.clone(), font_size, color, at, yy, w + 12., line_height, HAlign::Left, window, cx);
                at += w + space;
            }
        } else {
            paint_text(line, font.clone(), font_size, color, x, yy, width, line_height, align, window, cx);
        }
        let w = measure(line, font_size, window).min((width - 12.).max(0.));
        let xx = match align {
            HAlign::Right => x + width - w - 6.,
            HAlign::Center | HAlign::CenterAcross => x + (width - w) / 2.,
            _ => x + 6.,
        };
        if style.font.strike {
            fill(bounds(xx, yy + line_height * 0.52, w, 1.), color, window);
        }
        if style.font.underline != joysheet_engine::model::Underline::None {
            fill(bounds(xx, yy + line_height - 2., w, 1.), color, window);
            if matches!(style.font.underline, joysheet_engine::model::Underline::Double | joysheet_engine::model::Underline::DoubleAccounting) {
                fill(bounds(xx, yy + line_height - 4., w, 1.), color, window);
            }
        }
    }
}

impl Desktop {
    pub fn render_grid(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let view = cx.entity();
        let mut body = div()
            .id("worksheet")
            .flex_1()
            .min_h_0()
            .relative()
            .overflow_hidden()
            .bg(rgb(0xffffff))
            .capture_any_mouse_down(cx.listener(|this, event, window, cx| this.grid_mouse_down(event, window, cx)))
            .on_mouse_move(cx.listener(|this, event, window, cx| this.grid_mouse_move(event, window, cx)))
            .capture_any_mouse_up(cx.listener(|this, _, _, cx| {
                this.finish_object_drag(cx);
                this.finish_dimension_drag(cx);
                this.drag_anchor = None;
                this.edge_drag = None;
                this.reference_pick = None;
                this.bar_drag = None;
            }))
            .on_scroll(cx.listener(|this, event: &ScrollEvent, _, cx| {
                if this.validation_prompt.is_some()
                    || this.trust_request.is_some()
                    || this.confirm_close
                    || this.palette_open
                    || this.preview.is_some()
                {
                    return;
                }
                if event.modifiers.control || event.modifiers.platform {
                    this.zoom(if f32::from(event.delta.pixel_delta(px(20.)).y) > 0. { 10 } else { -10 }, cx);
                } else {
                    let delta = event.delta.pixel_delta(px(20.));
                    let mut dx = f32::from(delta.x);
                    let mut dy = f32::from(delta.y);
                    if event.modifiers.shift {
                        std::mem::swap(&mut dx, &mut dy);
                    }
                    this.scroll.0 = (this.scroll.0 - dx as f64).max(0.);
                    this.scroll.1 = (this.scroll.1 - dy as f64).max(0.);
                    cx.notify();
                }
                cx.stop_propagation();
            }))
            .child(SheetElement { view, a11y_actions: Default::default() });
        if let Some(d) = &self.resize_drag {
            let at = d.boundary + d.delta;
            body = body.child(if d.column {
                div().absolute().left(px(at)).top_0().w(px(2.)).h_full().bg(rgb(0x148256))
            } else {
                div().absolute().top(px(at)).left_0().h(px(2.)).w_full().bg(rgb(0x148256))
            });
        }
        for (b, clip) in self.object_selection_bounds() {
            body = body.child(
                div().absolute().left(clip.origin.x).top(clip.origin.y).w(clip.size.width).h(clip.size.height).overflow_hidden().child(
                    div()
                        .absolute()
                        .left(b.origin.x - clip.origin.x)
                        .top(b.origin.y - clip.origin.y)
                        .w(b.size.width)
                        .h(b.size.height)
                        .border_2()
                        .border_color(rgb(0x147d59))
                        .when(self.object_selection().len() == 1, |el| {
                            el.child(div().absolute().right(px(-4.)).bottom(px(-4.)).size(px(9.)).bg(rgb(0x147d59)))
                        }),
                ),
            );
        }
        if let (Some(view), Some(geo)) = (self.view(), self.geometry.as_ref()) {
            let active = self.draft.as_ref().map(|d| d.cell).unwrap_or_else(|| self.selection().active);
            let freeze = view.sheet().and_then(|s| s.freeze).unwrap_or((0, 0));
            let mut rect = self.layout.cell_rect(active, geo, freeze);
            if let Some(merge) = view.sheet().and_then(|s| s.merge_at(active)) {
                rect.width = ((geo.cols.start(merge.end.col.saturating_add(1)) - geo.cols.start(merge.start.col)) * self.layout.zoom) as f32;
                rect.height = ((geo.rows.start(merge.end.row.saturating_add(1)) - geo.rows.start(merge.start.row)) * self.layout.zoom) as f32;
            }
            let right = if active.col < freeze.1 { ROW_HEADER + self.layout.frozen_width } else { self.layout.width };
            let bottom = if active.row < freeze.0 { COL_HEADER + self.layout.frozen_height } else { self.layout.height };
            rect.width = rect.width.min((right - rect.x).max(0.));
            rect.height = rect.height.min((bottom - rect.y).max(0.));
            if rect.x >= ROW_HEADER && rect.y >= COL_HEADER && rect.x < self.layout.width && rect.y < self.layout.height {
                let visible = self.draft.as_ref().is_some_and(|d| d.inline);
                body = body.child(
                    div()
                        .absolute()
                        .key_context({
                            let mut k = KeyContext::default();
                            k.add(if self.draft.is_some() { "JoysheetEditor" } else { "JoysheetGrid" });
                            if self.draft.is_none()
                                && view.sheet().is_some_and(|s| view.workbook.styles.get(s.style_id(active)).num_fmt.as_str() == "checkbox")
                            {
                                k.add("JoysheetCheckbox");
                            }
                            k
                        })
                        .left(px(rect.x))
                        .top(px(rect.y))
                        .w(px(rect.width.max(1.)))
                        .h(px(rect.height.max(1.)))
                        .opacity(if visible { 1. } else { 0.001 })
                        .when(visible, |el| el.bg(rgb(0xffffff)).border_2().border_color(rgb(0x148256)))
                        .child(Input::new(&self.editor).id("cell-editor").appearance(false).bordered(false).size_full().aria_label(format!(
                            "{} {} {}",
                            view.sheet().map(|s| s.name.as_str()).unwrap_or(""),
                            active.a1(),
                            if self.draft.is_some() {
                                "单元格编辑器 / Cell editor".to_string()
                            } else {
                                view.sheet()
                                    .map(|s| joysheet_engine::display::cell_text(&view.workbook, s, active).chars().take(512).collect::<String>())
                                    .unwrap_or_default()
                            }
                        ))),
                );
            }
        }
        if self.draft.is_none()
            && let (Some(view), Some(geo)) = (self.view(), self.geometry.as_ref())
            && let Some(sheet) = view.sheet()
        {
            for (index, (cell, comment)) in sheet
                .comments
                .iter()
                .filter(|(at, c)| **at == self.selection().active || !c.threaded && (c.visible || view.show_all_notes))
                .take(24)
                .enumerate()
            {
                let r = self.layout.cell_rect(*cell, geo, sheet.freeze.unwrap_or((0, 0)));
                if r.x < ROW_HEADER || r.y < COL_HEADER || r.x > self.layout.width || r.y > self.layout.height {
                    continue;
                }
                body = body.child(
                    div()
                        .id(("comment-bubble", index))
                        .absolute()
                        .left(px((r.x + r.width + 8.).min((self.layout.width - 280.).max(ROW_HEADER))))
                        .top(px(r.y))
                        .w(px(260.))
                        .max_h(px(220.))
                        .p_3()
                        .bg(rgb(0xfffbe9))
                        .text_color(rgb(0x20332a))
                        .border_1()
                        .border_color(rgb(0xd7c58f))
                        .rounded_md()
                        .shadow_md()
                        .occlude()
                        .text_sm()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(format!("{} · {}", cell.a1(), comment.author))
                        .child(comment.text.clone())
                        .children(comment.replies.iter().take(20).map(|(author, text)| format!("↳ {author}: {text}"))),
                );
            }
        }
        body
    }
    fn grid_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.composing(window, cx) {
            cx.stop_propagation();
            return;
        }
        if self.validation_prompt.is_some() || self.confirm_close || self.trust_request.is_some() || self.palette_open || self.preview.is_some() {
            return;
        }
        if event.button != MouseButton::Left || !self.grid_bounds.contains(&event.position) {
            return;
        }
        let x = f32::from(event.position.x - self.grid_bounds.origin.x);
        let y = f32::from(event.position.y - self.grid_bounds.origin.y);
        if self.begin_dimension_drag(x, y, window, cx) {
            if event.click_count >= 2
                && let Some(d) = self.resize_drag.take()
            {
                let name = joysheet_engine::core::col_to_letters(d.index);
                self.issue(
                    if d.column { "home.autofitColumnWidth" } else { "home.autofitRowHeight" },
                    if d.column { json!({"cols":format!("{name}:{name}")}) } else { json!({"rows":format!("{0}:{0}",d.index+1)}) },
                    After::None,
                    cx,
                );
            }
            cx.stop_propagation();
            return;
        }
        if self.object_mouse_down(x, y, event.modifiers.control || event.modifiers.platform || event.modifiers.shift, window, cx) {
            cx.stop_propagation();
            return;
        }
        if x >= self.layout.width || y >= self.layout.height {
            if let Some(geo) = &self.geometry {
                let horizontal = y >= self.layout.height;
                let (h, v) = crate::grid_tools::axes(&self.layout, geo);
                let axis = if horizontal { h } else { v };
                let at = if horizontal { x - ROW_HEADER } else { y - COL_HEADER };
                let offset = if at >= axis.start && at <= axis.start + axis.thumb { at - axis.start } else { axis.thumb / 2. };
                self.bar_drag = Some(crate::grid_tools::ScrollDrag { horizontal, offset, travel: axis.travel, max: axis.max });
                self.move_scrollbar(x, y, cx);
            }
            cx.stop_propagation();
            return;
        }
        if y < COL_HEADER && x >= ROW_HEADER {
            if let Some(col) = self.layout.cols.iter().find(|col| x >= col.start && x < col.start + col.size) {
                let range = format!("{0}:{0}", joysheet_engine::core::col_to_letters(col.index));
                self.commit(After::Command("selection.set".into(), json!({"range":range})), window, cx);
            }
            cx.stop_propagation();
            return;
        }
        if x < ROW_HEADER && y >= COL_HEADER {
            if let Some(row) = self.layout.rows.iter().find(|row| y >= row.start && y < row.start + row.size) {
                if x < 16.
                    && let Some(info) =
                        self.view().and_then(|v| v.sheet()).and_then(|s| s.rows.get(&row.index)).filter(|r| r.outline > 0 || r.collapsed)
                {
                    self.commit(
                        After::Command(
                            if info.collapsed { "data.showDetail" } else { "data.hideDetail" }.into(),
                            json!({"rows":format!("{0}:{0}",row.index+1)}),
                        ),
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                    return;
                }
                self.commit(After::Command("selection.set".into(), json!({"range":format!("{0}:{0}",row.index+1)})), window, cx);
            }
            cx.stop_propagation();
            return;
        }
        let Some(cell) = self.layout.hit(x, y) else {
            return;
        };
        if self.draft.is_none()
            && self.view().and_then(|v| v.sheet().map(|s| (v, s))).is_some_and(|(v, s)| {
                v.workbook.styles.get(s.style_id(cell)).num_fmt.as_str() == "checkbox"
                    && matches!(s.value(cell), CellValue::Bool(_) | CellValue::Empty)
            })
            && let Some(geo) = self.geometry.as_ref()
        {
            let r = self.layout.cell_rect(cell, geo, self.view().and_then(|v| v.sheet()).and_then(|s| s.freeze).unwrap_or((0, 0)));
            if (x - r.x - r.width / 2.).abs() < 9. * self.layout.zoom as f32 {
                self.issue("selection.set", json!({"range":cell.a1()}), After::None, cx);
                self.issue("cell.toggleCheckbox", json!({"cell":cell.a1()}), After::None, cx);
                self.focus_grid(window, cx);
                cx.stop_propagation();
                return;
            }
        }
        if self.draft.is_none()
            && self.view().and_then(|v| v.sheet()).is_some_and(|s| filter_header(s, cell))
            && self.geometry.as_ref().is_some_and(|g| {
                x >= self.layout.cell_rect(cell, g, self.view().and_then(|v| v.sheet()).and_then(|s| s.freeze).unwrap_or((0, 0))).x
                    + self.layout.cell_rect(cell, g, (0, 0)).width
                    - 16.
            })
        {
            self.select(cell, false, cx);
            self.open_command("data.filterBy", window, cx);
            let column = joysheet_engine::core::col_to_letters(cell.col);
            for field in &self.command_fields {
                if field.key == "column" {
                    field.value.update(cx, |input, cx| input.set_value(column.clone(), window, cx));
                }
            }
            cx.stop_propagation();
            return;
        }
        if let Some(draft) = self.draft.as_ref() {
            if draft.text.starts_with('=') && (cell != draft.cell || self.view().is_some_and(|v| v.sheet != draft.sheet)) {
                let inline = draft.inline;
                let input = if inline { self.editor.clone() } else { self.formula.clone() };
                let start = input.read(cx).selected_range().start;
                let text = self.reference_text(&cell.a1());
                self.reference_pick =
                    Some(crate::desktop::ReferencePick { anchor: cell, range: start..start + text.len(), inline, text: text.clone() });
                self.insert_reference(&text, window, cx);
                cx.stop_propagation();
                return;
            }
            if draft.inline && cell == draft.cell {
                return;
            }
            self.commit(After::Select(cell, event.modifiers.shift), window, cx);
        } else {
            if event.modifiers.control || event.modifiers.platform {
                let mut ranges = self.selection().ranges;
                ranges.push(RangeRef::cell(cell));
                let text = ranges.iter().map(RangeRef::a1).collect::<Vec<_>>().join(",");
                self.issue("selection.set", json!({"range":text,"active":cell.a1()}), After::None, cx);
                self.focus_grid(window, cx);
                cx.stop_propagation();
                return;
            }
            self.select(cell, event.modifiers.shift, cx);
            self.drag_anchor = Some(cell);
            self.focus_grid(window, cx);
            if event.click_count >= 2 {
                self.begin_edit(None, true, window, cx);
            }
        }
        cx.stop_propagation();
    }
    fn grid_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.validation_prompt.is_some() || self.confirm_close || self.palette_open || self.preview.is_some() {
            return;
        }
        if !event.dragging() {
            return;
        }
        let x = f32::from(event.position.x - self.grid_bounds.origin.x);
        let y = f32::from(event.position.y - self.grid_bounds.origin.y);
        if let Some(d) = &mut self.resize_drag {
            d.delta = if d.column { x } else { y } - d.origin;
            cx.notify();
            return;
        }
        self.edge_drag = Some((x, y));
        if let Some(drag) = &mut self.object_drag {
            drag.delta = ((x - drag.start.0) / self.layout.zoom as f32, (y - drag.start.1) / self.layout.zoom as f32);
            cx.notify();
            return;
        }
        let reference_prefix = self.reference_text("");
        if let Some(pick) = self.reference_pick.as_mut() {
            if let Some(cell) = self.layout.hit(x, y) {
                let text = format!("{reference_prefix}{}", RangeRef::new(pick.anchor, cell).a1());
                if text != pick.text {
                    let input = if pick.inline { self.editor.clone() } else { self.formula.clone() };
                    let old = pick.range.clone();
                    pick.range.end = pick.range.start + text.len();
                    pick.text = text.clone();
                    input.update(cx, |input, cx| {
                        input.set_selected_range(old, cx);
                        input.replace_text_in_range(None, &text, window, cx);
                        input.focus(window, cx);
                    });
                }
            }
            return;
        }
        if self.bar_drag.is_some() {
            self.move_scrollbar(x, y, cx);
            return;
        }
        if let (Some(anchor), Some(cell)) = (self.drag_anchor, self.layout.hit(x, y)) {
            if self.draft.is_some() {
                return;
            }
            let range = RangeRef::new(anchor, cell);
            if self.selection().current() == range {
                return;
            }
            let selection = joysheet_engine::Selection::range(range);
            self.local_selection = Some(selection);
            self.issue("selection.set", json!({"range":range.a1(),"active":anchor.a1()}), After::None, cx);
        }
    }
    fn move_scrollbar(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let Some(d) = self.bar_drag else { return };
        let at = if d.horizontal { x - ROW_HEADER } else { y - COL_HEADER };
        let fraction = ((at - d.offset) / d.travel.max(1.)).clamp(0., 1.) as f64;
        if d.horizontal {
            self.scroll.0 = fraction * d.max;
        } else {
            self.scroll.1 = fraction * d.max;
        }
        cx.notify();
    }
}
