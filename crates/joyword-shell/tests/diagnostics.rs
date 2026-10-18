//! T211: launch rotation, file cap, redaction, and the test profile.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use joyword_shell::command::{ArgValue, FILE_OPEN};
use joyword_shell::diag::{AppLog, Event, Level, PanicReport, Profile, ProfileError, resolve_from};
use joyword_shell::host::Office;
use joyword_shell::l10n::Locale;
use joyword_shell::prefs::PrefsStore;

const BODY: &str = "甲乙双方确认本合同正文不得写入诊断日志";
const REVISION: &str = "删除原价款为人民币壹万元整";
const COMMENT: &str = "批注请核对第三条保密义务";
const SECRET: &str = "super-secret-value";
const PROSE: &str = "The parties agree this contract body must stay out of logs";

fn temp(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "joyword-diag-{name}-{}-{nanos}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn logs(dir: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("joyword-") && name.ends_with(".log") {
            found.push((name, fs::read_to_string(entry.path()).unwrap()));
        }
    }
    found.sort();
    found
}

fn assert_clean(text: &str) {
    assert!(!text.contains(BODY), "{text}");
    assert!(!text.contains(REVISION), "{text}");
    assert!(!text.contains(COMMENT), "{text}");
    assert!(!text.contains(SECRET), "{text}");
    assert!(!text.contains(PROSE), "{text}");
    assert!(!text.contains("合同正文引号样本"), "{text}");
}

