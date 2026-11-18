use super::catalog::{FillMode, Geometry, SubPath};
use super::{BezPath, Point, Rect};
use kurbo::{Arc, Vec2};

// ---------- path builder ----------

pub(super) struct Pb {
    pub(super) p: BezPath,
    pub(super) cur: Point,
}

impl Pb {
    pub(super) fn new() -> Self {
        Pb {
            p: BezPath::new(),
            cur: Point::ZERO,
        }
    }
    pub(super) fn m(&mut self, x: f64, y: f64) -> &mut Self {
        self.cur = Point::new(x, y);
        self.p.move_to(self.cur);
        self
    }
    pub(super) fn l(&mut self, x: f64, y: f64) -> &mut Self {
        self.cur = Point::new(x, y);
        self.p.line_to(self.cur);
        self
    }
    pub(super) fn c(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) -> &mut Self {
        self.cur = Point::new(x, y);
        self.p
            .curve_to(Point::new(x1, y1), Point::new(x2, y2), self.cur);
        self
    }
    /// Elliptic arc continuing from the current point, which lies on the ellipse at parametric
    /// angle `st` (degrees); sweeps `sw` degrees (positive = clockwise on screen).
    pub(super) fn arc(&mut self, wr: f64, hr: f64, st: f64, sw: f64) -> &mut Self {
        let (s, e) = (st.to_radians(), (st + sw).to_radians());
        let c = Point::new(self.cur.x - wr * s.cos(), self.cur.y - hr * s.sin());
        let a = Arc::new(c, Vec2::new(wr.abs(), hr.abs()), s, e - s, 0.0);
        a.to_cubic_beziers(0.05, |p1, p2, p| {
            self.p.curve_to(p1, p2, p);
        });
        self.cur = Point::new(c.x + wr * e.cos(), c.y + hr * e.sin());
        self
    }
    /// A DrawingML `arcTo`: angles are where the ray from the ellipse's centre at that angle
    /// meets it (not the ellipse's parameter), so elliptical arcs start and end where the
    /// standard's shapes expect.
    #[cfg(test)]
    pub(super) fn arc_spec(&mut self, wr: f64, hr: f64, st: f64, sw: f64) -> &mut Self {
        let par = |a: f64| {
            let a = a.to_radians();
            (wr * a.sin()).atan2(hr * a.cos())
        };
        let t0 = par(st);
        let mut d = par(st + sw) - t0;
        let tau = std::f64::consts::TAU;
        if sw.abs() >= 360.0 {
            d = tau.copysign(sw);
        } else {
            while sw > 0.0 && d < 0.0 {
                d += tau;
            }
            while sw < 0.0 && d > 0.0 {
                d -= tau;
            }
        }
        self.arc(wr, hr, t0.to_degrees(), d.to_degrees())
    }
    pub(super) fn z(&mut self) -> &mut Self {
        self.p.close_path();
        self
    }
    pub(super) fn poly(&mut self, pts: &[(f64, f64)]) -> &mut Self {
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 {
                self.m(*x, *y);
            } else {
                self.l(*x, *y);
            }
        }
        self.z()
    }
    pub(super) fn take(&mut self) -> BezPath {
        std::mem::take(&mut self.p)
    }
}

// Endpoints shared by a line and a quadrant must be numerically identical.
// A tiny closing segment can otherwise give the stroked seam an extra join.
fn direction(degrees: f64) -> (f64, f64) {
    match degrees.rem_euclid(360.) {
        0. => (1., 0.),
        90. => (0., 1.),
        180. => (-1., 0.),
        270. => (0., -1.),
        angle => {
            let (sin, cos) = angle.to_radians().sin_cos();
            (cos, sin)
        }
    }
}

