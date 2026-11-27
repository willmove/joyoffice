//! The wide EMF+ Gaussian uses both axes and keeps intermediate channels in Q8.

use crate::bitmap::{Rgba, premultiplied, straight};

const MAX_PIXELS: usize = 8_000_000;
const GROUPS_PER_SIDE: usize = 24;

struct Band {
    lo: isize,
    hi: isize,
    weight: u64,
}

struct Kernel {
    bands: Vec<Band>,
    weight: u64,
}

pub(crate) fn apply(
    image: &mut Rgba,
    radius: f64,
    pad: usize,
    preserve_color: bool,
    work: &mut usize,
) -> Option<()> {
    let w = image.w.checked_add(pad * 2)?;
    let h = image.h.checked_add(pad * 2)?;
    let size = w.checked_mul(h)?;
    if size == 0 || size > MAX_PIXELS {
        return None;
    }
    let kernel = Kernel::new(radius);
    *work = work.checked_sub(size.saturating_mul(kernel.bands.len() * 2 + 3))?;
    let mut first = vec![[0u16; 4]; size];
    for (row, scan) in image.px.chunks_exact(image.w * 4).enumerate() {
        let at = (row + pad) * w + pad;
        for (dest, &pixel) in first[at..at + image.w]
            .iter_mut()
            .zip(scan.as_chunks::<4>().0)
        {
            let value = if preserve_color {
                pixel
            } else {
                premultiplied(pixel)
            };
            *dest = value.map(|channel| u16::from(channel) << 8);
        }
    }
    let mut second = vec![[0u16; 4]; size];
    let mut prefix = vec![[0u64; 4]; w.max(h) + 1];
    for row in 0..h {
        kernel.line(&first, &mut second, (row * w, w, 1), &mut prefix);
    }
    for column in 0..w {
        kernel.line(&second, &mut first, (column, h, w), &mut prefix);
    }
    image.w = w;
    image.h = h;
    image.px.resize(size * 4, 0);
    for (target, value) in image.px.as_chunks_mut::<4>().0.iter_mut().zip(first) {
        let color = value.map(|value| (value >> 8) as u8);
        *target = if preserve_color {
            color
        } else {
            straight(color)
        };
    }
    Some(())
}

impl Kernel {
    fn new(radius: f64) -> Self {
        let support = radius.ceil() as usize;
        let stride = support.div_ceil(GROUPS_PER_SIDE);
        let variance = (radius * 0.505).powi(2);
        let unit = 1u64 << 20;
        let mut weight = unit;
        let mut bands = vec![Band {
            lo: 0,
            hi: 1,
            weight: unit,
        }];
        // Average each small band of taps. Prefix sums make its cost independent of the radius.
        for lo in (1..=support).step_by(stride) {
            let hi = (lo + stride).min(support + 1);
            let sum: f64 = (lo..hi)
                .map(|i| (-0.5 * (i as f64).powi(2) / variance).exp())
                .sum();
            let band_weight = (sum * unit as f64 / (hi - lo) as f64).round() as u64;
            weight += 2 * (hi - lo) as u64 * band_weight;
            bands.push(Band {
                lo: lo as isize,
                hi: hi as isize,
                weight: band_weight,
            });
            bands.push(Band {
                lo: 1 - hi as isize,
                hi: 1 - lo as isize,
                weight: band_weight,
            });
        }
        Self { bands, weight }
    }

    fn line(
        &self,
        source: &[[u16; 4]],
        target: &mut [[u16; 4]],
        (origin, count, step): (usize, usize, usize),
        prefix: &mut [[u64; 4]],
    ) {
        for i in 0..count {
            let sample = source[origin + i * step];
            prefix[i + 1] =
                std::array::from_fn(|channel| prefix[i][channel] + u64::from(sample[channel]));
        }
        let first = source[origin].map(u64::from);
        let last = source[origin + (count - 1) * step].map(u64::from);
        let stop = count as isize;
        for i in 0..count {
            let mut result = [0u64; 4];
            for band in &self.bands {
                let (lo, hi) = (i as isize + band.lo, i as isize + band.hi);
                let (l, h) = (lo.clamp(0, stop) as usize, hi.clamp(0, stop) as usize);
                let below = (hi.min(0) - lo.min(0)) as u64;
                let above = (hi.max(stop) - lo.max(stop)) as u64;
                for channel in 0..4 {
                    let sum = prefix[h][channel] - prefix[l][channel]
                        + first[channel] * below
                        + last[channel] * above;
                    result[channel] += sum * band.weight;
                }
            }
            target[origin + i * step] =
                result.map(|value| ((value + self.weight / 2) / self.weight) as u16);
        }
    }
}
