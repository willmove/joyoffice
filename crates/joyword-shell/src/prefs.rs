//! User identity and preferences (PLATFORM-008, T106).
//!
//! Stored as UTF-8 `key=value` lines with a `schema` version. Loading never
//! fails: a damaged file falls back to defaults, is kept beside the original
//! name and is reported; a file from a newer JoyWord is used read-only so an
//! older build never overwrites it; legacy keys are migrated. Unknown keys
//! are kept and written back. Unreadable or oversized files use read-only
//! defaults. Reads and serialized saves are limited to 256 KiB. Saving
//! replaces the file atomically.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::l10n::Locale;

pub const SCHEMA: u32 = 1;
const FILE_NAME: &str = "preferences.conf";
const SAVE_LOCK: &str = ".preferences-save.lock";
const MAX_RECENT: usize = 10;
const MAX_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    Centimeters,
    Millimeters,
    Inches,
    Points,
}

impl Units {
    fn tag(self) -> &'static str {
        match self {
            Units::Centimeters => "cm",
            Units::Millimeters => "mm",
            Units::Inches => "in",
            Units::Points => "pt",
        }
    }

    pub fn parse(tag: &str) -> Option<Self> {
        match tag {
            "cm" | "centimeters" => Some(Units::Centimeters),
            "mm" | "millimeters" => Some(Units::Millimeters),
            "in" | "inch" | "inches" => Some(Units::Inches),
            "pt" | "points" => Some(Units::Points),
            _ => None,
        }
    }

    pub(crate) fn engine_unit(self) -> joyword_geom::Unit {
        match self {
            Self::Centimeters => joyword_geom::Unit::Centimeters,
            Self::Millimeters => joyword_geom::Unit::Millimeters,
            Self::Inches => joyword_geom::Unit::Inches,
            Self::Points => joyword_geom::Unit::Points,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagePreset {
    A4,
    Letter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    /// Author of revisions and comments.
    pub author_name: String,
    /// Empty means derived from the name.
    pub author_initials: String,
    pub units: Units,
    /// Minutes between recovery snapshots; 0 disables them.
    pub autosave_minutes: u32,
    pub ui_locale: Locale,
    pub new_page: PagePreset,
    /// Most recent first.
    pub recent: Vec<PathBuf>,
    /// Keys this version does not know, written back unchanged.
    pub unknown: Vec<(String, String)>,
}

impl Preferences {
    /// Locale-dependent defaults for new installations.
    pub fn defaults_for(locale: Locale) -> Self {
        Self {
            author_name: String::new(),
            author_initials: String::new(),
            units: match locale {
                Locale::ZhCn => Units::Centimeters,
                Locale::En => Units::Inches,
            },
            autosave_minutes: 10,
            ui_locale: locale,
            new_page: match locale {
                Locale::ZhCn => PagePreset::A4,
                Locale::En => PagePreset::Letter,
            },
            recent: Vec::new(),
            unknown: Vec::new(),
        }
    }

    /// Initials shown on revisions and comments: explicit ones, else the
    /// first character of each word of the name (at most three).
    pub fn initials(&self) -> String {
        if !self.author_initials.trim().is_empty() {
            return self.author_initials.trim().to_owned();
        }
        self.author_name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .flat_map(char::to_uppercase)
            .take(3)
            .collect()
    }

    /// Moves `path` to the front of the recent list, without duplicates.
    pub fn add_recent(&mut self, path: PathBuf) {
        self.recent.retain(|existing| existing != &path);
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrefsNotice {
    /// The file could not be safely read; defaults are used without allowing
    /// a later save to replace the unobserved original.
    Unreadable {
        reason: String,
    },
    /// The file was unreadable; defaults are in use and the original was
    /// copied to `backup`.
    Corrupt {
        backup: PathBuf,
    },
    /// Written by a newer schema; used read-only.
    Newer {
        schema: u32,
    },
    Migrated {
        from: u32,
    },
    /// A value was invalid and replaced by its default.
    InvalidValue {
        key: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    pub prefs: Preferences,
    pub notices: Vec<PrefsNotice>,
    /// When false, `save` refuses to overwrite the file.
    pub writable: bool,
    baseline: Baseline,
}

// Each cloned editing session keeps its own observation. Successful saves
// advance only that session, never bless an independently edited stale clone.
type Observation = (PathBuf, Option<Vec<u8>>);
#[derive(Debug, Default)]
struct Baseline(Mutex<Option<Observation>>);
impl Baseline {
    fn new(path: PathBuf, bytes: Option<Vec<u8>>) -> Self {
        Self(Mutex::new(
            std::path::absolute(path).ok().map(|path| (path, bytes)),
        ))
    }
    fn snapshot(&self) -> Option<Observation> {
        self.0.lock().ok().and_then(|value| value.clone())
    }
}
impl Clone for Baseline {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.snapshot()))
    }
}
impl PartialEq for Baseline {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot() == other.snapshot()
    }
}
impl Eq for Baseline {}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

fn unescape(value: &str) -> Option<String> {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next()? {
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                _ => return None,
            }
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

// A recognizable future declaration takes precedence over old syntax and
// conflicting earlier declarations. Share this policy across load/migrate/save.
fn declared_future_schema(text: &str) -> Option<u32> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| key.trim() == "schema")
        .filter_map(|(_, value)| unescape(value.trim_end_matches('\r')))
        .filter_map(|value| value.parse::<u32>().ok())
        .filter(|schema| *schema > SCHEMA)
        .max()
}

