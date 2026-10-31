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
