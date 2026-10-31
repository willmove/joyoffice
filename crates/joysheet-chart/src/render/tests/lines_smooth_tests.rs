use super::*;

#[test]
fn long_runs_are_smoothed_uniformly_within_the_cap() {
    let plot = Bx { x: 0.0, y: 0.0, w: 3000.0, h: 100.0 };
    let pts: Vec<[f32; 2]> = (0..1500).map(|i| [i as f32 * 2.0, if i % 2 == 0 { 20.0 } else { 80.0 }]).collect();
    let out = smooth_run(pts, plot);
    assert!(out.len() <= MAX_LINE_PTS * 2 + 1, "{}", out.len());
    // Two sub-segments per segment all the way to the end, not 100 each and then a straight tail.
    assert_eq!(out.len(), 1499 * 2 + 1);
    assert_eq!(out.last().copied(), Some([2998.0, 80.0]));
}

#[test]
fn spline_overshoot_is_clamped_to_the_plot() {
    let plot = Bx { x: 0.0, y: 10.0, w: 300.0, h: 100.0 };
    // A step right at the plot edges: Catmull-Rom overshoots past both.
    let pts = vec![[0.0, 110.0], [100.0, 110.0], [110.0, 10.0], [200.0, 10.0], [300.0, 10.0]];
    let out = smooth_run(pts, plot);
    assert!(out.len() > 5);
    assert!(out.iter().all(|p| (0.0..=300.0).contains(&p[0]) && (10.0..=110.0).contains(&p[1])));
}
