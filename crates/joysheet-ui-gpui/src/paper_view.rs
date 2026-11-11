//! Live page-layout editing, using exactly the engine's print pagination.
use gpui_kit::*;
use joysheet_app::{
    WindowSnapshot,
    geometry::{COL_HEADER, GridLayout, ROW_HEADER},
};
use joysheet_engine::{model::SheetViewMode, paper::PageGrid};

pub(crate) fn background(
    view: &WindowSnapshot,
    layout: &GridLayout,
    paper: Option<&PageGrid>,
    area: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let page_layout = view.sheet().is_some_and(|s| s.view == SheetViewMode::PageLayout);
    window.paint_quad(fill(area, rgb(if page_layout { 0xe6e6e6 } else { 0xffffff })));
    let Some(paper) = paper.filter(|_| page_layout) else { return };
    let k = layout.zoom;
    let ((r0, r1), (c0, c1)) = paper.pages_in(
        layout.scroll_x / k,
        layout.scroll_y / k,
        (layout.scroll_x + layout.width as f64) / k,
        (layout.scroll_y + layout.height as f64) / k,
    );
    let Some(sheet) = view.sheet() else { return };
    for i in r0..=r1 {
        for j in c0..=c1 {
            let (x, y, w, h) = paper.page_rect(i, j);
            let origin = area.origin + point(px(ROW_HEADER + (x * k - layout.scroll_x) as f32), px(COL_HEADER + (y * k - layout.scroll_y) as f32));
            let page = Bounds::new(origin, size(px((w * k) as f32), px((h * k) as f32)));
            window.paint_quad(fill(page + point(px(2.), px(2.)), rgb(0xc4c4c4)));
            window.paint_quad(fill(page, rgb(0xffffff)));
            let (number, pages) = paper.page_number(i, j).unwrap_or((1, 1));
            let (date, time) = joysheet_engine::cmd::print::hf_date_time(&view.workbook);
            let context = joysheet_engine::cmd::print::HfContext { page: number, pages, sheet: sheet.name.clone(), date, time, ..Default::default() };
            let page_kind = sheet.print.hf_page(number == 1, number);
            for (footer, spec) in [(false, sheet.print.hf(page_kind, false)), (true, sheet.print.hf(page_kind, true))] {
                let sections = joysheet_engine::cmd::print::expand_header_footer(spec, &context);
                for (section, value) in sections.iter().enumerate() {
                    let x = match section {
                        0 => paper.hf_sides[0],
                        1 => w / 2.,
                        _ => w - paper.hf_sides[1],
                    };
                    let y = if footer { h - paper.margins[5] } else { paper.margins[4] };
                    let text = if value.text.is_empty() {
                        if section == 1 { if footer { "单击可添加页脚" } else { "单击可添加页眉" } } else { "" }
                    } else {
                        &value.text
                    };
                    let prim = joysheet_chart::Prim::Text {
                        x: x as f32,
                        y: y as f32,
                        text: text.into(),
                        size: 13. * paper.hf_text_k,
                        color: if value.text.is_empty() { [145, 145, 145, 255] } else { [32, 32, 32, 255] },
                        bold: false,
                        align: match section {
                            0 => joysheet_chart::HAlign::Left,
                            1 => joysheet_chart::HAlign::Center,
                            _ => joysheet_chart::HAlign::Right,
                        },
                        valign: joysheet_chart::VAlign::Middle,
                        rotation: 0.,
                    };
                    crate::charts::paint_primitive(&prim, origin, k as f32, window, cx);
                }
            }
        }
    }
}

