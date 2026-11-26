//! Integration tests for Phase C:
//! 1. Design tab built-in theme & variant application + PPTX preserving round-trip
//! 2. Expanded shape preset catalog (insert, switch, open-shape stroke, PPTX round-trip)
//! 3. Presenter View slide preview rendering
//! 4. Linux FFmpeg embedded video/audio inspection, playback, seek & slideshow compositing
#![allow(clippy::unwrap_used, clippy::expect_used)]

use joydeck_engine::{Command, ContentAction, Origin, Session};
use joydeck_model::{Fill, Geom, Presentation, SchemeSlot, ShapeId, SlideId};
use joydeck_shell::{
    Cancellation,
    playback::{Player as MediaPlayer, inspect},
    show::Player as ShowPlayer,
    show_media::{Control, Media},
};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn origins_for(p: &Presentation) -> BTreeMap<SlideId, Origin> {
    p.slides
        .iter()
        .map(|s| {
            let shapes = s.shapes.iter().map(|sh| (sh.id, sh.id)).collect();
            (
                s.id,
                Origin {
                    slide: s.id,
                    shapes,
                },
            )
        })
        .collect()
}

fn slide_bindings(p: &Presentation) -> Vec<joydeck_pptx::SlideBinding> {
    p.slides
        .iter()
        .map(|s| {
            let shapes: BTreeMap<ShapeId, ShapeId> =
                s.shapes.iter().map(|sh| (sh.id, sh.id)).collect();
            joydeck_pptx::SlideBinding {
                slide: s.id,
                source: s.id,
                shapes,
            }
        })
        .collect()
}

#[test]
fn apply_theme_and_variant_preserve_round_trip() {
    let fixture = repo_root().join("tests/fixtures/joydeck/r4-r5/independent-complex.pptx");
    let original_bytes = std::fs::read(&fixture).unwrap();
    let imported = joydeck_pptx::import(&original_bytes).unwrap();
    let mut session = Session::new(imported.clone(), origins_for(&imported), true);

    let orig_name = session.model().masters[0].theme.name.clone();
    assert_eq!(joydeck_model::theme::builtin_themes().len(), 10);
    session
        .execute(Command::Content {
            action: ContentAction::ApplyTheme {
                name: "深空科技".into(),
            },
        })
        .unwrap();
    assert_eq!(session.model().masters[0].theme.name, "深空科技");
    assert_eq!(
        session.model().masters[0].theme.colors.get(SchemeSlot::Lt1),
        joydeck_model::Rgba::rgb(0x0B, 0x11, 0x20)
    );
    assert_eq!(
        session.model().masters[0].theme.colors.get(SchemeSlot::Dk1),
        joydeck_model::Rgba::rgb(0xF8, 0xFA, 0xFC)
    );

    let bindings = slide_bindings(&imported);
    let saved =
        joydeck_pptx::export_preserving(&original_bytes, &imported, session.model(), &bindings)
            .unwrap();
    let reloaded = joydeck_pptx::import(&saved).unwrap();
    assert_eq!(reloaded.masters[0].theme.name, "深空科技");
    assert_eq!(
        reloaded.masters[0].theme.colors.get(SchemeSlot::Lt1),
        joydeck_model::Rgba::rgb(0x0B, 0x11, 0x20)
    );
    assert_eq!(
        reloaded.masters[0].theme.colors.get(SchemeSlot::Accent1),
        joydeck_model::Rgba::rgb(0x38, 0xBD, 0xF8)
    );

    // Apply dark/swapped variant (index 3 swaps Lt1 <-> Dk1)
    session
        .execute(Command::Content {
            action: ContentAction::ApplyThemeVariant { index: 3 },
        })
        .unwrap();
    assert_eq!(
        session.model().masters[0].theme.colors.get(SchemeSlot::Lt1),
        joydeck_model::Rgba::rgb(0xF8, 0xFA, 0xFC)
    );
    let saved_v =
        joydeck_pptx::export_preserving(&original_bytes, &imported, session.model(), &bindings)
            .unwrap();
    let reloaded_v = joydeck_pptx::import(&saved_v).unwrap();
    assert_eq!(
        reloaded_v.masters[0].theme.colors.get(SchemeSlot::Lt1),
        joydeck_model::Rgba::rgb(0xF8, 0xFA, 0xFC)
    );

    // Undo restores previous theme and original theme cleanly
    session.execute(Command::Undo).unwrap();
    assert_eq!(
        session.model().masters[0].theme.colors.get(SchemeSlot::Lt1),
        joydeck_model::Rgba::rgb(0x0B, 0x11, 0x20)
    );
    session.execute(Command::Undo).unwrap();
    assert_eq!(session.model().masters[0].theme.name, orig_name);

    // Invalid theme / variant index rejected
    assert!(
        session
            .execute(Command::Content {
                action: ContentAction::ApplyTheme {
                    name: "NoSuchTheme".into(),
                },
            })
            .is_err()
    );
    assert!(
        session
            .execute(Command::Content {
                action: ContentAction::ApplyThemeVariant { index: 99 },
            })
            .is_err()
    );
}

