//! DrawingML straight and bent callouts (§20.1.9). Leader vertices are stored
//! as signed (height, width) adjustments; moving a vertex changes only its pair.

use super::{Geometry, Point, Rect, SubPath, adjustment_defaults, fill, fill_only, stroke_only};
use kurbo::BezPath;

#[derive(Clone, Copy)]
pub struct LineCallout {
    defaults: &'static [f64],
    accent: bool,
    border: bool,
    limit: i32,
}

impl LineCallout {
    pub fn named(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(['1', '2', '3'])?;
        let (accent, border) = match stem {
            "callout" => (false, false),
            "borderCallout" => (false, true),
            "accentCallout" => (true, false),
            "accentBorderCallout" => (true, true),
            _ => return None,
        };
        Some(Self {
            defaults: adjustment_defaults(name),
            accent,
            border,
            // Preserve the existing paint budget for imported leaders. The first
            // bordered preset historically supported the full DrawingML range.
            limit: if name == "borderCallout1" {
                i32::MAX
            } else {
                10_000_000
            },
        })
    }

    pub fn vertices(
        self,
        w: f64,
        h: f64,
        adj: &[f64],
    ) -> impl ExactSizeIterator<Item = Point> + '_ {
        let axis = move |i: usize, extent: f64| {
            let value = adj.get(i).copied().unwrap_or(self.defaults[i]);
            let limit = f64::from(self.limit);
            let fraction = if value.is_finite() {
                value.clamp(-limit, limit) / 100_000.0
            } else {
                0.0
            };
            extent * fraction
        };
        (0..self.defaults.len() / 2).map(move |i| Point::new(axis(2 * i + 1, w), axis(2 * i, h)))
    }

    /// Convert a local pointer to the horizontal and vertical DrawingML values.
    pub fn values(self, w: f64, h: f64, point: Point) -> Option<(i32, i32)> {
        if !(w > 0. && h > 0. && w.is_finite() && h.is_finite() && point.is_finite()) {
            return None;
        }
        let limit = f64::from(self.limit);
        let ratio = |p: f64, size: f64| (p / size * 100_000.).round().clamp(-limit, limit) as i32;
        Some((ratio(point.x, w), ratio(point.y, h)))
    }

    /// Keep any extra guides in the model. Fill omitted positions with the
    /// preset defaults before changing the requested pair.
    pub fn set_vertex(self, adj: &mut Vec<f64>, index: u8, x: i32, y: i32) -> bool {
        let i = 2 * usize::from(index);
        if i + 1 >= self.defaults.len()
            || [x, y].iter().any(|v| v.unsigned_abs() > self.limit as u32)
        {
            return false;
        }
        if adj.get(i).copied().unwrap_or(self.defaults[i]) == f64::from(y)
            && adj.get(i + 1).copied().unwrap_or(self.defaults[i + 1]) == f64::from(x)
        {
            return true;
        }
        if adj.len() < self.defaults.len() {
            adj.extend_from_slice(&self.defaults[adj.len()..]);
        }
        adj[i] = f64::from(y);
        adj[i + 1] = f64::from(x);
        true
    }

    pub(super) fn geometry(self, w: f64, h: f64, adj: &[f64]) -> Geometry {
        let rect = Rect::new(0., 0., w, h);
        let mut body = BezPath::new();
        body.move_to((0., 0.));
        body.line_to((w, 0.));
        body.line_to((w, h));
        body.line_to((0., h));
        body.close_path();
        let mut paths: Vec<SubPath> = vec![if self.border {
            fill(body)
        } else {
            fill_only(body)
        }];
        let mut leader = BezPath::new();
        for (i, point) in self.vertices(w, h, adj).enumerate() {
            if i == 0 {
                leader.move_to(point);
                if self.accent {
                    let mut accent = BezPath::new();
                    accent.move_to((point.x, 0.));
                    accent.line_to((point.x, h));
                    paths.push(stroke_only(accent));
                }
            } else {
                leader.line_to(point);
            }
        }
        paths.push(stroke_only(leader));
        Geometry {
            paths,
            text_rect: rect,
            handles: vec![],
            sites: vec![
                Point::new(w, h / 2.),
                Point::new(w / 2., h),
                Point::new(0., h / 2.),
                Point::new(w / 2., 0.),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::CATALOG;

    #[test]
    fn all_leader_vertices_use_independent_signed_axes() -> Result<(), &'static str> {
        let mut count = 0;
        for entry in CATALOG {
            let Some(kind) = LineCallout::named(entry.name) else {
                continue;
            };
            count += 1;
            let adj = [
                24000., -35000., 50000., 100000., -6000., 207000., 83000., 31000.,
            ];
            for (w, h) in [(140., 75.), (56., 115.), (90., 90.)] {
                let g = kind.geometry(w, h, &adj);
                assert_eq!(
                    g.sites,
                    [(w, h / 2.), (w / 2., h), (0., h / 2.), (w / 2., 0.)].map(Point::from)
                );
                assert_eq!(g.text_rect, Rect::new(0., 0., w, h));
                for (i, point) in kind.vertices(w, h, &adj).enumerate() {
                    let (x, y) = kind.values(w, h, point).ok_or("missing adjustment")?;
                    assert_eq!((f64::from(x), f64::from(y)), (adj[2 * i + 1], adj[2 * i]));
                    let mut old = entry.defaults.to_vec();
                    old.push(123456.);
                    let mut edited = old.clone();
                    assert!(kind.set_vertex(&mut edited, i as u8, x, y));
                    let mut expected = old;
                    expected[2 * i..=2 * i + 1].copy_from_slice(&[f64::from(y), f64::from(x)]);
                    assert_eq!(edited, expected);
                }
                let mut untouched = vec![];
                assert!(kind.set_vertex(
                    &mut untouched,
                    0,
                    entry.defaults[1] as i32,
                    entry.defaults[0] as i32
                ));
                assert!(untouched.is_empty());
                assert!(!kind.set_vertex(&mut untouched, 200, 1, 1) && untouched.is_empty());
                assert!(kind.values(0., h, Point::ZERO).is_none());
                assert!(kind.values(w, h, Point::new(f64::NAN, 0.)).is_none());
                assert!(
                    kind.vertices(w, h, &[f64::INFINITY, -f64::MAX])
                        .all(|p| p.is_finite())
                );
            }
        }
        assert_eq!(count, 12);
        Ok(())
    }
}
