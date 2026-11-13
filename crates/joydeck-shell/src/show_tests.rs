use super::show::*;
use crate::Cancellation;
use joydeck_model::Animation;
use std::sync::Arc;

#[test]
fn pause_and_large_tick_sample_real_entrance_pixels() {
    let mut session = joydeck_engine::Session::new(
        joydeck_model::Presentation::default(),
        Default::default(),
        true,
    );
    session
        .execute(joydeck_engine::Command::InsertRect {
            x: 100.,
            y: 100.,
            width: 120.,
            height: 80.,
        })
        .unwrap();
    let mut model = (*session.snapshot().model).clone();
    let id = model.slides[0].shapes.last().unwrap().id;
    Arc::make_mut(&mut model.slides[0])
        .animations
        .push(Animation {
            shape: id,
            duration_ms: 1000,
            ..Default::default()
        });
    let model = Arc::new(model);
    let mut p = Player::new(model.clone(), None, None).unwrap();
    let cancel = Cancellation::default();
    let before = p.frame(0.25, &cancel).unwrap();
    p.navigate("next", None).unwrap();
    p.advance(0.5).unwrap();
    let middle = p.frame(0.25, &cancel).unwrap();
    assert_ne!(before.pixels, middle.pixels);
    let elapsed = p.elapsed;
    p.navigate("pause", None).unwrap();
    p.advance(2.).unwrap();
    assert_eq!(p.elapsed, elapsed);
    p.navigate("pause", None).unwrap();
    p.advance(5.).unwrap();
    assert_eq!(p.state.step, 1);
    let after = p.frame(0.25, &cancel).unwrap();
    assert_ne!(middle.pixels, after.pixels);
    p.navigate("black", None).unwrap();
    let blank = p.frame(0.1, &cancel).unwrap();
    assert!(
        blank
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255])
    );
    assert!(p.advance(f64::NAN).is_err());
    assert_eq!(model.slides[0].animations[0].shape, id);
}

#[test]
fn explicit_custom_show_reports_actual_order_and_skips_hidden_page() {
    let mut session = joydeck_engine::Session::new(
        joydeck_model::Presentation::default(),
        Default::default(),
        true,
    );
    session.execute(joydeck_engine::Command::NewSlide).unwrap();
    session.execute(joydeck_engine::Command::NewSlide).unwrap();
    let mut model = (*session.snapshot().model).clone();
    Arc::make_mut(&mut model.slides[1]).hidden = true;
    model.custom_shows.push(joydeck_model::CustomShow {
        name: "reverse".into(),
        slides: vec![model.slides[2].id, model.slides[1].id, model.slides[0].id],
    });
    let mut player = Player::new(Arc::new(model), None, Some("reverse")).unwrap();
    assert_eq!(player.status()["order"], serde_json::json!([3, 1]));
    assert_eq!(player.state.slide, 2);
    player.navigate("next", None).unwrap();
    assert_eq!(player.state.slide, 0);
}

#[test]
fn viewport_targets_use_physical_pixels_and_keep_the_existing_allocation_budget() {
    use crate::show_render::RenderTarget;
    for (dpi, width, height) in [
        (1., 960, 540),
        (1.5, 1440, 810),
        (2., 1920, 1080),
        (4., 3840, 2160),
    ] {
        let target = RenderTarget::fit((960., 540.), (960., 540.), dpi).unwrap();
        assert_eq!((target.width, target.height), (width, height));
    }
    assert_eq!(
        RenderTarget::fit((960., 540.), (1920., 1080.), 1.).unwrap(),
        RenderTarget::fit((960., 540.), (960., 540.), 2.).unwrap()
    );
    for slide in [(960., 540.), (4096., 1.), (1., 4096.), (4096., 4096.)] {
        let target = RenderTarget::fit(slide, (16384., 16384.), 4.).unwrap();
        assert!(target.width <= 8192 && target.height <= 8192);
        assert!(u64::from(target.width) * u64::from(target.height) <= 16_000_000);
        assert!(target.scale() <= 8.);
    }
    for bad in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(RenderTarget::fit((960., 540.), (1920., 1080.), bad).is_err());
        assert!(RenderTarget::fit((bad, 540.), (1920., 1080.), 1.).is_err());
        assert!(RenderTarget::fit((960., 540.), (bad, 1080.), 1.).is_err());
    }
    assert!(RenderTarget::new(4096., 4096., 1.).is_err());
    assert!(RenderTarget::new(960., 540., f64::NAN).is_err());
}

