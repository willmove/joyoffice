#[test]
fn upstream_icon_catalog_resolves_to_embedded_assets() {
    use gpui_kit::AssetSource;
    for name in [
        "check",
        "close",
        "plus",
        "minus",
        "chart",
        "line chart",
        "pie chart",
        "scatter",
        "table",
        "sum",
        "filter",
        "lock",
        "comment",
        "note",
        "picture",
        "book",
        "folder",
        "search",
        "calculator",
        "link",
        "pen",
        "share",
        "settings",
        "home",
        "save",
        "theme",
        "shapes",
        "text",
        "trace",
        "validation",
    ] {
        let icon = super::shape_icon(name).unwrap_or_else(|| panic!("Missing icon: {name}"));
        assert!(gpui_kit::assets::AllAssets.load(icon.path().as_ref()).unwrap().is_some(), "Missing embedded SVG: {name}");
    }
}
#[test]
fn frozen_clip_uses_absolute_anchor_including_offsets() {
    let mut sheet = joysheet_engine::model::Sheet::new("Test");
    sheet.freeze = Some((1, 1));
    let geo = joysheet_app::geometry::SheetGeometry::new(&sheet);
    let layout = joysheet_app::geometry::GridLayout::new(&sheet, &geo, 220., 240., 1200., 700.);
    let mut anchor = joysheet_engine::model::Anchor { cell: joysheet_engine::core::CellRef::new(11, 4), ..Default::default() };
    let origin = gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.));
    let clip = super::object_clip(&anchor, &layout, &geo, (1, 1), origin);
    assert!(f32::from(clip.origin.x) > joysheet_app::geometry::ROW_HEADER);
    anchor.dx = -(geo.cols.start(4) as f32);
    let clip = super::object_clip(&anchor, &layout, &geo, (1, 1), origin);
    assert_eq!(f32::from(clip.origin.x), joysheet_app::geometry::ROW_HEADER);
}
#[test]
fn resize_retains_anchor_and_clamps_dimensions() {
    let mut a = joysheet_engine::model::Anchor::default();
    let cell = a.cell;
    super::apply_delta(&mut a, (-100000., 100000.), true);
    assert_eq!((a.width, a.height), (1., 10000.));
    assert_eq!(a.cell, cell);
    super::apply_delta(&mut a, (30., -20.), false);
    assert_eq!((a.dx, a.dy), (30., -20.));
}
