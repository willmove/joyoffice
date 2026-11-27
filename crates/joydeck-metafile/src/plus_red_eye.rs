//! Soft red-eye correction over bounded correction regions.

use crate::bitmap::Rgba;

pub(crate) fn apply(image: &mut Rgba, regions: &[[i32; 4]], work: &mut usize) -> Option<()> {
    if regions.is_empty() {
        return Some(());
    }
    let pixels = image.w.checked_mul(image.h)?;
    *work = work.checked_sub(pixels)?;
    let mut corrections = vec![0u8; pixels];
    for &[left, top, right, bottom] in regions {
        if left >= right || top >= bottom {
            continue;
        }
        let clamp = |x: i32, limit: usize| (x.max(0) as usize).min(limit);
        let (x0, x1) = (clamp(left, image.w), clamp(right, image.w));
        let (y0, y1) = (clamp(top, image.h), clamp(bottom, image.h));
        *work = work.checked_sub((x1 - x0).saturating_mul(y1 - y0))?;
        let [l, t, r, b] = [left, top, right, bottom].map(f64::from);
        let (w, h) = (r - l, b - t);
        let diameter = w.min(h);
        let center = |position, extent: f64| {
            position + 0.5 * (extent - (extent - diameter).min(diameter * 0.12))
        };
        let (cx, cy) = (center(l, w), center(t, h));
        let scale = 2.0 / diameter;
        let region_strength = (diameter / 12.0).min(1.0).powi(2) * 0.61;
        for y in y0..y1 {
            let dy = (y as f64 + 0.5 - cy) * scale;
            for x in x0..x1 {
                let dx = (x as f64 + 0.5 - cx) * scale;
                let distance = dx.hypot(dy).min(1.0);
                let index = y * image.w + x;
                let [r, g, b, a] = image.px.as_chunks::<4>().0[index].map(f64::from);
                let other = g.max(b);
                let excess = (r - other).max(0.0);
                // Red chroma is reduced inside the eye. Taper the correction outward and
                // suppress warm neutrals so the edge of the selection cannot leave a seam.
                let redness =
                    ((excess - other * 0.22 - 8.0) / (other * 0.5 + 16.0)).clamp(0.0, 1.0);
                let edge = 1.0 - distance * (0.7 + 0.3 * distance);
                let removed =
                    (excess * redness * edge * region_strength * (a / 255.0)).round() as u8;
                corrections[index] = corrections[index].max(removed);
            }
        }
    }
    for ([r, g, b, _], removed) in image.px.as_chunks_mut::<4>().0.iter_mut().zip(corrections) {
        *r = r.saturating_sub(removed);
        let neutral = (f64::from(removed) * 0.45).round() as u8;
        *g = g.saturating_add(neutral);
        *b = b.saturating_add(neutral);
    }
    Some(())
}
