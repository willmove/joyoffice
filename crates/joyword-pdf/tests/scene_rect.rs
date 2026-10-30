//! A filled rectangle is part of the shared page scene, so it changes the PDF.

use joyword_foundation::Emu;
use joyword_layout::{PageScene, SceneRect};
use joyword_pdf::render;

#[test]
fn a_scene_rect_changes_the_pdf_bytes() {
    let page = PageScene {
        width: Emu::new(72 * Emu::PER_POINT),
        height: Emu::new(72 * Emu::PER_POINT),
        runs: Vec::new(),
        rects: vec![SceneRect {
            screen_only: false,
            x: Emu::new(Emu::PER_POINT),
            y: Emu::new(Emu::PER_POINT),
            width: Emu::new(10 * Emu::PER_POINT),
            height: Emu::new(4 * Emu::PER_POINT),
            red: 255,
            green: 0,
            blue: 0,
        }],
        images: Vec::new(),
    };
    let painted = render(std::slice::from_ref(&page), &[]).unwrap();
    let mut blank = page;
    blank.rects.clear();
    let plain = render(&[blank], &[]).unwrap();
    assert_ne!(painted, plain);
    assert!(painted.starts_with(b"%PDF"));
}

#[test]
fn editing_overlays_are_not_printed_but_same_color_document_ink_remains() {
    let ink = SceneRect {
        screen_only: false,
        x: Emu::new(Emu::PER_POINT),
        y: Emu::new(Emu::PER_POINT),
        width: Emu::new(10 * Emu::PER_POINT),
        height: Emu::new(4 * Emu::PER_POINT),
        red: 128,
        green: 128,
        blue: 128,
    };
    let mut page = PageScene {
        width: Emu::new(72 * Emu::PER_POINT),
        height: Emu::new(72 * Emu::PER_POINT),
        runs: Vec::new(),
        rects: vec![ink],
        images: Vec::new(),
    };
    let document = render(std::slice::from_ref(&page), &[]).unwrap();
    page.rects.push(SceneRect {
        screen_only: true,
        ..ink
    });
    assert_eq!(render(std::slice::from_ref(&page), &[]).unwrap(), document);
    page.rects[0].screen_only = true;
    let overlays_only = render(std::slice::from_ref(&page), &[]).unwrap();
    page.rects.clear();
    assert_eq!(render(&[page], &[]).unwrap(), overlays_only);
    assert_ne!(document, overlays_only);
}