pub(crate) fn breaks(
    view: &WindowSnapshot,
    layout: &GridLayout,
    paper: Option<&PageGrid>,
    geo: &joysheet_app::geometry::SheetGeometry,
    area: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(sheet) = view.sheet().filter(|s| s.view == SheetViewMode::PageBreakPreview) else { return };
    let Some(paper) = paper else { return };
    let clip = Bounds::new(
        area.origin + point(px(ROW_HEADER), px(COL_HEADER)),
        size(px((layout.width - ROW_HEADER).max(0.)), px((layout.height - COL_HEADER).max(0.))),
    );
    let Some(range) = joysheet_engine::cmd::print::print_range(sheet) else { return };
    let first = layout.cell_rect(range.start, geo, (0, 0));
    let last = layout.cell_rect(range.end, geo, (0, 0));
    let (left, top, right, bottom) = (first.x, first.y, last.x + last.width, last.y + last.height);
    window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
        for (x, y, w, h) in [
            (ROW_HEADER, COL_HEADER, left - ROW_HEADER, layout.height - COL_HEADER),
            (right, COL_HEADER, layout.width - right, layout.height - COL_HEADER),
            (left, COL_HEADER, right - left, top - COL_HEADER),
            (left, bottom, right - left, layout.height - bottom),
        ] {
            if w > 0. && h > 0. {
                window.paint_quad(fill(Bounds::new(area.origin + point(px(x), px(y)), size(px(w), px(h))), rgba(0x80808090)));
            }
        }
        crate::charts::path(
            &[[left, top], [right, top], [right, bottom], [left, bottom]],
            true,
            None,
            Some(([45, 82, 204, 255], 2.)),
            area.origin,
            1.,
            window,
        );
    });
    let printed = Bounds::new(area.origin + point(px(left), px(top)), size(px(right - left), px(bottom - top)));
    window.with_content_mask(Some(ContentMask { bounds: clip.intersect(&printed) }), |window| {
        for &(row, _) in paper.row_bands.iter().skip(1) {
            let y = layout.cell_rect(joysheet_engine::core::CellRef::new(row, 0), geo, (0, 0)).y;
            if y < COL_HEADER || y > layout.height {
                continue;
            }
            let manual = sheet.print.row_breaks.contains(&row);
            let p = joysheet_chart::Prim::Line {
                pts: vec![[ROW_HEADER, y], [layout.width, y]],
                color: [45, 82, 204, 255],
                width: if manual { 2. } else { 1. },
                dash: !manual,
            };
            crate::charts::paint_primitive(&p, area.origin, 1., window, cx);
        }
        for &(col, _) in paper.col_bands.iter().skip(1) {
            let x = layout.cell_rect(joysheet_engine::core::CellRef::new(0, col), geo, (0, 0)).x;
            if x < ROW_HEADER || x > layout.width {
                continue;
            }
            let manual = sheet.print.col_breaks.contains(&col);
            let p = joysheet_chart::Prim::Line {
                pts: vec![[x, COL_HEADER], [x, layout.height]],
                color: [45, 82, 204, 255],
                width: if manual { 2. } else { 1. },
                dash: !manual,
            };
            crate::charts::paint_primitive(&p, area.origin, 1., window, cx);
        }
        if let (Some(r0), Some(r1), Some(c0), Some(c1)) = (layout.rows.first(), layout.rows.last(), layout.cols.first(), layout.cols.last()) {
            let rows = paper.row_bands.partition_point(|b| b.1 < r0.index)..paper.row_bands.partition_point(|b| b.0 <= r1.index);
            let cols = paper.col_bands.partition_point(|b| b.1 < c0.index)..paper.col_bands.partition_point(|b| b.0 <= c1.index);
            for i in rows.take(8) {
                for j in cols.clone().take(8) {
                    if let Some((number, _)) = paper.page_number(i, j) {
                        let (r0, r1) = paper.row_bands[i];
                        let (c0, c1) = paper.col_bands[j];
                        let (r0, r1, c0, c1) = (r0.max(range.start.row), r1.min(range.end.row), c0.max(range.start.col), c1.min(range.end.col));
                        if r0 > r1 || c0 > c1 {
                            continue;
                        }
                        let a = layout.cell_rect(joysheet_engine::core::CellRef::new(r0, c0), geo, (0, 0));
                        let b = layout.cell_rect(joysheet_engine::core::CellRef::new(r1, c1), geo, (0, 0));
                        let p = joysheet_chart::Prim::Text {
                            x: (a.x + b.x + b.width) / 2.,
                            y: (a.y + b.y + b.height) / 2.,
                            text: format!("第 {number} 页"),
                            size: 56_f32.min((b.x + b.width - a.x) / 4.),
                            color: [120, 120, 120, 100],
                            bold: true,
                            align: joysheet_chart::HAlign::Center,
                            valign: joysheet_chart::VAlign::Middle,
                            rotation: 0.,
                        };
                        crate::charts::paint_primitive(&p, area.origin, 1., window, cx);
                    }
                }
            }
        }
    });
}

