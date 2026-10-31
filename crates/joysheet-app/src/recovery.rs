//! Recovery copies serialize immutable snapshots on a dedicated bounded worker.
use joysheet_engine::model::Workbook;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Clone)]
pub struct RecoveryDocument {
    pub doc: u64,
    pub title: String,
    pub original: Option<String>,
    pub revision: u64,
    pub workbook: Arc<Workbook>,
    pub drafts: Vec<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RecoveryEntry {
    pub id: String,
    pub title: String,
    pub original: Option<String>,
    pub revision: u64,
    pub saved: u64,
    pub drafts: Vec<Value>,
}
#[derive(Serialize, Deserialize)]
struct Record {
    schema: u32,
    entry: RecoveryEntry,
    workbook: Workbook,
}
// Never retain a workbook Arc here: an idle recovery cache must not pin old large sheets.
#[derive(Clone, PartialEq)]
struct RecoveryKey {
    revision: u64,
    title: String,
    original: Option<String>,
    drafts: Vec<Value>,
}
impl From<&RecoveryDocument> for RecoveryKey {
    fn from(d: &RecoveryDocument) -> Self {
        Self { revision: d.revision, title: d.title.clone(), original: d.original.clone(), drafts: d.drafts.clone() }
    }
}
pub struct RecoveryService {
    profile: Arc<Mutex<serde_json::Value>>,
    directory: PathBuf,
    sender: mpsc::SyncSender<Vec<RecoveryDocument>>,
    session: String,
    closed: Arc<Mutex<std::collections::HashSet<u64>>>,
    error: Arc<Mutex<Option<String>>>,
}
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|t| t.as_millis() as u64).unwrap_or_default()
}
fn checked(directory: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return Err("Invalid recovery id".into());
    }
    Ok(directory.join(format!("{id}.recovery.json")))
}
impl RecoveryService {
    pub fn start(directory: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let session = format!("{}-{}", now(), std::process::id());
        let (sender, receiver) = mpsc::sync_channel::<Vec<RecoveryDocument>>(1);
        let dir = directory.clone();
        let prefix = session.clone();
        let closed = Arc::new(Mutex::new(std::collections::HashSet::new()));
        let worker_closed = closed.clone();
        let error = Arc::new(Mutex::new(None));
        let worker_error = error.clone();
        let profile = Arc::new(Mutex::new(serde_json::json!({"busy":false,"completed":0})));
        let worker_profile = profile.clone();
        std::thread::Builder::new()
            .name("joysheet-recovery".into())
            .spawn(move || {
                let mut prior = Vec::<PathBuf>::new();
                let mut saved = std::collections::HashMap::<u64, RecoveryKey>::new();
                while let Ok(documents) = receiver.recv() {
                    let mut current = Vec::new();
                    let active: std::collections::HashSet<_> = documents.iter().map(|d| d.doc).collect();
                    saved.retain(|doc, _| active.contains(doc));
                    for d in documents {
                        let doc = d.doc;
                        if worker_closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&doc) {
                            saved.remove(&doc);
                            continue;
                        }
                        let id = format!("{prefix}-{}", d.doc);
                        let Ok(path) = checked(&dir, &id) else { continue };
                        current.push(path.clone());
                        let key = RecoveryKey::from(&d);
                        if saved.get(&doc) == Some(&key) && path.is_file() {
                            let mut p = worker_profile.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            p["skipped"] = serde_json::json!(p["skipped"].as_u64().unwrap_or(0).saturating_add(1));
                            continue;
                        }
                        let began = Instant::now();
                        {
                            let mut p = worker_profile.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            p["busy"] = serde_json::json!(true);
                            p["doc"] = serde_json::json!(doc);
                            p["revision"] = serde_json::json!(d.revision);
                        }
                        let record = Record {
                            schema: 1,
                            entry: RecoveryEntry { id, title: d.title, original: d.original, revision: d.revision, saved: now(), drafts: d.drafts },
                            workbook: (*d.workbook).clone(),
                        };
                        let temporary = path.with_extension("tmp");
                        let mut serialize_ms=0.;let mut io_ms=0.;let mut byte_count=0;
                        let result = (|| -> Result<(), String> {
                            let t=Instant::now();
                            let bytes = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
                            serialize_ms=t.elapsed().as_secs_f64()*1000.;byte_count=bytes.len();
                            if bytes.len() > 256 * 1024 * 1024 {
                                return Err("Recovery copy exceeds 256 MiB; save the workbook explicitly".into());
                            }
                            let io=Instant::now();
                            std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
                            let closed = worker_closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            if closed.contains(&doc) {
                                let _ = std::fs::remove_file(&temporary);
                                return Ok(());
                            }
                            let r=std::fs::rename(&temporary, &path).map_err(|e| e.to_string());
                            io_ms=io.elapsed().as_secs_f64()*1000.;r
                        })();
                        {
                            let mut p = worker_profile.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                            let count = p["completed"].as_u64().unwrap_or(0).saturating_add(1);
                            let skipped = p["skipped"].as_u64().unwrap_or(0);
                            *p = serde_json::json!({"busy":false,"completed":count,"skipped":skipped,"doc":doc,"serializeMs":serialize_ms,"ioMs":io_ms,"bytes":byte_count,"totalMs":began.elapsed().as_secs_f64()*1000.,"success":result.is_ok()});
                        }
                        if let Err(e) = result {
                            let _ = std::fs::remove_file(&temporary);
                            eprintln!("Recovery copy failed: {e}");
                            *worker_error.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(e);
                        } else {
                            saved.insert(doc, key);
                        }
                    }
                    for path in prior.iter().filter(|p| !current.contains(p)) {
                        if let Err(e) = std::fs::remove_file(path)
                            && e.kind() != std::io::ErrorKind::NotFound
                        {
                            eprintln!("Recovery cleanup failed: {e}");
                        }
                    }
                    prior = current;
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { directory, sender, session, closed, error, profile })
    }
    /// Closing a document cancels an in-flight backup before removing its copy.
    pub fn forget(&self, doc: u64) -> Result<(), String> {
        let mut closed = self.closed.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        closed.insert(doc);
        let path = checked(&self.directory, &format!("{}-{doc}", self.session))?;
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn take_error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take()
    }
    pub fn profile(&self) -> serde_json::Value {
        self.profile.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }
    pub fn submit(&self, documents: Vec<RecoveryDocument>) -> Result<(), String> {
        match self.sender.try_send(documents) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn list(&self) -> Result<Vec<RecoveryEntry>, String> {
        list(&self.directory, Some(&self.session))
    }
    pub fn restore(&self, id: &str) -> Result<(RecoveryEntry, Vec<u8>), String> {
        let path = checked(&self.directory, id)?;
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 256 * 1024 * 1024 {
            return Err("Recovery copy exceeds 256 MiB".into());
        }
        let record: Record = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if record.schema != 1 {
            return Err("Unknown recovery format".into());
        }
        let bytes = serde_json::to_vec(&record.workbook).map_err(|e| e.to_string())?;
        Ok((record.entry, bytes))
    }
    pub fn discard(&self, id: &str) -> Result<(), String> {
        std::fs::remove_file(checked(&self.directory, id)?).map_err(|e| e.to_string())
    }
}
fn list(directory: &Path, exclude: Option<&str>) -> Result<Vec<RecoveryEntry>, String> {
    let mut entries = Vec::new();
    for file in std::fs::read_dir(directory).map_err(|e| e.to_string())?.take(128) {
        let file = file.map_err(|e| e.to_string())?;
        let path = file.path();
        let name = file.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".recovery.json")
            || exclude.is_some_and(|e| name.starts_with(e))
            || file.metadata().map_err(|e| e.to_string())?.len() > 256 * 1024 * 1024
        {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path)
            && let Ok(record) = serde_json::from_slice::<Record>(&bytes)
            && record.schema == 1
        {
            entries.push(record.entry);
        }
    }
    entries.sort_by_key(|r| std::cmp::Reverse(r.saved));
    Ok(entries)
}
#[cfg(test)]
mod tests {
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
}
