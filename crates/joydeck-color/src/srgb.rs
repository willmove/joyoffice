use serde::{Deserialize, Serialize};

/// Eight-bit sRGB channels with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Rgba {
    fn default() -> Self {
        Self::BLACK
    }
}

impl Rgba {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self::BLACK.with_alpha(0);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    #[allow(
        clippy::self_named_constructors,
        reason = "Keep the public constructor"
    )]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Accepts three, six or eight hex digits, with an optional leading hash.
    pub fn from_hex(input: &str) -> Option<Self> {
        let input = input.trim();
        let hex = input.strip_prefix('#').unwrap_or(input);
        if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        match hex.len() {
            3 => {
                let mut nibbles = hex
                    .chars()
                    .filter_map(|ch| ch.to_digit(16).map(|d| d as u8));
                Some(Self::rgb(
                    nibbles.next()? * 17,
                    nibbles.next()? * 17,
                    nibbles.next()? * 17,
                ))
            }
            6 | 8 => {
                let [r, g, b] = joyoffice_drawingml::parse_srgb(hex.get(..6)?)?;
                let alpha = hex
                    .get(6..8)
                    .map_or(Some(255), |tail| u8::from_str_radix(tail, 16).ok())?;
                Some(Self::rgba(r, g, b, alpha))
            }
            _ => None,
        }
    }

    pub fn hex(&self) -> String {
        joyoffice_drawingml::srgb_hex([self.r, self.g, self.b])
    }

    pub(super) const fn channels(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn to_f32(self) -> [f32; 4] {
        self.channels().map(|channel| f32::from(channel) / 255.0)
    }

    pub fn from_f64(r: f64, g: f64, b: f64, a: f64) -> Self {
        let [r, g, b, a] = [r, g, b, a].map(quantize);
        Self { r, g, b, a }
    }

    /// Encode a linear scRGB triplet as opaque sRGB.
    pub fn from_linear_rgb(r: f64, g: f64, b: f64) -> Self {
        Self::rgb(encoded(r), encoded(g), encoded(b))
    }

    /// Relative luminance in linear sRGB.
    pub fn luminance(self) -> f64 {
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    pub fn contrast(self, other: Self) -> f64 {
        let [left, right] = [self.luminance(), other.luminance()];
        (left.max(right) + 0.05) / (left.min(right) + 0.05)
    }

    pub fn to_hsl(self) -> (f64, f64, f64) {
        let [r, g, b, _] = self.channels().map(unit);
        let light = r.max(g).max(b);
        let dark = r.min(g).min(b);
        let chroma = light - dark;
        let lightness = (light + dark) * 0.5;
        if chroma == 0.0 {
            return (0.0, 0.0, lightness);
        }
        let sector = if light == r {
            ((g - b) / chroma).rem_euclid(6.0)
        } else if light == g {
            2.0 + (b - r) / chroma
        } else {
            4.0 + (r - g) / chroma
        };
        let range = if lightness <= 0.5 {
            light + dark
        } else {
            (2.0 - light) - dark
        };
        (sector * 60.0, chroma / range, lightness)
    }

    /// HSL in degrees and unit fractions. The channel triangle waves follow CSS Color 4.
    pub fn from_hsl(hue: f64, saturation: f64, lightness: f64, a: u8) -> Self {
        let l = lightness.clamp(0.0, 1.0);
        let s = saturation.clamp(0.0, 1.0);
        if s == 0.0 {
            let gray = quantize(l);
            return Self::rgba(gray, gray, gray, a);
        }
        let hue = hue.rem_euclid(360.0) / 30.0;
        let amplitude = s * l.min(1.0 - l);
        let channel = |offset: f64| {
            let k = (offset + hue) % 12.0;
            quantize(l - amplitude * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0))
        };
        Self::rgba(channel(0.0), channel(8.0), channel(4.0), a)
    }

    /// Interpolate components in sRGB with straight alpha.
    pub fn lerp(self, other: Self, t: f64) -> Self {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mix = |start, end| {
            let start = f64::from(start);
            (start + (f64::from(end) - start) * t).round() as u8
        };
        Self::rgba(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
            mix(self.a, other.a),
        )
    }
}

pub(super) fn unit(channel: u8) -> f64 {
    f64::from(channel) / 255.0
}

pub(super) fn quantize(value: f64) -> u8 {
    if value.is_finite() {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    } else {
        0
    }
}

pub(super) fn linear(channel: u8) -> f64 {
    let value = unit(channel);
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn encoded(linear: f64) -> u8 {
    let value = linear.clamp(0.0, 1.0);
    quantize(if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    })
}
