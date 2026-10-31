use super::*;
fn wait_profile(service: &RecoveryService, predicate: impl Fn(&Value) -> bool) {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if predicate(&service.profile()) {
            return;
        }
        assert!(Instant::now() < deadline, "recovery worker did not finish");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
#[test]
fn unchanged_backups_skip_but_draft_changes_and_failed_writes_retry() {
    let dir = std::env::temp_dir().join(format!("joysheet-recovery-dedup-{}-{}", now(), std::process::id()));
    let service = RecoveryService::start(dir.clone()).unwrap();
    let mut s = joysheet_engine::Session::new();
    s.execute("file.new", serde_json::json!({})).unwrap();
    let mut d = RecoveryDocument {
        doc: 1,
        title: "Draft".into(),
        original: None,
        revision: 1,
        workbook: s.active().unwrap().wb.clone(),
        drafts: vec![serde_json::json!({"text":"中文"})],
    };
    let path = checked(&dir, &format!("{}-1", service.session)).unwrap();
    std::fs::create_dir(&path).unwrap(); // Renaming the copy must fail, then retry the same key.
    service.submit(vec![d.clone()]).unwrap();
    wait_profile(&service, |p| p["completed"] == 1);
    assert_eq!(service.profile()["success"], false);
    std::fs::remove_dir(&path).unwrap();
    service.submit(vec![d.clone()]).unwrap();
    wait_profile(&service, |p| p["completed"] == 2);
    assert_eq!(service.profile()["success"], true);
    let bytes = std::fs::read(&path).unwrap();
    service.submit(vec![d.clone()]).unwrap();
    wait_profile(&service, |p| p["skipped"] == 1);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    d.drafts[0]["text"] = serde_json::json!("尚未提交");
    service.submit(vec![d.clone()]).unwrap();
    wait_profile(&service, |p| p["completed"] == 3);
    let record: Record = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(record.entry.drafts[0]["text"], "尚未提交");
    service.submit(Vec::new()).unwrap();
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while path.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    service.submit(vec![d]).unwrap();
    wait_profile(&service, |p| p["completed"] == 4);
    assert!(path.is_file());
    service.forget(1).unwrap();
    drop(service);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn forgotten_document_cannot_reappear_from_queued_backup() {
    let dir = std::env::temp_dir().join(format!("joysheet-recovery-forget-{}-{}", now(), std::process::id()));
    let service = RecoveryService::start(dir.clone()).unwrap();
    let mut s = joysheet_engine::Session::new();
    s.execute("file.new", serde_json::json!({})).unwrap();
    let workbook = s.active().unwrap().wb.clone();
    let first = RecoveryDocument { doc: 1, title: "closed".into(), original: None, revision: 1, workbook, drafts: Vec::new() };
    service.forget(1).unwrap();
    let second = RecoveryDocument { doc: 2, title: "barrier".into(), ..first.clone() };
    service.submit(vec![first, second]).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let entries = list(&dir, None).unwrap();
        if entries.iter().any(|e| e.title == "barrier") {
            assert!(!entries.iter().any(|e| e.title == "closed"));
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    service.forget(2).unwrap();
    drop(service);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn recovery_preserves_formula_and_draft_without_changing_saved_workbook() {
    let dir = std::env::temp_dir().join(format!("joysheet-recovery-test-{}-{}", now(), std::process::id()));
    let service = RecoveryService::start(dir.clone()).unwrap();
    let mut s = joysheet_engine::Session::new();
    s.execute("file.new", serde_json::json!({})).unwrap();
    s.execute("cell.set", serde_json::json!({"cell":"A1","input":"=2+3"})).unwrap();
    let d = s.active().unwrap();
    let workbook = d.wb.clone();
    service
        .submit(vec![RecoveryDocument {
            doc: 1,
            title: "Report".into(),
            original: Some("original.xlsx".into()),
            revision: d.revision,
            workbook: workbook.clone(),
            drafts: vec![serde_json::json!({"cell":"B2","text":"未提交草稿"})],
        }])
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let entry = loop {
        if let Some(entry) = list(&dir, None).unwrap().first() {
            break entry.clone();
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let (meta, bytes) = service.restore(&entry.id).unwrap();
    assert_eq!(meta.drafts[0]["text"], "未提交草稿");
    let (restored, _) = joysheet_engine::io::open_bytes_with("Recovered.json", &bytes, None, false).unwrap();
    assert_eq!(restored.sheets[0].value(joysheet_engine::core::CellRef::new(0, 0)).display(), "5");
    assert_eq!(workbook.sheets[0].cells.len(), 1);
    assert!(service.restore("../outside").is_err());
    service.discard(&entry.id).unwrap();
    assert!(service.restore(&entry.id).is_err());
    drop(service);
    let _ = std::fs::remove_dir_all(dir);
}