/// Parses file content. `Err(())` means the file as a whole is unusable.
fn parse(text: &str, fallback: Locale) -> Result<(Preferences, Vec<PrefsNotice>, u32), ()> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(())?;
        entries.push((key.trim().to_owned(), unescape(value).ok_or(())?));
    }
    let schema = match entries.iter().find(|(key, _)| key == "schema") {
        Some((_, value)) => value.parse::<u32>().map_err(|_| ())?,
        None => 0,
    };
    let mut notices = Vec::new();
    if schema == 0 {
        // Schema 0 (pre-release builds) used `author` and `unit`.
        for (key, _) in &mut entries {
            match key.as_str() {
                "author" => *key = "author.name".into(),
                "unit" => *key = "units".into(),
                _ => {}
            }
        }
        notices.push(PrefsNotice::Migrated { from: 0 });
    }
    let locale = entries
        .iter()
        .find(|(key, _)| key == "ui.locale")
        .and_then(|(_, value)| Locale::from_tag(value))
        .unwrap_or(fallback);
    let mut prefs = Preferences::defaults_for(locale);
    let mut invalid = |key: &str| {
        notices.push(PrefsNotice::InvalidValue {
            key: key.to_owned(),
        })
    };
    for (key, value) in entries {
        match key.as_str() {
            "schema" => {}
            "author.name" => prefs.author_name = value,
            "author.initials" => prefs.author_initials = value,
            "units" => match Units::parse(&value) {
                Some(units) => prefs.units = units,
                None => invalid(&key),
            },
            "autosave.minutes" => match value.parse::<u32>() {
                Ok(minutes) if minutes <= 120 => prefs.autosave_minutes = minutes,
                _ => invalid(&key),
            },
            "ui.locale" => {
                if Locale::from_tag(&value).is_none() {
                    invalid(&key);
                }
            }
            "new.page" => match value.as_str() {
                "A4" => prefs.new_page = PagePreset::A4,
                "Letter" => prefs.new_page = PagePreset::Letter,
                _ => invalid(&key),
            },
            "recent" => {
                if prefs.recent.len() < MAX_RECENT && !value.is_empty() {
                    prefs.recent.push(PathBuf::from(value));
                }
            }
            _ => prefs.unknown.push((key, value)),
        }
    }
    Ok((prefs, notices, schema))
}

fn serialize(prefs: &Preferences) -> String {
    let mut out = String::from("# JoyWord preferences. Edit while JoyWord is closed.\n");
    let _ = writeln!(out, "schema={SCHEMA}");
    let page = match prefs.new_page {
        PagePreset::A4 => "A4",
        PagePreset::Letter => "Letter",
    };
    for (key, value) in [
        ("author.name", prefs.author_name.as_str()),
        ("author.initials", prefs.author_initials.as_str()),
        ("units", prefs.units.tag()),
        ("ui.locale", prefs.ui_locale.tag()),
        ("new.page", page),
    ] {
        let _ = writeln!(out, "{key}={}", escape(value));
    }
    let _ = writeln!(out, "autosave.minutes={}", prefs.autosave_minutes);
    for path in &prefs.recent {
        let _ = writeln!(out, "recent={}", escape(&path.to_string_lossy()));
    }
    for (key, value) in &prefs.unknown {
        let _ = writeln!(out, "{}={}", key, escape(value));
    }
    out
}

/// Per-user configuration directory, without creating it.
pub fn default_config_dir() -> Option<PathBuf> {
    let env = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if let Some(path) = env("JOYWORD_CONFIG_DIR") {
        return Some(path);
    }
    if cfg!(target_os = "macos") {
        env("HOME").map(|home| home.join("Library/Application Support/JoyWord"))
    } else if cfg!(target_os = "windows") {
        env("APPDATA").map(|data| data.join("JoyWord"))
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|home| home.join(".config")))
            .map(|config| config.join("joyword"))
    }
}

