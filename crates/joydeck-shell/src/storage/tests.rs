use super::*;
use crate::edit::{Command, Session};
fn root() -> PathBuf {
    private_directory("store-test").unwrap()
}
#[test]
fn concurrent_recovery_sessions_have_independent_leases_and_snapshots() {
    let root = root();
    let workers = (0..16)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || Recovery::new(root))
        })
        .collect::<Vec<_>>();
    let sessions = workers
        .into_iter()
        .map(|w| w.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        sessions
            .iter()
            .map(|r| &r.session)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        16
    );
    let d = Session::new().deck;
    for r in &sessions {
        r.write(1, &d, None, None).unwrap();
    }
    let reader = Recovery::new(root.clone());
    let entries = reader.list().unwrap();
    assert_eq!(entries.len(), 16);
    assert!(entries.iter().all(|r| r.active));
    assert!(reader.prune(0).is_err());
    drop(sessions);
    assert!(reader.list().unwrap().iter().all(|r| !r.active));
    reader.prune(0).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn concurrent_private_directories_are_unique_and_private() {
    let workers = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                (0..16)
                    .map(|_| private_directory("media-race").unwrap())
                    .collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let paths = workers
        .into_iter()
        .flat_map(|w| w.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        paths
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        128
    );
    for path in paths {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
        std::fs::remove_dir(path).unwrap();
    }
}
#[test]
fn conflict_and_recovery_restart_locks_quota() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("source.joydeck");
    let mut s = Session::new();
    let fp = save(&path, &s.deck, None).unwrap();
    std::fs::write(&path, b"external").unwrap();
    assert!(save(&path, &s.deck, Some(fp)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"external");
    let a = Recovery::new(root.clone());
    for rev in 1..=5 {
        s.execute(s.id, Command::AddPage).unwrap();
        a.write(rev, &s.deck, Some(path.clone()), None).unwrap();
    }
    let b = Recovery::new(root.clone());
    assert!(b.list().unwrap().iter().all(|r| r.active));
    assert!(b.prune(0).is_err());
    assert_eq!(b.list().unwrap().len(), 3);
    drop(a);
    let list = b.list().unwrap();
    assert_eq!(list.len(), 3);
    assert!(!list[0].active);
    assert_eq!(b.restore(&list[0].record).unwrap().slides.len(), 6);
    std::fs::remove_file(&path).unwrap();
    b.prune(0).unwrap();
    assert!(b.list().unwrap().is_empty());
    for e in std::fs::read_dir(&root).unwrap() {
        std::fs::remove_file(e.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn clean_revision_and_corruption() {
    let root = root();
    let r = Recovery::new(root.clone());
    let s = Session::new();
    r.write(1, &s.deck, None, None).unwrap();
    r.clean(1).unwrap();
    r.write(1, &s.deck, None, None).unwrap();
    assert!(r.list().unwrap().is_empty());
    r.write(2, &s.deck, None, None).unwrap();
    let entry = r.list().unwrap().remove(0);
    std::fs::write(root.join(&entry.record.snapshot), b"broken").unwrap();
    assert!(r.list().unwrap()[0].problem.is_some());
    r.discard().unwrap();
    drop(r);
    let fresh = Recovery::new(root.clone());
    fresh.write(1, &s.deck, None, None).unwrap();
    let row = fresh.list().unwrap().remove(0).record;
    let meta = root.join(&row.snapshot).with_extension("json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&meta).unwrap()).unwrap();
    json["revision"] = serde_json::json!(999);
    std::fs::write(meta, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(fresh.list().unwrap().iter().all(|e| e.problem.is_some()));
    assert!(root.join(row.snapshot).exists());
    fresh.discard().unwrap();
    drop(fresh);
    for e in std::fs::read_dir(&root).unwrap() {
        std::fs::remove_file(e.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn legacy_snapshot_is_visible_and_never_cleaned_automatically() {
    let root = std::env::temp_dir().join(format!("joydeck-legacy-{}", stamp()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("session-999-1.joydeck");
    edit::save(&path, &Session::new().deck).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let service = Recovery::new(root.clone());
    let list = service.list().unwrap();
    assert!(list[0].record.legacy);
    service.restore(&list[0].record).unwrap();
    service.clean_restored(&list[0].record).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(service.prune(0).is_err());
    drop(service);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[cfg(windows)]
#[test]
fn readonly_replace_failure_preserves_original() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("protected.joydeck");
    let mut s = Session::new();
    let fp = save(&path, &s.deck, None).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    let original_permissions = permissions.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    s.execute(s.id, Command::AddPage).unwrap();
    assert!(save(&path, &s.deck, Some(fp)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let permissions = original_permissions;
    std::fs::set_permissions(&path, permissions).unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
