//! A scene image becomes a PDF image XObject.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{PageScene, SceneImage};
use joyword_model::sample_png_1x1;
use joyword_pdf::render;

#[test]
fn a_scene_image_is_embedded_and_not_omitted() {
    let png = sample_png_1x1();
    let page = PageScene {
        width: Emu::new(72 * Emu::PER_POINT),
        height: Emu::new(72 * Emu::PER_POINT),
        runs: Vec::new(),
        rects: Vec::new(),
        images: vec![SceneImage {
            media_type: Arc::from("image/png"),
            bytes: Arc::from(png.as_slice()),
            x: Emu::new(Emu::PER_POINT),
            y: Emu::new(Emu::PER_POINT),
            width: Emu::new(36 * Emu::PER_POINT),
            height: Emu::new(36 * Emu::PER_POINT),
        }],
    };
    let pdf = render(std::slice::from_ref(&page), &[]).unwrap();
    assert!(pdf.windows(8).any(|window| window == b"/Subtype"));
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains("/Image") || text.contains("/Subtype"),
        "pdf did not embed an image xobject"
    );
    let mut blank = page;
    blank.images.clear();
    let plain = render(&[blank], &[]).unwrap();
    assert_ne!(pdf, plain);
}