#[test]
fn expanded_shape_presets_insert_switch_and_round_trip() {
    let fixture = repo_root().join("tests/fixtures/joydeck/r4-r5/independent-complex.pptx");
    let original_bytes = std::fs::read(&fixture).unwrap();
    let imported = joydeck_pptx::import(&original_bytes).unwrap();
    let mut session = Session::new(imported.clone(), origins_for(&imported), true);

    let presets = [
        ("star5", false),
        ("rightArrow", false),
        ("flowChartDecision", false),
        ("wedgeRoundRectCallout", false),
        ("heart", false),
        ("line", true),
        ("curvedConnector3", true),
    ];

    for (preset, expect_open) in presets {
        session
            .execute(Command::InsertShape {
                preset: preset.into(),
                x: 80.0,
                y: 90.0,
                width: 180.0,
                height: 110.0,
            })
            .unwrap();
        let inserted = session.model().slides[0].shapes.last().unwrap();
        assert_eq!(inserted.geom.preset_name(), Some(preset));
        if expect_open {
            assert_eq!(inserted.fill, Some(Fill::None));
            assert!(inserted.line.is_some());
        } else {
            assert!(matches!(inserted.fill, Some(Fill::Solid { .. })));
        }
    }

    // Switch an existing closed shape's preset via ShapePreset
    let star_id = session.model().slides[0]
        .shapes
        .iter()
        .find(|s| s.geom.preset_name() == Some("star5"))
        .unwrap()
        .id;
    session
        .execute(Command::Select {
            page: 1,
            shapes: vec![star_id],
        })
        .unwrap();
    session
        .execute(Command::Content {
            action: ContentAction::ShapePreset {
                name: "cloudCallout".into(),
            },
        })
        .unwrap();
    let updated = session.model().slides[0]
        .shapes
        .iter()
        .find(|s| s.id == star_id)
        .unwrap();
    assert_eq!(updated.geom, Geom::preset("cloudCallout"));

    // Invalid presets rejected
    assert!(
        session
            .execute(Command::InsertShape {
                preset: "not-a-preset".into(),
                x: 10.0,
                y: 10.0,
                width: 100.0,
                height: 100.0,
            })
            .is_err()
    );
    assert!(
        session
            .execute(Command::InsertShape {
                preset: "textBox".into(),
                x: 10.0,
                y: 10.0,
                width: 100.0,
                height: 100.0,
            })
            .is_err()
    );

    // Preserving PPTX save & reload retains all new preset names
    let bindings = slide_bindings(&imported);
    let saved =
        joydeck_pptx::export_preserving(&original_bytes, &imported, session.model(), &bindings)
            .unwrap();
    let reloaded = joydeck_pptx::import(&saved).unwrap();
    let names: Vec<_> = reloaded.slides[0]
        .shapes
        .iter()
        .filter_map(|s| s.geom.preset_name())
        .collect();
    for expected in [
        "cloudCallout",
        "rightArrow",
        "flowChartDecision",
        "wedgeRoundRectCallout",
        "heart",
        "line",
        "curvedConnector3",
    ] {
        assert!(
            names.contains(&expected),
            "expected preset {expected} in reloaded shapes {names:?}"
        );
    }
}

