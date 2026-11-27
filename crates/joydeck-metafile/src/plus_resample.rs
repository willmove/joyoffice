//! A premultiplied area prefilter for EMF+ HighQuality interpolation.

use kurbo::Affine;

use crate::bitmap::{Rgba, premultiplied, straight};

pub(crate) fn magnification(target: Affine, source: [f64; 4]) -> [f64; 2] {
    let [a, b, c, d, _, _] = target.as_coeffs();
    [a.hypot(b) / source[2].abs(), c.hypot(d) / source[3].abs()]
}

pub(crate) fn downsample(
    image: &Rgba,
    rect: &mut [f64; 4],
    magnification: [f64; 2],
    work: &mut usize,
) -> Option<Rgba> {
    let [w, h] = [image.w, image.h];
    let extent = |size: usize, gain: f64| (size as f64 * gain.min(1.0)).ceil() as usize;
    let (out_w, out_h) = (extent(w, magnification[0]), extent(h, magnification[1]));
    if out_w == 0 || out_h == 0 || (out_w, out_h) == (w, h) {
        return None;
    }
    let dimensions = w.checked_mul(h)? + out_w.checked_mul(h)? + out_w.checked_mul(out_h)?;
    *work = work.checked_sub(dimensions * 2)?;
    let mut intermediate = vec![[0.0f32; 4]; out_w * h];
    let mut reduced = vec![[0u8; 4]; out_w * out_h];
    let mut prefix = vec![[0.0; 4]; w.max(h) + 1];
    let x_taps = Sample::for_axis(w, out_w);
    let y_taps = Sample::for_axis(h, out_h);
    let rgba = image.px.as_chunks::<4>().0;
    for (row, scan) in rgba.chunks_exact(w).enumerate() {
        for (column, &value) in scan.iter().enumerate() {
            let color = premultiplied(value);
            prefix[column + 1] = std::array::from_fn(|i| prefix[column][i] + f64::from(color[i]));
        }
        for (slot, tap) in intermediate[row * out_w..][..out_w].iter_mut().zip(&x_taps) {
            *slot = tap.average(&prefix[..=w]).map(|value| value as f32);
        }
    }
    for x in 0..out_w {
        for y in 0..h {
            let color = intermediate[y * out_w + x];
            prefix[y + 1] = std::array::from_fn(|i| prefix[y][i] + f64::from(color[i]));
        }
        for (y, tap) in y_taps.iter().enumerate() {
            let color = tap.average(&prefix[..=h]).map(|value| value.round() as u8);
            reduced[y * out_w + x] = straight(color);
        }
    }
    let (sx, sy) = (out_w as f64 / w as f64, out_h as f64 / h as f64);
    *rect = [rect[0] * sx, rect[1] * sy, rect[2] * sx, rect[3] * sy];
    Some(Rgba {
        w: out_w,
        h: out_h,
        px: reduced.into_flattened(),
        bottom_up: false,
    })
}

pub(crate) fn subsample(
    image: &Rgba,
    source: &mut [f64; 4],
    magnification: [f64; 2],
    work: &mut usize,
) -> Option<Rgba> {
    let (w, h) = (image.w, image.h);
    if *source != [0.0, 0.0, w as f64, h as f64] {
        return None;
    }
    let target = |size: usize, gain: f64| {
        let scaled = size as f64 * gain;
        let pixels = scaled.round() as usize;
        (pixels > 0 && (scaled - pixels as f64).abs() < 1e-6).then_some(pixels)
    };
    let (out_w, out_h) = (target(w, magnification[0])?, target(h, magnification[1])?);
    if out_w > w || out_h > h || (out_w, out_h) == (w, h) {
        return None;
    }
    *work = work.checked_sub(out_w * out_h)?;
    let pixels = image.px.as_chunks::<4>().0;
    let mut px = Vec::with_capacity(out_w * out_h * 4);
    for y in 0..out_h {
        let row = (y * h / out_h) * w;
        for x in 0..out_w {
            px.extend_from_slice(&pixels[row + x * w / out_w]);
        }
    }
    // GDI+ anchors whole device pixels to the first source texel when shrinking.
    *source = [0.0, 0.0, out_w as f64, out_h as f64];
    Some(Rgba {
        w: out_w,
        h: out_h,
        px,
        bottom_up: false,
    })
}

struct Sample {
    low: f64,
    high: f64,
}

impl Sample {
    fn for_axis(source: usize, output: usize) -> Vec<Self> {
        let window = source as f64 / output as f64;
        (0..output)
            .map(|i| {
                // In native high-quality shrinking the first sample is anchored to the
                // first source pixel center. Its footprint may extend past the bitmap.
                let center = i as f64 * window + 0.5;
                Self {
                    low: center - window * 0.5,
                    high: center + window * 0.5,
                }
            })
            .collect()
    }

    fn average(&self, prefix: &[[f64; 4]]) -> [f64; 4] {
        let lo = integral(self.low, prefix);
        let hi = integral(self.high, prefix);
        std::array::from_fn(|c| (hi[c] - lo[c]) / (self.high - self.low))
    }
}

fn integral(at: f64, prefix: &[[f64; 4]]) -> [f64; 4] {
    let i = (at as usize).min(prefix.len() - 2);
    let remainder = at - i as f64;
    std::array::from_fn(|c| prefix[i][c] + (prefix[i + 1][c] - prefix[i][c]) * remainder)
}
