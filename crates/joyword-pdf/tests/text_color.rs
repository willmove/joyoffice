//! sRGB on a glyph run is part of the PDF, because the writer paints the same
//! page scene as the screen.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, GlyphRun, LoadedFont, PageScene, shape};
use joyword_pdf::render;

fn font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}

#[test]
fn a_non_black_run_changes_the_pdf_bytes() {
    let font = font();
    let shaped = shape(
        &font,
        "W",
        Emu::new(18 * Emu::PER_POINT),
        FeatureSet::default(),
    );
    let mut page = PageScene {
        width: Emu::new(100 * Emu::PER_POINT),
        height: Emu::new(60 * Emu::PER_POINT),
        runs: vec![GlyphRun::place(
            &shaped,
            Emu::new(10 * Emu::PER_POINT),
            Emu::new(30 * Emu::PER_POINT),
        )],
        rects: Vec::new(),
        images: Vec::new(),
    };
    let black = render(std::slice::from_ref(&page), std::slice::from_ref(&font)).unwrap();
    page.runs[0].color = [200, 0, 0];
    let red = render(std::slice::from_ref(&page), std::slice::from_ref(&font)).unwrap();
    assert!(black.starts_with(b"%PDF"));
    assert_ne!(black, red);
}
