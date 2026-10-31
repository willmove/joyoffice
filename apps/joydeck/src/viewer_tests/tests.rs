use super::*;

#[test]
fn malformed_preview_pixels_are_reported_instead_of_panicking() {
    let raster = preview::Raster {
        width: 2,
        height: 2,
        bgra: vec![0; 3],
    };
    assert!(image_of(raster).is_err());
}
