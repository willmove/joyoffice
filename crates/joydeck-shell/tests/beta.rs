extern crate joydeck_shell as joydeck;
use joydeck::{
    compare,
    edit::{self, Command, Session},
    preferences, video,
};
#[cfg(target_os = "macos")]
use std::sync::Arc;
use std::{
    path::PathBuf,
    sync::atomic::AtomicBool,
    time::{SystemTime, UNIX_EPOCH},
};
fn temp(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "joydeck-beta-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&p).unwrap();
    p
}
#[test]
fn preferences_migration_quarantine_future_and_atomic_validation() {
    let p = temp("preferences");
    std::fs::write(
        p.join("preferences.json"),
        br#"{"schema":1,"zoom":1.25,"notes":true}"#,
    )
    .unwrap();
    let loaded = preferences::load(&p).unwrap().0;
    assert_eq!(loaded.schema, 2);
    assert_eq!(loaded.zoom, 1.25);
    assert!(loaded.notes);
    preferences::update(&p, |v| v.inspector = true).unwrap();
    let bytes = std::fs::read(p.join("preferences.json")).unwrap();
    assert!(preferences::update(&p, |v| v.zoom = f32::NAN).is_err());
    assert_eq!(std::fs::read(p.join("preferences.json")).unwrap(), bytes);
    std::fs::write(p.join("preferences.json"), b"broken").unwrap();
    let (value, notice) = preferences::load(&p).unwrap();
    assert_eq!(value, Default::default());
    assert!(notice.is_some());
    let corrupt = std::fs::read_dir(&p)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("preferences-corrupt")
        })
        .unwrap();
    assert_eq!(std::fs::read(corrupt.path()).unwrap(), b"broken");
    let future = br#"{"schema":99,"future":"preserve"}"#;
    std::fs::write(p.join("preferences.json"), future).unwrap();
    assert!(preferences::load(&p).is_err());
    assert!(preferences::update(&p, |v| v.notes = false).is_err());
    assert_eq!(std::fs::read(p.join("preferences.json")).unwrap(), future);
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn preferences_concurrent_updates_keep_recent_and_layout() {
    let p = temp("concurrent");
    let mut workers = vec![];
    for i in 0..24 {
        let root = p.clone();
        workers.push(std::thread::spawn(move || {
            let file = root.join(format!("{i}.joydeck"));
            std::fs::write(&file, b"fixture").unwrap();
            preferences::remember(&root, &file).unwrap();
        }));
    }
    for w in workers {
        w.join().unwrap();
    }
    preferences::update(&p, |v| v.notes = true).unwrap();
    let value = preferences::load(&p).unwrap().0;
    assert_eq!(value.recent.len(), 12);
    assert_eq!(value.generation, 25);
    assert!(value.notes);
    assert_eq!(
        value
            .recent
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        12
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(p.join("preferences.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    assert!(
        !std::fs::read_dir(&p).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn offline_compare_accept_stale_guard_delete_and_undo() {
    let mut session = Session::new();
    session
        .execute(session.id, Command::AddObject(0, edit::object("text")))
        .unwrap();
    let original = session.deck.clone();
    let mut other = (*original).clone();
    other.slides[0].objects[0].text = "reviewed 中文😀".into();
    let report = compare::compare(&original, &other).unwrap();
    assert_eq!(report.differences.len(), 1);
    let command = compare::accept_object(&original, &other, &report, 0).unwrap();
    session.execute(session.id, command).unwrap();
    assert_eq!(session.deck.slides[0].objects[0].text, "reviewed 中文😀");
    assert!(compare::accept_object(&session.deck, &other, &report, 0).is_err());
    session.undo();
    assert_eq!(session.deck, original);
    session.redo();
    let mut deleted = (*session.deck).clone();
    deleted.slides[0].objects.clear();
    let report = compare::compare(&session.deck, &deleted).unwrap();
    let index = report
        .differences
        .iter()
        .position(|d| d.kind == "object.deleted")
        .unwrap();
    session
        .execute(
            session.id,
            compare::accept_object(&session.deck, &deleted, &report, index).unwrap(),
        )
        .unwrap();
    assert!(session.deck.slides[0].objects.is_empty());
    session.undo();
    assert_eq!(session.deck.slides[0].objects.len(), 1);
    // Ordering, speaker notes and hidden status must not disappear in a comparison.
    let mut other = (*session.deck).clone();
    other.slides[0].notes = "changed".into();
    other.slides[0].hidden = true;
    assert!(
        compare::compare(&session.deck, &other)
            .unwrap()
            .differences
            .iter()
            .any(|r| r.fields.contains(&"notes".into()) && r.fields.contains(&"hidden".into()))
    );
}
#[test]
fn video_plan_and_cancel_do_not_touch_existing_output() {
    let d = Session::new().deck;
    assert_eq!(
        video::plan(&d, &[0], &Default::default()).unwrap().frames,
        30
    );
    assert!(video::plan(&d, &[usize::MAX], &Default::default()).is_err());
    assert!(
        video::plan(
            &d,
            &[0],
            &video::Options {
                width: 1919,
                ..Default::default()
            }
        )
        .is_err()
    );
    let p = temp("cancel");
    let out = p.join("video.mp4");
    std::fs::write(&out, b"existing").unwrap();
    assert!(video::export(&out, &d, &[0], &Default::default(), &AtomicBool::new(true)).is_err());
    assert_eq!(std::fs::read(&out).unwrap(), b"existing");
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
#[cfg(target_os = "macos")]
fn mp4_actual_encode_probe_and_running_cancel() {
    let p = temp("encode");
    let mut d = (*Session::new().deck).clone();
    d.slides[0].background = "#f0a020".into();
    let options = video::Options {
        width: 320,
        fps: 10,
        seconds_per_page: 1,
    };
    let out = p.join("video.mp4");
    let plan = video::export(&out, &d, &[0], &options, &AtomicBool::new(false)).unwrap();
    assert_eq!(plan.frames, 10);
    let asset = joydeck::playback::import(&out).unwrap();
    assert_eq!(asset.info.width, 320);
    assert!((asset.info.duration - 1.).abs() < 0.05);
    assert_eq!(asset.info.profiles, ["H264-Main-8bit"]);
    let bytes = std::fs::read(&out).unwrap();
    assert!(video::export(&out, &d, &[0], &options, &AtomicBool::new(false)).is_err());
    assert_eq!(std::fs::read(&out).unwrap(), bytes);
    let cancelled = p.join("cancelled.mp4");
    let flag = Arc::new(AtomicBool::new(false));
    let stop = flag.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    assert!(
        video::export(
            &cancelled,
            &d,
            &[0],
            &video::Options {
                width: 1920,
                fps: 30,
                seconds_per_page: 30
            },
            &flag
        )
        .is_err()
    );
    worker.join().unwrap();
    assert!(!cancelled.exists());
    std::fs::remove_dir_all(p).unwrap();
}

#[test]
fn file_events_are_local_decoded_and_bounded() {
    let (url, expected, local_url, local_path) = if cfg!(target_os = "windows") {
        (
            "file:///C:/Temp/a%20b%F0%9F%98%80.joydeck",
            "C:/Temp/a b😀.joydeck",
            "file://localhost/C:/Temp/a+b.joydeck",
            "C:/Temp/a+b.joydeck",
        )
    } else {
        (
            "file:///tmp/a%20b%F0%9F%98%80.joydeck",
            "/tmp/a b😀.joydeck",
            "file://localhost/tmp/a+b.joydeck",
            "/tmp/a+b.joydeck",
        )
    };
    assert_eq!(
        preferences::local_file_url(url).unwrap(),
        PathBuf::from(expected)
    );
    assert_eq!(
        preferences::local_file_url(local_url).unwrap(),
        PathBuf::from(local_path)
    );
    for url in [
        "https://example.com/file",
        "file://remote/tmp/x",
        "file:///tmp/%GG",
        "file:///tmp/%00",
        "file://localhostevil/tmp/x",
    ] {
        assert!(preferences::local_file_url(url).is_err(), "{url}");
    }
}
#[test]
fn title_captions_match_video_timeline_and_keep_notes_private() {
    let mut d = (*Session::new().deck).clone();
    d.slides[0].title = "A < B & C\n新标题".into();
    d.slides[0].notes = "private notes".into();
    let vtt = video::captions(&d, &[0], &Default::default()).unwrap();
    assert!(vtt.starts_with("WEBVTT\n\n1\n00:00:00.000 --> 00:00:02.000"));
    assert!(vtt.contains("A &lt; B &amp; C 新标题"));
    assert!(!vtt.contains("private notes"));
}

#[test]
fn accepting_added_object_preserves_identity_through_recompare_and_history() {
    let mut s = Session::new();
    let original = s.deck.clone();
    let mut reference = (*original).clone();
    let mut o = edit::object("rect");
    o.id = "reference-stable-object".into();
    reference.slides[0].objects.push(o.clone());
    let report = compare::compare(&s.deck, &reference).unwrap();
    let index = report
        .differences
        .iter()
        .position(|r| r.kind == "object.added")
        .unwrap();
    s.execute(
        s.id,
        compare::accept_object(&s.deck, &reference, &report, index).unwrap(),
    )
    .unwrap();
    assert_eq!(s.deck.slides[0].objects[0].id, "reference-stable-object");
    assert!(
        compare::compare(&s.deck, &reference)
            .unwrap()
            .differences
            .is_empty()
    );
    let accepted = s.deck.clone();
    s.undo();
    assert_eq!(s.deck, original);
    s.redo();
    assert_eq!(s.deck, accepted);
    assert!(s.execute(s.id, Command::AdoptObject(0, o)).is_err());
    assert_eq!(s.deck, accepted);
}

#[test]
fn comparison_rejects_legacy_objects_without_identity() {
    let mut d = (*Session::new().deck).clone();
    let mut o = edit::object("rect");
    o.id.clear();
    d.slides[0].objects.push(o);
    assert!(d.validate().is_ok());
    assert!(compare::compare(&d, &d).unwrap_err().contains("稳定ID"));
}
