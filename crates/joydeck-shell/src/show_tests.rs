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
