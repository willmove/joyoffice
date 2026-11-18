//! Fixed DrawingML flowchart silhouettes, text frames and ordered glue sites.
//! Dimensions follow ECMA-376 Part 1's DrawingML preset geometry attachment.
//! Coordinates on each symbol's design grid are scaled independently in X and Y.
use super::builder::{Pb, ellipse_arc, fill, fill_only, mid_sites, stroke_only};
use super::catalog::{FillMode, Geometry, SubPath};
use kurbo::{BezPath, Point, Rect};

#[derive(Clone, Copy)]
enum Step {
    Move(f64, f64),
    Line(f64, f64),
    Curve((f64, f64), (f64, f64), (f64, f64)),
    Arc(f64, f64, f64, f64),
    Close,
}
use Step::{Arc, Close, Curve, Line, Move};

struct Frame {
    w: f64,
    h: f64,
}

impl Frame {
    fn path(&self, grid: f64, steps: &[Step]) -> BezPath {
        let (x, y) = (self.w / grid, self.h / grid);
        let mut path = Pb::new();
        for &step in steps {
            match step {
                Move(a, b) => path.m(a * x, b * y),
                Line(a, b) => path.l(a * x, b * y),
                Curve((a, b), (c, d), (e, f)) => path.c(a * x, b * y, c * x, d * y, e * x, f * y),
                Arc(rx, ry, start, sweep) => ellipse_arc(&mut path, rx * x, ry * y, start, sweep),
                Close => path.z(),
            };
        }
        path.take()
    }

    fn polygon(&self, grid: f64, points: &[(f64, f64)]) -> BezPath {
        let mut p = BezPath::new();
        for (i, &(x, y)) in points.iter().enumerate() {
            let point = Point::new(self.w * x / grid, self.h * y / grid);
            if i == 0 {
                p.move_to(point);
            } else {
                p.line_to(point);
            }
        }
        p.close_path();
        p
    }

    fn sites(&self, points: &[(f64, f64)]) -> Vec<Point> {
        points
            .iter()
            .map(|&(x, y)| Point::new(x * self.w, y * self.h))
            .collect()
    }

    fn rect(&self, l: f64, t: f64, r: f64, b: f64) -> Rect {
        Rect::new(l * self.w, t * self.h, r * self.w, b * self.h)
    }
}

// Strokes inside a filled silhouette are an independent layer. This preserves
// its shared brush, transparency and the join where the outline meets a divider.
fn divided(outline: BezPath, dividers: BezPath) -> Vec<SubPath> {
    vec![
        fill_only(outline.clone()),
        stroke_only(dividers),
        stroke_only(outline),
    ]
}

fn paper_stack(frame: &Frame) -> Vec<SubPath> {
    // Each sheet terminates at the visible edge of the sheet in front.
    const GRID: f64 = 21600.;
    let faces = frame.path(
        GRID,
        &[
            Move(0., 20782.),
            Curve((9298., 23542.), (9298., 18022.), (18595., 18022.)),
            Line(18595., 3675.),
            Line(0., 3675.),
            Close,
            Move(1532., 3675.),
            Line(1532., 1815.),
            Line(20000., 1815.),
            Line(20000., 16252.),
            Curve((19298., 16252.), (18595., 16352.), (18595., 16352.)),
            Line(18595., 3675.),
            Close,
            Move(2972., 1815.),
            Line(2972., 0.),
            Line(21600., 0.),
            Line(21600., 14392.),
            Curve((20800., 14392.), (20000., 14467.), (20000., 14467.)),
            Line(20000., 1815.),
            Close,
        ],
    );
    let edges = frame.path(
        GRID,
        &[
            Move(0., 3675.),
            Line(18595., 3675.),
            Line(18595., 18022.),
            Curve((9298., 18022.), (9298., 23542.), (0., 20782.)),
            Close,
            Move(1532., 3675.),
            Line(1532., 1815.),
            Line(20000., 1815.),
            Line(20000., 16252.),
            Curve((19298., 16252.), (18595., 16352.), (18595., 16352.)),
            Move(2972., 1815.),
            Line(2972., 0.),
            Line(21600., 0.),
            Line(21600., 14392.),
            Curve((20800., 14392.), (20000., 14467.), (20000., 14467.)),
        ],
    );
    let silhouette = frame.path(
        GRID,
        &[
            Move(0., 20782.),
            Curve((9298., 23542.), (9298., 18022.), (18595., 18022.)),
            Line(18595., 16352.),
            Curve((18595., 16352.), (19298., 16252.), (20000., 16252.)),
            Line(20000., 14467.),
            Curve((20000., 14467.), (20800., 14392.), (21600., 14392.)),
            Line(21600., 0.),
            Line(2972., 0.),
            Line(2972., 1815.),
            Line(1532., 1815.),
            Line(1532., 3675.),
            Line(0., 3675.),
            Close,
        ],
    );
    vec![
        fill_only(faces),
        stroke_only(edges),
        SubPath {
            path: silhouette,
            fill: FillMode::None,
            stroke: false,
            even_odd: false,
        },
    ]
}

