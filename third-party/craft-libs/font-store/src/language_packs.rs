//! The store's second item kind, `language-pack`: a language's spelling dictionary, hyphenation
//! patterns and licence texts, published by storytold/craft-i18n as `<locale>.tar.zst` release
//! assets listed in an `index.json`, and read at runtime by `craft-proof` (`craft_proof::packs`).
//!
//! It reuses the store's folder, consent ([`FontStore::enable_network`]), HTTP client and sha256
//! checks: the app pins the index by URL and sha256 ([`PackIndexPin`]), each archive is checked
//! against the size and sha256 the index gives, then unpacked into `.staging/` and moved into
//! `language-packs/<locale>/` with one rename. Packs have no update checks of their own: a new
//! pin is a new index.
//!
//! ```text
//! <store>/language-packs/<locale>/   pack.toml, spelling.aff, spelling.dic, hyphenation.pat, LICENCES/…
//! <store>/language-packs.json        which packs are installed, from which index entry
//! <store>/indexes/language-packs.json  the last pack index fetched
//! ```
//!
//! **Developer override (testing only):** `CRAFT_LANGUAGE_PACK_INDEX=<url or path>` replaces the
//! pinned index URL with an `http://` URL on a loopback host (`127.0.0.1`, `localhost`, `[::1]`),
//! an `https://` URL or a local file path, and skips the index's sha256 pin. Archives are still
//! checked against the sha256 and size in that index, and may then also be loopback `http://`
//! URLs or local paths. It is off unless the variable is set, and logged as a warning when used.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

mod contents;
#[cfg(test)]
use contents::MAX_PACK_FRAMES;
use contents::unpack;
mod transaction;
mod transfer;

use crate::http::Cancel;
#[cfg(test)]
use crate::http::{Client, Timeouts};
use crate::index::IndexError;
use crate::store::{Error, FontStore, INDEX_CACHE, Progress, Stage, io, read_limited};
#[cfg(test)]
use crate::verify::VerifyError;
use crate::verify::sha256_hex;

/// The store's folder of installed Language packs.
pub const LANGUAGE_PACKS: &str = "language-packs";
/// The developer override of the pinned pack index (see the module docs). Testing only.
pub const LANGUAGE_PACK_INDEX_ENV: &str = "CRAFT_LANGUAGE_PACK_INDEX";
/// Largest pack index read.
pub const MAX_PACK_INDEX_BYTES: u64 = 4 << 20;
/// Largest pack archive downloaded.
pub const MAX_PACK_ARCHIVE_BYTES: u64 = 128 << 20;
/// Largest file unpacked from a pack (craft-proof's dictionary cap).
pub const MAX_PACK_FILE_BYTES: u64 = 64 << 20;
/// Largest total unpacked size of a pack.
pub const MAX_PACK_UNPACKED_BYTES: u64 = 256 << 20;
/// Most files and folders in a pack archive.
pub const MAX_PACK_ENTRIES: usize = 256;
/// Most packs in an index.
const MAX_PACKS: usize = 1024;
const INSTALLED: &str = "language-packs.json";
const INDEX_CACHE_NAME: &str = "language-packs.json";

/// The pack index an app trusts: `index.json` of a craft-i18n `language-packs-<n>` release, by
/// URL and sha256 (the app's Pin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackIndexPin {
    /// `https://github.com/storytold/craft-i18n/releases/download/language-packs-<n>/index.json`.
    pub url: &'static str,
    /// The sha256 of that file, lowercase hex.
    pub sha256: &'static str,
}

/// `index.json` of a pack release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackIndex {
    /// The format; 1.
    pub format: u32,
    /// The release (`language-packs-1`).
    pub release: String,
    /// The packs, by locale.
    pub packs: Vec<PackEntry>,
}

