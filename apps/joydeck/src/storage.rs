//! File transactions and recoverable snapshots. All methods are background work.
use crate::{
    document::{Deck, MAX_FILE, Result},
    edit,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
/// Random names prevent concurrent allocations from colliding on coarse or adjusted wall clocks.
pub(crate) fn private_directory(label: &str) -> Result<PathBuf> {
    for _ in 0..8 {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let suffix = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let path =
            std::env::temp_dir().join(format!("joydeck-{label}-{}-{suffix}", std::process::id()));
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("无法分配独立临时目录".into())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub size: u64,
    pub sha256: String,
}
pub fn fingerprint(path: &Path) -> Result<Fingerprint> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    let mut f = f.take(MAX_FILE + 1);
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("文件超过预算".into());
    }
    Ok(Fingerprint {
        size: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    })
}
pub fn save(path: &Path, deck: &Deck, expected: Option<Fingerprint>) -> Result<Fingerprint> {
    if let Some(e) = &expected
        && fingerprint(path).as_ref() != Ok(e)
    {
        return Err("文件已被外部修改或删除，请重新打开或另存".into());
    }
    let temp = path.with_file_name(format!(
        ".prepare-{}-{}.joydeck",
        std::process::id(),
        stamp()
    ));
    edit::save_new(&temp, deck)?;
    let prepared = fingerprint(&temp)?;
    let result = (|| {
        if let Some(e) = expected {
            if fingerprint(path)? != e {
                return Err("提交前检测到外部修改，请另存".into());
            }
            std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        } else {
            std::fs::hard_link(&temp, path)
                .map_err(|e| format!("无法新建目标（可能已存在）：{e}"))?;
            let _ = std::fs::remove_file(&temp);
        }
        Ok(prepared)
    })();
    let _ = std::fs::remove_file(&temp);
    result
}
pub fn open(path: &Path) -> Result<(Deck, Fingerprint)> {
    let before = fingerprint(path)?;
    let deck = edit::open_editable(path)?;
    if fingerprint(path)? != before {
        return Err("读取期间文件发生变化".into());
    }
    Ok((deck, before))
}
fn stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Record {
    #[serde(default)]
    pub legacy: bool,
    pub session: String,
    pub revision: u64,
    pub source: Option<PathBuf>,
    pub source_fingerprint: Option<Fingerprint>,
    pub created: u128,
    pub snapshot: PathBuf,
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub record: Record,
    pub active: bool,
    pub problem: Option<String>,
}
struct State {
    lease: Option<File>,
    latest: u64,
    clean: Option<u64>,
    closed: bool,
}
#[derive(Clone)]
pub struct Recovery {
    pub root: PathBuf,
    pub session: String,
    state: Arc<Mutex<State>>,
}
impl Recovery {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            session: format!("{}-{}", std::process::id(), stamp()),
            state: Arc::new(Mutex::new(State {
                lease: None,
                latest: 0,
                clean: None,
                closed: false,
            })),
        }
    }
    pub fn write(
        &self,
        revision: u64,
        deck: &Deck,
        source: Option<PathBuf>,
        fp: Option<Fingerprint>,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| "恢复锁错误")?;
        if state.closed || revision < state.latest || state.clean.is_some_and(|r| revision <= r) {
            return Ok(());
        }
        let reserve = serde_json::to_vec(deck).map_err(|e| e.to_string())?.len() as u64 + 65536;
        self.prune((1024u64 * 1024 * 1024).saturating_sub(reserve))?;
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        if state.lease.is_none() {
            let f = OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .open(self.root.join(format!("{}.lock", self.session)))
                .map_err(|e| e.to_string())?;
            f.try_lock().map_err(|e| e.to_string())?;
            state.lease = Some(f);
        }
        let snapshot = self
            .root
            .join(format!("{}-{revision}.joydeck", self.session));
        edit::save(&snapshot, deck)?;
        let record = Record {
            legacy: false,
            session: self.session.clone(),
            revision,
            source,
            source_fingerprint: fp,
            created: stamp(),
            snapshot: snapshot.file_name().unwrap().into(),
        };
        let bytes = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
        let meta = snapshot.with_extension("json");
        let temp = meta.with_extension("pending");
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        f.write_all(&bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        std::fs::rename(&temp, &meta).map_err(|e| e.to_string())?;
        state.latest = revision;
        let mut mine = records(&self.root)?
            .into_iter()
            .filter(|r| r.session == self.session)
            .collect::<Vec<_>>();
        mine.sort_by_key(|r| r.created);
        while mine.len() > 3 {
            remove_record(&self.root, &mine.remove(0))?;
        }
        drop(state);
        self.prune(1024 * 1024 * 1024)?;
        Ok(())
    }
    pub fn clean(&self, revision: u64) -> Result<()> {
        let mut s = self.state.lock().map_err(|_| "恢复锁错误")?;
        s.clean = Some(s.clean.map_or(revision, |v| v.max(revision)));
        for r in records(&self.root)?
            .into_iter()
            .filter(|r| r.session == self.session && r.revision <= revision)
        {
            remove_record(&self.root, &r)?;
        }
        Ok(())
    }
    pub fn clean_restored(&self, r: &Record) -> Result<()> {
        valid_record(r)?;
        if r.legacy {
            return Ok(());
        }
        let own = if r.session == self.session {
            Some(self.state.lock().map_err(|_| "恢复锁错误")?)
        } else {
            None
        };
        let lock = if own.is_none() {
            Some(lease(&self.root, &r.session)?)
        } else {
            None
        };
        if let Some(f) = &lock {
            f.try_lock().map_err(|_| "源恢复会话仍被使用")?;
        }
        for candidate in records(&self.root)?
            .into_iter()
            .filter(|v| v.session == r.session && v.revision <= r.revision)
        {
            remove_record(&self.root, &candidate)?;
        }
        Ok(())
    }
    pub fn discard(&self) -> Result<()> {
        let mut s = self.state.lock().map_err(|_| "恢复锁错误")?;
        s.closed = true;
        for r in records(&self.root)?
            .into_iter()
            .filter(|r| r.session == self.session)
        {
            remove_record(&self.root, &r)?;
        }
        s.lease = None;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for r in records(&self.root)? {
            let lease = if r.legacy || r.session == self.session {
                None
            } else {
                Some(lease(&self.root, &r.session)?)
            };
            let active = lease.as_ref().is_some_and(|f| f.try_lock().is_err());
            let path = self.root.join(&r.snapshot);
            let problem = if active {
                None
            } else {
                match edit::open_editable(&path) {
                    Ok(_) => None,
                    Err(e) => {
                        if !r.legacy {
                            let _ = std::fs::rename(&path, path.with_extension("bad"));
                        }
                        Some(format!("快照损坏或版本不支持，已隔离：{e}"))
                    }
                }
            };
            entries.push(Entry {
                record: r,
                active,
                problem,
            });
        }
        if self.root.exists() {
            for e in std::fs::read_dir(&self.root)
                .map_err(|e| e.to_string())?
                .take(20000)
            {
                let p = e.map_err(|e| e.to_string())?.path();
                if p.extension().is_some_and(|v| v == "index-bad") {
                    entries.push(Entry {
                        record: Record {
                            legacy: false,
                            session: "0".into(),
                            revision: 0,
                            source: Some(p),
                            source_fingerprint: None,
                            created: 0,
                            snapshot: PathBuf::new(),
                        },
                        active: false,
                        problem: Some("恢复索引损坏，已隔离并保留原始文件".into()),
                    });
                }
            }
        }
        entries.sort_by_key(|e| std::cmp::Reverse(e.record.created));
        Ok(entries)
    }
    pub fn restore(&self, r: &Record) -> Result<Deck> {
        valid_record(r)?;
        if r.legacy {
            return open(&self.root.join(&r.snapshot)).map(|v| v.0);
        }
        let f = if r.session == self.session {
            None
        } else {
            Some(lease(&self.root, &r.session)?)
        };
        if let Some(f) = f.as_ref() {
            f.try_lock().map_err(|_| "快照仍被另一实例使用")?;
        }
        edit::open_editable(&self.root.join(&r.snapshot))
    }
    pub fn prune(&self, budget: u64) -> Result<()> {
        let mut all = records(&self.root)?;
        all.sort_by_key(|r| r.created);
        let mut total = if self.root.exists() {
            std::fs::read_dir(&self.root)
                .map_err(|e| e.to_string())?
                .filter_map(|e| e.ok())
                .map(|e| e.metadata().map_or(0, |m| m.len()))
                .sum::<u64>()
        } else {
            0
        };
        for r in all {
            if total <= budget {
                break;
            }
            if r.legacy || r.session == self.session {
                continue;
            }
            let f = lease(&self.root, &r.session)?;
            if f.try_lock().is_err() {
                continue;
            }
            let size = std::fs::metadata(self.root.join(&r.snapshot)).map_or(0, |m| m.len())
                + std::fs::metadata(self.root.join(&r.snapshot).with_extension("json"))
                    .map_or(0, |m| m.len());
            if remove_record(&self.root, &r).is_ok() {
                total = total.saturating_sub(size);
            }
        }
        if total > budget {
            return Err("恢复存储预算已满，活跃会话快照未删除".into());
        }
        Ok(())
    }
}
fn lease(root: &Path, session: &str) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(format!("{session}.lock")))
        .map_err(|e| e.to_string())
}
fn valid_record(r: &Record) -> Result<()> {
    let name = r
        .snapshot
        .file_name()
        .map(|v| v.to_string_lossy())
        .ok_or("恢复路径为空")?;
    let safe_name = if r.legacy {
        name.strip_prefix("session-").is_some_and(|v| {
            v.strip_suffix(".joydeck")
                .is_some_and(|v| v.bytes().all(|b| b.is_ascii_digit() || b == b'-'))
        })
    } else {
        name == format!("{}-{}.joydeck", r.session, r.revision)
    };
    if r.session.is_empty()
        || !r.session.bytes().all(|b| b.is_ascii_digit() || b == b'-')
        || r.snapshot.components().count() != 1
        || !safe_name
        || r.snapshot.extension().is_none_or(|e| e != "joydeck")
    {
        return Err("恢复索引路径无效".into());
    }
    Ok(())
}
fn records(root: &Path) -> Result<Vec<Record>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let mut count = 0;
    for e in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        count += 1;
        if count > 20000 {
            return Err("恢复目录项超限，需要清理".into());
        }
        let p = e.map_err(|e| e.to_string())?.path();
        if p.extension().is_some_and(|e| e == "joydeck")
            && p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("session-"))
            && !p.with_extension("json").exists()
        {
            let digest = Sha256::digest(p.file_name().unwrap().to_string_lossy().as_bytes());
            let id = u64::from_le_bytes(digest[..8].try_into().unwrap());
            let created = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            let r = Record {
                legacy: true,
                session: format!("0-{id}"),
                revision: 0,
                source: Some(p.clone()),
                source_fingerprint: None,
                created,
                snapshot: p.file_name().unwrap().into(),
            };
            if valid_record(&r).is_ok() {
                out.push(r);
            }
            continue;
        }
        if p.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
        if bytes.len() > 65536 {
            let _ = std::fs::rename(&p, p.with_extension("index-bad"));
            continue;
        }
        match serde_json::from_slice::<Record>(&bytes) {
            Ok(r) if valid_record(&r).is_ok() && p.file_stem() == r.snapshot.file_stem() => {
                out.push(r)
            }
            _ => {
                let _ = std::fs::rename(&p, p.with_extension("index-bad"));
            }
        }
    }
    Ok(out)
}
fn remove_record(root: &Path, r: &Record) -> Result<()> {
    if r.legacy {
        return Ok(());
    }
    valid_record(r)?;
    for path in [
        root.join(&r.snapshot),
        root.join(&r.snapshot).with_extension("json"),
        root.join(&r.snapshot).with_extension("bad"),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("无法清理恢复文件 {}：{e}", path.display())),
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{Command, Session};
    fn root() -> PathBuf {
        std::env::temp_dir().join(format!("joydeck-store-{}", stamp()))
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
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions).unwrap();
        s.execute(s.id, Command::AddPage).unwrap();
        assert!(save(&path, &s.deck, Some(fp)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
