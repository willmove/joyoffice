//! Bounded bitmap effects for MS-EMFPLUS SerializableObject records.

use crate::{MAX_PIXELS, bitmap::Rgba, bytes::Bytes, plus_shapes::floats};

const MAX_FILTER_PIXELS: usize = 8_000_000;
pub const EFFECT_WORK: usize = 48_000_000;

pub enum Effect {
    Matrix(Box<[[f64; 5]; 4]>),
    Lookup(Box<[[u8; 256]; 4]>),
    Tone {
        selected: usize,
        curve: Box<[u8; 256]>,
    },
    Balance([i32; 3]),
    Hsl([i32; 3]),
    Tint(i32, i32),
    RedEye(Vec<[i32; 4]>),
    Blur(f64, bool),
    Sharpen(f64, f64),
}

pub fn read(record: Bytes<'_>) -> Option<Effect> {
    let size = record.u32(16)? as usize;
    let data = Bytes(record.slice(20, size)?);
    let guid = (
        record.u32(0)?,
        record.u16(4)?,
        record.u16(6)?,
        u64::from_be_bytes(*record.0.get(8..16)?.first_chunk()?),
    );
    let signed = |offset, low, high| data.i32(offset).map(|v| v.clamp(low, high));
    let percentage = |offset| signed(offset, -100, 100);
    let triple = || Some([percentage(0)?, percentage(4)?, percentage(8)?]);
    Some(match guid {
        (0x718F2615, 0x7933, 0x40E3, 0xA5115F68FE14DD74) => {
            // This is a row-vector matrix. The last group supplies the constant offsets.
            let mut matrix = [[0.0; 5]; 4];
            let components = floats::<25>(data, 0)?;
            for (target, row) in matrix.iter_mut().enumerate() {
                for (input, component) in row.iter_mut().enumerate() {
                    *component = components[input * 5 + target];
                }
            }
            Effect::Matrix(Box::new(matrix))
        }
        (0xA7CE72A9, 0x0F7F, 0x40D7, 0xB3CCD0C02D5C3212) => {
            let mut tables = [[0; 256]; 4];
            for (target, encoded) in tables.iter_mut().zip([2, 1, 0, 3]) {
                target.copy_from_slice(data.slice(encoded * 256, 256)?);
            }
            Effect::Lookup(Box::new(tables))
        }
        (0xD3A1DBE1, 0x8EC4, 0x4C17, 0x9F4CEA97AD1C343D) => {
            let brightness = f64::from(signed(0, -255, 255)?) / 255.0;
            let contrast = f64::from(percentage(4)?) / 100.0;
            tone(0, |x| (x - 0.5) * slope(contrast) + 0.5 + brightness)
        }
        (0x537E597D, 0x251E, 0x48DA, 0x966429CA496B70F8) => Effect::Balance(triple()?),
        (0xDD6A0022, 0x58E4, 0x4A67, 0x9D9BD48EB881A53D) => {
            let mode = data.u32(0)?;
            let channel = data.u32(4)? as usize;
            if mode > 7 || channel > 3 {
                return None;
            }
            let strength = f64::from(signed(8, -255, 255)?);
            tone(channel, |x| curve(x, mode, strength))
        }
        (0x8B2DD6C3, 0xEB07, 0x4D87, 0xA5F07108E26A9C5F) => {
            Effect::Hsl([signed(0, -180, 180)?, percentage(4)?, percentage(8)?])
        }
        (0x99C354EC, 0x2A31, 0x4F3A, 0x8C3417A803B33A25) => {
            levels(signed(0, 0, 100)?, percentage(4)?, signed(8, 0, 100)?)
        }
        (0x74D29D05, 0x69A4, 0x4266, 0x95493CC52836B632) => {
            let count = usize::try_from(data.i32(0)?).ok()?;
            if count > 256 {
                return None;
            }
            let mut regions = Vec::with_capacity(count);
            for i in 0..count {
                let at = 4 + i * 16;
                regions.push([
                    data.i32(at)?,
                    data.i32(at + 4)?,
                    data.i32(at + 8)?,
                    data.i32(at + 12)?,
                ]);
            }
            Effect::RedEye(regions)
        }
        (0x633C80A4, 0x1843, 0x482B, 0x9EF2BE2834C5FDD4) => {
            let [radius] = floats(data, 0)?;
            Effect::Blur(radius.clamp(0.0, 255.0), data.u32(4)? != 0)
        }
        (0x63CBF3EE, 0xC526, 0x402C, 0x8F7162C540BF5142) => {
            let [radius, amount] = floats(data, 0)?;
            Effect::Sharpen(radius.clamp(0.0, 255.0), amount.clamp(0.0, 100.0) / 100.0)
        }
        (0x1077AF00, 0x2848, 0x4441, 0x948944AD4C2D7A2C) => {
            Effect::Tint(signed(0, -180, 180)?, percentage(4)?)
        }
        _ => return None,
    })
}

