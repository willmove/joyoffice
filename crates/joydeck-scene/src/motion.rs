use joydeck_model::document::Result;
pub use joydeck_model::motion::*;
/// Pixel fallback for basic slide transitions; both inputs have identical dimensions.
pub fn composite(
    a: &crate::preview::Raster,
    b: &mut crate::preview::Raster,
    kind: &str,
    p: f32,
) -> Result<()> {
    if a.width != b.width || a.height != b.height || a.bgra.len() != b.bgra.len() || !p.is_finite()
    {
        return Err("转场缓冲尺寸或进度无效".into());
    }
    let p = p.clamp(0., 1.);
    let w = b.width as usize;
    let offset = (w as f32 * p).round() as usize;
    if kind == "fade" {
        for (x, y) in a.bgra.iter().zip(&mut b.bgra) {
            *y = (*x as f32 * (1. - p) + *y as f32 * p).round() as u8;
        }
        return Ok(());
    }
    let current = b.bgra.clone();
    for y in 0..b.height as usize {
        for x in 0..w {
            let dst = (y * w + x) * 4;
            let (source, sx) = if kind == "push" {
                if x < w - offset {
                    (&a.bgra, x + offset)
                } else {
                    (&current, x - (w - offset))
                }
            } else if x < offset {
                (&current, x)
            } else {
                (&a.bgra, x)
            };
            b.bgra[dst..dst + 4].copy_from_slice(&source[(y * w + sx) * 4..(y * w + sx) * 4 + 4]);
        }
    }
    Ok(())
}
