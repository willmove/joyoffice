use joyword_foundation::Emu;
use joyword_layout::{
    FeatureSet, GlyphRun, LoadedFont, PageScene, PaintedUnderline, UnderlinePaint,
    decoration_rects, shape,
};
use joyword_render::{
    Projection, RasterCancellation, RasterError, RasterLimit, RasterLimits, raster_size, rasterize,
};
use std::sync::Arc;

fn pt(n: i64) -> Emu {
    Emu::new(n * Emu::PER_POINT)
}
fn font() -> LoadedFont {
    LoadedFont::from_bytes(
        Arc::from(
            include_bytes!("../../../tests/fixtures/fonts/joyoffice-test-serif.ttf").as_slice(),
        ),
        0,
    )
    .unwrap()
}
fn scene(font: &LoadedFont, text: &str) -> PageScene {
    PageScene {
        width: pt(100),
        height: pt(60),
        rects: Vec::new(),
        images: Vec::new(),
        runs: vec![GlyphRun::place(
            &shape(font, text, pt(18), FeatureSet::default()),
            pt(10),
            pt(30),
        )],
    }
}
fn render(scene: &PageScene, font: &LoadedFont) -> joyword_render::RasterPage {
    rasterize(
        scene,
        std::slice::from_ref(font),
        Projection::new(72.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap()
}
fn ink_bounds(page: &joyword_render::RasterPage) -> (u32, u32, u32, u32) {
    let mut b = (u32::MAX, u32::MAX, 0, 0);
    for (i, p) in page.rgba().as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(p[3], 255);
        if p[0] < 250 {
            let (x, y) = (i as u32 % page.width(), i as u32 / page.width());
            b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
        }
    }
    assert_ne!(b.0, u32::MAX);
    b
}
#[test]
fn srgb_text_and_its_underline_paint_the_run_colour() {
    let font = font();
    let mut run = GlyphRun::place(
        &shape(&font, "W", pt(18), FeatureSet::default()),
        pt(10),
        pt(30),
    );
    run.color = [200, 0, 0];
    run.underline = Some(UnderlinePaint {
        kind: PaintedUnderline::Single,
        offset: pt(2),
        thickness: pt(2),
    });
    let mut scene = scene(&font, "W");
    scene.runs = vec![run.clone()];
    scene.rects = decoration_rects(&run, false, false, &[]);
    let page = render(&scene, &font);
    let coloured = page
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|pixel| pixel[0] >= 180 && pixel[1] < 40 && pixel[2] < 40 && pixel[3] == 255);
    assert!(coloured, "expected red ink from the run colour");
}

#[test]
fn subpixel_underlines_remain_visible_between_pixel_centres() {
    let font = font();
    for phase in 0..5 {
        let mut scene = scene(&font, "");
        scene.rects.push(joyword_layout::SceneRect {
            screen_only: false,
            x: pt(10),
            y: Emu::new(pt(40).raw() + phase * Emu::PER_POINT / 5),
            width: pt(20),
            height: Emu::new(Emu::PER_POINT / 5),
            red: 0,
            green: 0,
            blue: 255,
        });
        let page = render(&scene, &font);
        assert!(
            page.rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] < 250 && pixel[2] > pixel[0]),
            "thin underline vanished at subpixel phase {phase}"
        );
    }
}

#[test]
fn narrow_fractional_rects_keep_their_pixels_without_panicking() {
    let font = font();
    for phase in 1..5 {
        let mut vertical = scene(&font, "");
        vertical.rects.push(joyword_layout::SceneRect {
            screen_only: false,
            x: Emu::new(pt(10).raw() + phase * Emu::PER_POINT / 5),
            y: pt(10),
            width: Emu::new(pt(1).raw() + Emu::PER_POINT / 5),
            height: pt(10),
            red: 0,
            green: 0,
            blue: 255,
        });
        let page = render(&vertical, &font);
        assert!(
            page.rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] < 250 && pixel[2] > pixel[0]),
            "narrow rect vanished at subpixel phase {phase}"
        );

        let mut horizontal = scene(&font, "");
        horizontal.rects.push(joyword_layout::SceneRect {
            screen_only: false,
            x: pt(10),
            y: Emu::new(pt(40).raw() + phase * Emu::PER_POINT / 5),
            width: pt(20),
            height: Emu::new(pt(1).raw() + Emu::PER_POINT / 5),
            red: 0,
            green: 0,
            blue: 255,
        });
        let page = render(&horizontal, &font);
        assert!(
            page.rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] < 250 && pixel[2] > pixel[0]),
            "narrow horizontal rect vanished at subpixel phase {phase}"
        );
    }
}

