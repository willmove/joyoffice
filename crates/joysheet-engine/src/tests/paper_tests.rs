use joysheet_model::{Orientation, Sheet};

use super::*;

fn wb_with(rows: u32, cols: u32) -> Workbook {
    let mut wb = Workbook::new();
    if let Some(sh) = wb.sheet_mut(0) {
        for r in 0..rows {
            for c in 0..cols {
                sh.set_value(CellRef::new(r, c), joysheet_core::Value::Number(1.0));
            }
        }
    }
    wb
}

#[test]
fn pages_match_print_pagination_and_cells_map_both_ways() {
    let wb = wb_with(120, 14);
    let g = PageGrid::build(&wb, 0, 400, 40).unwrap();
    let printed = print::layout_sheet(&wb, 0, &PrintOptions::default(), None).unwrap();
    // The printed area starts at A1, so the view's page edges are the printed ones (the
    // last printed page simply ends with the data).
    let starts = |b: &[(u32, u32)], n: usize| b.iter().take(n).map(|x| x.0).collect::<Vec<_>>();
    assert!(printed.row_bands.len() > 1 && printed.col_bands.len() > 1);
    assert_eq!(starts(&g.row_bands, printed.row_bands.len()), starts(&printed.row_bands, usize::MAX));
    assert_eq!(starts(&g.col_bands, printed.col_bands.len()), starts(&printed.col_bands, usize::MAX));
    assert_eq!(g.data_rows, (0, printed.row_bands.len() - 1));
    assert!(g.covers(400, 40));
    // Every cell maps to a point inside its own page's margins and back.
    for (r, c) in [(0, 0), (5, 3), (60, 9), (119, 13), (300, 30)] {
        let (x, y) = (g.col_x(c) + 1.0, g.row_y(r) + 1.0);
        assert_eq!((g.row_at(y), g.col_at(x)), (r, c), "cell ({r},{c})");
        assert!(g.on_cells(x, y));
        let (i, j) = g.page_at(x, y).unwrap();
        let (cx, cy, _, _) = g.content_rect(i, j);
        assert!(x >= cx && y >= cy);
    }
    // The first row of the second page starts below the first page plus a gap.
    let (a, _) = g.row_bands[1];
    assert!((g.row_y(a) - (g.page_y(1) + g.margins[2])).abs() < 1e-6);
    assert!(g.row_y(a) > g.page_h + g.gap);
}

#[test]
fn margins_gaps_and_header_zones() {
    let mut wb = wb_with(10, 3);
    if let Some(sh) = wb.sheet_mut(0) {
        sh.print.orientation = Orientation::Landscape;
    }
    let g = PageGrid::build(&wb, 0, 50, 20).unwrap();
    assert!(g.page_w > g.page_h, "landscape pages are wider than tall");
    // A point in the top margin hits the header, thirds pick the section.
    let (px, py, w, h) = g.page_rect(0, 0);
    let mid_top = (px + w / 2.0, py + g.margins[2] / 2.0);
    assert_eq!(g.hf_zone_at(mid_top.0, mid_top.1), Some(((0, 0), HfZone { footer: false, section: 1 })));
    assert_eq!(g.hf_zone_at(px + g.margins[0] + 2.0, py + h - 2.0).map(|z| z.1), Some(HfZone { footer: true, section: 0 }));
    assert_eq!(g.hf_zone_at(px + w - g.margins[1] - 2.0, py + 3.0).map(|z| z.1.section), Some(2));
    assert!(!g.on_cells(mid_top.0, mid_top.1), "margins aren't cells");
    // The gap between pages is no page at all.
    assert_eq!(g.page_at(px + w + g.gap / 2.0, py + 10.0), None);
    assert_eq!(g.hf_zone_at(px - 1.0, py + 3.0), None);
    // Page numbers count printed pages only.
    assert_eq!(g.page_number(0, 0), Some((1, 1)));
    assert_eq!(g.page_number(0, 1), None);
    assert!(g.is_data_page(0, 0) && !g.is_data_page(1, 0));
}

#[test]
fn header_zones_and_text_follow_scale_with_document_and_align_with_page_margins() {
    let mut wb = wb_with(10, 3);
    if let Some(sh) = wb.sheet_mut(0) {
        sh.print.scale = 50;
        sh.print.margins = [2.0, 2.0, 0.75, 0.75, 0.3, 0.3];
    }
    let g = PageGrid::build(&wb, 0, 50, 20).unwrap();
    let (px, py, _, _) = g.page_rect(0, 0);
    // Both on: the sections sit between the 2-inch side margins and the text scales with the cells.
    assert_eq!(g.hf_sides, [g.margins[0], g.margins[1]]);
    assert!((g.hf_text_k - 1.0).abs() < 1e-6);
    assert_eq!(g.hf_zone_at(px + 1000.0, py + 3.0).map(|z| z.1.section), Some(2));
    if let Some(sh) = wb.sheet_mut(0) {
        sh.print.hf_scale_with_doc = false;
        sh.print.hf_align_with_margins = false;
    }
    let g = PageGrid::build(&wb, 0, 50, 20).unwrap();
    // Both off: 0.7 inch (67.2 sheet pixels, 134.4 layout units at 50%) on each side, and the
    // text keeps its point size on paper.
    assert!(g.hf_sides.iter().all(|s| (s - 134.4).abs() < 1e-3), "{:?}", g.hf_sides);
    assert!((g.hf_text_k - 2.0).abs() < 1e-6);
    assert_eq!(g.hf_zone_at(px + 1000.0, py + 3.0).map(|z| z.1.section), Some(1));
}

#[test]
fn print_scale_and_hostile_settings_stay_bounded() {
    let mut wb = wb_with(3, 3);
    if let Some(sh) = wb.sheet_mut(0) {
        sh.print.scale = 50;
        sh.print.margins = [f32::NAN, 1e9, -4.0, f32::INFINITY, 0.0, 0.0];
        sh.print.row_breaks = vec![0, 1, 2, u32::MAX];
    }
    let g = PageGrid::build(&wb, 0, MAX_ROWS, MAX_COLS).unwrap();
    assert!(g.rows_complete && g.cols_complete);
    assert!(g.page_w.is_finite() && g.page_h.is_finite() && g.page_w > 0.0);
    let (ex, ey) = g.extent();
    assert!(ex.is_finite() && ey.is_finite());
    assert_eq!(g.row_at(ey * 2.0), MAX_ROWS - 1);
    assert_eq!(g.col_at(-5.0), 0);
    assert_eq!(need_for(&Sheet::new("S"), f64::MAX, f64::MAX), (MAX_ROWS - 1, MAX_COLS - 1));
    assert!(PageGrid::build(&wb, 7, 0, 0).is_none());
}