#[test]
fn high_dpi_small_slides_render_above_four_times_and_cache_invalidation_keeps_pixels() {
    use crate::show_render::{RasterCache, RenderTarget};
    let mut model = joydeck_model::Presentation {
        slide_size: joydeck_geom::Size::new(720., 405.),
        ..Default::default()
    };
    Arc::make_mut(&mut model.slides[0]).shapes.clear();
    let mut player = Player::new(Arc::new(model), None, None).unwrap();
    let cancel = Cancellation::default();
    let mut cache = RasterCache::default();
    let target = RenderTarget::fit((720., 405.), (1920., 1080.), 2.).unwrap();
    assert!(target.scale() > 4.);
    let mut first = cache.render(&player, target, true, &cancel).unwrap();
    assert_eq!((first.width, first.height), (3840, 2160));
    first.pixels[0] = 17; // A media overlay must not contaminate the retained page raster.
    player.advance(0.5).unwrap();
    let second = cache.render(&player, target, true, &cancel).unwrap();
    assert_ne!(second.pixels[0], 17);
    assert_eq!((cache.renders, cache.hits), (1, 1));
    let low = RenderTarget::fit((720., 405.), (960., 540.), 1.).unwrap();
    let resized = cache.render(&player, low, true, &cancel).unwrap();
    assert_eq!((resized.width, resized.height), (960, 540));
    player.navigate("black", None).unwrap();
    let black = cache.render(&player, low, true, &cancel).unwrap();
    assert!(
        black
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255])
    );
    assert_eq!(cache.renders, 3);
    cancel.cancel();
    assert!(cache.render(&player, low, true, &cancel).is_err());
}

#[test]
fn animation_and_transition_frames_bypass_static_media_cache() {
    use crate::show_render::{RasterCache, RenderTarget};
    let mut session = joydeck_engine::Session::new(
        joydeck_model::Presentation::default(),
        Default::default(),
        true,
    );
    session
        .execute(joydeck_engine::Command::InsertRect {
            x: 100.,
            y: 100.,
            width: 120.,
            height: 80.,
        })
        .unwrap();
    let mut model = (*session.snapshot().model).clone();
    let slide = Arc::make_mut(&mut model.slides[0]);
    let id = slide.shapes.last().unwrap().id;
    slide.animations.push(Animation {
        shape: id,
        duration_ms: 1000,
        ..Default::default()
    });
    let mut player = Player::new(Arc::new(model), None, None).unwrap();
    let target = RenderTarget::new(960., 540., 0.25).unwrap();
    let cancel = Cancellation::default();
    let mut cache = RasterCache::default();
    player.navigate("next", None).unwrap();
    let start = cache.render(&player, target, true, &cancel).unwrap();
    player.advance(0.5).unwrap();
    let middle = cache.render(&player, target, true, &cancel).unwrap();
    assert_ne!(start.pixels, middle.pixels);
    assert_eq!(
        middle.pixels,
        player.frame(target.scale(), &cancel).unwrap().pixels
    );
    assert_eq!((cache.renders, cache.hits), (2, 0));
    let mut model = (*player.model).clone();
    let slide = Arc::make_mut(&mut model.slides[0]);
    slide.animations.clear();
    slide.transition = Some(joydeck_model::Transition {
        kind: "fade".into(),
        duration_ms: 1000,
        ..Default::default()
    });
    let mut transition = Player::new(Arc::new(model), None, None).unwrap();
    let start = cache.render(&transition, target, true, &cancel).unwrap();
    transition.advance(0.5).unwrap();
    let middle = cache.render(&transition, target, true, &cancel).unwrap();
    assert_ne!(start.pixels, middle.pixels);
    assert_eq!(
        middle.pixels,
        transition.frame(target.scale(), &cancel).unwrap().pixels
    );
    assert_eq!((cache.renders, cache.hits), (4, 0));
}