#[test]
fn presenter_preview_slide_renders_valid_raster() {
    let deck_path = repo_root().join("tests/fixtures/joydeck/r6-r7/delivery-media.pptx");
    let bytes = std::fs::read(&deck_path).unwrap();
    let model = Arc::new(joydeck_pptx::import(&bytes).unwrap());
    let show = ShowPlayer::new(model, Some(0), None).unwrap();
    let cancel = Cancellation::default();

    let img0 = show.preview_slide(0, 0.5, &cancel).unwrap();
    assert!(img0.width > 100 && img0.height > 50);
    assert_eq!(img0.pixels.len(), (img0.width * img0.height * 4) as usize);

    let img1 = show.preview_slide(1, 0.5, &cancel).unwrap();
    assert!(img1.width > 100 && img1.height > 50);
    assert_eq!(img1.pixels.len(), (img1.width * img1.height * 4) as usize);

    assert!(show.preview_slide(999, 0.5, &cancel).is_err());
}

#[test]
fn embedded_video_inspect_play_seek_and_composite() {
    let mp4_path = repo_root().join("tests/fixtures/joydeck/media/original-h264-aac.mp4");
    let info = match inspect(&mp4_path) {
        Ok(v) => v,
        Err(e) if e.contains("requires ffmpeg") || e.contains("requires ffprobe") => {
            // Skip cleanly on environments without ffmpeg/ffprobe installed.
            return;
        }
        Err(e) => panic!("unexpected inspect error: {e}"),
    };
    assert_eq!(info["ready"], true);
    assert_eq!(info["video"], true);
    assert!(info["duration"].as_f64().unwrap_or(0.0) > 0.1);

    let bytes = std::fs::read(&mp4_path).unwrap();
    let mut player = MediaPlayer::new(&bytes, "mp4").unwrap();
    let st = player
        .control(json!({"command": "volume", "value": 0.0}))
        .unwrap();
    assert_eq!(st["paused"], true);

    let st = player.control(json!({"command": "play"})).unwrap();
    assert_eq!(st["paused"], false);

    // Wait briefly for the first decoded BGRA frame.
    let mut frame = None;
    for _ in 0..30 {
        if let Some(f) = player.frame().unwrap() {
            frame = Some(f);
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let frame = frame.expect("expected at least one decoded video frame");
    assert!(frame.width > 0 && frame.width <= 640);
    assert!(frame.height > 0 && frame.height <= 360);
    assert_eq!(frame.bgra.len(), (frame.width * frame.height * 4) as usize);
    assert!(frame.bgra.iter().any(|&b| b != 0));

    let st_seek = player
        .control(json!({"command": "seek", "seconds": 0.5}))
        .unwrap();
    assert!(st_seek["seconds"].as_f64().unwrap_or(0.0) >= 0.45);

    let st_pause = player.control(json!({"command": "pause"})).unwrap();
    assert_eq!(st_pause["paused"], true);
    drop(player);

    // Also test audio-only MP3 fixture
    let mp3_path = repo_root().join("tests/fixtures/joydeck/media/original-tone.mp3");
    let mp3_info = inspect(&mp3_path).unwrap();
    assert_eq!(mp3_info["ready"], true);
    assert_eq!(mp3_info["audio"], true);
    assert_eq!(mp3_info["video"], false);

    // End-to-end slideshow Media::composite on delivery-media.pptx slide 0
    let deck_path = repo_root().join("tests/fixtures/joydeck/r6-r7/delivery-media.pptx");
    let deck_bytes = std::fs::read(&deck_path).unwrap();
    let model = Arc::new(joydeck_pptx::import(&deck_bytes).unwrap());
    let show = ShowPlayer::new(model, Some(0), None).unwrap();
    let cancel = Cancellation::default();
    let mut raster = show.frame(0.5, &cancel).unwrap();
    let control = Control {
        enabled: Some(true),
        muted: true,
        seek: None,
    };
    let mut media = Media::default();
    let mut status = json!(null);
    for _ in 0..30 {
        status = media.composite(&show, &control, &mut raster).unwrap();
        if status["frame_seconds"].is_f64() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(status["ready"], true);
    assert!(
        status["frame_seconds"].is_f64(),
        "expected composited video frame_seconds in {status}"
    );
}
