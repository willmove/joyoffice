pub fn dashed(points: &[[f32; 2]], on: f32, off: f32) -> Vec<Vec<[f32; 2]>> {
    let mut segments = Vec::new();
    let mut phase = 0.;
    for pair in points.windows(2) {
        let Some(a) = pair.first() else { continue };
        let Some(b) = pair.get(1) else { continue };
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = dx.hypot(dy);
        if !length.is_finite() || length <= 0. {
            continue;
        }
        let mut t = 0.;
        while t < length && segments.len() < 10000 {
            let visible = phase < on;
            let step = (if visible { on - phase } else { on + off - phase }).min(length - t).max(0.001);
            if visible {
                segments
                    .push(vec![[a[0] + dx * t / length, a[1] + dy * t / length], [a[0] + dx * (t + step) / length, a[1] + dy * (t + step) / length]]);
            }
            t += step;
            phase = (phase + step) % (on + off);
        }
    }
    segments
}
#[cfg(test)]
#[path = "tests/chart_dash_tests.rs"]
mod tests;
