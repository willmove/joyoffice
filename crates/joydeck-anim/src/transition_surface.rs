//! Continuous motion curves and textured surfaces for the cloth and paper presets.
use crate::transition::{Layer, P, Source, corners};

/// Interpolate motion landmarks in normalized slide time. Knots at the contact
/// and release of the surface preserve its overshoot when frame rates change.
pub(crate) fn track<const N: usize>(t: f64, keys: &[(f64, [f64; N])]) -> [f64; N] {
    for pair in keys.windows(2) {
        let ((a, start), (b, end)) = (pair[0], pair[1]);
        if t <= b {
            let mix = ((t - a) / (b - a)).clamp(0., 1.);
            return std::array::from_fn(|i| start[i] + mix * (end[i] - start[i]));
        }
    }
    keys[keys.len() - 1].1
}

/// Draw a deformed texture in tiles, with continuous UVs and per-vertex shade.
/// The same geometry is consumed by the CPU and hardware compositors.
pub(crate) fn texture_grid(
    layers: &mut Vec<Layer>,
    source: Source,
    region: [f64; 2],
    columns: usize,
    rows: usize,
    project: impl Fn(P) -> (P, f64),
) {
    for row in 0..rows {
        for column in 0..columns {
            let x = |col: usize| region[0] + (region[1] - region[0]) * col as f64 / columns as f64;
            let rect = [
                x(column),
                row as f64 / rows as f64,
                x(column + 1),
                (row + 1) as f64 / rows as f64,
            ];
            let points = corners(rect).map(&project);
            let tile = Layer::quad(source, rect, points.map(|p| p.0), 1.);
            layers.push(tile);
            let fade = points.map(|p| p.1.clamp(0., 1.));
            if fade.iter().any(|&s| s > 0.) {
                layers.push(tile.shade(1.).faded(fade));
            }
        }
    }
}