// Opening first can block indefinitely on a FIFO before the byte budget is
// checked. Unix nonblocking open also handles replacement after a path check.
fn open_preferences(path: &Path) -> io::Result<fs::File> {
    #[cfg(unix)]
    let file: fs::File = {
        use rustix::fs::{Mode, OFlags, open};
        open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?
        .into()
    };
    #[cfg(not(unix))]
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preferences are not a regular file",
        ));
    }
    Ok(file)
}

fn read_preferences(path: &Path) -> io::Result<Vec<u8>> {
    let file = open_preferences(path)?;
    if file.metadata()?.len() > MAX_BYTES as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preferences exceed 256 KiB",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preferences exceed 256 KiB",
        ));
    }
    Ok(bytes)
}

// Permanent sidecar: deleting it on unlock would permit split-inode locks.
// Each save opens a fresh handle and tries exactly once; Drop releases the lock.
fn acquire_save_lock(directory: &Path) -> io::Result<fs::File> {
    let path = directory.join(SAVE_LOCK);
    #[cfg(unix)]
    let file: fs::File = {
        use rustix::fs::{Mode, OFlags, open};
        open(
            &path,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?
        .into()
    };
    #[cfg(not(unix))]
    let file = {
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options
                .custom_flags(winsafe::co::FILE_FLAG::OPEN_REPARSE_POINT.raw())
                .share_mode(
                    winsafe::co::FILE_SHARE::READ.raw() | winsafe::co::FILE_SHARE::WRITE.raw(),
                );
        }
        options.open(&path)?
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || !fs::symlink_metadata(&path)?.is_file() {
        return Err(io::Error::other(
            "preferences save lock is not a regular file",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & winsafe::co::FILE_ATTRIBUTE::REPARSE_POINT.raw() != 0 {
            return Err(io::Error::other("preferences save lock is a reparse point"));
        }
    }
    file.try_lock()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current = fs::symlink_metadata(&path)?;
        if !current.is_file()
            || metadata.nlink() != 1
            || current.nlink() != 1
            || metadata.dev() != current.dev()
            || metadata.ino() != current.ino()
        {
            return Err(io::Error::other(
                "preferences save lock changed while opening",
            ));
        }
    }
    Ok(file)
}

pub struct PrefsStore {
    dir: PathBuf,
}

impl PrefsStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(FILE_NAME)
    }

    /// Use the old installation's defaults until this editor saves its own
    /// preferences. Reading the old file never invokes corruption recovery.
    pub fn load_with_legacy(&self, legacy_dir: &Path, fallback: Locale) -> Loaded {
        if !self
            .path()
            .symlink_metadata()
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
        {
            return self.load(fallback);
        }

        let mut text = String::new();
        let legacy = open_preferences(&legacy_dir.join(FILE_NAME))
            .and_then(|file| file.take(MAX_BYTES as u64 + 1).read_to_string(&mut text));
        if legacy.is_ok()
            && text.len() <= MAX_BYTES
            && declared_future_schema(&text).is_none()
            && let Ok((prefs, notices, schema)) = parse(&text, fallback)
            && schema <= SCHEMA
        {
            return Loaded {
                prefs,
                notices,
                writable: true,
                baseline: Baseline::new(self.path(), None),
            };
        }
        self.load(fallback)
    }

    /// Loads preferences; see the module documentation. `fallback` chooses
    /// defaults (normally the system UI language).
    pub fn load(&self, fallback: Locale) -> Loaded {
        let path = self.path();
        let bytes = match read_preferences(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                // A missing target behind a symlink is not a missing config.
                let missing = error.kind() == io::ErrorKind::NotFound
                    && path
                        .symlink_metadata()
                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound);
                return Loaded {
                    prefs: Preferences::defaults_for(fallback),
                    notices: if missing {
                        Vec::new()
                    } else {
                        vec![PrefsNotice::Unreadable {
                            reason: error.to_string(),
                        }]
                    },
                    writable: missing,
                    baseline: if missing {
                        Baseline::new(path, None)
                    } else {
                        Baseline::default()
                    },
                };
            }
        };
        let parsed = std::str::from_utf8(&bytes)
            .map_err(|_| ())
            .and_then(|text| parse(text, fallback));
        if let Some(schema) = std::str::from_utf8(&bytes)
            .ok()
            .and_then(declared_future_schema)
        {
            let (prefs, mut notices, _) =
                parsed.unwrap_or_else(|()| (Preferences::defaults_for(fallback), Vec::new(), 0));
            notices.push(PrefsNotice::Newer { schema });
            return Loaded {
                prefs,
                notices,
                writable: false,
                baseline: Baseline::new(path, Some(bytes)),
            };
        }
        match parsed {
            Ok((prefs, notices, _)) => Loaded {
                prefs,
                notices,
                writable: true,
                baseline: Baseline::new(path, Some(bytes)),
            },
            Err(()) => {
                let backup = self.backup_path();
                // create_new also protects a backup created by another process.
                // Exhaustion or a competing writer preserves the original read-only.
                let mut options = fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    // Backups contain the original private preferences. Restrict
                    // access at creation, before any bytes are written.
                    options.mode(0o600);
                }
                let copied = options
                    .open(&backup)
                    .and_then(|mut file| {
                        file.write_all(&bytes)?;
                        file.sync_all()
                    })
                    .is_ok();
                Loaded {
                    prefs: Preferences::defaults_for(fallback),
                    notices: vec![PrefsNotice::Corrupt { backup }],
                    writable: copied,
                    baseline: Baseline::new(path, Some(bytes)),
                }
            }
        }
    }

    fn backup_path(&self) -> PathBuf {
        (0..1024)
            .map(|n| self.dir.join(format!("preferences.corrupt-{n}.conf")))
            .find(|candidate| !candidate.exists())
            .unwrap_or_else(|| self.dir.join("preferences.corrupt-1023.conf"))
    }

    fn check_observation(&self, observation: &Observation) -> io::Result<()> {
        let path = self.path();
        if std::path::absolute(&path)? != observation.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "preferences belong to another store",
            ));
        }
        let current = match read_preferences(&path) {
            Ok(bytes) => Some(bytes),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && path
                        .symlink_metadata()
                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        if current != observation.1 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "preferences changed since loading; reload before saving",
            ));
        }
        Ok(())
    }

    // The loaded writable flag is only a past observation: a newer installation
    // can publish preferences while this process remains open.
    fn check_current_schema(&self) -> io::Result<()> {
        let path = self.path();
        let bytes = match read_preferences(&path) {
            Ok(bytes) => bytes,
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    && path
                        .symlink_metadata()
                        .is_err_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        if let Ok(text) = std::str::from_utf8(&bytes)
            && declared_future_schema(text).is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "preferences now belong to a newer JoyWord; reload before saving",
            ));
        }
        Ok(())
    }

    /// Writes atomically: a temporary file in the same directory, flushed,
    /// then renamed over the original.
    pub fn save(&self, loaded: &Loaded) -> io::Result<()> {
        if !loaded.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "preferences could not be safely read, belong to a newer JoyWord, or could not be backed up",
            ));
        }
        let serialized = serialize(&loaded.prefs);
        if serialized.len() > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "preferences exceed 256 KiB",
            ));
        }
        let mut baseline = loaded
            .baseline
            .0
            .lock()
            .map_err(|_| io::Error::other("preferences baseline unavailable"))?;
        let observation = baseline
            .as_mut()
            .ok_or_else(|| io::Error::other("preferences baseline unavailable"))?;
        self.check_observation(observation)?;
        self.check_current_schema()?;
        fs::create_dir_all(&self.dir)?;
        let _save_lock = acquire_save_lock(&self.dir)?;
        // Another cooperating process may have published after our preflight.
        self.check_observation(observation)?;
        self.check_current_schema()?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.dir)?;
        temporary.write_all(serialized.as_bytes())?;
        temporary.as_file().sync_all()?;
        self.check_observation(observation)?;
        self.check_current_schema()?;
        temporary
            .persist(self.path())
            .map_err(|error| error.error)?;
        observation.1 = Some(serialized.into_bytes());
        Ok(())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
