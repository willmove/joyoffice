use super::*;

#[test]
fn rejects_invalid_pages_layout_and_nonfinite_options() {
    let p = Presentation::default();
    for opts in [
        PdfOptions {
            dpi: f64::NAN,
            ..Default::default()
        },
        PdfOptions {
            slides: Some(vec![99]),
            ..Default::default()
        },
        PdfOptions {
            layout: PageLayout::Handouts { per_page: 0 },
            ..Default::default()
        },
        PdfOptions {
            paper: (f64::INFINITY, 792.),
            ..Default::default()
        },
    ] {
        assert!(export(&p, &opts).is_err());
    }
    assert!(export_with(&p, &PdfOptions::default(), &|| true).is_err());
}

#[test]
fn clusters_and_handout_layout_keep_unicode_and_aspect() {
    assert_eq!(
        glyph_text_ranges("中😀fi", (0, 4), &[(0, '中'), (1, '😀'), (2, 'f')], 3),
        vec![0..3, 3..7, 7..9]
    );
    for n in [1, 2, 3, 4, 6, 9] {
        let boxes = handout_boxes(n, (612., 792.), 16. / 9.);
        assert_eq!(boxes.len(), usize::from(n));
        assert!(boxes.iter().all(|b| b.x0 >= 0.
            && b.y0 >= 0.
            && b.x1 <= 612.
            && b.y1 <= 792.
            && (b.width() / b.height() - 16. / 9.).abs() < 0.001));
    }
}
