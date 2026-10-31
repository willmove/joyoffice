use joydeck::{
    document::Deck,
    edit::{self, Command, Session},
    preview,
    scene::{SceneCache, bench},
};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
fn sample() -> Deck {
    let mut s = Session::new();
    for (i, kind) in [
        "rect",
        "ellipse",
        "line",
        "roundRect",
        "arrow",
        "text",
        "text",
    ]
    .iter()
    .enumerate()
    {
        let mut o = edit::object(kind);
        o.x = i as f32 * 90. - 10.;
        o.y = 100. + (i % 2) as f32 * 80.;
        o.width = 120.;
        o.height = 95.;
        o.stroke_width = if i == 5 { 0. } else { 3. };
        o.rotation = if i == 5 { 0. } else { 17. };
        if i == 6 {
            o.opacity = 0.5;
            o.text = "Alpha overlapping".into();
        } else if i == 5 {
            o.text = "ABC é\nsecond".into();
        }
        s.execute(s.id, Command::AddObject(0, o)).unwrap();
    }
    (*s.deck).clone()
}
#[test]
fn scene_cpu_reference_preserves_geometry_text_opacity_and_z_order() {
    let d = sample();
    let cancel = AtomicBool::new(false);
    let mut c = SceneCache::default();
    let scene = c.build(&d, 0, 9, 4, &cancel).unwrap();
    let a = scene.raster(960, &cancel).unwrap();
    let b = preview::render(&d, 0, 960, &cancel).unwrap();
    assert_eq!((a.width, a.height), (b.width, b.height));
    let mean = a
        .bgra
        .iter()
        .zip(&b.bgra)
        .map(|(a, b)| a.abs_diff(*b) as f64)
        .sum::<f64>()
        / a.bgra.len() as f64;
    assert!(mean < 0.5, "CPU scene/reference mean channel error {mean}");
    assert_eq!(scene.document, 9);
    assert_eq!(scene.revision, 4);
    assert!(scene.coverage().0 >= 5);
    assert!(scene.coverage().1 >= 1);
    assert!(scene.coverage().2 > 0);
}
#[test]
fn movement_reuses_outlines_and_only_changed_styles_invalidate() {
    let mut d = sample();
    let mut c = SceneCache::default();
    let cancel = AtomicBool::new(false);
    let a = c.build(&d, 0, 1, 0, &cancel).unwrap();
    let builds = c.stats.builds;
    d.slides[0].objects[5].x += 50.;
    let b = c.build(&d, 0, 1, 1, &cancel).unwrap();
    assert_eq!(c.stats.builds, builds);
    for (a, b) in a.nodes.iter().zip(&b.nodes) {
        assert!(Arc::ptr_eq(&a.geometry, &b.geometry));
    }
    d.slides[0].objects[5].fill = "#ff0000".into();
    let updated = c.build(&d, 0, 1, 2, &cancel).unwrap();
    assert_eq!(c.stats.builds, builds + 1);
    assert!(!Arc::ptr_eq(
        &updated.nodes[5].geometry,
        &b.nodes[5].geometry
    ));
    assert!(c.stats.bytes < joydeck::scene::CACHE_BUDGET);
}
#[test]
fn effects_are_isolated_and_images_are_real_bounded_rgba() {
    let effects = bench::effect().unwrap();
    assert_eq!(effects.coverage().1, 30);
    assert!(
        effects
            .nodes
            .iter()
            .all(|n| n.geometry.paths.is_empty() && n.geometry.fallback.is_some())
    );
    let mut cache = SceneCache::default();
    let scene = bench::edit(&mut cache).unwrap();
    assert_eq!(scene.nodes.len(), 200);
    let image = &scene.nodes[180].geometry;
    assert_eq!(image.fallback.as_ref().unwrap().rgba.len(), 512 * 512 * 4);
    assert!(!Arc::ptr_eq(image, &scene.nodes[199].geometry));
    assert_ne!(
        image.fallback.as_ref().unwrap().rgba[0],
        scene.nodes[199].geometry.fallback.as_ref().unwrap().rgba[0]
    );
    assert!(
        scene.coverage().2 >= 7500,
        "benchmark must contain actual shaped glyphs: {}",
        scene.coverage().2
    );
}
#[test]
fn cancellation_and_invalid_model_do_not_publish_a_scene() {
    let mut cache = SceneCache::default();
    assert!(
        cache
            .build(&sample(), 0, 1, 0, &AtomicBool::new(true))
            .is_err()
    );
    let mut d = sample();
    d.slides[0].objects[0].width = f32::NAN;
    assert!(cache.build(&d, 0, 1, 0, &AtomicBool::new(false)).is_err());
}
#[test]
fn benchmark_cpu_measurement_is_recorded_without_claiming_native_latency() {
    let mut cache = SceneCache::default();
    let scene = bench::edit(&mut cache).unwrap();
    let before = cache.stats.builds;
    let start = Instant::now();
    let raster = scene.raster(1200, &AtomicBool::new(false)).unwrap();
    assert_eq!(raster.width, 1200);
    assert_eq!(cache.stats.builds, before);
    eprintln!(
        "BENCH_CPU frame_ms={} glyphs={}",
        start.elapsed().as_secs_f64() * 1000.,
        scene.coverage().2
    );
}

