//! Soft wipe and split masks. The incoming texture stays in slide coordinates.
//! Equal-opacity contours are parallel lines, so only the moving band needs a mesh.
use crate::transition::{FULL, Layer, P, R, Source, corners};
use crate::transition_geom::clip;

struct Sweep {
    /// Distance from the entering edge in units of one complete sweep.
    normal: P,
    offset: f64,
    bounds: R,
}

impl Sweep {
    fn draw(&self, clock: f64, result: &mut Vec<Layer>) {
        let distance = |opacity_position: f64| ((clock - opacity_position) / 1.02).clamp(0.0, 1.0);
        let mut start = 0.0;
        // PowerPoint's feather has a smooth cubic profile. Eight linear segments
        // bound its interpolation error to under 0.012 throughout the moving band.
        for step in 0..=8 {
            let end = distance(1.0 - f64::from(step) / 8.0);
            if end > start {
                self.band(start, end, clock, result);
            }
            start = end;
        }
    }

    fn opacity(&self, point: P, clock: f64) -> f64 {
        let distance = self.normal[0] * point[0] + self.normal[1] * point[1] + self.offset;
        let u = (clock - 1.02 * distance).clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    }

    fn band(&self, low: f64, high: f64, clock: f64, result: &mut Vec<Layer>) {
        let [nx, ny] = self.normal;
        if nx == 0.0 || ny == 0.0 {
            let axis = usize::from(nx == 0.0);
            let near = (low - self.offset) / self.normal[axis];
            let far = (high - self.offset) / self.normal[axis];
            let mut rect = self.bounds;
            rect[axis] = near.min(far);
            rect[axis + 2] = near.max(far);
            let fade = corners(rect).map(|p| self.opacity(p, clock));
            result.push(Layer::inplace(Source::New, rect, 1.0).faded(fade));
            return;
        }
        let before = clip(&corners(self.bounds), self.normal, high - self.offset);
        let polygon = clip(&before, [-nx, -ny], self.offset - low);
        if let Some((&first, tail)) = polygon.split_first() {
            for edge in tail.windows(2) {
                let triangle = [first, edge[0], edge[1]];
                result.push(Layer::tri_uv(
                    Source::New,
                    triangle,
                    triangle,
                    triangle.map(|p| self.opacity(p, clock)),
                    1.0,
                ));
            }
        }
    }
}

pub(crate) fn layers(split: bool, option: &str, t: f64) -> Vec<Layer> {
    // Fitted to one direction in a 30 fps PowerPoint movie. The other eleven
    // direction/orientation cases share its timing and were held out for validation.
    let clock = 2.02 * t + 0.032;
    let mut result = vec![Layer::full(Source::Old, 1.0)];
    if split {
        let axis = usize::from(option.starts_with("horz"));
        let inward = option.ends_with("In");
        for half in 0..2 {
            let start = f64::from(half) * 0.5;
            let increasing = (half == 0) == inward;
            let mut normal = [0.0; 2];
            normal[axis] = if increasing { 2.0 } else { -2.0 };
            let offset = if increasing {
                -2.0 * start
            } else {
                2.0 * start + 1.0
            };
            let mut bounds = FULL;
            bounds[axis] = start;
            bounds[axis + 2] = start + 0.5;
            Sweep {
                normal,
                offset,
                bounds,
            }
            .draw(clock, &mut result);
        }
    } else {
        let (normal, offset) = match option {
            "l" => ([-1., 0.], 1.),
            "u" => ([0., -1.], 1.),
            "d" => ([0., 1.], 0.),
            "lu" => ([-0.5, -0.5], 1.),
            "ru" => ([0.5, -0.5], 0.5),
            "ld" => ([-0.5, 0.5], 0.5),
            "rd" => ([0.5, 0.5], 0.),
            _ => ([1., 0.], 0.),
        };
        Sweep {
            normal,
            offset,
            bounds: FULL,
        }
        .draw(clock, &mut result);
    }
    result
}
