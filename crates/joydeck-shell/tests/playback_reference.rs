use std::path::Path;
use std::sync::Arc;

use joydeck_anim::{ShowState, Timeline};
use joydeck_model::anim::{ANIMATIONS, AnimStart};
use joydeck_model::{
    AnimClass, Animation, ColorRef, Fill, Geom, Presentation, Rgba, Shape, ShapeId, Slide, SlideId,
    TextBody, Transition, Xfrm,
};
use joydeck_shell::{Cancellation, document::Document, show::Player};
use sha2::{Digest, Sha256};

fn shape(id: u32, name: &str, x: f64, y: f64, color: [u8; 3]) -> Shape {
    Shape {
        id: ShapeId(id),
        name: name.into(),
        xfrm: Some(Xfrm::new(x, y, 148., 88.)),
        fill: Some(Fill::solid(ColorRef::rgb(Rgba::rgb(
            color[0], color[1], color[2],
        )))),
        ..Default::default()
    }
}

fn frame(label: &str, player: &Player) -> joydeck_render::Image {
    let image = player.frame(0.5, &Cancellation::default()).unwrap();
    assert_eq!(
        image.pixels.len(),
        (image.width * image.height * 4) as usize
    );
    println!(
        "PLAY frame {label} {}x{} {:x} {}",
        image.width,
        image.height,
        Sha256::digest(&image.pixels),
        player.status()
    );
    image
}