#[test]
fn local_alpha_crosses_gpui_boundary_once_in_bgra_order() {
    let bitmap = joydeck::scene::Bitmap {
        width: 2,
        height: 1,
        rgba: Arc::new(vec![64, 32, 16, 128, 0, 0, 0, 0]),
    };
    assert_eq!(bitmap.gpui_bgra(), vec![32, 64, 128, 128, 0, 0, 0, 0]);
}

#[test]
fn cache_eviction_is_bounded_and_clear_does_not_mutate_model() {
    let d = sample();
    let original = d.clone();
    let cancel = AtomicBool::new(false);
    let mut cache = SceneCache::with_budget(4000);
    let first = cache.build(&d, 0, 1, 0, &cancel).unwrap();
    assert!(cache.stats.bytes <= 4000);
    assert!(cache.stats.evictions > 0);
    cache.clear();
    assert_eq!(cache.stats.bytes, 0);
    assert_eq!(d, original);
    let second = cache.build(&d, 0, 1, 0, &cancel).unwrap();
    assert_eq!(first.nodes.len(), second.nodes.len());
    assert_eq!(
        first.raster(960, &cancel).unwrap().bgra,
        second.raster(960, &cancel).unwrap().bgra
    );
}

#[test]
fn shared_glyph_outlines_keep_layout_positions_without_reshaping() {
    let mut s = Session::new();
    let mut o = edit::object("text");
    o.text = "A A".into();
    s.execute(s.id, Command::AddObject(0, o)).unwrap();
    let mut cache = SceneCache::default();
    let scene = cache
        .build(&s.deck, 0, s.id, s.revision, &AtomicBool::new(false))
        .unwrap();
    let geometry = &scene.nodes[0].geometry;
    assert!(geometry.fallback.is_none());
    assert_eq!(geometry.missing_glyphs, 0);
    assert_eq!(geometry.glyph_masks.len(), 2);
    assert_eq!(
        geometry.glyph_masks[0].glyph_id,
        geometry.glyph_masks[1].glyph_id
    );
    assert!(Arc::ptr_eq(
        &geometry.glyph_masks[0].path,
        &geometry.glyph_masks[1].path
    ));
    assert!(geometry.glyph_masks[1].position.0 > geometry.glyph_masks[0].position.0);
}

#[test]
fn oversized_text_is_rejected_before_shaping_without_changing_document() {
    let mut s = Session::new();
    let mut object = edit::object("text");
    object.text = "A".repeat(50_001);
    s.execute(s.id, Command::AddObject(0, object)).unwrap();
    let before = s.deck.clone();
    let cancel = AtomicBool::new(false);
    let mut cache = SceneCache::default();
    assert!(cache.build(&s.deck, 0, s.id, s.revision, &cancel).is_err());
    assert_eq!(cache.stats.builds, 0);
    assert!(preview::render(&s.deck, 0, 960, &cancel).is_err());
    assert!(Arc::ptr_eq(&before, &s.deck));
    assert_eq!(s.deck.slides[0].objects[0].text.len(), 50_001);
}

#[test]
fn image_primitive_rejects_invalid_dimensions_and_payloads() {
    let bitmap = joydeck::scene::Bitmap {
        width: 2,
        height: 2,
        rgba: Arc::new(vec![255; 15]),
    };
    assert!(joydeck::scene::Node::image("bad".into(), (0., 0.), (100., 100.), bitmap).is_err());
    let bitmap = joydeck::scene::Bitmap {
        width: 2,
        height: 2,
        rgba: Arc::new(vec![255; 16]),
    };
    assert!(
        joydeck::scene::Node::image("bad".into(), (0., 0.), (f32::NAN, 100.), bitmap.clone())
            .is_err()
    );
    assert!(joydeck::scene::Node::image("valid".into(), (0., 0.), (100., 100.), bitmap).is_ok());
}
