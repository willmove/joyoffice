//! T204 error path: a scene font whose embedding bits forbid embedding refuses
//! export with [`PdfError::UnembeddableFont`]; allowed bits still embed.

use std::sync::Arc;

use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, LoadedFont, PageScene, shape};
use joyword_pdf::PdfError;

fn pt(value: i64) -> Emu {
    Emu::new(value * Emu::PER_POINT)
}

/// The committed fixture with its OS/2 `fsType` patched. The table directory
/// is walked to find `OS/2`; `fsType` is the fifth u16 of that table.
fn font_with_fs_type(fs_type: u16) -> LoadedFont {
    let mut bytes =
        include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").to_vec();
    let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let record = (0..count)
        .map(|index| 12 + 16 * index)
        .find(|&at| bytes[at..at + 4] == *b"OS/2")
        .expect("fixture carries an OS/2 table");
    let table = u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
    bytes[table + 8..table + 10].copy_from_slice(&fs_type.to_be_bytes());
    LoadedFont::from_bytes(Arc::from(bytes.as_slice()), 0).expect("patched font still loads")
}

fn scene(font: &LoadedFont) -> PageScene {
    let shaped = shape(font, "A", pt(12), FeatureSet::default());
    let run = joyword_layout::GlyphRun::place(&shaped, pt(12), pt(24));
    PageScene {
        width: pt(200),
        height: pt(200),
        runs: vec![run],
        rects: Vec::new(),
        images: Vec::new(),
    }
}

#[test]
fn a_restricted_license_font_refuses_export_with_no_bytes() {
    // fsType & 0xF == 2 is Restricted License embedding (OpenType OS/2).
    let font = font_with_fs_type(2);
    let scene = scene(&font);
    let error = joyword_pdf::render(std::slice::from_ref(&scene), std::slice::from_ref(&font))
        .expect_err("a restricted-license font must not produce PDF bytes");
    assert_eq!(error, PdfError::UnembeddableFont(font.key()));
    assert_ne!(error, PdfError::MissingFont(font.key()));
}

#[test]
fn the_restriction_reads_the_low_nibble_like_krilla() {
    // 0x12 sets bit 4 outside the sub-field; only the low nibble counts, so
    // this is still Restricted License and refuses (krilla text/cid.rs).
    let restricted = font_with_fs_type(0x12);
    let page = scene(&restricted);
    let error = joyword_pdf::render(
        std::slice::from_ref(&page),
        std::slice::from_ref(&restricted),
    )
    .expect_err("fsType 0x12 is Restricted License by the low nibble");
    assert_eq!(error, PdfError::UnembeddableFont(restricted.key()));
    // 6 is bits 1+2 together; the spec reads the least restrictive variant,
    // so this face stays embeddable.
    let mixed = font_with_fs_type(6);
    let page = scene(&mixed);
    let pdf =
        joyword_pdf::render(std::slice::from_ref(&page), std::slice::from_ref(&mixed)).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
}

#[test]
fn an_allowed_embedding_bit_still_exports() {
    let font = font_with_fs_type(8);
    let scene = scene(&font);
    let pdf = joyword_pdf::render(std::slice::from_ref(&scene), std::slice::from_ref(&font))
        .expect("editable-embedding bits do not refuse export");
    assert!(pdf.starts_with(b"%PDF-"));
}
