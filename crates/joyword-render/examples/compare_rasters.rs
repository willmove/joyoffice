//! Independent PDF raster vs CPU page: dimensions, every ink line's bounds,
//! and bidirectional coverage of every dark pixel by actual antialiased ink
//! within one device pixel (dark <150, coverage <250 on opaque white).
use tiny_skia::Pixmap;
fn dark(p: &Pixmap, x: u32, y: u32) -> bool {
    p.pixel(x, y).unwrap().red() < 150
}
fn bands(p: &Pixmap) -> Vec<[u32; 4]> {
    let mut result: Vec<[u32; 4]> = Vec::new();
    for y in 0..p.height() {
        let xs: Vec<_> = (0..p.width()).filter(|x| dark(p, *x, y)).collect();
        let (Some(left), Some(right)) = (xs.first(), xs.last()) else {
            continue;
        };
        if let Some(last) = result.last_mut()
            && y == last[3] + 1
        {
            last[0] = last[0].min(*left);
            last[2] = last[2].max(*right);
            last[3] = y;
        } else {
            result.push([*left, y, *right, y]);
        }
    }
    result
}
fn uncovered(a: &Pixmap, b: &Pixmap) -> u64 {
    let mut count = 0;
    for y in 0..a.height() {
        for x in 0..a.width() {
            if !dark(a, x, y) {
                continue;
            }
            let found = (y.saturating_sub(1)..=(y + 1).min(b.height() - 1)).any(|ny| {
                (x.saturating_sub(1)..=(x + 1).min(b.width() - 1))
                    .any(|nx| b.pixel(nx, ny).unwrap().red() < 250)
            });
            if !found {
                count += 1;
            }
        }
    }
    count
}
fn compare(a: &Pixmap, b: &Pixmap) -> Result<(usize, u32), String> {
    if (a.width(), a.height()) != (b.width(), b.height()) {
        return Err("image dimensions differ".into());
    }
    let (ab, bb) = (bands(a), bands(b));
    if ab.len() != bb.len() {
        return Err(format!("ink lines {} != {}", ab.len(), bb.len()));
    }
    let worst = ab
        .iter()
        .zip(&bb)
        .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)))
        .max()
        .unwrap_or(0);
    if worst > 1 {
        return Err(format!("line ink bounds differ by {worst} px"));
    }
    let mismatch = uncovered(a, b) + uncovered(b, a);
    if mismatch != 0 {
        return Err(format!(
            "{mismatch} dark pixels are not covered within 1 px"
        ));
    }
    Ok((ab.len(), worst))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: compare_rasters <cpu.png> <pdf.png>".into());
    }
    let a = Pixmap::load_png(&args[1])?;
    let b = Pixmap::load_png(&args[2])?;
    let (lines, worst) = compare(&a, &b)?;
    println!(
        "{}x{}; {lines} ink lines; worst line-box deviation {worst} px; all dark pixels covered within 1 px",
        a.width(),
        a.height()
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn image(offset: u32, center: bool) -> Pixmap {
        let mut p = Pixmap::new(50, 40).unwrap();
        p.fill(tiny_skia::Color::WHITE);
        for base in [4, 20, 35] {
            if base == 20 && !center {
                continue;
            }
            for x in base + offset..base + offset + 5 {
                for y in 10..25 {
                    p.pixels_mut()[(y * 50 + x) as usize] =
                        tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 255).unwrap();
                }
            }
        }
        p
    }
    #[test]
    fn identical_images_pass() {
        assert_eq!(compare(&image(0, true), &image(0, true)), Ok((1, 0)));
    }
    #[test]
    fn one_pixel_raster_edge_tolerance_is_explicit() {
        assert_eq!(compare(&image(0, true), &image(1, true)), Ok((1, 1)));
    }
    #[test]
    fn two_pixel_shift_is_rejected() {
        assert!(compare(&image(0, true), &image(2, true)).is_err());
    }
    #[test]
    fn interior_glyph_loss_is_rejected_even_when_line_bounds_match() {
        assert!(
            compare(&image(0, true), &image(0, false))
                .unwrap_err()
                .contains("dark pixels")
        );
    }
}
