//! Scene images are painted by the same raster consumer as glyphs.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{PageScene, SceneImage};
use joyword_model::sample_png_1x1;
use joyword_render::{Projection, RasterCancellation, RasterLimits, rasterize};

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

#[test]
fn a_red_image_paints_a_non_white_pixel() {
    let page = PageScene {
        width: pt(40),
        height: pt(40),
        runs: Vec::new(),
        rects: Vec::new(),
        images: vec![SceneImage {
            media_type: Arc::from("image/png"),
            bytes: Arc::from(sample_png_1x1()),
            x: pt(8),
            y: pt(8),
            width: pt(16),
            height: pt(16),
        }],
    };
    let raster = rasterize(
        &page,
        &[],
        Projection::new(72.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let red = raster
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[0] > 200 && pixel[1] < 40 && pixel[2] < 40 && pixel[3] == 255);
    assert!(red, "the admitted red pixel was not painted");
}
