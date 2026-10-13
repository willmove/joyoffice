//! Shared physical units for the M0 spike. No document editing is implemented here.
//! Screen scale is an output transform and never changes stored page geometry.

/// A physical document length, in English Metric Units (914,400 per inch).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Emu(i64);

impl Emu {
    pub const PER_INCH: i64 = 914_400;
    pub const PER_POINT: i64 = 12_700;
    pub const PER_TWIP: i64 = 635;
    pub const PER_MM: i64 = 36_000;

    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    pub fn from_twips(value: i64) -> Option<Self> {
        value.checked_mul(Self::PER_TWIP).map(Self)
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn points(self) -> f64 {
        self.0 as f64 / Self::PER_POINT as f64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageSize {
    pub width: Emu,
    pub height: Emu,
}

impl PageSize {
    pub const A4: Self = Self {
        width: Emu::new(210 * Emu::PER_MM),
        height: Emu::new(297 * Emu::PER_MM),
    };
}

/// A validated projection from document points to logical screen pixels.
/// The platform's device-pixel scale is applied by the renderer, not pagination.
#[derive(Clone, Copy, Debug)]
pub struct ScreenScale(f64);

impl ScreenScale {
    pub fn new(zoom: f64) -> Option<Self> {
        if zoom.is_finite() && (0.1..=5.0).contains(&zoom) {
            Some(Self(zoom))
        } else {
            None
        }
    }

    pub fn logical_pixels(self, length: Emu) -> f64 {
        length.points() * (96.0 / 72.0) * self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inches_points_and_twips_agree() {
        let inch = Emu::from_twips(1440).unwrap();
        assert_eq!(inch.raw(), Emu::PER_INCH);
        assert_eq!(inch.points(), 72.0);
        assert_eq!(ScreenScale::new(1.0).unwrap().logical_pixels(inch), 96.0);
    }

    #[test]
    fn screen_zoom_does_not_mutate_physical_geometry() {
        let page = PageSize::A4;
        let one = ScreenScale::new(1.0).unwrap().logical_pixels(page.width);
        let two = ScreenScale::new(2.0).unwrap().logical_pixels(page.width);
        assert_eq!(two, one * 2.0);
        assert_eq!(page.width.raw(), 7_560_000);
        assert_eq!(page.height.raw(), 10_692_000);
    }

    #[test]
    fn rejects_invalid_scale_and_overflow() {
        for zoom in [f64::NAN, f64::INFINITY, -1.0, 0.0, 5.1] {
            assert!(ScreenScale::new(zoom).is_none());
        }
        assert!(Emu::from_twips(i64::MAX).is_none());
        assert!(Emu::new(i64::MAX).checked_add(Emu::new(1)).is_none());
    }
}
