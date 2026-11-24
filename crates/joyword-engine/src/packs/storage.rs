//! First-party language-pack store compatible with existing user asset directories:
//!
//! ```text
//! <store>/settings.json               consent flag (`network`), preserving unknown keys
//! <store>/indexes/language-packs.json cached release index
//! <store>/language-packs.json         installed language packs inventory
//! <store>/language-packs/<locale>/    unpacked pack.toml, spelling.{aff,dic}, hyphenation.pat, LICENCES/
//! <store>/.staging/                   temporary unpack dirs
//! <store>/.lock                       interprocess writer lock
//! <store>/.language-pack-change/      recoverable publication journal
//! ```
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::Cancel;
use super::contents::{self, unpack};
use super::http::{Client, FetchError};
use super::transaction;
use super::transfer;

pub(super) const LANGUAGE_PACKS: &str = "language-packs";
pub(super) const LANGUAGE_PACK_INDEX_ENV: &str = "CRAFT_LANGUAGE_PACK_INDEX";
pub(super) const MAX_PACK_INDEX_BYTES: u64 = 4 << 20;
pub(super) const MAX_PACK_ARCHIVE_BYTES: u64 = 128 << 20;
pub(super) const MAX_PACK_FILE_BYTES: u64 = 64 << 20;
pub(super) const MAX_PACK_UNPACKED_BYTES: u64 = 256 << 20;
pub(super) const MAX_PACK_ENTRIES: usize = 256;
pub(super) const MAX_PACKS: usize = 1024;

const MAX_PACK_TOML_BYTES: u64 = 1 << 20;
const MAX_SETTINGS_BYTES: u64 = 1 << 20;
const SETTINGS: &str = "settings.json";
const INDEX_CACHE: &str = "indexes";
const INDEX_CACHE_NAME: &str = "language-packs.json";
const JOYWORD_INDEX_CACHE: &str = "indexes/joyword-language-packs.json";
const JOYWORD_INDEX: &[u8] = include_bytes!("../../data/language-pack-index.json");
const LEGACY_INDEX_SHA256: &str =
    "b35c7aef0591177f18233b551da16035dcd615549ff874e169ffd19f16fea74b";
