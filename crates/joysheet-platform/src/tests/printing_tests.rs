#[test]
fn pathological_pdf_page_dimensions_keep_a_bounded_raster() {
    for (w, h) in [(595., 842.), (1., 1000000.), (1000000., 1.)] {
        let (x, y) = super::preview_size(w, h).unwrap();
        assert!((1..=1200).contains(&x));
        assert!((1..=2000).contains(&y));
    }
    assert!(super::preview_size(f32::NAN, 100.).is_err());
    assert!(super::preview_size(100., 0.).is_err());
}
#[test]
fn invalid_pdf_render_cleans_up_owned_directory() {
    let dir = super::directory().unwrap();
    std::fs::write(dir.join("document.pdf"), b"broken pdf").unwrap();
    assert!(super::render(dir.clone()).is_err());
    assert!(!dir.exists());
}
