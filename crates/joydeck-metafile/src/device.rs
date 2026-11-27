//! GDI world, page and device coordinate spaces.

use kurbo::{Affine, Point};

use crate::geom::P;

pub type Xform = [f64; 6];
const ANISOTROPIC: u32 = 8;
const SCREEN_PX_PER_MM: f64 = 96.0 / 25.4;

/// Compose record transforms in GDI application order.
pub fn mul(first: Xform, second: Xform) -> Xform {
    (Affine::new(second) * Affine::new(first)).as_coeffs()
}

#[derive(Clone, Copy)]
pub struct Mapping {
    pub window_origin: P,
    pub window_size: P,
    pub viewport_origin: P,
    pub viewport_size: P,
    pub mode: u32,
    world: Affine,
    density: P,
}

impl Default for Mapping {
    fn default() -> Self {
        Self {
            window_origin: (0.0, 0.0),
            window_size: (1.0, 1.0),
            viewport_origin: (0.0, 0.0),
            viewport_size: (1.0, 1.0),
            mode: 1,
            world: Affine::IDENTITY,
            density: (SCREEN_PX_PER_MM, SCREEN_PX_PER_MM),
        }
    }
}

impl Mapping {
    pub fn set_density(&mut self, (x, y): P) {
        if x.is_finite() && y.is_finite() && x > 0.0 && y > 0.0 {
            self.density = (x, y);
        }
    }

    pub fn place(&mut self, window: Option<[f64; 4]>, size: P) {
        if let Some([x, y, width, height]) = window {
            self.mode = ANISOTROPIC;
            self.window_origin = (x, y);
            self.window_size = (width, height);
            self.viewport_origin = (0.0, 0.0);
        }
        self.viewport_size = size;
    }

    pub fn modify_world(&mut self, transform: Xform, mode: u32) {
        let transform = Affine::new(transform);
        self.world = match mode {
            1 => Affine::IDENTITY,
            2 => self.world * transform,
            3 => transform * self.world,
            4 => transform,
            _ => self.world,
        };
    }

    pub fn world(&self) -> Xform {
        self.world.as_coeffs()
    }

    pub fn device(&self, logical: P) -> P {
        let point = self.world * Point::from(logical);
        let (sx, sy) = self.page_scale();
        (
            self.viewport_origin.0 + (point.x - self.window_origin.0) * sx,
            self.viewport_origin.1 + (point.y - self.window_origin.1) * sy,
        )
    }

    /// Map a local origin and the two directions one logical unit from it.
    pub fn basis(&self, (x, y): P) -> (P, P, P) {
        let origin = self.device((x, y));
        let axis = |point| {
            let (x, y) = self.device(point);
            (x - origin.0, y - origin.1)
        };
        (origin, axis((x + 1.0, y)), axis((x, y + 1.0)))
    }

    pub fn scale(&self) -> f64 {
        let (x, y) = self.page_scale();
        self.world.determinant().abs().sqrt() * (x.abs() + y.abs()) / 2.0
    }

    pub fn uses_extents(&self) -> bool {
        matches!(self.mode, 7 | ANISOTROPIC)
    }

    pub fn window_rect(&self) -> [f64; 4] {
        let (x, y) = self.window_origin;
        let (w, h) = self.window_size;
        let (l, t) = self.device((x, y));
        let (r, b) = self.device((x + w, y + h));
        [l.min(r), t.min(b), (r - l).abs(), (b - t).abs()]
    }

    pub fn bounds(&self, [l, t, r, b]: [f64; 4]) -> Option<[f64; 4]> {
        let mut low = (f64::INFINITY, f64::INFINITY);
        let mut high = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for point in [(l, t), (r, t), (l, b), (r, b)] {
            let (x, y) = self.device(point);
            if !(x.is_finite() && y.is_finite()) {
                return None;
            }
            low = (low.0.min(x), low.1.min(y));
            high = (high.0.max(x), high.1.max(y));
        }
        Some([low.0, low.1, high.0, high.1])
    }

    fn page_scale(&self) -> P {
        let millimeters = match self.mode {
            1 => return (1.0, 1.0),
            2 => 0.1,
            3 => 0.01,
            4 => 25.4 / 100.0,
            5 => 25.4 / 1000.0,
            6 => 25.4 / 1440.0,
            mode => {
                let divide = |viewport, window| {
                    if window == 0.0 {
                        1.0
                    } else {
                        viewport / window
                    }
                };
                let x: f64 = divide(self.viewport_size.0, self.window_size.0);
                let y: f64 = divide(self.viewport_size.1, self.window_size.1);
                return if mode == 7 {
                    let magnitude = x.abs().min(y.abs());
                    (magnitude.copysign(x), magnitude.copysign(y))
                } else {
                    (x, y)
                };
            }
        };
        (millimeters * self.density.0, -millimeters * self.density.1)
    }
}