pub(super) fn ellipse_arc(path: &mut Pb, rx: f64, ry: f64, start: f64, sweep: f64) -> &mut Pb {
    if sweep == 0. {
        return path;
    }
    let (sx, sy) = direction(start);
    let (cx, cy) = (path.cur.x - rx * sx, path.cur.y - ry * sy);
    // A quarter-circle cubic stays within 0.000273 radii. Bisect large
    // quadrants to keep board and export curves smooth at larger dimensions.
    let radius = rx.max(ry);
    let divisions = if radius <= 180. {
        1.
    } else {
        (radius * 0.000273 / 0.05)
            .powf(1. / 6.)
            .ceil()
            .clamp(1., 16.)
    };
    let pieces = (sweep.abs() * divisions / 90.).ceil() as u32;
    let step = sweep / f64::from(pieces);
    let tangent = (step.to_radians() / 4.).tan() * 4. / 3.;
    for index in 0..pieces {
        let (x0, y0) = direction(start + step * f64::from(index));
        let (x1, y1) = direction(start + step * f64::from(index + 1));
        path.c(
            cx + rx * (x0 - tangent * y0),
            cy + ry * (y0 + tangent * x0),
            cx + rx * (x1 + tangent * y1),
            cy + ry * (y1 - tangent * x1),
            cx + rx * x1,
            cy + ry * y1,
        );
    }
    path
}

pub(super) fn fill(p: BezPath) -> SubPath {
    SubPath {
        path: p,
        fill: FillMode::Norm,
        stroke: true,
        even_odd: false,
    }
}

pub(super) fn stroke_only(p: BezPath) -> SubPath {
    SubPath {
        path: p,
        fill: FillMode::None,
        stroke: true,
        even_odd: false,
    }
}

pub(super) fn fill_only(p: BezPath) -> SubPath {
    SubPath {
        path: p,
        fill: FillMode::Norm,
        stroke: false,
        even_odd: false,
    }
}

/// Parametric angle on an rx×ry ellipse for a visual angle `deg` (file angles are visual).
pub(super) fn vis(deg: f64, rx: f64, ry: f64) -> f64 {
    let t = deg.to_radians();
    (rx * t.sin()).atan2(ry * t.cos()).to_degrees()
}

#[derive(Clone, Copy)]
pub(super) enum Corner {
    Square,
    Round(f64),
    Snip(f64),
}

/// A rectangle with each corner (TL, TR, BR, BL) square, rounded or snipped.
pub(super) fn corner_rect(w: f64, h: f64, c: [Corner; 4]) -> BezPath {
    let mut b = Pb::new();
    let size = |k: Corner| match k {
        Corner::Square => 0.0,
        Corner::Round(r) | Corner::Snip(r) => r.max(0.0),
    };
    let s0 = size(c[0]);
    b.m(s0, 0.0);
    // top edge → TR
    let s1 = size(c[1]);
    b.l(w - s1, 0.0);
    match c[1] {
        Corner::Round(r) if r > 0.0 => {
            b.arc(r, r, 270.0, 90.0);
        }
        Corner::Snip(_) => {
            b.l(w, s1);
        }
        _ => {}
    }
    let s2 = size(c[2]);
    b.l(w, h - s2);
    match c[2] {
        Corner::Round(r) if r > 0.0 => {
            b.arc(r, r, 0.0, 90.0);
        }
        Corner::Snip(_) => {
            b.l(w - s2, h);
        }
        _ => {}
    }
    let s3 = size(c[3]);
    b.l(s3, h);
    match c[3] {
        Corner::Round(r) if r > 0.0 => {
            b.arc(r, r, 90.0, 90.0);
        }
        Corner::Snip(_) => {
            b.l(0.0, h - s3);
        }
        _ => {}
    }
    b.l(0.0, s0);
    match c[0] {
        Corner::Round(r) if r > 0.0 => {
            b.arc(r, r, 180.0, 90.0);
        }
        Corner::Snip(_) => {
            b.l(s0, 0.0);
        }
        _ => {}
    }
    b.z();
    b.take()
}

pub(super) fn mid_sites(w: f64, h: f64) -> Vec<Point> {
    vec![
        Point::new(w / 2.0, 0.0),
        Point::new(0.0, h / 2.0),
        Point::new(w / 2.0, h),
        Point::new(w, h / 2.0),
    ]
}

/// Orientation for single-axis arrows built along +x.
#[derive(Clone, Copy)]
pub(super) enum Dir {
    Right,
    Left,
    Up,
    Down,
}

pub(super) fn len_thick(d: Dir, w: f64, h: f64) -> (f64, f64) {
    match d {
        Dir::Right | Dir::Left => (w, h),
        Dir::Up | Dir::Down => (h, w),
    }
}

pub(super) fn simple(path: BezPath, text: Rect, w: f64, h: f64) -> Geometry {
    Geometry {
        paths: vec![fill(path)],
        text_rect: text,
        handles: vec![],
        sites: mid_sites(w, h),
    }
}
