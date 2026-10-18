//! Admission tests for still images. Panic sites belong in this file, which
//! the no-panic gate skips, rather than in the library module.

use joyword_foundation::Emu;
use joyword_model::{
    ImageLimits, ImageRefuse, Measure, accept_image, decode_rgba, display_extent, extent_emu,
    sample_png_1x1,
};

#[test]
fn a_one_pixel_png_is_admitted_and_decoded() {
    let bytes = sample_png_1x1();
    let image = accept_image(&bytes, &ImageLimits::default()).unwrap();
    assert_eq!(image.media_type, "image/png");
    assert_eq!((image.width_px, image.height_px), (1, 1));
    let decoded = decode_rgba(&bytes, &ImageLimits::default()).unwrap();
    assert_eq!(&decoded.rgba[..3], &[255, 0, 0]);
    let (width, height) = display_extent(1, 1);
    assert_eq!(width, Measure::Emu(Emu::PER_INCH / 96));
    assert_eq!(height, width);
}

#[test]
fn active_remote_and_oversize_payloads_are_refused_before_use() {
    assert_eq!(
        accept_image(b"https://example.invalid/a.png", &ImageLimits::default()),
        Err(ImageRefuse::Remote)
    );
    assert_eq!(
        accept_image(br"\\server\share\a.png", &ImageLimits::default()),
        Err(ImageRefuse::Remote)
    );
    assert_eq!(
        accept_image(b"<svg onload='alert(1)'></svg>", &ImageLimits::default()),
        Err(ImageRefuse::Active)
    );
    assert_eq!(
        accept_image(
            &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
            &ImageLimits::default()
        ),
        Err(ImageRefuse::Active)
    );
    assert_eq!(
        accept_image(b"not an image", &ImageLimits::default()),
        Err(ImageRefuse::Malformed)
    );
    let png = sample_png_1x1();
    assert!(matches!(
        accept_image(
            &png,
            &ImageLimits {
                max_bytes: 8,
                ..ImageLimits::default()
            }
        ),
        Err(ImageRefuse::OversizeBytes { .. })
    ));
    assert!(matches!(
        accept_image(
            &png,
            &ImageLimits {
                max_pixels: 0,
                ..ImageLimits::default()
            }
        ),
        Err(ImageRefuse::OversizePixels { .. })
    ));
    assert_eq!(extent_emu(0, 10), Err(ImageRefuse::Malformed));
    assert!(matches!(
        extent_emu(100 * Emu::PER_INCH + 1, 10),
        Err(ImageRefuse::OversizeDimension { .. })
    ));
}

#[test]
fn a_large_pixel_count_is_shrunk_for_display_without_changing_bytes() {
    let (width, height) = display_extent(10_000, 5_000);
    let Measure::Emu(width) = width else {
        panic!("width");
    };
    let Measure::Emu(height) = height else {
        panic!("height");
    };
    let max_side = 6 * Emu::PER_INCH;
    assert!(width <= max_side && height <= max_side);
    assert_eq!(width, height * 2);
}
