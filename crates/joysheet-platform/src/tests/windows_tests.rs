use super::*;
#[test]
fn layouts_cover_negative_monitor_without_leaving_work_area() {
    let area = Rect { x: -1920, y: -100, width: 1920, height: 1080 };
    for mode in ["horizontal", "vertical", "tiled", "cascade"] {
        let slots = layout(area, 3, mode).unwrap();
        assert_eq!(slots.len(), 3);
        for r in slots {
            assert!(r.x >= area.x && r.y >= area.y && r.x + r.width <= area.x + area.width && r.y + r.height <= area.y + area.height);
        }
    }
    assert!(layout(area, 129, "tiled").is_err());
    assert!(layout(area, 8, "vertical").is_err());
}
