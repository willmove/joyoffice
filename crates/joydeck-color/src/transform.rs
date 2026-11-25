use crate::srgb::{encoded, linear, quantize};
use crate::{ColorTransform, Rgba};

const PERCENT: f64 = 100_000.0;
const ANGLE: f64 = 60_000.0;

/// Apply the document's operations in order with an eight-bit result at each step.
pub fn apply(base: Rgba, transforms: &[ColorTransform]) -> Rgba {
    transforms.iter().copied().fold(base, transformed)
}

fn transformed(color: Rgba, op: ColorTransform) -> Rgba {
    use ColorTransform::*;

    let percent = |value| f64::from(value) / PERCENT;
    match op {
        Tint(value) | Shade(value) => {
            let value = percent(value).clamp(0.0, 1.0);
            let white = if matches!(op, Tint(_)) {
                1.0 - value
            } else {
                0.0
            };
            let rgb = |component| encoded(linear(component) * value + white);
            Rgba::rgba(rgb(color.r), rgb(color.g), rgb(color.b), color.a)
        }
        LumMod(v) => hsl(color, |h, s, l| (h, s, l * percent(v))),
        LumOff(v) => hsl(color, |h, s, l| (h, s, l + percent(v))),
        SatMod(v) => hsl(color, |h, s, l| (h, s * percent(v), l)),
        SatOff(v) => hsl(color, |h, s, l| (h, s + percent(v), l)),
        HueMod(v) => hsl(color, |h, s, l| (h * percent(v), s, l)),
        HueOff(v) => hsl(color, |h, s, l| (h + f64::from(v) / ANGLE, s, l)),
        Alpha(v) => color.with_alpha(quantize(percent(v))),
        AlphaMod(v) => color.with_alpha((f64::from(color.a) * percent(v)).round() as u8),
        AlphaOff(v) => color.with_alpha((f64::from(color.a) + percent(v) * 255.0).round() as u8),
        Comp => {
            // A half-turn in HSL mirrors each channel across the midpoint of the RGB range.
            let [r, g, b, _] = color.channels().map(u16::from);
            let sum = r.min(g).min(b) + r.max(g).max(b);
            Rgba::rgba((sum - r) as u8, (sum - g) as u8, (sum - b) as u8, color.a)
        }
        Inv => {
            let invert = |byte| encoded(1.0 - linear(byte));
            Rgba::rgba(invert(color.r), invert(color.g), invert(color.b), color.a)
        }
        Gray => {
            // DrawingML grayscale as rendered by Office uses luma of encoded RGB.
            let y = (0.2126 * f64::from(color.r)
                + 0.7152 * f64::from(color.g)
                + 0.0722 * f64::from(color.b))
            .round() as u8;
            Rgba::rgba(y, y, y, color.a)
        }
        RedMod(v) => Rgba {
            r: encoded(linear(color.r) * percent(v)),
            ..color
        },
        GreenMod(v) => Rgba {
            g: encoded(linear(color.g) * percent(v)),
            ..color
        },
        BlueMod(v) => Rgba {
            b: encoded(linear(color.b) * percent(v)),
            ..color
        },
    }
}

fn hsl(color: Rgba, change: impl FnOnce(f64, f64, f64) -> (f64, f64, f64)) -> Rgba {
    let (h, s, l) = color.to_hsl();
    let (h, s, l) = change(h, s, l);
    Rgba::from_hsl(h, s, l, color.a)
}

/// The five lightness adjustments shown with a theme swatch.
pub fn theme_variants(color: Rgba) -> [(Vec<ColorTransform>, &'static str); 5] {
    use ColorTransform::{LumMod, LumOff};

    let (_, _, lightness) = color.to_hsl();
    let levels = if lightness < 0.2 {
        [
            (90, "Lighter 90%"),
            (75, "Lighter 75%"),
            (50, "Lighter 50%"),
            (25, "Lighter 25%"),
            (10, "Lighter 10%"),
        ]
    } else if lightness > 0.8 {
        [
            (-5, "Darker 5%"),
            (-15, "Darker 15%"),
            (-25, "Darker 25%"),
            (-35, "Darker 35%"),
            (-50, "Darker 50%"),
        ]
    } else {
        [
            (80, "Lighter 80%"),
            (60, "Lighter 60%"),
            (40, "Lighter 40%"),
            (-25, "Darker 25%"),
            (-50, "Darker 50%"),
        ]
    };
    levels.map(|(level, label): (i32, _)| {
        let mut ops = vec![LumMod((100 - level.abs()) * 1_000)];
        if level > 0 {
            ops.push(LumOff(level * 1_000));
        }
        (ops, label)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ColorTransform::{BlueMod, Gray, GreenMod, Inv, RedMod};

    #[test]
    fn linear_rgb_arithmetic_and_encoded_gray_match_office() {
        let color = Rgba::rgba(0x24, 0x68, 0xAC, 128);
        for (ops, [r, g, b]) in [
            (&[Inv][..], [0xFD, 0xEF, 0xCA]),
            (&[Gray][..], [0x5E; 3]),
            (&[Inv, Gray][..], [0xEF; 3]),
            (&[Gray, Inv][..], [0xF2; 3]),
            (
                &[RedMod(200_000), GreenMod(60_000), BlueMod(45_000)][..],
                [0x35, 0x51, 0x77],
            ),
        ] {
            assert_eq!(apply(color, ops), Rgba::rgba(r, g, b, color.a));
        }
    }
}