/// One pack of a [`PackIndex`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackEntry {
    /// Lowercase BCP 47 tag (`nl`, `pt-br`); the pack's folder name.
    pub locale: String,
    /// The language's own name (`Nederlands`).
    pub name: String,
    /// The craft-i18n commit it was built from.
    #[serde(default)]
    pub version: String,
    /// The archive (`<locale>.tar.zst`).
    pub url: String,
    /// The archive's sha256, lowercase hex.
    pub sha256: String,
    /// The archive's size in bytes.
    pub size: u64,
    /// SPDX licence expression.
    pub licence: String,
    /// The licence class: `Embeddable` or `Download-only`.
    #[serde(default)]
    pub class: String,
    /// Whether it has a spelling dictionary.
    #[serde(default)]
    pub spelling: bool,
    /// Whether it has hyphenation patterns.
    #[serde(default)]
    pub hyphenation: bool,
    /// Where the data comes from (an `https://` page), for display; optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// An installed pack, as recorded when it was installed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledLanguagePack {
    /// Lowercase BCP 47 tag; the folder under [`LANGUAGE_PACKS`].
    pub locale: String,
    /// The language's own name.
    pub name: String,
    /// The release it came from.
    pub release: String,
    /// The craft-i18n commit it was built from.
    pub version: String,
    /// SPDX licence expression.
    pub licence: String,
    /// The archive's sha256.
    pub sha256: String,
    /// The archive's size in bytes.
    pub size: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Installed {
    version: u32,
    packs: Vec<InstalledLanguagePack>,
}

impl PackIndex {
    /// Parses and checks an index: format 1, every locale a lowercase tag and listed once, every
    /// archive at most [`MAX_PACK_ARCHIVE_BYTES`] with a sha256. Archive URLs must be `https://`
    /// (with `local`, the developer override, also loopback `http://` or a path).
    pub fn from_json(bytes: &[u8]) -> Result<Self, Error> {
        Self::parse(bytes, false)
    }

    fn parse(bytes: &[u8], local: bool) -> Result<Self, Error> {
        contents::parse_index(bytes, local)
    }

    /// The pack for `locale`.
    pub fn pack(&self, locale: &str) -> Option<&PackEntry> {
        self.packs.iter().find(|p| p.locale == locale)
    }
}

/// Where the pack index comes from: the pin, or the developer override.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IndexSource {
    /// The pinned URL, checked against the pinned sha256.
    Pinned(PackIndexPin),
    /// `CRAFT_LANGUAGE_PACK_INDEX`: no index pin; loopback `http://` and paths allowed.
    Override(String),
}

impl IndexSource {
    /// The pin, unless [`LANGUAGE_PACK_INDEX_ENV`] is set.
    fn from_env(pin: &PackIndexPin) -> Self {
        match language_pack_index_overridden() {
            Some(v) => {
                log::warn!(
                    "{LANGUAGE_PACK_INDEX_ENV}={v}: the Language pack index is NOT the pinned one and its sha256 is not checked (testing only)"
                );
                Self::Override(v)
            }
            None => Self::Pinned(*pin),
        }
    }

    fn is_override(&self) -> bool {
        matches!(self, Self::Override(_))
    }
}

