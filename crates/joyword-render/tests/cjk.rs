//! Real CJK faces are required by the dedicated CI job, never silently skipped there.
use joyword_foundation::Emu;
use joyword_layout::{FeatureSet, GlyphRun, LoadedFont, PageScene, shape};
use joyword_render::{Projection, RasterCancellation, RasterError, RasterLimits, rasterize};
use std::path::Path;
use std::sync::Arc;
fn font() -> Option<LoadedFont> {
    let candidates = if let Ok(spec) = std::env::var("JOYWORD_TEST_FONT") {
        let (p, i) = spec.rsplit_once(':').expect("path:face");
        vec![(p.to_owned(), i.parse().unwrap())]
    } else {
        vec![
            ("/System/Library/Fonts/Supplemental/Songti.ttc".into(), 6),
            (
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
                2,
            ),
            ("C:\\Windows\\Fonts\\simsun.ttc".into(), 0),
        ]
    };
    for (path, face) in candidates {
        if Path::new(&path).is_file() || std::env::var_os("JOYWORD_TEST_FONT").is_some() {
            let data: Arc<[u8]> = std::fs::read(&path)
                .expect("read requested CJK font")
                .into();
            let font = LoadedFont::from_bytes(data, face).expect("usable CJK face");
            eprintln!("EXECUTED {path} face {face} {}", font.key().hash_hex());
            return Some(font);
        }
    }
    assert!(
        std::env::var_os("JOYWORD_REQUIRE_TEST_FONT").is_none(),
        "required CJK font missing"
    );
    eprintln!("SKIPPED: CJK font unavailable");
    None
}
fn scene(font: &LoadedFont) -> PageScene {
    let run = shape(
        font,
        "甲方：青禾工作室；English AV",
        Emu::new(12 * Emu::PER_POINT),
        FeatureSet::default(),
    );
    assert!(run.missing.is_empty());
    PageScene {
        width: Emu::new(400 * Emu::PER_POINT),
        height: Emu::new(60 * Emu::PER_POINT),
        rects: Vec::new(),
        images: Vec::new(),
        runs: vec![GlyphRun::place(
            &run,
            Emu::new(12 * Emu::PER_POINT),
            Emu::new(30 * Emu::PER_POINT),
        )],
    }
}
#[test]
fn cjk_scene_raster_keeps_source_positions_and_complete_ink() {
    let Some(font) = font() else { return };
    let s = scene(&font);
    let before = s.clone();
    let p = rasterize(
        &s,
        std::slice::from_ref(&font),
        Projection::new(144.0, 1.0).unwrap(),
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    assert_eq!(s, before);
    assert_eq!(p.stats().glyphs, s.runs[0].glyphs.len());
    assert!(
        p.rgba()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[0] < 150)
            .count()
            > 1000
    );
    assert_eq!((p.width(), p.height()), (800, 120));
}
#[test]
fn collection_face_is_not_replaced_by_another_face_with_the_same_file_hash() {
    let Some(font) = font() else { return };
    let data = font.data();
    if !data.starts_with(b"ttcf") {
        assert!(
            std::env::var_os("JOYWORD_REQUIRE_TEST_FONT").is_none(),
            "collection fixture required"
        );
        return;
    }
    let count = u32::from_be_bytes(data[8..12].try_into().unwrap());
    assert!(count > 1);
    let other = LoadedFont::from_bytes(data.clone(), (font.key().index() + 1) % count).unwrap();
    assert_eq!(font.key().hash_hex(), other.key().hash_hex());
    assert_ne!(font.key(), other.key());
    let s = scene(&font);
    let projection = Projection::new(96.0, 1.0).unwrap();
    assert_eq!(
        rasterize(
            &s,
            std::slice::from_ref(&other),
            projection,
            RasterLimits::default(),
            &RasterCancellation::default()
        ),
        Err(RasterError::MissingFont(font.key()))
    );
    let correct = rasterize(
        &s,
        std::slice::from_ref(&font),
        projection,
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    let both = rasterize(
        &s,
        &[other, font],
        projection,
        RasterLimits::default(),
        &RasterCancellation::default(),
    )
    .unwrap();
    assert_eq!(correct, both);
}