#[test]
fn exact_scene_is_deterministic_opaque_and_not_mutated() {
    let font = font();
    let s = scene(&font, "AA fi");
    let before = s.clone();
    let bytes = font.data().clone();
    let first = render(&s, &font);
    let second = render(&s, &font);
    assert_eq!(first, second);
    assert_eq!((first.width(), first.height()), (100, 60));
    let b = ink_bounds(&first);
    assert!(b.0 >= 10 && b.2 < 60 && b.1 < 30 && b.3 <= 30, "{b:?}");
    assert_eq!(s, before);
    assert_eq!(font.data(), &bytes);
    assert_eq!(first.stats().glyphs, s.runs[0].glyphs.len());
    assert!(first.stats().outlines < first.stats().glyphs);
}
#[test]
fn translated_origin_translates_pixels_without_reshaping() {
    let font = font();
    let s = scene(&font, "AV");
    let a = render(&s, &font);
    let mut moved = s.clone();
    moved.runs[0].origin_x = pt(17);
    moved.runs[0].baseline_y = pt(35);
    let b = render(&moved, &font);
    for y in 0..a.height() - 5 {
        for x in 0..a.width() - 7 {
            let i = ((y * a.width() + x) * 4) as usize;
            let j = (((y + 5) * b.width() + x + 7) * 4) as usize;
            assert_eq!(&a.rgba()[i..i + 4], &b.rgba()[j..j + 4]);
        }
    }
    assert_eq!(
        ink_bounds(&b),
        (
            ink_bounds(&a).0 + 7,
            ink_bounds(&a).1 + 5,
            ink_bounds(&a).2 + 7,
            ink_bounds(&a).3 + 5
        )
    );
}
#[test]
fn glyph_offsets_use_font_upward_y_and_page_downward_y() {
    let font = font();
    let s = scene(&font, "A");
    let a = render(&s, &font);
    let mut shifted = s.clone();
    shifted.runs[0].glyphs[0].x_offset = pt(4);
    shifted.runs[0].glyphs[0].y_offset = pt(3);
    let b = render(&shifted, &font);
    let ab = ink_bounds(&a);
    let bb = ink_bounds(&b);
    assert_eq!(bb, (ab.0 + 4, ab.1 - 3, ab.2 + 4, ab.3 - 3));
}
#[test]
fn zoom_and_dpi_only_change_raster_projection() {
    let font = font();
    let s = scene(&font, "AV fi");
    let before = s.clone();
    let a = rasterize(
        &s,
        std::slice::from_ref(&font),
        Projection::new(144.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let b = rasterize(
        &s,
        std::slice::from_ref(&font),
        Projection::new(72.0, 2.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    assert_eq!(a, b);
    assert_eq!((a.width(), a.height()), (200, 120));
    assert_eq!(s, before);
    let normal = render(&s, &font);
    let n = ink_bounds(&normal);
    let large = ink_bounds(&a);
    assert!((i64::from(large.0) - 2 * i64::from(n.0)).abs() <= 1);
    assert!((i64::from(large.1) - 2 * i64::from(n.1)).abs() <= 1);
}
#[test]
fn a_tab_gap_may_skip_the_pen_forward_but_not_back() {
    let font = font();
    let solid = scene(&font, "AV");
    let mut gapped = solid.clone();
    let gap = pt(30).raw();
    let second = &mut gapped.runs[0].glyphs[1];
    second.x = Emu::new(second.x.raw() + gap);
    let page = render(&gapped, &font);
    let together = ink_bounds(&render(&solid, &font));
    let apart = ink_bounds(&page);
    assert_eq!(apart.0, together.0, "the first glyph does not move");
    assert_eq!(
        apart.2,
        together.2 + 30,
        "the gap moves only the second glyph"
    );

    let mut backwards = solid.clone();
    backwards.runs[0].glyphs[1].x = Emu::new(backwards.runs[0].glyphs[0].x.raw() + 1);
    assert_eq!(
        rasterize(
            &backwards,
            std::slice::from_ref(&font),
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &RasterCancellation::default()
        ),
        Err(RasterError::InvalidGeometry)
    );
}
#[test]
fn whitespace_and_empty_pages_have_no_ink_but_keep_geometry() {
    let font = font();
    let s = scene(&font, "   ");
    let p = render(&s, &font);
    assert!(p.rgba().iter().all(|c| *c == 255));
    assert_eq!(p.stats().glyphs, 3);
    let mut empty = s;
    empty.runs.clear();
    assert_eq!(render(&empty, &font).rgba(), p.rgba());
}
#[test]
fn png_decodes_to_the_same_pixels() {
    let font = font();
    let p = render(&scene(&font, "AV"), &font);
    let png = p.png().unwrap();
    let decoded = tiny_skia::Pixmap::decode_png(&png).unwrap();
    assert_eq!(decoded.data(), p.rgba());
    assert_eq!((decoded.width(), decoded.height()), (p.width(), p.height()));
}
#[test]
fn exact_font_bytes_and_face_identity_are_required() {
    let font = font();
    let s = scene(&font, "AV");
    assert_eq!(
        rasterize(
            &s,
            &[],
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &RasterCancellation::default()
        ),
        Err(RasterError::MissingFont(font.key()))
    );
    let mut different = font.data().to_vec();
    different.push(0);
    let different = LoadedFont::from_bytes(different.into(), 0).unwrap();
    assert_eq!(
        rasterize(
            &s,
            &[different],
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &RasterCancellation::default()
        ),
        Err(RasterError::MissingFont(font.key()))
    );
    assert_eq!(
        rasterize(
            &s,
            &[font.clone(), font.clone()],
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &RasterCancellation::default()
        ),
        Err(RasterError::DuplicateFont(font.key()))
    );
}
#[test]
fn every_budget_fails_before_publishing_a_partial_page() {
    let font = font();
    let s = scene(&font, "AV");
    let before = render(&s, &font);
    let d = RasterLimits::default();
    for (limits, expected) in [
        (
            RasterLimits {
                max_dimension: 0,
                ..d
            },
            RasterLimit::Dimensions,
        ),
        (RasterLimits { max_pixels: 0, ..d }, RasterLimit::Pixels),
        (RasterLimits { max_fonts: 0, ..d }, RasterLimit::Fonts),
        (
            RasterLimits {
                max_font_bytes: 0,
                ..d
            },
            RasterLimit::FontBytes,
        ),
        (RasterLimits { max_runs: 0, ..d }, RasterLimit::Runs),
        (RasterLimits { max_glyphs: 0, ..d }, RasterLimit::Glyphs),
        (
            RasterLimits {
                max_outlines: 0,
                ..d
            },
            RasterLimit::Outlines,
        ),
        (
            RasterLimits {
                max_commands: 0,
                ..d
            },
            RasterLimit::Commands,
        ),
        (
            RasterLimits {
                max_work_pixels: 0,
                ..d
            },
            RasterLimit::WorkPixels,
        ),
        (
            RasterLimits {
                max_edge_work: 0,
                ..d
            },
            RasterLimit::EdgeWork,
        ),
    ] {
        assert_eq!(
            rasterize(
                &s,
                std::slice::from_ref(&font),
                Projection::new(72.0, 1.0).unwrap(),
                limits,
                &RasterCancellation::default()
            ),
            Err(RasterError::LimitExceeded(expected))
        );
        assert_eq!(render(&s, &font), before);
    }
}
#[test]
fn outline_cache_still_charges_repeated_raster_work() {
    let font = font();
    let s = scene(&font, "AA");
    let mut overlap = s.clone();
    let mut second = s.runs[0].clone();
    second.origin_x = pt(15);
    overlap.runs.push(second);
    let first = render(&s, &font);
    let double = render(&overlap, &font);
    assert_eq!(first.stats().outlines, double.stats().outlines);
    assert_eq!(first.stats().commands, double.stats().commands);
    assert!(double.stats().work_pixels > first.stats().work_pixels);
    assert!(double.stats().edge_work > first.stats().edge_work);
    assert_eq!(
        rasterize(
            &overlap,
            std::slice::from_ref(&font),
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits {
                max_edge_work: first.stats().edge_work,
                ..RasterLimits::default()
            },
            &RasterCancellation::default()
        ),
        Err(RasterError::LimitExceeded(RasterLimit::EdgeWork))
    );
    assert_eq!(
        rasterize(
            &overlap,
            &[font],
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits {
                max_work_pixels: first.stats().work_pixels,
                ..RasterLimits::default()
            },
            &RasterCancellation::default()
        ),
        Err(RasterError::LimitExceeded(RasterLimit::WorkPixels))
    );
}
#[test]
fn invalid_scenes_and_overflow_are_rejected() {
    let font = font();
    for case in 0..8 {
        let mut s = scene(&font, "AV");
        match case {
            0 => s.width = pt(0),
            1 => s.runs[0].size = pt(0),
            2 => s.runs[0].glyphs[0].glyph_id = u16::MAX,
            3 => s.runs[0].glyphs[0].cluster = usize::MAX,
            4 => s.runs[0].glyphs.clear(),
            5 => {
                s.runs[0].origin_x = Emu::new(i64::MAX);
                s.runs[0].glyphs[0].x_offset = pt(1);
            }
            6 => {
                s.runs[0].baseline_y = Emu::new(i64::MAX);
                s.runs[0].glyphs[0].y_offset = Emu::new(-1);
            }
            _ => s.runs[0].glyphs[0].advance = Emu::new(-1),
        }
        assert!(
            rasterize(
                &s,
                std::slice::from_ref(&font),
                Projection::new(72.0, 1.0).unwrap(),
                RasterLimits::default(),
                &RasterCancellation::default()
            )
            .is_err(),
            "{case}"
        );
    }
    for (dpi, zoom) in [
        (f64::NAN, 1.0),
        (72.0, f64::INFINITY),
        (0.0, 1.0),
        (72.0, -1.0),
    ] {
        assert!(Projection::new(dpi, zoom).is_err());
    }
}
#[test]
fn hard_pixel_ceiling_cannot_be_raised() {
    let font = font();
    let mut s = scene(&font, "A");
    s.width = pt(5000);
    s.height = pt(5000);
    assert_eq!(
        rasterize(
            &s,
            &[font],
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits {
                max_dimension: u32::MAX,
                max_pixels: u64::MAX,
                ..RasterLimits::default()
            },
            &RasterCancellation::default()
        ),
        Err(RasterError::LimitExceeded(RasterLimit::Pixels))
    );
}
#[test]
fn raster_size_answers_exactly_what_rasterize_gives_or_refuses() {
    let font = font();
    let fonts = std::slice::from_ref(&font);
    let at = |dpi, zoom| Projection::new(dpi, zoom).unwrap();
    let tight = |max_dimension, max_pixels| RasterLimits {
        max_dimension,
        max_pixels,
        ..RasterLimits::default()
    };
    let raised = tight(u32::MAX, u64::MAX);
    let mut cases = Vec::new();
    for (dpi, zoom) in [(72.0, 1.0), (96.0, 0.8), (192.0, 1.5), (144.0, 0.37)] {
        cases.push((
            scene(&font, "AV fi"),
            at(dpi, zoom),
            RasterLimits::default(),
        ));
    }
    // 100 x 60 pt at 72 dpi: exactly at, then just over, each ceiling.
    for limits in [tight(100, 6000), tight(99, 6000), tight(100, 5999)] {
        cases.push((scene(&font, "A"), at(72.0, 1.0), limits));
    }
    let mut big = scene(&font, "A");
    (big.width, big.height) = (pt(5000), pt(5000));
    cases.push((big, at(72.0, 1.0), raised));
    let mut empty = scene(&font, "A");
    empty.height = pt(0);
    cases.push((empty, at(72.0, 1.0), RasterLimits::default()));
    let mut seen = Vec::new();
    for (s, projection, limits) in &cases {
        let size = raster_size(s, *projection, *limits);
        let page = rasterize(s, fonts, *projection, *limits, &Default::default());
        assert_eq!(size, page.map(|p| (p.width(), p.height())));
        seen.push(size.err());
    }
    for expected in [
        RasterError::LimitExceeded(RasterLimit::Dimensions),
        RasterError::LimitExceeded(RasterLimit::Pixels),
        RasterError::InvalidPage,
    ] {
        assert!(seen.contains(&Some(expected.clone())), "{expected:?}");
    }
    assert_eq!(
        raster_size(&cases[0].0, at(72.0, 1.0), RasterLimits::default()),
        Ok((100, 60))
    );
}
#[test]
fn cancellation_is_shared_monotonic_and_keeps_existing_pixels() {
    let font = font();
    let s = scene(&font, "AV");
    let existing = render(&s, &font);
    let token = RasterCancellation::default();
    let clone = token.clone();
    clone.cancel();
    assert!(token.is_cancelled());
    assert_eq!(
        rasterize(
            &s,
            std::slice::from_ref(&font),
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &token
        ),
        Err(RasterError::Cancelled)
    );
    assert_eq!(render(&s, &font), existing);
}

fn retag_table(mut bytes: Vec<u8>, from: &[u8; 4], to: &[u8; 4]) -> Vec<u8> {
    let n = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
    for i in 0..usize::from(n) {
        let at = 12 + 16 * i;
        if &bytes[at..at + 4] == from {
            bytes[at..at + 4].copy_from_slice(to);
            return bytes;
        }
    }
    panic!("fixture table missing");
}
#[test]
fn color_font_is_refused_instead_of_silently_drawing_base_outlines() {
    let font = font();
    let bytes = retag_table(font.data().to_vec(), b"name", b"COLR");
    let font = LoadedFont::from_bytes(bytes.into(), 0).unwrap();
    let s = scene(&font, "AV");
    assert_eq!(
        rasterize(
            &s,
            std::slice::from_ref(&font),
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &RasterCancellation::default()
        ),
        Err(RasterError::UnsupportedFont(font.key()))
    );
}
#[test]
fn missing_outline_table_is_an_error_instead_of_blank_success() {
    let font = font();
    let bytes = retag_table(font.data().to_vec(), b"glyf", b"xxxx");
    let font = LoadedFont::from_bytes(bytes.into(), 0).unwrap();
    let s = scene(&font, "AV");
    assert!(matches!(
        rasterize(
            &s,
            std::slice::from_ref(&font),
            Projection::new(72.0, 1.0).unwrap(),
            RasterLimits::default(),
            &RasterCancellation::default()
        ),
        Err(RasterError::Outline { .. })
    ));
}

#[test]
fn legacy_ebdt_strikes_do_not_disable_existing_scalable_outlines() {
    let original = font();
    // SimSun contains EBDT alongside glyf. Table presence must not prevent
    // this declared outline renderer from using unchanged scalable glyphs.
    let bytes = retag_table(original.data().to_vec(), b"name", b"EBDT");
    let font = LoadedFont::from_bytes(bytes.into(), 0).unwrap();
    let a = render(&scene(&original, "AV fi"), &original);
    let b = render(&scene(&font, "AV fi"), &font);
    assert_eq!(a.rgba(), b.rgba());
    assert_eq!(a.stats(), b.stats());
}
