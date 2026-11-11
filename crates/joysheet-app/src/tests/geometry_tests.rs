use super::*;
#[test]
fn used_extent_follows_expansion_clear_and_undo() {
    let mut s = joysheet_engine::Session::new();
    s.execute("file.new", serde_json::json!({})).unwrap();
    for (command, params, expected) in [
        ("cell.set", serde_json::json!({"cell":"B2","input":"1"}), CellRef::new(1, 1)),
        ("cell.set", serde_json::json!({"cell":"T1000000","input":"2"}), CellRef::new(999_999, 19)),
        ("cell.set", serde_json::json!({"cell":"T1000000","input":""}), CellRef::new(1, 1)),
        ("edit.undo", serde_json::json!({}), CellRef::new(999_999, 19)),
    ] {
        s.execute(command, params).unwrap();
        let sheet = &s.active().unwrap().wb.sheets[0];
        assert_eq!(SheetGeometry::new(sheet).used_end, expected);
    }
}
#[test]
fn millionth_row_is_precise_and_bounded() {
    let sh = Sheet::new("Large");
    let geo = SheetGeometry::new(&sh);
    let y = geo.rows.start(999_999) + 0.25;
    let view = GridLayout::new(&sh, &geo, 0.0, y, 1280.0, 720.0);
    assert_eq!(view.rows.first().unwrap().index, 999_999);
    assert!((view.rows.first().unwrap().start - (COL_HEADER - 0.25)).abs() < 0.01);
    assert!(view.painted_cell_bound() < 1000);
}
#[test]
fn frozen_cells_win_hit_testing() {
    let mut sh = Sheet::new("Freeze");
    sh.freeze = Some((1, 1));
    sh.zoom = 125;
    let geo = SheetGeometry::new(&sh);
    let view = GridLayout::new(&sh, &geo, 300.0, 500.0, 1000.0, 600.0);
    assert_eq!(view.hit(ROW_HEADER + 4.0, COL_HEADER + 4.0), Some(CellRef::new(0, 0)));
    for row in view.rows.iter().filter(|r| !r.frozen && r.start >= COL_HEADER + view.frozen_height) {
        for col in view.cols.iter().filter(|c| !c.frozen && c.start >= ROW_HEADER + view.frozen_width) {
            let point = (col.start + 1.0, row.start + 1.0);
            assert_eq!(view.hit(point.0, point.1), Some(CellRef::new(row.index, col.index)));
        }
    }
}
#[test]
fn invalid_scroll_and_far_selection_are_safe() {
    let sh = Sheet::new("Sheet");
    let geo = SheetGeometry::new(&sh);
    let view = GridLayout::new(&sh, &geo, f64::NAN, f64::INFINITY, 800.0, 600.0);
    assert_eq!(view.scroll_y, 0.0);
    let (x, y) = view.reveal(CellRef::new(MAX_ROWS - 1, MAX_COLS - 1), &geo, (0, 0));
    let far = GridLayout::new(&sh, &geo, x, y, 800.0, 600.0);
    assert!(far.rows.iter().any(|r| r.index == MAX_ROWS - 1));
    assert!(far.cols.iter().any(|c| c.index == MAX_COLS - 1));
}
#[test]
fn hidden_prefix_and_custom_dimensions_share_hit_geometry() {
    let mut sh = Sheet::new("Hidden");
    for row in 0..128 {
        sh.rows.insert(row, joysheet_engine::model::LineInfo { hidden: true, ..Default::default() });
    }
    sh.cols.insert(0, joysheet_engine::model::LineInfo { size: Some(137.), ..Default::default() });
    let geo = SheetGeometry::new(&sh);
    let view = GridLayout::new(&sh, &geo, 0., 0., 800., 600.);
    assert_eq!(view.rows.first().unwrap().index, 128);
    assert_eq!(view.hit(ROW_HEADER + 136., COL_HEADER + 2.), Some(CellRef::new(128, 0)));
    assert_eq!(view.hit(ROW_HEADER + 138., COL_HEADER + 2.), Some(CellRef::new(128, 1)));
}

#[test]
fn page_layout_cells_and_margins_share_paginated_hit_geometry() {
    let mut s = joysheet_engine::Session::new();
    s.execute("file.new", serde_json::json!({})).unwrap();
    s.execute("cell.set", serde_json::json!({"cell":"J150","input":"End"})).unwrap();
    let wb = &s.active().unwrap().wb;
    let sheet = wb.active().unwrap();
    let geo = SheetGeometry::new(sheet);
    let paper = Arc::new(joysheet_engine::paper::PageGrid::build(wb, 0, 200, 20).unwrap());
    let k = sheet.zoom as f64 / 100. * paper.scale as f64;
    for (row, col) in [(0, 0), (paper.row_bands[1].0, 0), (100, 9)] {
        let x = (paper.col_x(col) * k - ROW_HEADER as f64 - 40.).max(0.);
        let y = (paper.row_y(row) * k - COL_HEADER as f64 - 80.).max(0.);
        let layout = GridLayout::new(sheet, &geo, x, y, 1000., 700.).with_paper(paper.clone(), &geo);
        let at = CellRef::new(row, col);
        let rect = layout.cell_rect(at, &geo, (0, 0));
        assert_eq!(layout.hit(rect.x + rect.width / 2., rect.y + rect.height / 2.), Some(at));
    }
    let layout = GridLayout::new(sheet, &geo, 0., 0., 1000., 700.).with_paper(paper.clone(), &geo);
    assert_eq!(layout.hit(ROW_HEADER + (paper.page_x(0) * k) as f32 + 1., COL_HEADER + (paper.page_y(0) * k) as f32 + 1.), None);
}