/// The developer override of the pack index, when [`LANGUAGE_PACK_INDEX_ENV`] is set: the URL or
/// path that replaces the pinned index, whose sha256 is then **not** checked. Apps show a warning
/// in their Language pack interface while it is set. It works in release builds too, for
/// end-to-end testing against a local index.
pub fn language_pack_index_overridden() -> Option<String> {
    std::env::var(LANGUAGE_PACK_INDEX_ENV)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// A source the developer override may read: `Ok(url)` for an `https://` URL or an `http://`
/// one on a loopback host, `Err(path)` for a local file path; `None` for anything else.
fn local_source(s: &str) -> Option<Result<&str, PathBuf>> {
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

/// A lowercase BCP 47 tag as packs are named (craft-proof's rule): ASCII lowercase letters and
/// digits in parts of 1 to 8 joined by `-`, at most 35 characters.
fn is_locale(s: &str) -> bool {
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

fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl FontStore {
    /// The folder of installed Language packs (`<store>/language-packs`): give it to
    /// `craft_proof::packs::set_packs_dir`.
    pub fn language_packs_dir(&self) -> PathBuf {
        self.dir().join(LANGUAGE_PACKS)
    }

    /// Restore the previous dictionary and its record after an interrupted change.
    /// Opening a healthy store only checks for the presence of the journal.
    pub fn open_language_packs(&self) -> PathBuf {
        if let Err(error) = transaction::recover_on_open(self) {
            log::warn!("recovering installed language packs: {error}");
        }
        self.language_packs_dir()
    }

    /// The pack index saved by the last [`FontStore::fetch_language_pack_index`], when it is
    /// `pin`'s (or the developer override is set).
    pub fn cached_language_pack_index(&self, pin: &PackIndexPin) -> Option<PackIndex> {
        self.cached_pack_index_from(&IndexSource::from_env(pin))
    }

    pub(crate) fn cached_pack_index_from(&self, source: &IndexSource) -> Option<PackIndex> {
        let bytes = read_limited(
            &self.dir().join(INDEX_CACHE).join(INDEX_CACHE_NAME),
            MAX_PACK_INDEX_BYTES,
        )?;
        match source {
            IndexSource::Pinned(pin) if sha256_hex(&bytes) != pin.sha256 => None,
            _ => PackIndex::parse(&bytes, source.is_override()).ok(),
        }
    }

    /// Downloads the pack index `pin` names, checks it against the pin and saves it for
    /// [`FontStore::cached_language_pack_index`]. Needs the network on.
    pub fn fetch_language_pack_index(
        &self,
        pin: &PackIndexPin,
        cancel: &Cancel,
    ) -> Result<PackIndex, Error> {
        self.fetch_pack_index_from(&IndexSource::from_env(pin), cancel)
    }

    pub(crate) fn fetch_pack_index_from(
        &self,
        source: &IndexSource,
        cancel: &Cancel,
    ) -> Result<PackIndex, Error> {
        let bytes = transfer::index(self, source, cancel)?;
        let index = PackIndex::parse(&bytes, source.is_override())?;
        std::fs::create_dir_all(self.dir().join(INDEX_CACHE))
            .map_err(io("creating the index folder"))?;
        self.write_file(&format!("{INDEX_CACHE}/{INDEX_CACHE_NAME}"), &bytes)?;
        Ok(index)
    }

    /// Downloads `pack` from `index`, checks its size and sha256, unpacks it and moves it into
    /// `language-packs/<locale>/` in one rename, replacing an older one. Needs the network on.
    /// Then call `craft_proof::packs::reload()`.
    pub fn install_language_pack(
        &self,
        index: &PackIndex,
        pack: &PackEntry,
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<InstalledLanguagePack, Error> {
        let local = language_pack_index_overridden().is_some();
        self.install_pack_from(local, index, pack, cancel, progress)
    }

    pub(crate) fn install_pack_from(
        &self,
        local: bool,
        index: &PackIndex,
        pack: &PackEntry,
        cancel: &Cancel,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<InstalledLanguagePack, Error> {
        self.require_network()?;
        // The entry is checked again here: callers may build one by hand.
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
        let stage = Stage::new(self.dir())?;
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

    /// The installed Language packs whose folder is there, by locale. A pending
    /// publication is recovered under the writer lock before inspecting the inventory.
    pub fn installed_language_packs(&self) -> Vec<InstalledLanguagePack> {
        let dir = self.open_language_packs();
        // Read only: without the store lock this never writes the record (an install or removal
        // may be under way); a record that can't be read is listed from the folders and repaired
        // by the next install or removal, which hold the lock.
        let installed = self
            .read_record()
            .unwrap_or_else(|| self.installed_from_disk());
        installed
            .packs
            .into_iter()
            .filter(|p| is_locale(&p.locale) && dir.join(&p.locale).is_dir())
            .map(|p| InstalledLanguagePack {
                name: plain_text(&p.name),
                release: plain_text(&p.release),
                version: plain_text(&p.version),
                licence: plain_text(&p.licence),
                ..p
            })
            .collect()
    }

    /// Removes the pack for `locale`: its folder disappears at once (one rename) and is deleted
    /// after. No network needed. Then call `craft_proof::packs::reload()`.
    pub fn remove_language_pack(&self, locale: &str) -> Result<(), Error> {
        if !is_locale(locale) {
            return Err(Error::NotInstalled(locale.chars().take(40).collect()));
        }
        let stage = Stage::new(self.dir())?;
        transaction::publish(self, &stage, locale, None, None)
    }

    /// The record of installed packs. One that can't be read is copied aside as
    /// `language-packs.json.bad` and rebuilt from the pack folders on disk (written atomically
    /// over it), so no installed pack is ever dropped from it; a missing one is rebuilt the same
    /// way when pack folders are there (none: empty).
    fn read_installed(&self) -> Result<Installed, Error> {
        let path = self.dir().join(INSTALLED);
        let there = std::fs::symlink_metadata(&path).is_ok();
        if let Some(i) = self.read_record() {
            return Ok(i);
        }
        let rebuilt = self.installed_from_disk();
        if !there && rebuilt.packs.is_empty() {
            return Ok(Installed::default());
        }
        log::warn!(
            "{}: {}; rebuilt from the Language pack folders",
            path.display(),
            if there { "can't be read" } else { "missing" }
        );
        if there {
            let _ = std::fs::copy(&path, self.dir().join(format!("{INSTALLED}.bad")));
        }
        self.write_installed(&rebuilt)?;
        Ok(rebuilt)
    }

    /// The record as written, if there is one that can be read.
    fn read_record(&self) -> Option<Installed> {
        read_limited(&self.dir().join(INSTALLED), MAX_PACK_INDEX_BYTES)
            .and_then(|b| serde_json::from_slice::<Installed>(&b).ok())
    }

    /// The installed packs as their folders say: each folder named by a locale, with the name
    /// and licence from its `pack.toml` (release, version and sha256 unknown).
    fn installed_from_disk(&self) -> Installed {
        let mut packs: Vec<InstalledLanguagePack> = std::fs::read_dir(self.language_packs_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|l| is_locale(l))
            .take(MAX_PACKS)
            .map(|locale| {
                let info = read_limited(
                    &self.language_packs_dir().join(&locale).join("pack.toml"),
                    MAX_PACK_TOML_BYTES,
                )
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|t| t.parse::<toml::Table>().ok());
                let field = |k: &str| {
                    info.as_ref()
                        .and_then(|t| t.get(k))
                        .and_then(toml::Value::as_str)
                        .map(plain_text)
                };
                InstalledLanguagePack {
                    name: field("name").unwrap_or_else(|| locale.clone()),
                    licence: field("licence").unwrap_or_default(),
                    locale,
                    release: String::new(),
                    version: String::new(),
                    sha256: String::new(),
                    size: 0,
                }
            })
            .collect();
        packs.sort_by(|a, b| a.locale.cmp(&b.locale));
        Installed { version: 1, packs }
    }

    fn write_installed(&self, installed: &Installed) -> Result<(), Error> {
        let bytes = serde_json::to_vec_pretty(&Installed {
            version: 1,
            packs: installed.packs.clone(),
        })
        .map_err(|e| io("saving the Language pack list")(std::io::Error::other(e)))?;
        self.write_file(INSTALLED, &bytes)
    }
}

/// Largest `pack.toml` read back.
const MAX_PACK_TOML_BYTES: u64 = 1 << 20;

/// A name Windows reserves for a device (`CON`, `nul.txt`, `COM1`, `lpt9.md`), with or without an
/// extension, in any case.
fn windows_reserved(n: &str) -> bool {
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

/// Text from an index or `pack.toml` shown to people (a pack's name): without control,
/// bidirectional or other format characters (U+202E would reverse what follows), trimmed, at most
/// 200 characters.
fn plain_text(s: &str) -> String {
    s.chars()
        .filter(|&c| !c.is_control() && !is_format(c))
        .take(200)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Unicode format characters (general category Cf): bidirectional controls, zero-width ones, the
/// soft hyphen, tags.
fn is_format(c: char) -> bool {
    matches!(c,
        '\u{ad}' | '\u{600}'..='\u{605}' | '\u{61c}' | '\u{6dd}' | '\u{70f}' | '\u{890}'..='\u{891}' | '\u{8e2}' | '\u{180e}'
        | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206f}' | '\u{feff}'
        | '\u{fff9}'..='\u{fffb}' | '\u{110bd}' | '\u{110cd}' | '\u{13430}'..='\u{1343f}' | '\u{1bca0}'..='\u{1bca3}'
        | '\u{1d173}'..='\u{1d17a}' | '\u{e0001}' | '\u{e0020}'..='\u{e007f}')
}

#[cfg(test)]
mod tests;