#[test]
fn all_object_effect_options_sample_before_during_and_after() {
    let s = shape(1, "Target", 150., 170., [200, 50, 42]);
    for &(effect, _, class, _, duration, options) in ANIMATIONS {
        let options = if options.is_empty() {
            &[""][..]
        } else {
            options
        };
        for &option in options {
            let slide = Slide {
                shapes: vec![s.clone()],
                animations: vec![Animation {
                    shape: s.id,
                    class,
                    effect: effect.into(),
                    option: option.into(),
                    duration_ms: duration.max(1),
                    delay_ms: 125,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let tl = Timeline::new(&slide, 960., 540.);
            assert_eq!(tl.effects().len(), 1);
            for fraction in [0., 0.25, 0.5, 0.875, 1.] {
                let time = 0.125 + fraction * f64::from(duration.max(1)) / 1000.;
                let state = tl.state(s.id, None, 0, time);
                println!(
                    "PLAY effect {:?} {effect} {option:?} {fraction} {}",
                    class,
                    serde_json::to_string(&state).unwrap()
                );
            }
            let held = tl.state(s.id, None, 1, 0.);
            assert_eq!(held.visible, class != AnimClass::Exit);
            println!(
                "PLAY held {:?} {effect} {option:?} {}",
                class,
                serde_json::to_string(&held).unwrap()
            );
        }
    }
}

#[test]
fn overlapping_triggers_and_main_steps_reach_the_expected_final_frame() {
    let mut model = Presentation::default();
    let page = Arc::make_mut(&mut model.slides[0]);
    let target = shape(10, "Target", 300., 70., [195, 40, 35]);
    let first = shape(11, "Hide target", 60., 210., [40, 90, 200]);
    let second = shape(12, "Reveal target", 60., 340., [36, 160, 75]);
    page.shapes = vec![target.clone(), first.clone(), second.clone()];
    page.animations = vec![
        Animation {
            shape: target.id,
            class: AnimClass::Path,
            effect: "customPath".into(),
            path: Some("M 0 0 C .02 0 .03 .02 .1 .08 E".into()),
            duration_ms: 1000,
            delay_ms: 250,
            ..Default::default()
        },
        Animation {
            shape: target.id,
            class: AnimClass::Emphasis,
            effect: "spin".into(),
            start: AnimStart::WithPrevious,
            duration_ms: 500,
            auto_reverse: true,
            repeat: 2,
            ..Default::default()
        },
        Animation {
            shape: target.id,
            class: AnimClass::Emphasis,
            effect: "transparency".into(),
            start: AnimStart::AfterPrevious,
            amount: Some(0.5),
            delay_ms: 125,
            duration_ms: 500,
            ..Default::default()
        },
        Animation {
            shape: target.id,
            class: AnimClass::Exit,
            effect: "fadeOut".into(),
            trigger: Some(first.id),
            duration_ms: 400,
            ..Default::default()
        },
        Animation {
            shape: target.id,
            trigger: Some(second.id),
            duration_ms: 200,
            ..Default::default()
        },
    ];
    let model = Arc::new(model);
    let mut player = Player::new(model.clone(), None, None).unwrap();
    let tl = ShowState::timeline(&model, 0);
    assert_eq!(tl.steps(), 1);
    let starts: Vec<_> = tl.effects().iter().take(3).map(|e| e.start).collect();
    assert_eq!(starts, [0.25, 0., 2.125]);
    let initial = frame("combined/start", &player);
    player.navigate("next", None).unwrap();
    player.advance(0.75).unwrap();
    frame("combined/main-with", &player);
    player.click(80., 260.).unwrap();
    player.advance(0.125).unwrap();
    frame("combined/first-trigger", &player);
    player.click(80., 370.).unwrap();
    player.advance(0.125).unwrap();
    frame("combined/both-triggers", &player);
    let stamps = player.state.trigger_stamps();
    assert_eq!(
        stamps.iter().map(|s| (s.trigger, s.at)).collect::<Vec<_>>(),
        [(first.id, 1), (second.id, 2)]
    );
    println!("PLAY stamps {stamps:?}");
    player.navigate("pause", None).unwrap();
    let frozen = frame("combined/paused", &player);
    player.advance(10.).unwrap();
    assert_eq!(
        frozen.pixels,
        frame("combined/still-paused", &player).pixels
    );
    player.navigate("pause", None).unwrap();
    player.advance(2.).unwrap();
    assert_eq!(player.state.step, 1);
    assert!(!player.animated());
    let state = player.state.anim_state(&tl, target.id, player.elapsed);
    assert!(state.visible && state.opacity == 0.5 && state.rotate == 0. && state.offset_x > 90.);
    println!("PLAY terminal {}", serde_json::to_string(&state).unwrap());
    let finished = frame("combined/finished", &player);
    assert_ne!(initial.pixels, finished.pixels);
    player.advance(0.5).unwrap();
    assert_eq!(finished.pixels, frame("combined/settled", &player).pixels);
    assert!(!player.navigate("next", None).unwrap());
    let end = frame("combined/end-screen", &player);
    assert!(
        end.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 0, 255])
    );
    assert!(player.navigate("next", None).unwrap());
    player.navigate("previous", None).unwrap();
    assert_eq!(
        finished.pixels,
        frame("combined/back-from-end", &player).pixels
    );
}

fn morph_slide(model: &Presentation, new: bool) -> Slide {
    let id = if new { 20 } else { 10 };
    let offset = if new { 370. } else { 0. };
    let mut outline = shape(
        id,
        "!!Outline",
        180. + offset,
        72.,
        if new { [40, 125, 215] } else { [190, 43, 32] },
    );
    outline.geom = Geom::preset(if new { "star5" } else { "ellipse" });
    let mut label = shape(id + 1, "!!Text", 180. + offset, 220., [240, 240, 240]);
    label.text = Some(TextBody::from_text(if new {
        "red שלום red to"
    } else {
        "old red red שלום"
    }));
    if new {
        label.xfrm = Some(Xfrm {
            rot: 22.,
            ..Xfrm::new(550., 220., 205., 96.)
        });
    }
    let mut unique = shape(id + 2, "", 180. + offset, 345., [28, 182, 84]);
    unique.text = Some(TextBody::from_text(if new { "New words" } else { "Gone" }));
    Slide {
        id: SlideId(id),
        layout: model.slides[0].layout,
        shapes: vec![label, outline, unique],
        ..Default::default()
    }
}

#[test]
fn three_morph_modes_render_both_endpoints_and_in_between() {
    for option in ["objects", "words", "characters"] {
        let mut model = Presentation::default();
        let before = morph_slide(&model, false);
        let mut after = morph_slide(&model, true);
        after.transition = Some(Transition {
            kind: "morph".into(),
            option: option.into(),
            duration_ms: 2000,
            ..Default::default()
        });
        assert_eq!(
            joydeck_anim::morph_pairs(&before, &after),
            [(ShapeId(11), ShapeId(21)), (ShapeId(10), ShapeId(20))]
        );
        model.slides = vec![Arc::new(before), Arc::new(after)];
        if let Some(output) = std::env::var_os("JOYDECK_STAGE4_OUTPUT") {
            let path = Path::new(&output).join(format!("morph-{option}.pptx"));
            std::fs::write(path, joydeck_pptx::export(&model).unwrap()).unwrap();
        }
        let mut player = Player::new(Arc::new(model), None, None).unwrap();
        let old = frame(&format!("{option}/original"), &player);
        player.navigate("next", None).unwrap();
        let mut previous = 0.;
        for time in [0., 0.5, 0.75, 1., 1.75, 2.] {
            player.advance(time - previous).unwrap();
            let current = frame(&format!("{option}/{time}"), &player);
            if time == 1. {
                assert_ne!(current.pixels, old.pixels);
            }
            previous = time;
        }
        assert!(!player.animated());
        let end = frame(&format!("{option}/held"), &player);
        let destination = joydeck_render::render_slide(
            &player.model,
            1,
            &joydeck_render::RenderOpts {
                scale: 0.5,
                ..Default::default()
            },
        );
        assert_eq!(end.pixels, destination.pixels);
    }
}

#[test]
fn office_authored_timing_enters_and_leaves_the_main_step() {
    let default = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/joydeck/m10-powerpoint-authored-motion.pptx");
    let sample = std::env::var_os("JOYDECK_STAGE4_OFFICE").map_or(default, Into::into);
    let doc = Document::open(&sample, &Cancellation::default()).unwrap();
    let page = doc
        .presentation
        .slides
        .iter()
        .position(|s| !s.animations.is_empty())
        .unwrap();
    let model = Arc::new(doc.presentation);
    let mut player = Player::new(model.clone(), Some(page), None).unwrap();
    let tl = ShowState::timeline(&model, page);
    assert!(tl.steps() > 0);
    println!("PLAY office-timing {:?}", tl.effects());
    let old = frame("office/original", &player);
    for i in 0..tl.steps() {
        if !player.state.playing {
            player.navigate("next", None).unwrap();
        }
        let seconds = tl.step_duration(i);
        player.advance(seconds / 2.).unwrap();
        frame(&format!("office/step-{i}/middle"), &player);
        player.advance(seconds / 2. + 0.125).unwrap();
        frame(&format!("office/step-{i}/done"), &player);
    }
    assert_eq!(player.state.step, tl.steps());
    assert!(!player.animated());
    let end = frame("office/settled", &player);
    assert_ne!(old.pixels, end.pixels);
    player.advance(0.25).unwrap();
    assert_eq!(
        end.pixels,
        player.frame(0.5, &Cancellation::default()).unwrap().pixels
    );
}