pub(super) fn build(name: &str, w: f64, h: f64) -> Option<Geometry> {
    let kind = name.strip_prefix("flowChart")?;
    let f = Frame { w, h };
    let mut text = f.rect(0., 0., 1., 1.);
    let mut sites = mid_sites(w, h);
    let square = || f.polygon(1., &[(0., 0.), (1., 0.), (1., 1.), (0., 1.)]);
    let diamond = || f.polygon(2., &[(0., 1.), (1., 0.), (2., 1.), (1., 2.)]);
    let inverted = || f.polygon(2., &[(0., 0.), (2., 0.), (1., 2.)]);
    let ellipse = || {
        f.path(
            2.,
            &[
                Move(0., 1.),
                Arc(1., 1., 180., 90.),
                Arc(1., 1., 270., 90.),
                Arc(1., 1., 0., 90.),
                Arc(1., 1., 90., 90.),
                Close,
            ],
        )
    };
    let inset = (1. - std::f64::consts::FRAC_1_SQRT_2) / 2.;
    let far = 1. - inset;
    let paths = match kind {
        "Process" => vec![fill(square())],
        "AlternateProcess" => {
            let radius = w.min(h) / 6.;
            let pad = radius * 29289. / 100000.;
            text = Rect::new(pad, pad, w - pad, h - pad);
            let mut p = Pb::new();
            p.m(0., radius);
            ellipse_arc(&mut p, radius, radius, 180., 90.).l(w - radius, 0.);
            ellipse_arc(&mut p, radius, radius, 270., 90.).l(w, h - radius);
            ellipse_arc(&mut p, radius, radius, 0., 90.).l(radius, h);
            ellipse_arc(&mut p, radius, radius, 90., 90.).z();
            vec![fill(p.take())]
        }
        "Collate" => {
            text = f.rect(0.25, 0.25, 0.75, 0.75);
            sites = f.sites(&[(0.5, 0.), (0.5, 0.5), (0.5, 1.)]);
            vec![fill(f.polygon(
                2.,
                &[(0., 0.), (2., 0.), (1., 1.), (2., 2.), (0., 2.), (1., 1.)],
            ))]
        }
        "Decision" | "Sort" => {
            text = f.rect(0.25, 0.25, 0.75, 0.75);
            if kind == "Sort" {
                divided(diamond(), f.path(2., &[Move(0., 1.), Line(2., 1.)]))
            } else {
                vec![fill(diamond())]
            }
        }
        "Connector" | "Or" | "SummingJunction" => {
            text = f.rect(inset, inset, far, far);
            sites = f.sites(&[
                (0.5, 0.),
                (inset, inset),
                (0., 0.5),
                (inset, far),
                (0.5, 1.),
                (far, far),
                (1., 0.5),
                (far, inset),
            ]);
            match kind {
                "Or" => divided(
                    ellipse(),
                    f.path(
                        2.,
                        &[Move(1., 0.), Line(1., 2.), Move(0., 1.), Line(2., 1.)],
                    ),
                ),
                "SummingJunction" => divided(
                    ellipse(),
                    f.path(
                        1.,
                        &[
                            Move(inset, inset),
                            Line(far, far),
                            Move(far, inset),
                            Line(inset, far),
                        ],
                    ),
                ),
                _ => vec![fill(ellipse())],
            }
        }
        "Delay" => {
            text = f.rect(0., inset, far, far);
            vec![fill(f.path(
                2.,
                &[
                    Move(0., 0.),
                    Line(1., 0.),
                    Arc(1., 1., 270., 180.),
                    Line(0., 2.),
                    Close,
                ],
            ))]
        }
        "Display" => {
            text = f.rect(1. / 6., 0., 5. / 6., 1.);
            vec![fill(f.path(
                6.,
                &[
                    Move(0., 3.),
                    Line(1., 0.),
                    Line(5., 0.),
                    Arc(1., 3., 270., 180.),
                    Line(1., 6.),
                    Close,
                ],
            ))]
        }
        "Document" => {
            text = f.rect(0., 0., 1., 17322. / 21600.);
            sites[2].y = h * 20172. / 21600.;
            vec![fill(f.path(
                21600.,
                &[
                    Move(0., 0.),
                    Line(21600., 0.),
                    Line(21600., 17322.),
                    Curve((10800., 17322.), (10800., 23922.), (0., 20172.)),
                    Close,
                ],
            ))]
        }
        "Multidocument" => {
            text = f.rect(0., 3675. / 21600., 18595. / 21600., 20782. / 21600.);
            sites[0].x = w * 12286. / 21600.;
            sites[2] = Point::new(w * 9298. / 21600., h * 20782. / 21600.);
            paper_stack(&f)
        }
        "Extract" | "Merge" | "OfflineStorage" => {
            let extract = kind == "Extract";
            text = if extract {
                f.rect(0.25, 0.5, 0.75, 1.)
            } else {
                f.rect(0.25, 0., 0.75, 0.5)
            };
            sites[1].x = w / 4.;
            sites[3].x = w * 0.75;
            if kind == "OfflineStorage" {
                sites.reverse();
                divided(inverted(), f.path(5., &[Move(2., 4.), Line(3., 4.)]))
            } else if extract {
                vec![fill(f.polygon(2., &[(0., 2.), (1., 0.), (2., 2.)]))]
            } else {
                vec![fill(inverted())]
            }
        }
        "InputOutput" => {
            text = f.rect(0.2, 0., 0.8, 1.);
            sites = f.sites(&[
                (0.6, 0.),
                (0.5, 0.),
                (0.1, 0.5),
                (0.4, 1.),
                (0.5, 1.),
                (0.9, 0.5),
            ]);
            vec![fill(
                f.polygon(5., &[(0., 5.), (1., 0.), (5., 0.), (4., 5.)]),
            )]
        }
        "InternalStorage" | "PredefinedProcess" => {
            let mut bars = vec![Move(1., 0.), Line(1., 8.)];
            if kind == "InternalStorage" {
                text = f.rect(0.125, 0.125, 1., 1.);
                bars.extend([Move(0., 1.), Line(8., 1.)]);
            } else {
                text = f.rect(0.125, 0., 0.875, 1.);
                bars.extend([Move(7., 0.), Line(7., 8.)]);
            }
            divided(square(), f.path(8., &bars))
        }
        "MagneticDisk" => {
            text = f.rect(0., 1. / 3., 1., 5. / 6.);
            sites.insert(0, Point::new(w / 2., h / 3.));
            divided(
                f.path(
                    6.,
                    &[
                        Move(0., 1.),
                        Arc(3., 1., 180., 180.),
                        Line(6., 5.),
                        Arc(3., 1., 0., 180.),
                        Close,
                    ],
                ),
                f.path(6., &[Move(6., 1.), Arc(3., 1., 0., 180.)]),
            )
        }
        "MagneticDrum" => {
            text = f.rect(1. / 6., 0., 2. / 3., 1.);
            sites.insert(3, Point::new(2. * w / 3., h / 2.));
            divided(
                f.path(
                    6.,
                    &[
                        Move(1., 0.),
                        Line(5., 0.),
                        Arc(1., 3., 270., 180.),
                        Line(1., 6.),
                        Arc(1., 3., 90., 180.),
                        Close,
                    ],
                ),
                f.path(6., &[Move(5., 6.), Arc(1., 3., 90., 180.)]),
            )
        }
        "MagneticTape" => {
            text = f.rect(inset, inset, far, far);
            // The ray to the lower-right diagonal of the box has ellipse parameter 45°.
            vec![fill(f.path(
                2.,
                &[
                    Move(1., 2.),
                    Arc(1., 1., 90., 90.),
                    Arc(1., 1., 180., 90.),
                    Arc(1., 1., 270., 90.),
                    Arc(1., 1., 0., 45.),
                    Line(2., 2. * far),
                    Line(2., 2.),
                    Close,
                ],
            ))]
        }
        "ManualInput" | "PunchedCard" => {
            text = f.rect(0., 0.2, 1., 1.);
            if kind == "ManualInput" {
                sites[0].y = h / 10.;
                vec![fill(
                    f.polygon(5., &[(0., 1.), (5., 0.), (5., 5.), (0., 5.)]),
                )]
            } else {
                vec![fill(f.polygon(
                    5.,
                    &[(0., 1.), (1., 0.), (5., 0.), (5., 5.), (0., 5.)],
                ))]
            }
        }
        "ManualOperation" => {
            text = f.rect(0.2, 0., 0.8, 1.);
            sites[1].x = w / 10.;
            sites[3].x = w * 0.9;
            vec![fill(
                f.polygon(5., &[(0., 0.), (5., 0.), (4., 5.), (1., 5.)]),
            )]
        }
        "OffpageConnector" => {
            text = f.rect(0., 0., 1., 0.8);
            vec![fill(f.polygon(
                10.,
                &[(0., 0.), (10., 0.), (10., 8.), (5., 10.), (0., 8.)],
            ))]
        }
        "OnlineStorage" => {
            text = f.rect(1. / 6., 0., 5. / 6., 1.);
            sites[3].x = 5. * w / 6.;
            vec![fill(f.path(
                6.,
                &[
                    Move(1., 0.),
                    Line(6., 0.),
                    Arc(1., 3., 270., -180.),
                    Line(1., 6.),
                    Arc(1., 3., 90., 180.),
                    Close,
                ],
            ))]
        }
        "Preparation" => {
            text = f.rect(0.2, 0., 0.8, 1.);
            vec![fill(f.polygon(
                10.,
                &[
                    (0., 5.),
                    (2., 0.),
                    (8., 0.),
                    (10., 5.),
                    (8., 10.),
                    (2., 10.),
                ],
            ))]
        }
        "PunchedTape" => {
            text = f.rect(0., 0.2, 1., 0.8);
            sites[0].y = h / 10.;
            sites[2].y = 0.9 * h;
            vec![fill(f.path(
                20.,
                &[
                    Move(0., 2.),
                    Arc(5., 2., 180., -180.),
                    Arc(5., 2., 180., 180.),
                    Line(20., 18.),
                    Arc(5., 2., 0., -180.),
                    Arc(5., 2., 0., 180.),
                    Close,
                ],
            ))]
        }
        "Terminator" => {
            text = f.rect(
                1018. / 21600.,
                3163. / 21600.,
                20582. / 21600.,
                18437. / 21600.,
            );
            vec![fill(f.path(
                21600.,
                &[
                    Move(3475., 0.),
                    Line(18125., 0.),
                    Arc(3475., 10800., 270., 180.),
                    Line(3475., 21600.),
                    Arc(3475., 10800., 90., 180.),
                    Close,
                ],
            ))]
        }
        _ => return None,
    };
    Some(Geometry {
        paths,
        text_rect: text,
        sites,
        handles: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use crate::preset::{CATALOG, Category};
    use std::collections::BTreeSet;

    #[test]
    fn gallery_text_frames_and_ordered_connector_sites_match_drawingml()
    -> Result<(), Box<dyn std::error::Error>> {
        let expected: BTreeSet<_> = CATALOG
            .iter()
            .filter(|p| p.category == Category::Flowchart)
            .map(|p| p.name)
            .collect();
        let mut found = BTreeSet::new();
        let mut count = 0;
        for line in include_str!("../../tests/flowchart-geometry.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
        {
            let mut fields = line.split_whitespace();
            let name = fields.next().ok_or("missing preset name")?;
            let values = fields
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()?;
            let geometry =
                crate::preset::build(name, values[0], values[1], &[]).ok_or("missing flowchart")?;
            let rect = geometry.text_rect;
            let actual = [rect.x0, rect.y0, rect.x1, rect.y1]
                .into_iter()
                .chain(geometry.sites.iter().flat_map(|p| [p.x, p.y]))
                .collect::<Vec<_>>();
            assert_eq!(actual.len(), values.len() - 2, "{name}");
            for (i, (got, want)) in actual.iter().zip(&values[2..]).enumerate() {
                assert!(
                    (got - want).abs() < 1e-7,
                    "{name} coordinate {i}: {got} vs {want}"
                );
            }
            assert!(geometry.handles.is_empty());
            found.insert(name);
            count += 1;
        }
        assert_eq!(found, expected);
        assert_eq!(count, 3 * expected.len());
        Ok(())
    }
}