#[test]
fn sample_body_text_never_appears_in_the_log() {
    let dir = temp("body");
    let log = Arc::new(AppLog::open(dir.join("logs")).unwrap());
    let payload =
        format!("{BODY} {REVISION} {COMMENT} password={SECRET} {PROSE} '合同正文引号样本'");
    log.record_untrusted(Level::Error, "ui.input", &payload)
        .unwrap();
    log.record_panic_report(PanicReport {
        file: Some("src/editor.rs"),
        line: 40,
        column: 8,
        message: Some(&payload),
        backtrace: "0: joyword_shell::diag::record_panic_report\n             at src/editor.rs:40:8\n",
    })
    .unwrap();
    log.withhold("SAFEBODYTOKEN12").unwrap();
    log.record_untrusted(Level::Error, "ui.input", "code SAFEBODYTOKEN12")
        .unwrap();

    let mut office = Office::new();
    office.set_diagnostics(Arc::clone(&log));
    let missing = dir.join(format!("{BODY}.docx"));
    let opened = office.invoke(FILE_OPEN, &[("path", ArgValue::Path(missing))]);
    assert!(opened.is_err());

    let text = fs::read_to_string(log.path()).unwrap();
    assert_clean(&text);
    assert!(!text.contains("SAFEBODYTOKEN12"), "{text}");
    assert!(text.contains("event=ui.input"), "{text}");
    assert!(text.contains("event=panic"), "{text}");
    assert!(text.contains("event=file.open"), "{text}");
    assert!(text.contains("event=panic.frame"), "{text}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn launches_rotate_and_a_crash_keeps_the_previous_log() {
    let dir = temp("rotate");
    let first = AppLog::open(&dir).unwrap();
    first
        .record(Event {
            level: Level::Info,
            event: "launch.marker",
            kind: Some("first"),
            count: None,
        })
        .unwrap();
    let first_path = first.path();
    let first_bytes = fs::read(&first_path).unwrap();
    drop(first);

    let second = AppLog::open(&dir).unwrap();
    assert_ne!(second.path(), first_path);
    assert_eq!(fs::read(&first_path).unwrap(), first_bytes);
    second
        .record_panic_report(PanicReport {
            file: Some("src/diag.rs"),
            line: 12,
            column: 4,
            message: Some(&format!("{BODY} password={SECRET}")),
            backtrace: "0: joyword_shell::host::Office::dispatch\n             at src/diag.rs:12:4\n",
        })
        .unwrap();
    assert_eq!(fs::read(&first_path).unwrap(), first_bytes);
    let second_text = fs::read_to_string(second.path()).unwrap();
    assert!(second_text.contains("event=panic"));
    assert!(second_text.contains("count=12") || second_text.contains("line"));
    assert_clean(&second_text);
    drop(second);

    let _third = AppLog::open(&dir).unwrap();
    assert_eq!(fs::read(&first_path).unwrap(), first_bytes);
    assert!(logs(&dir).len() >= 3);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn retention_drops_only_older_launches_after_the_new_file_exists() {
    let dir = temp("retain");
    let mut previous = None;
    for index in 0..6 {
        let log = AppLog::open(&dir).unwrap();
        log.record(Event {
            level: Level::Info,
            event: "launch.marker",
            kind: Some("index"),
            count: Some(index),
        })
        .unwrap();
        previous = Some(log.path());
    }
    let names = logs(&dir);
    assert_eq!(names.len(), 5);
    let previous = previous.unwrap();
    assert!(previous.exists());
    assert!(names.iter().any(|(name, _)| previous.ends_with(name)));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_log_file_stops_at_its_cap() {
    let dir = temp("cap");
    let log = AppLog::open_capped(&dir, 512).unwrap();
    let mut filled = false;
    for index in 0..80 {
        if log
            .record(Event {
                level: Level::Info,
                event: "cap.probe",
                kind: Some("n"),
                count: Some(index),
            })
            .is_err()
        {
            filled = true;
            break;
        }
    }
    assert!(filled);
    let path = log.path();
    let capped = fs::metadata(&path).unwrap().len();
    assert!(capped <= 512, "{capped}");
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("event=log.opened") || text.contains("event=cap.probe"));
    let _ = log.record(Event {
        level: Level::Info,
        event: "cap.probe",
        kind: Some("n"),
        count: Some(99),
    });
    assert_eq!(fs::metadata(&path).unwrap().len(), capped);
    let refused = AppLog::open_capped(dir.join("tiny"), 8).unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput);
    assert!(logs(&dir.join("tiny")).is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn redaction_removes_credentials_quotes_and_prose() {
    let redacted = joyword_shell::diag::redact(&format!(
        "password={SECRET} Authorization: Bearer {SECRET} '{COMMENT}' \"{BODY}\" {PROSE} https://user:{SECRET}@example.com/a"
    ));
    assert_clean(&redacted);
    assert!(redacted.contains("<redacted>"));
    let err = {
        let dir = temp("event");
        let log = AppLog::open(&dir).unwrap();
        let before = fs::read(log.path()).unwrap();
        let err = log
            .record_untrusted(Level::Error, BODY, "kept")
            .unwrap_err();
        assert_eq!(fs::read(log.path()).unwrap(), before);
        let _ = fs::remove_dir_all(dir);
        err
    };
    assert!(!err.to_string().contains(BODY));
}

#[test]
fn the_test_profile_isolates_preferences_and_logs() {
    let user = temp("user-profile");
    let isolated = temp("test-profile");
    let paths = resolve_from(Some("test"), Some(&isolated), Some(&user)).unwrap();
    assert_eq!(paths.profile, Profile::Test);
    assert_eq!(paths.config_dir, isolated);
    assert_eq!(paths.log_dir, isolated.join("logs"));
    assert_ne!(paths.config_dir, user);

    let user_paths = resolve_from(None, None, Some(&user)).unwrap();
    assert_eq!(user_paths.profile, Profile::User);
    assert_eq!(user_paths.config_dir, user);
    assert_eq!(user_paths.log_dir, user.join("logs"));
    assert_eq!(
        resolve_from(Some("user"), None, Some(&user))
            .unwrap()
            .config_dir,
        user
    );
    assert_eq!(
        resolve_from(Some("debug"), None, Some(&user)).unwrap_err(),
        ProfileError::Unknown
    );
    assert_eq!(
        resolve_from(Some("test"), Some(&user), Some(&user)).unwrap_err(),
        ProfileError::TestProfileNotIsolated
    );

    let store = PrefsStore::new(&paths.config_dir);
    let mut loaded = store.load(Locale::ZhCn);
    loaded.prefs.author_name = "测试作者".into();
    store.save(&loaded).unwrap();
    assert!(paths.config_dir.join("preferences.conf").is_file());
    assert!(!user.join("preferences.conf").exists());
    let log = AppLog::open(&paths.log_dir).unwrap();
    assert!(log.path().starts_with(&paths.log_dir));
    assert!(!log.path().starts_with(&user));
    let _ = fs::remove_dir_all(&user);
    let _ = fs::remove_dir_all(&isolated);
}

#[test]
fn the_panic_hook_records_the_location_and_redacts_the_payload() {
    let dir = temp("hook");
    let log = Arc::new(AppLog::open(&dir).unwrap());
    joyword_shell::diag::install_panic_hook(Arc::clone(&log)).unwrap();
    let caught = std::panic::catch_unwind(|| {
        panic!("编辑失败 {BODY} password={SECRET} {PROSE}");
    });
    assert!(caught.is_err());
    let text = fs::read_to_string(log.path()).unwrap();
    assert_clean(&text);
    assert!(text.contains("event=panic"), "{text}");
    assert!(text.contains("event=panic.location"), "{text}");
    assert!(text.contains("diagnostics.rs"), "{text}");
    let _ = fs::remove_dir_all(dir);
}
