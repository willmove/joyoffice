extern crate joydeck_shell as joydeck;
use joydeck::{
    edit::{Command, Session},
    playback::{self, Clip, Player},
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/joydeck/media")
        .join(name)
}
#[test]
fn media_resources_and_params_undo_native_roundtrip() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let a = playback::import(&fixture("original-tone.wav")).unwrap();
    assert_eq!(a.info.codecs, vec!["lpcm"]);
    let mut s = Session::new();
    let mut p = s.deck.playback.clone();
    let id = a.id();
    p.assets.insert(id.clone(), Arc::new(a));
    p.clips.push(Clip {
        id: "audio-1".into(),
        slide: s.deck.slides[0].id.clone(),
        asset: id,
        start: 0.,
        end: 4.,
        volume: 0.01,
        looping: false,
        autoplay: false,
        cross_slide: true,
    });
    s.execute(s.id, Command::Playback(p)).unwrap();
    let baseline = s.deck.clone();
    let mut bad = s.deck.playback.clone();
    bad.clips[0].end = 9.;
    assert!(s.execute(s.id, Command::Playback(bad)).is_err());
    assert_eq!(baseline, s.deck);
    let path = std::env::temp_dir().join(format!(
        "joydeck-media-{}-{}-{}.joydeck",
        std::process::id(),
        s.id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    joydeck::edit::save_new(&path, &s.deck).unwrap();
    let opened = joydeck::edit::open_editable(&path).unwrap();
    assert_eq!(opened.playback, s.deck.playback);
    std::fs::remove_file(path).unwrap();
    s.undo();
    assert!(s.deck.playback.clips.is_empty());
    s.redo();
    assert_eq!(s.deck, baseline);
}
fn wait(p: &Player, predicate: impl Fn(&playback::Status) -> bool) {
    let start = Instant::now();
    loop {
        let status = p.status.lock().unwrap().clone();
        assert!(status.error.is_none(), "{:?}", status.error);
        if predicate(&status) {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(12),
            "adapter timeout ready={} seconds={} gen={} frame={}",
            status.ready,
            status.seconds,
            status.generation,
            status.sequence
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn h264_real_decode_pause_twenty_seeks_and_bounded_frame() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let asset = playback::import(&fixture("original-h264-aac.mp4")).unwrap();
    assert_eq!((asset.info.width, asset.info.height), (1920, 1080));
    let clip = Clip {
        id: "v".into(),
        slide: "page-1".into(),
        asset: asset.id(),
        start: 0.,
        end: 4.8,
        volume: 0.,
        looping: true,
        autoplay: true,
        cross_slide: false,
    };
    let mut p = Player::new(&asset, &clip).unwrap();
    wait(&p, |s| s.ready && s.sequence > 2 && s.seconds > 0.2);
    p.control(serde_json::json!({"command":"pause"})).unwrap();
    wait(&p, |s| !s.playing);
    let at = p.status.lock().unwrap().seconds;
    std::thread::sleep(Duration::from_millis(120));
    assert!((p.status.lock().unwrap().seconds - at).abs() < 0.06);
    for i in 0..20 {
        let generation = p.status.lock().unwrap().generation;
        let target = 0.2 + (i % 4) as f64;
        p.control(serde_json::json!({"command":"seek","seconds":target}))
            .unwrap();
        wait(&p, |s| {
            s.generation > generation && (s.seconds - target).abs() < 0.08 && s.frame.is_some()
        });
        let s = p.status.lock().unwrap();
        let f = s.frame.as_ref().unwrap();
        assert_eq!(f.bgra.len(), f.width as usize * f.height as usize * 4);
        assert!(f.width <= 960 && f.height <= 960);
    }
    p.control(serde_json::json!({"command":"seek","seconds":4.7}))
        .unwrap();
    wait(&p, |s| s.seconds > 4.6);
    let generation = p.status.lock().unwrap().generation;
    p.control(serde_json::json!({"command":"play"})).unwrap();
    wait(&p, |s| s.generation > generation && s.seconds < 1.);
}
#[test]
fn malformed_media_fails_without_document_changes() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let path = std::env::temp_dir().join(format!("joydeck-bad-{}.mp4", std::process::id()));
    std::fs::write(&path, b"not a media stream").unwrap();
    assert!(playback::import(&path).is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn mp3_probe_and_real_default_device_playback() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let asset = playback::import(&fixture("original-tone.mp3")).unwrap();
    assert_eq!(asset.info.codecs, vec![".mp3"]);
    let clip = Clip {
        id: "mp3".into(),
        slide: "page-1".into(),
        asset: asset.id(),
        start: 0.,
        end: 4.,
        volume: 0.01,
        looping: false,
        autoplay: true,
        cross_slide: true,
    };
    let p = Player::new(&asset, &clip).unwrap();
    wait(&p, |s| s.playing && s.seconds > 0.1);
}