#[derive(Clone)]
pub(crate) struct BreakDrag {
    pub column: bool,
    pub from: u32,
    pub to: u32,
    pub doc: u64,
    pub sheet: usize,
}
impl crate::desktop::Desktop {
    pub(crate) fn begin_paper_edit(&mut self, x: f32, y: f32, w: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(view) = self.view() else { return false };
        let Some(sheet) = view.sheet() else { return false };
        if sheet.view == SheetViewMode::PageLayout
            && let Some(p) = &self.layout.paper
        {
            let lx = (x - ROW_HEADER) as f64 / self.layout.zoom + self.layout.scroll_x / self.layout.zoom;
            let ly = (y - COL_HEADER) as f64 / self.layout.zoom + self.layout.scroll_y / self.layout.zoom;
            if let Some(((i, j), zone)) = p.hf_zone_at(lx, ly) {
                let number = p.page_number(i, j).map(|p| p.0).unwrap_or(1);
                let page_kind = sheet.print.hf_page(number == 1, number);
                let spec = sheet.print.hf(page_kind, zone.footer);
                let parts = joysheet_engine::cmd::print::split_header_footer(spec);
                let values = serde_json::json!({"page":page_kind.id(),"part":if zone.footer{"footer"}else{"header"},"section":(["left","center","right"][zone.section]),"text":joysheet_engine::cmd::print::codes_to_fields(&parts[zone.section])});
                self.open_command("pageLayout.headerFooter", w, cx);
                self.make_command_fields(&values, w, cx);
                self.params.update(cx, |s, cx| s.set_value(values.to_string(), w, cx));
                return true;
            }
        }
        if sheet.view != SheetViewMode::PageBreakPreview || x < ROW_HEADER || y < COL_HEADER {
            return false;
        }
        let (Some(p), Some(geo)) = (&self.page_grid, &self.geometry) else { return false };
        let Some(range) = joysheet_engine::cmd::print::print_range(sheet) else { return false };
        let first = self.layout.cell_rect(range.start, geo, (0, 0));
        let last = self.layout.cell_rect(range.end, geo, (0, 0));
        let mut hit = None;
        for &(row, _) in p.row_bands.iter().skip(1) {
            if x >= first.x
                && x <= last.x + last.width
                && (self.layout.cell_rect(joysheet_engine::core::CellRef::new(row, 0), geo, (0, 0)).y - y).abs() < 5.
            {
                hit = Some((false, row));
                break;
            }
        }
        for &(col, _) in p.col_bands.iter().skip(1) {
            if y >= first.y
                && y <= last.y + last.height
                && (self.layout.cell_rect(joysheet_engine::core::CellRef::new(0, col), geo, (0, 0)).x - x).abs() < 5.
            {
                hit = Some((true, col));
                break;
            }
        }
        if let Some((column, from)) = hit {
            self.break_drag = Some(BreakDrag { column, from, to: from, doc: view.doc, sheet: view.sheet });
            self.focus_grid(w, cx);
            return true;
        }
        false
    }
    pub(crate) fn finish_break_drag(&mut self, cx: &mut Context<Self>) {
        let Some(d) = self.break_drag.take() else { return };
        if d.from != d.to && self.view().is_some_and(|v| v.doc == d.doc && v.sheet == d.sheet) {
            self.issue(
                "pageLayout.moveBreak",
                serde_json::json!({"axis":if d.column{"column"}else{"row"},"from":d.from,"to":d.to}),
                crate::desktop::After::None,
                cx,
            );
        }
    }
}