#[allow(clippy::panic, reason = "Test assertions")]
mod tests {
    use super::*;

    #[test]
    fn oversized_preferences_are_preserved_read_only_without_backups() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let original = format!("schema=1\nfuture.key={}\n", "x".repeat(MAX_BYTES));
        fs::write(store.path(), &original)?;
        let loaded = store.load(Locale::En);
        assert!(!loaded.writable);
        assert!(matches!(
            &loaded.notices[..],
            [PrefsNotice::Unreadable { .. }]
        ));
        assert_eq!(
            store.save(&loaded).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(fs::read_to_string(store.path())?, original);
        assert_eq!(fs::read_dir(dir.path())?.count(), 1);
        Ok(())
    }

    #[test]
    fn preferences_at_the_read_limit_are_accepted() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let mut original = String::from("schema=1\nauthor.name=At limit\n#");
        original.extend(std::iter::repeat_n('x', MAX_BYTES - original.len()));
        fs::write(store.path(), original)?;
        let loaded = store.load(Locale::En);
        assert!(loaded.writable);
        assert!(loaded.notices.is_empty());
        assert_eq!(loaded.prefs.author_name, "At limit");
        Ok(())
    }

    #[test]
    fn oversized_save_neither_replaces_existing_config_nor_creates_a_directory() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        for exists in [true, false] {
            let store =
                PrefsStore::new(dir.path().join(if exists { "existing" } else { "missing" }));
            let mut loaded = store.load(Locale::En);
            if exists {
                store.save(&loaded)?;
            }
            let original = fs::read(store.path()).ok();
            loaded.prefs.author_name = "x".repeat(MAX_BYTES);
            assert_eq!(
                store.save(&loaded).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(fs::read(store.path()).ok(), original);
            assert_eq!(store.dir().exists(), exists);
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_preferences_cannot_be_replaced_with_defaults() -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let original = "schema=1\nauthor.name=Preserve me\n";
        fs::write(store.path(), original)?;
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o000))?;
        let denied = fs::File::open(store.path()).is_err();
        let loaded = store.load(Locale::En);
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o600))?;
        // Privileged runners may bypass mode bits; the directory-error test
        // below still verifies the read-failure contract on those runners.
        if denied {
            assert!(!loaded.writable);
            assert!(store.save(&loaded).is_err());
            assert_eq!(fs::read_to_string(store.path())?, original);
        }
        Ok(())
    }

    #[test]
    fn unreadable_config_path_is_not_treated_as_a_missing_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        fs::create_dir(store.path())?;
        let loaded = store.load(Locale::En);
        assert!(!loaded.writable);
        assert!(matches!(
            &loaded.notices[..],
            [PrefsNotice::Unreadable { .. }]
        ));
        assert!(store.save(&loaded).is_err());
        assert!(store.path().is_dir());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn dangling_config_link_is_preserved_read_only() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let missing = dir.path().join("missing.conf");
        std::os::unix::fs::symlink(&missing, store.path())?;
        let loaded = store.load(Locale::En);
        assert!(!loaded.writable);
        assert!(store.save(&loaded).is_err());
        assert_eq!(fs::read_link(store.path())?, missing);
        assert!(!missing.exists());
        Ok(())
    }

    #[test]
    fn legacy_preferences_are_read_only_and_active_preferences_win() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy = dir.path().join(FILE_NAME);
        let old = "schema=1\nui.locale=en\nauthor.name=R8 reviewer\nfuture.key=retained\n";
        fs::write(&legacy, old)?;
        let store = PrefsStore::new(dir.path().join("migration-preview"));
        let mut loaded = store.load_with_legacy(dir.path(), Locale::ZhCn);
        assert_eq!(loaded.prefs.ui_locale, Locale::En);
        assert_eq!(loaded.prefs.author_name, "R8 reviewer");
        assert!(!store.path().exists());
        loaded.prefs.ui_locale = Locale::ZhCn;
        store.save(&loaded)?;
        let active = store.load_with_legacy(dir.path(), Locale::En);
        assert_eq!(active.prefs.ui_locale, Locale::ZhCn);
        assert_eq!(
            active.prefs.unknown,
            [("future.key".into(), "retained".into())]
        );
        assert_eq!(fs::read_to_string(&legacy)?, old);
        Ok(())
    }

    #[test]
    fn exhausted_backup_names_preserve_original_and_existing_backups() {
        let dir = tempfile::tempdir().unwrap();
        let store = PrefsStore::new(dir.path());
        fs::write(store.path(), [0xff, 0xfe]).unwrap();
        for n in 0..1024 {
            fs::write(
                dir.path().join(format!("preferences.corrupt-{n}.conf")),
                "keep",
            )
            .unwrap();
        }
        let loaded = store.load(Locale::ZhCn);
        assert!(!loaded.writable);
        assert!(store.save(&loaded).is_err());
        assert_eq!(fs::read(store.path()).unwrap(), [0xff, 0xfe]);
        assert_eq!(
            fs::read_to_string(dir.path().join("preferences.corrupt-1023.conf")).unwrap(),
            "keep"
        );
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("joyword-prefs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trip_keeps_values_unknown_keys_and_escapes() {
        let dir = temp_dir("roundtrip");
        let store = PrefsStore::new(&dir);
        let first = store.load(Locale::ZhCn);
        assert_eq!(first.prefs, Preferences::defaults_for(Locale::ZhCn));
        let mut loaded = first;
        loaded.prefs.author_name = "张 三\\审阅".into();
        loaded.prefs.autosave_minutes = 5;
        loaded.prefs.add_recent(PathBuf::from("/合同/a.docx"));
        loaded.prefs.add_recent(PathBuf::from("/合同/b\nc.docx"));
        loaded.prefs.add_recent(PathBuf::from("/合同/a.docx"));
        loaded
            .prefs
            .unknown
            .push(("future.flag".into(), "on".into()));
        store.save(&loaded).unwrap();
        let again = store.load(Locale::En);
        assert_eq!(again.prefs, loaded.prefs);
        assert!(again.notices.is_empty());
        assert_eq!(again.prefs.recent[0], PathBuf::from("/合同/a.docx"));
        assert_eq!(again.prefs.initials(), "张三");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_files_fall_back_to_defaults_and_are_kept() {
        let dir = temp_dir("corrupt");
        let store = PrefsStore::new(&dir);
        fs::write(store.path(), b"\xff\xfe not utf-8").unwrap();
        let loaded = store.load(Locale::En);
        assert_eq!(loaded.prefs, Preferences::defaults_for(Locale::En));
        let PrefsNotice::Corrupt { backup } = &loaded.notices[0] else {
            panic!("expected a corrupt notice")
        };
        assert_eq!(fs::read(backup).unwrap(), b"\xff\xfe not utf-8");
        assert!(loaded.writable);
        fs::write(store.path(), "schema=1\nno separator line\n").unwrap();
        assert!(matches!(
            store.load(Locale::En).notices[0],
            PrefsNotice::Corrupt { .. }
        ));
        fs::write(store.path(), "schema=1\nbad=\\q\n").unwrap();
        assert!(matches!(
            store.load(Locale::En).notices[0],
            PrefsNotice::Corrupt { .. }
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_values_newer_schemas_and_migration() {
        let dir = temp_dir("schemas");
        let store = PrefsStore::new(&dir);
        fs::write(
            store.path(),
            "schema=1\nunits=furlong\nautosave.minutes=999\nauthor.name=A\n",
        )
        .unwrap();
        let loaded = store.load(Locale::ZhCn);
        assert_eq!(loaded.prefs.units, Units::Centimeters);
        assert_eq!(loaded.prefs.autosave_minutes, 10);
        assert_eq!(loaded.prefs.author_name, "A");
        assert_eq!(loaded.notices.len(), 2);

        fs::write(store.path(), "schema=7\nauthor.name=Future\n").unwrap();
        let newer = store.load(Locale::ZhCn);
        assert_eq!(newer.prefs.author_name, "Future");
        assert!(newer.notices.contains(&PrefsNotice::Newer { schema: 7 }));
        assert!(store.save(&newer).is_err());
        assert_eq!(
            fs::read_to_string(store.path()).unwrap(),
            "schema=7\nauthor.name=Future\n"
        );

        fs::write(store.path(), "author=Legacy Name\nunit=in\n").unwrap();
        let migrated = store.load(Locale::ZhCn);
        assert_eq!(migrated.prefs.author_name, "Legacy Name");
        assert_eq!(migrated.prefs.units, Units::Inches);
        assert_eq!(migrated.prefs.initials(), "LN");
        assert!(
            migrated
                .notices
                .contains(&PrefsNotice::Migrated { from: 0 })
        );
        store.save(&migrated).unwrap();
        assert!(
            fs::read_to_string(store.path())
                .unwrap()
                .contains("schema=1\n")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn fifo_preferences_do_not_block_or_become_writable() -> io::Result<()> {
        use rustix::fs::{CWD, Mode, mkfifoat};
        use std::os::unix::fs::FileTypeExt;
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        mkfifoat(CWD, store.path(), Mode::RUSR | Mode::WUSR)?;
        let loaded = store.load(Locale::En);
        assert!(!loaded.writable);
        assert!(matches!(
            loaded.notices.as_slice(),
            [PrefsNotice::Unreadable { .. }]
        ));
        assert!(store.save(&loaded).is_err());
        assert!(fs::symlink_metadata(store.path())?.file_type().is_fifo());
        assert_eq!(fs::read_dir(dir.path())?.count(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn legacy_fifo_preferences_fall_back_without_touching_the_legacy_path() -> io::Result<()> {
        use rustix::fs::{CWD, Mode, mkfifoat};
        use std::os::unix::fs::FileTypeExt;
        let legacy = tempfile::tempdir()?;
        let current = tempfile::tempdir()?;
        let path = legacy.path().join(FILE_NAME);
        mkfifoat(CWD, &path, Mode::RUSR | Mode::WUSR)?;
        let store = PrefsStore::new(current.path());
        let loaded = store.load_with_legacy(legacy.path(), Locale::En);
        assert_eq!(loaded.prefs, Preferences::defaults_for(Locale::En));
        assert!(loaded.writable); // only the missing current file may be created
        assert!(!store.path().exists());
        assert!(fs::symlink_metadata(&path)?.file_type().is_fifo());
        assert_eq!(fs::read_dir(legacy.path())?.count(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn regular_preferences_symlink_keeps_existing_read_compatibility() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let target = dir.path().join("regular.conf");
        fs::write(&target, "schema=1\nauthor.name=Reader\n")?;
        let store = PrefsStore::new(dir.path());
        std::os::unix::fs::symlink(&target, store.path())?;
        let loaded = store.load(Locale::En);
        assert!(loaded.writable);
        assert_eq!(loaded.prefs.author_name, "Reader");
        assert_eq!(
            fs::read_to_string(&target)?,
            "schema=1\nauthor.name=Reader\n"
        );
        Ok(())
    }

    #[test]
    fn stale_writable_preferences_cannot_overwrite_a_newer_schema() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let mut loaded = store.load(Locale::En);
        store.save(&loaded)?;
        loaded.prefs.author_name = "old editor".into();
        let future = "schema=99\nfuture.important=keep\n";
        fs::write(store.path(), future)?;
        assert!(loaded.writable);
        assert!(store.save(&loaded).is_err());
        assert_eq!(fs::read_to_string(store.path())?, future);
        assert_eq!(fs::read_dir(dir.path())?.count(), 2);
        Ok(())
    }

    #[test]
    fn save_guard_recognizes_future_schema_even_when_new_syntax_is_unknown() -> io::Result<()> {
        for future in [
            "schema=99\nnew syntax without equals\n",
            "schema=1\nschema=99\n",
        ] {
            let dir = tempfile::tempdir()?;
            let store = PrefsStore::new(dir.path());
            let loaded = store.load(Locale::En);
            fs::write(store.path(), future)?;
            assert!(store.save(&loaded).is_err());
            assert_eq!(fs::read_to_string(store.path())?, future);
            assert_eq!(fs::read_dir(dir.path())?.count(), 1);
        }
        Ok(())
    }

    #[test]
    fn stale_same_schema_save_keeps_completed_external_changes() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        fs::write(store.path(), "schema=1\nauthor.name=First\n")?;
        let mut stale = store.load(Locale::En);
        let mut other = store.load(Locale::En);
        other.prefs.author_name = "Second".into();
        store.save(&other)?;
        let external = fs::read(store.path())?;
        stale.prefs.author_name = "Stale".into();
        assert!(store.save(&stale).is_err());
        assert_eq!(fs::read(store.path())?, external);
        Ok(())
    }

    #[test]
    fn successful_saves_advance_only_the_current_sessions_baseline() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let mut active = store.load(Locale::En);
        let mut stale = active.clone();
        active.prefs.author_name = "One".into();
        store.save(&active)?;
        active.prefs.author_name = "Two".into();
        store.save(&active)?;
        let expected = fs::read(store.path())?;
        stale.prefs.author_name = "Old".into();
        assert!(store.save(&stale).is_err());
        assert_eq!(fs::read(store.path())?, expected);
        assert_eq!(fs::read_dir(dir.path())?.count(), 2);
        Ok(())
    }

    #[test]
    fn changed_deleted_or_cross_store_preferences_are_not_overwritten() -> io::Result<()> {
        for mode in 0..3 {
            let dir = tempfile::tempdir()?;
            let store = PrefsStore::new(dir.path());
            let original = "schema=1\nauthor.name=One\n";
            fs::write(store.path(), original)?;
            let loaded = store.load(Locale::En);
            match mode {
                0 => {
                    let changed = "schema=1\nauthor.name=Two\n";
                    assert_eq!(original.len(), changed.len());
                    fs::write(store.path(), changed)?;
                    assert!(store.save(&loaded).is_err());
                    assert_eq!(fs::read_to_string(store.path())?, changed);
                }
                1 => {
                    fs::remove_file(store.path())?;
                    assert!(store.save(&loaded).is_err());
                    assert!(!store.path().exists());
                }
                _ => {
                    let other = PrefsStore::new(dir.path().join("other"));
                    assert!(other.save(&loaded).is_err());
                    assert!(!other.dir().exists());
                }
            }
        }
        Ok(())
    }

    // Subprocess entry: no environment variable means an ordinary no-op test.
    #[test]
    fn preference_lock_child() -> io::Result<()> {
        let Some(directory) = std::env::var_os("JOYWORD_TEST_PREF_LOCK_DIR") else {
            return Ok(());
        };
        let _lock = acquire_save_lock(Path::new(&directory))?;
        println!("PREFERENCE_LOCK_READY");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        Ok(())
    }

    #[test]
    fn separate_process_lock_refuses_save_and_crash_releases_it() -> io::Result<()> {
        use std::io::BufRead;
        use std::process::{Command, Stdio};
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let original = "schema=1\nauthor.name=Before\n";
        fs::write(store.path(), original)?;
        let mut loaded = store.load(Locale::En);
        loaded.prefs.author_name = "After".into();
        let mut child = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "prefs::tests::preference_lock_child",
                "--nocapture",
            ])
            .env("JOYWORD_TEST_PREF_LOCK_DIR", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing child stdout"))?;
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in io::BufReader::new(stdout).lines().map_while(Result::ok) {
                if line.contains("PREFERENCE_LOCK_READY") {
                    let _ = send.send(());
                    break;
                }
            }
        });
        if receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::other("lock subprocess did not become ready"));
        }
        let result = store.save(&loaded);
        child.kill()?;
        child.wait()?;
        assert_eq!(
            result.err().map(|e| e.kind()),
            Some(io::ErrorKind::WouldBlock)
        );
        assert_eq!(fs::read_to_string(store.path())?, original);
        store.save(&loaded)?;
        assert_eq!(store.load(Locale::En).prefs.author_name, "After");
        assert!(dir.path().join(SAVE_LOCK).is_file());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_save_lock_paths_refuse_without_touching_preferences() -> io::Result<()> {
        use rustix::fs::{CWD, Mode, mkfifoat};
        for mode in 0..3 {
            let dir = tempfile::tempdir()?;
            let store = PrefsStore::new(dir.path());
            let original = "schema=1\nauthor.name=Keep\n";
            fs::write(store.path(), original)?;
            let loaded = store.load(Locale::En);
            let lock = dir.path().join(SAVE_LOCK);
            match mode {
                0 => std::os::unix::fs::symlink(store.path(), &lock)?,
                1 => mkfifoat(CWD, &lock, Mode::RUSR | Mode::WUSR)?,
                _ => fs::create_dir(&lock)?,
            }
            assert!(store.save(&loaded).is_err());
            assert_eq!(fs::read_to_string(store.path())?, original);
        }
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn corrupt_preferences_backup_does_not_grant_group_or_other_access() -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir()?;
        let store = PrefsStore::new(dir.path());
        let original = b"private damaged preferences without an equals sign";
        fs::write(store.path(), original)?;
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o600))?;
        let loaded = store.load(Locale::En);
        assert!(loaded.writable);
        let [PrefsNotice::Corrupt { backup }] = &loaded.notices[..] else {
            return Err(io::Error::other("expected corruption backup"));
        };
        assert_eq!(fs::read(backup)?, original);
        assert_eq!(fs::read(store.path())?, original);
        assert_eq!(fs::metadata(backup)?.permissions().mode() & 0o077, 0);
        Ok(())
    }
    #[test]
    fn future_preferences_remain_read_only_even_with_unrecognized_syntax() -> io::Result<()> {
        for original in [
            "schema=99\nfuture syntax without equals\n",
            "schema=1\nauthor.name=Future\nschema=99\n",
        ] {
            let dir = tempfile::tempdir()?;
            let store = PrefsStore::new(dir.path());
            fs::write(store.path(), original)?;
            let loaded = store.load(Locale::En);
            assert!(!loaded.writable);
            assert!(loaded.notices.contains(&PrefsNotice::Newer { schema: 99 }));
            assert_eq!(fs::read_to_string(store.path())?, original);
            assert_eq!(fs::read_dir(dir.path())?.count(), 1);
            assert!(store.save(&loaded).is_err());
        }
        Ok(())
    }

    #[test]
    fn legacy_preferences_with_a_future_declaration_are_not_migrated() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy = dir.path().join("legacy");
        fs::create_dir(&legacy)?;
        let original = "schema=1\nauthor.name=Future owner\nschema=99\n";
        fs::write(legacy.join(FILE_NAME), original)?;
        let store = PrefsStore::new(dir.path().join("current"));
        let loaded = store.load_with_legacy(&legacy, Locale::En);
        assert_eq!(loaded.prefs, Preferences::defaults_for(Locale::En));
        assert_eq!(fs::read_to_string(legacy.join(FILE_NAME))?, original);
        assert!(!store.path().exists());
        Ok(())
    }
}