const INSTALLED: &str = "language-packs.json";
const STAGING: &str = ".staging";
const LOCK: &str = ".lock";
const STALE: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Network {
    #[default]
    Off,
    On,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Caller {
    User,
    Agent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Progress {
    pub done: u64,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum VerifyError {
    #[error("size mismatch: expected {expected} bytes, got {actual}")]
    Size { expected: u64, actual: u64 },
    #[error("sha256 mismatch")]
    Sha256,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum IndexError {
    #[error("the index is larger than {max} bytes")]
    TooLarge { max: usize },
    #[error("{0}")]
    Json(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("downloading fonts is turned off")]
    NetworkOff,
    #[error("only the user can turn on downloading fonts")]
    NotAllowed,
    #[error("{0} is not an https URL")]
    BadUrl(String),
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error("downloading {what} failed: {}", reasons.join("; "))]
    Download { what: String, reasons: Vec<String> },
    #[error("{file} was rejected: {error}")]
    Verify { file: String, error: VerifyError },
    #[error("{0} is not installed")]
    NotInstalled(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{what}: {source}")]
    Io {
        what: String,
        source: std::io::Error,
    },
}

pub(super) fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    let what = what.into();
    move |source| Error::Io { what, source }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PackIndex {
    pub format: u32,
    pub release: String,
    pub packs: Vec<PackEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PackEntry {
    pub locale: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_url: Option<String>,
    pub sha256: String,
    pub size: u64,
    pub licence: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub spelling: bool,
    #[serde(default)]
    pub hyphenation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct InstalledLanguagePack {
    pub locale: String,
    pub name: String,
    pub release: String,
    pub version: String,
    pub licence: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Installed {
    pub version: u32,
    pub packs: Vec<InstalledLanguagePack>,
}

impl PackIndex {
    #[cfg(test)]
    pub(super) fn from_json(bytes: &[u8]) -> Result<Self, Error> {
        Self::parse(bytes, false)
    }

    pub(super) fn parse(bytes: &[u8], local: bool) -> Result<Self, Error> {
        contents::parse_index(bytes, local)
    }

    pub(super) fn pack(&self, locale: &str) -> Option<&PackEntry> {
        self.packs.iter().find(|p| p.locale == locale)
    }
}

pub(super) fn language_pack_index_overridden() -> Option<String> {
    std::env::var(LANGUAGE_PACK_INDEX_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn local_source(s: &str) -> Option<Result<&str, PathBuf>> {
    if s.starts_with("https://") {
        return Some(Ok(s));
    }
    if let Some(rest) = s.strip_prefix("http://") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
        let name = if host.starts_with('[') {
            host.split(']')
                .next()
                .map(|h| format!("{h}]"))
                .unwrap_or_default()
        } else {
            host.split(':').next().unwrap_or_default().to_string()
        };
        return ["127.0.0.1", "localhost", "[::1]"]
            .contains(&name.as_str())
            .then_some(Ok(s));
    }
    if s.contains("://") {
        return None;
    }
    Some(Err(PathBuf::from(s)))
}

pub(super) fn is_locale(s: &str) -> bool {
    !windows_reserved(s)
        && !s.is_empty()
        && s.len() <= 35
        && s.split('-').all(|p| {
            !p.is_empty()
                && p.len() <= 8
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

pub(super) fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(super) fn windows_reserved(n: &str) -> bool {
    let stem = n
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem
                .as_bytes()
                .get(3)
                .is_some_and(|d| (b'1'..=b'9').contains(d)))
}

pub(super) fn plain_text(s: &str) -> String {
    s.chars()
        .filter(|&c| !c.is_control() && !is_format(c))
        .take(200)
        .collect::<String>()
        .trim()
        .to_string()
}

pub(super) fn is_format(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{600}'..='\u{605}'
            | '\u{61c}'
            | '\u{6dd}'
            | '\u{70f}'
            | '\u{890}'..='\u{891}'
            | '\u{8e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

pub(super) fn read_limited(path: &Path, max: u64) -> Option<Vec<u8>> {
    if fs::metadata(path).ok()?.len() > max {
        return None;
    }
    fs::read(path).ok()
}

fn atomic_replace(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let parent = dest
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing parent"))?;
    fs::create_dir_all(parent)?;
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("store");
    let pid = std::process::id();
    let mut last_err = None;
    for _ in 0..128 {
        let tmp = parent.join(format!(
            ".{name}.tmp-{pid}-{}",
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(mut file) => {
                let write_res = file.write_all(bytes).and_then(|()| file.sync_all());
                drop(file);
                if let Err(e) = write_res {
                    let _ = fs::remove_file(&tmp);
                    return Err(e);
                }
                if let Err(e) = fs::rename(&tmp, dest) {
                    let _ = fs::remove_file(&tmp);
                    return Err(e);
                }
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last_err = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("could not allocate temporary file")))
}

pub(super) struct LockGuard {
    _file: File,
}

pub(super) struct Stage {
    pub(super) dir: PathBuf,
}

impl Stage {
    pub(super) fn new(store: &Path) -> Result<Self, Error> {
        static N: AtomicU64 = AtomicU64::new(0);
        let staging = store.join(STAGING);
        fs::create_dir_all(&staging).map_err(io("creating a download folder"))?;
        let mut last = None;
        for _ in 0..1000 {
            let dir = staging.join(format!(
                "{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&dir) {
                Ok(()) => return Ok(Self { dir }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
                Err(e) => return Err(io("creating a download folder")(e)),
            }
        }
        Err(io("creating a download folder")(
            last.unwrap_or_else(|| std::io::Error::other("no free name")),
        ))
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub(super) struct PackStore {
    dir: PathBuf,
    caller: Caller,
    client: Client,
}

impl PackStore {
    pub(super) fn new(dir: PathBuf, caller: Caller) -> Self {
        Self {
            dir,
            caller,
            client: Client::default(),
        }
    }

    pub(super) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(super) fn language_packs_dir(&self) -> PathBuf {
        self.dir.join(LANGUAGE_PACKS)
    }

    pub(super) fn open_language_packs(&self) -> PathBuf {
        if let Err(error) = transaction::recover_on_open(self) {
            log::warn!("recovering installed language packs: {error}");
        }
        self.language_packs_dir()
    }

    pub(super) fn lock(&self) -> Result<LockGuard, Error> {
        fs::create_dir_all(&self.dir).map_err(io("creating the language-pack store folder"))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.dir.join(LOCK))
            .map_err(io("opening the store lock"))?;
        file.lock().map_err(io("locking the store"))?;
        Ok(LockGuard { _file: file })
    }

    pub(super) fn network(&self) -> Network {
        let Some(bytes) = read_limited(&self.dir.join(SETTINGS), MAX_SETTINGS_BYTES) else {
            return Network::Off;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return Network::Off;
        };
        match value.get("network").and_then(|v| v.as_str()) {
            Some("on") => Network::On,
            _ => Network::Off,
        }
    }

    pub(super) fn enable_network(&self) -> Result<(), Error> {
        if self.caller != Caller::User {
            return Err(Error::NotAllowed);
        }
        let _guard = self.lock()?;
        // Preserve any unrelated settings (e.g. legacy user_sources) in settings.json.
        let mut object = read_limited(&self.dir.join(SETTINGS), MAX_SETTINGS_BYTES)
            .and_then(|b| {
                serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&b).ok()
            })
            .unwrap_or_default();
        object.insert("network".into(), serde_json::Value::String("on".into()));
        let bytes = serde_json::to_vec_pretty(&object)
            .map_err(|e| io("saving store settings")(std::io::Error::other(e)))?;
        self.write_file(SETTINGS, &bytes)
    }

    pub(super) fn require_network(&self) -> Result<(), Error> {
        if self.network() == Network::On {
            Ok(())
        } else {
            Err(Error::NetworkOff)
        }
    }

    pub(super) fn fetch(
        &self,
        urls: &[String],
        max: u64,
        what: &str,
        cancel: &Cancel,
        progress: &mut dyn FnMut(u64),
        verify: &dyn Fn(&[u8]) -> Result<(), VerifyError>,
    ) -> Result<Vec<u8>, Error> {
        let mut reasons = Vec::new();
        for url in urls {
            match self.client.get(url, max, cancel, progress) {
                Ok(bytes) => match verify(&bytes) {
                    Ok(()) => return Ok(bytes),
                    Err(error) => {
                        if urls.len() == 1 {
                            return Err(Error::Verify {
                                file: what.to_owned(),
                                error,
                            });
                        }
                        reasons.push(format!("{url}: {error}"));
                    }
                },
                Err(FetchError::Cancelled) => return Err(Error::Cancelled),
                Err(error) => reasons.push(format!("{url}: {error}")),
            }
        }
        Err(Error::Download {
            what: what.to_owned(),
            reasons,
        })
    }

    pub(super) fn write_file(&self, name: &str, bytes: &[u8]) -> Result<(), Error> {
        fs::create_dir_all(&self.dir).map_err(io("creating the store folder"))?;
        atomic_replace(&self.dir.join(name), bytes).map_err(io(format!("saving {name}")))
    }

    fn sweep_staging(&self) {
        for e in fs::read_dir(self.dir.join(STAGING))
            .into_iter()
            .flatten()
            .flatten()
        {
            let old = e
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| SystemTime::now().duration_since(t).is_ok_and(|d| d > STALE));
            if old {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }

    pub(super) fn cached_language_pack_index(&self) -> Option<PackIndex> {
        let legacy = || {
            crate::io::read_regular_file_within(
                &self.dir.join(INDEX_CACHE).join(INDEX_CACHE_NAME),
                MAX_PACK_INDEX_BYTES,
            )
        };
        if language_pack_index_overridden().is_some() {
            return PackIndex::parse(&legacy()?, true).ok();
        }
        let first_party = crate::io::read_regular_file_within(
            &self.dir.join(JOYWORD_INDEX_CACHE),
            MAX_PACK_INDEX_BYTES,
        );
        // A verified legacy cache also indicates the user has already requested
        // the catalogue. Keep its bytes available to older installations.
        if first_party.as_deref() == Some(JOYWORD_INDEX)
            || legacy().is_some_and(|bytes| sha256_hex(&bytes) == LEGACY_INDEX_SHA256)
        {
            return PackIndex::parse(JOYWORD_INDEX, false).ok();
        }
        None
    }

    pub(super) fn fetch_language_pack_index(&self, cancel: &Cancel) -> Result<PackIndex, Error> {
        self.require_network()?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if let Some(location) = language_pack_index_overridden() {
            log::warn!("{LANGUAGE_PACK_INDEX_ENV} is set; using an unpinned test catalogue");
            let bytes = transfer::index(self, &location, cancel)?;
            let index = PackIndex::parse(&bytes, true)?;
            self.write_file(&format!("{INDEX_CACHE}/{INDEX_CACHE_NAME}"), &bytes)?;
            return Ok(index);
        }
        let index = PackIndex::parse(JOYWORD_INDEX, false)?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        self.write_file(JOYWORD_INDEX_CACHE, JOYWORD_INDEX)?;
        Ok(index)
    }

    pub(super) fn install_language_pack(
        &self,
        index: &PackIndex,
        pack: &PackEntry,
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<InstalledLanguagePack, Error> {
        let local = language_pack_index_overridden().is_some();
        self.require_network()?;
        if !is_locale(&pack.locale)
            || !is_sha256(&pack.sha256)
            || pack.size > MAX_PACK_ARCHIVE_BYTES
        {
            let locale: String = pack.locale.chars().take(40).collect();
            return Err(Error::Index(IndexError::Invalid(format!(
                "Language pack {locale:?}: bad locale, sha256 or size"
            ))));
        }
        let bytes = transfer::archive(self, pack, local, cancel, progress)?;
        self.sweep_staging();
        let stage = Stage::new(&self.dir)?;
        let unpacked = stage.dir.join(&pack.locale);
        unpack(&bytes, &unpacked, pack).map_err(|e| Error::Io {
            what: format!("unpacking {}.tar.zst", pack.locale),
            source: std::io::Error::other(e),
        })?;
        let record = InstalledLanguagePack {
            locale: pack.locale.clone(),
            name: plain_text(&pack.name),
            release: plain_text(&index.release),
            version: plain_text(&pack.version),
            licence: plain_text(&pack.licence),
            sha256: pack.sha256.clone(),
            size: pack.size,
        };
        transaction::publish(
            self,
            &stage,
            &pack.locale,
            Some((&unpacked, &record)),
            Some(cancel),
        )?;
        Ok(record)
    }

    pub(super) fn remove_language_pack(&self, locale: &str) -> Result<(), Error> {
        if !is_locale(locale) {
            return Err(Error::NotInstalled(locale.chars().take(40).collect()));
        }
        let stage = Stage::new(&self.dir)?;
        transaction::publish(self, &stage, locale, None, None)
    }

    pub(super) fn read_installed(&self) -> Result<Installed, Error> {
        let path = self.dir.join(INSTALLED);
        let saved = crate::io::read_regular_file_within(&path, MAX_PACK_INDEX_BYTES);
        if let Some(bytes) = &saved
            && let Ok(inventory) = serde_json::from_slice(bytes)
        {
            return Ok(inventory);
        }
        let record_metadata = fs::symlink_metadata(&path).ok();
        let had_record = saved.is_some() || record_metadata.is_some();
        let packs = self.scan_installed_packs();
        if !had_record && packs.is_empty() {
            return Ok(Installed::default());
        }
        let inventory = Installed { version: 1, packs };
        log::warn!(
            "{}: {}; rebuilt from the Language pack folders",
            path.display(),
            if had_record {
                "can't be read"
            } else {
                "missing"
            }
        );
        let quarantine = self.dir.join(format!("{INSTALLED}.bad"));
        if let Some(bytes) = saved {
            let _ = atomic_replace(&quarantine, &bytes);
        } else if record_metadata.is_some_and(|m| m.is_file()) {
            // Preserve an oversized or unreadable regular file without reading
            // it into memory. If publication fails, restore the original name.
            fs::rename(&path, &quarantine).map_err(io("preserving the Language pack list"))?;
            if let Err(error) = self.write_installed(&inventory) {
                let _ = fs::rename(&quarantine, &path);
                return Err(error);
            }
            return Ok(inventory);
        }
        self.write_installed(&inventory)?;
        Ok(inventory)
    }

    fn scan_installed_packs(&self) -> Vec<InstalledLanguagePack> {
        #[derive(Default, Deserialize)]
        struct PackLabel {
            name: Option<toml::Value>,
            licence: Option<toml::Value>,
        }

        let mut packs = Vec::new();
        let Ok(entries) = fs::read_dir(self.language_packs_dir()) else {
            return packs;
        };
        for entry in entries.filter_map(Result::ok) {
            let Ok(locale) = entry.file_name().into_string() else {
                continue;
            };
            if !is_locale(&locale) {
                continue;
            }
            let label: PackLabel = crate::io::read_regular_file_within(
                &entry.path().join("pack.toml"),
                MAX_PACK_TOML_BYTES,
            )
            .and_then(|bytes| toml::from_str(std::str::from_utf8(&bytes).ok()?).ok())
            .unwrap_or_default();
            packs.push(InstalledLanguagePack {
                name: label
                    .name
                    .as_ref()
                    .and_then(toml::Value::as_str)
                    .map_or_else(|| locale.clone(), plain_text),
                licence: label
                    .licence
                    .as_ref()
                    .and_then(toml::Value::as_str)
                    .map_or_else(String::new, plain_text),
                locale,
                release: String::new(),
                version: String::new(),
                sha256: String::new(),
                size: 0,
            });
            if packs.len() == MAX_PACKS {
                break;
            }
        }
        packs.sort_unstable_by(|left, right| left.locale.cmp(&right.locale));
        packs
    }

    pub(super) fn write_installed(&self, installed: &Installed) -> Result<(), Error> {
        let bytes = serde_json::to_vec_pretty(&Installed {
            version: 1,
            packs: installed.packs.clone(),
        })
        .map_err(|e| io("saving the Language pack list")(std::io::Error::other(e)))?;
        self.write_file(INSTALLED, &bytes)
    }
}
