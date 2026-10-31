use super::*;

#[test]
fn dashed_geometry_stays_within_the_dash_budget() {
    // Forty very long dashed lines, each allowed 10_000 dashes per segment: before the
    // shared cap they eagerly allocated ~1.6 MB of segment pairs × points and a like
    // number of quads before any of it reached a 4-pixel canvas (finding: dash expansion).
    let prims: Vec<Prim> =
        (0..40).map(|i| Prim::Line { pts: vec![[0.0, i as f32], [900.0, i as f32]], color: [0, 0, 0, 255], width: 1.0, dash: true }).collect();
    let img = rasterize(&prims, 2, 2, [255, 255, 255, 255]);
    assert_eq!(img.len(), 2 * 2 * 4);
    // And the geometry itself is bounded even for one huge dashed polyline.
    assert!(stroke_paths(&[[0.0, 0.0], [1e6, 0.0]], 1.0, true, false).len() <= 65_536);
    let prim = [Prim::Line { pts: vec![[0.0, 0.0], [10.0, 0.0]], color: [0, 0, 0, 255], width: 1.0, dash: true }];
    assert_eq!(rasterize(&prim, 1, 1, [0, 0, 0, 255]).len(), 4);
}