fn tone(selected: usize, transform: impl Fn(f64) -> f64) -> Effect {
    let curve = std::array::from_fn(|index| byte(transform(index as f64 / 255.0)));
    Effect::Tone {
        selected,
        curve: Box::new(curve),
    }
}

fn byte(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn slope(normalized: f64) -> f64 {
    if normalized < 0.0 {
        1.0 + normalized
    } else {
        1.0 / (1.0 - normalized).max(0.005)
    }
}

fn levels(highlight: i32, midtone: i32, shadow: i32) -> Effect {
    let [highlight, midtone, shadow] = [highlight, midtone, shadow].map(|v| f64::from(v) / 100.0);
    tone(0, |x| {
        x + (x - highlight).max(0.0) * (1.0 - x) + midtone * x * (1.0 - x)
            - (shadow - x).max(0.0) * x
    })
}

fn curve(x: f64, mode: u32, strength: f64) -> f64 {
    let p = (strength / 100.0).clamp(-1.0, 1.0);
    match mode {
        0 => x * 2_f64.powf(strength / 100.0),
        1 => x.powf(2_f64.powf(-p)),
        2 => (x - 0.5) * slope(p) + 0.5,
        3 => x + p * (x - 0.5).max(0.0) * (1.0 - x) * 2.0,
        4 => x + p * (0.5 - x).max(0.0) * x * 2.0,
        5 => x + p * 4.0 * x * (1.0 - x) * 0.25,
        6 => x / (strength.clamp(1.0, 255.0) / 255.0),
        7 => {
            let black = strength.clamp(0.0, 254.0) / 255.0;
            (x - black) / (1.0 - black)
        }
        _ => x,
    }
}

impl Effect {
    /// The nonzero return value is the transparent padding added around an expanded blur.
    pub fn apply(&self, image: &mut Rgba, work: &mut usize) -> Option<usize> {
        let pixels = image.w.checked_mul(image.h)?;
        *work = work.checked_sub(pixels)?;
        match self {
            Self::Blur(radius, expand) => {
                if *radius < 1.0 {
                    return Some(0);
                }
                let pad = if *expand { radius.ceil() as usize } else { 0 };
                *image = blurred(image, *radius, pad, work)?;
                return Some(pad);
            }
            Self::Sharpen(radius, amount) => {
                if *radius >= 1.0 && *amount > 0.0 {
                    let blurred = blurred(image, *radius, 0, work)?;
                    for (pixel, b) in image
                        .px
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .zip(blurred.px.as_chunks::<4>().0)
                    {
                        for index in 0..3 {
                            let source = f64::from(pixel[index]);
                            pixel[index] = (source + amount * (source - f64::from(b[index])))
                                .round()
                                .clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
            Self::RedEye(rects) => {
                for &[x0, y0, x1, y1] in rects {
                    let (x0, y0) = (x0.max(0) as usize, y0.max(0) as usize);
                    let (x1, y1) = (
                        (x1.max(0) as usize).min(image.w),
                        (y1.max(0) as usize).min(image.h),
                    );
                    let w = x1.saturating_sub(x0);
                    if w == 0 || y1 <= y0 {
                        continue;
                    }
                    let area = w.saturating_mul(y1.saturating_sub(y0));
                    *work = work.checked_sub(area)?;
                    for y in y0..y1 {
                        let offset = (y * image.w + x0.min(image.w)) * 4;
                        let line = image.px.get_mut(offset..offset + w * 4)?;
                        for [r, g, b, _] in line.as_chunks_mut::<4>().0 {
                            let neutral = (u16::from(*g) + u16::from(*b)) / 2;
                            if u16::from(*r) > neutral.saturating_add(22) {
                                *r = neutral as u8;
                            }
                        }
                    }
                }
            }
            _ => {
                for pixel in image.px.as_chunks_mut::<4>().0 {
                    *pixel = self.color(*pixel);
                }
            }
        }
        Some(0)
    }

    fn color(&self, mut rgba: [u8; 4]) -> [u8; 4] {
        match self {
            Self::Matrix(matrix) => {
                let [r, g, b, a] = rgba.map(|value| f64::from(value) / 255.0);
                matrix
                    .each_ref()
                    .map(|row| byte(row[0] * r + row[1] * g + row[2] * b + row[3] * a + row[4]))
            }
            Self::Lookup(table) => std::array::from_fn(|i| table[i][usize::from(rgba[i])]),
            Self::Tone { selected, curve } => {
                for (i, color) in rgba[..3].iter_mut().enumerate() {
                    if *selected == 0 || *selected == i + 1 {
                        *color = curve[usize::from(*color)];
                    }
                }
                rgba
            }
            Self::Balance(balance) => {
                for (color, delta) in rgba[..3].iter_mut().zip(balance) {
                    let x = f64::from(*color) / 255.0;
                    *color = byte(adjust(x, f64::from(*delta) / 100.0));
                }
                rgba
            }
            Self::Hsl([angle, saturation, light]) => {
                let [h, s, l] = rgb_to_hsl(rgba);
                rgba[..3].copy_from_slice(&hsl_to_rgb(
                    h + f64::from(*angle) / 360.0,
                    adjust(s, f64::from(*saturation) / 100.0),
                    adjust(l, f64::from(*light) / 100.0),
                ));
                rgba
            }
            Self::Tint(angle, amount) => {
                let [h, s, l] = rgb_to_hsl(rgba);
                let target = (f64::from(*angle) / 360.0).rem_euclid(1.0);
                let distance = (h - target + 0.5).rem_euclid(1.0) - 0.5;
                let strength = f64::from(*amount) / 100.0 * (1.0 - 2.0 * distance.abs());
                rgba[..3].copy_from_slice(&hsl_to_rgb(h, s, adjust(l, strength)));
                rgba
            }
            _ => rgba,
        }
    }
}

fn adjust(value: f64, strength: f64) -> f64 {
    value + strength * if strength >= 0.0 { 1.0 - value } else { value }
}

fn rgb_to_hsl([r, g, b, _]: [u8; 4]) -> [f64; 3] {
    let [r, g, b] = [r, g, b].map(|v| f64::from(v) / 255.0);
    let hi = r.max(g).max(b);
    let lo = r.min(g).min(b);
    let light = (hi + lo) * 0.5;
    let chroma = hi - lo;
    if chroma == 0.0 {
        return [0.0, 0.0, light];
    }
    let hue = if hi == r {
        (g - b) / chroma
    } else if hi == g {
        (b - r) / chroma + 2.0
    } else {
        (r - g) / chroma + 4.0
    };
    [
        hue.rem_euclid(6.0) / 6.0,
        chroma / (1.0 - (2.0 * light - 1.0).abs()),
        light,
    ]
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [u8; 3] {
    let spread = s * l.min(1.0 - l);
    [0.0, 8.0, 4.0].map(|n| {
        let k = (n + 12.0 * h).rem_euclid(12.0);
        byte(l - spread * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0))
    })
}

fn blurred(image: &Rgba, radius: f64, pad: usize, work: &mut usize) -> Option<Rgba> {
    let w = image.w.checked_add(pad * 2)?;
    let h = image.h.checked_add(pad * 2)?;
    let size = w.checked_mul(h)?;
    if size > MAX_FILTER_PIXELS || size > MAX_PIXELS {
        return None;
    }
    let support = radius.floor() as usize;
    let samples = 2 * support + 1;
    *work = work.checked_sub(size.saturating_mul(2 * samples + 1))?;
    let sigma = radius * 0.5;
    let weights: Vec<u32> = (0..samples)
        .map(|i| {
            let x = (i as f64 - support as f64) / sigma;
            ((-0.5 * x * x).exp() * 4096.0).round() as u32
        })
        .collect();
    let total = weights.iter().sum::<u32>();
    let mut premultiplied = vec![0u8; size * 4];
    for (y, row) in image.px.chunks_exact(image.w * 4).enumerate() {
        let at = ((y + pad) * w + pad) * 4;
        for (pixel, dest) in row
            .as_chunks::<4>()
            .0
            .iter()
            .zip(premultiplied[at..at + row.len()].as_chunks_mut::<4>().0)
        {
            let a = u16::from(pixel[3]);
            *dest = [
                (u16::from(pixel[0]) * a / 255) as u8,
                (u16::from(pixel[1]) * a / 255) as u8,
                (u16::from(pixel[2]) * a / 255) as u8,
                pixel[3],
            ];
        }
    }
    let mut temp = vec![0u8; size * 4];
    for y in 0..h {
        gaussian_line(&premultiplied, &mut temp, (y * w, w, 1), &weights, total);
    }
    for x in 0..w {
        gaussian_line(&temp, &mut premultiplied, (x, h, w), &weights, total);
    }
    for [r, g, b, a] in premultiplied.as_chunks_mut::<4>().0 {
        if *a != 0 {
            for c in [r, g, b] {
                *c = ((u32::from(*c) * 255 + u32::from(*a) / 2) / u32::from(*a)).min(255) as u8;
            }
        }
    }
    Some(Rgba {
        w,
        h,
        px: premultiplied,
        bottom_up: false,
    })
}

fn gaussian_line(
    input: &[u8],
    output: &mut [u8],
    (origin, count, step): (usize, usize, usize),
    weights: &[u32],
    total: u32,
) {
    let radius = weights.len() / 2;
    let input = input.as_chunks::<4>().0;
    let output = output.as_chunks_mut::<4>().0;
    for position in 0..count {
        let mut sum = [0u32; 4];
        for (offset, &weight) in weights.iter().enumerate() {
            let x = (position + offset).saturating_sub(radius).min(count - 1);
            for (sum, channel) in sum.iter_mut().zip(input[origin + x * step]) {
                *sum += u32::from(channel) * weight;
            }
        }
        output[origin + position * step] = sum.map(|value| ((value + total / 2) / total) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_levels_and_inactive_curve_ranges_do_not_change_pixels() {
        let neutral = levels(100, 0, 0);
        for value in 0..=255 {
            assert_eq!(neutral.color([value; 4]), [value; 4]);
        }
        assert_eq!(curve(0.25, 3, 100.0), 0.25);
        assert_eq!(curve(0.75, 4, 100.0), 0.75);
        assert!(curve(0.75, 3, 100.0) > 0.75);
        assert!(curve(0.25, 4, -100.0) < 0.25);
    }

    #[test]
    fn spatial_effects_preserve_transparent_color_boundaries_and_charge_for_work() {
        let mut image = Rgba {
            w: 1,
            h: 1,
            bottom_up: false,
            px: vec![255, 0, 0, 255],
        };
        assert_eq!(Effect::Blur(1.0, true).apply(&mut image, &mut 10), None);
        assert_eq!(image.px, [255, 0, 0, 255]);
        assert_eq!(Effect::Blur(1.0, true).apply(&mut image, &mut 100), Some(1));
        assert_eq!((image.w, image.h), (3, 3));
        let pixels = image.px.as_chunks::<4>().0;
        for pixel in pixels {
            assert_eq!(&pixel[..3], &[255, 0, 0]);
        }
        assert!((150..=165).contains(&pixels[4][3]));
        assert!(pixels[4][3] > pixels[1][3] && pixels[1][3] > pixels[0][3]);
        assert_eq!(pixels[1], pixels[3]);
    }
}
