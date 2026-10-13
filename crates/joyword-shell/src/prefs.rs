//! User identity and preferences (PLATFORM-008, T106).
//!
//! Stored as UTF-8 `key=value` lines with a `schema` version. Loading never
//! fails: a damaged file falls back to defaults, is kept beside the original
//! name and is reported; a file from a newer JoyWord is used read-only so an
//! older build never overwrites it; legacy keys are migrated. Unknown keys
//! are kept and written back. Saving replaces the file atomically.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use crate::l10n::Locale;

pub const SCHEMA: u32 = 1;
const FILE_NAME: &str = "preferences.conf";
const MAX_RECENT: usize = 10;

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

    fn parse(tag: &str) -> Option<Self> {
        match tag {
            "cm" => Some(Units::Centimeters),
            "mm" => Some(Units::Millimeters),
            "in" | "inch" => Some(Units::Inches),
            "pt" => Some(Units::Points),
            _ => None,
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
}

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

    /// Loads preferences; see the module documentation. `fallback` chooses
    /// defaults (normally the system UI language).
    pub fn load(&self, fallback: Locale) -> Loaded {
        let path = self.path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                return Loaded {
                    prefs: Preferences::defaults_for(fallback),
                    notices: Vec::new(),
                    writable: true,
                };
            }
        };
        let parsed = std::str::from_utf8(&bytes)
            .map_err(|_| ())
            .and_then(|text| parse(text, fallback));
        match parsed {
            Ok((prefs, notices, schema)) if schema > SCHEMA => {
                let mut notices = notices;
                notices.push(PrefsNotice::Newer { schema });
                Loaded {
                    prefs,
                    notices,
                    writable: false,
                }
            }
            Ok((prefs, notices, _)) => Loaded {
                prefs,
                notices,
                writable: true,
            },
            Err(()) => {
                let backup = self.backup_path();
                // Keep the damaged file; if even copying fails, refuse to
                // overwrite it later.
                let copied = fs::write(&backup, &bytes).is_ok();
                Loaded {
                    prefs: Preferences::defaults_for(fallback),
                    notices: vec![PrefsNotice::Corrupt { backup }],
                    writable: copied,
                }
            }
        }
    }

    fn backup_path(&self) -> PathBuf {
        (0..)
            .map(|n| self.dir.join(format!("preferences.corrupt-{n}.conf")))
            .find(|candidate| !candidate.exists())
            .expect("an unused backup name exists")
    }

    /// Writes atomically: a temporary file in the same directory, flushed,
    /// then renamed over the original.
    pub fn save(&self, loaded: &Loaded) -> io::Result<()> {
        if !loaded.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "preferences belong to a newer JoyWord or could not be backed up",
            ));
        }
        fs::create_dir_all(&self.dir)?;
        let temporary = self.dir.join(format!(".{FILE_NAME}.tmp"));
        {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(serialize(&loaded.prefs).as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&temporary, self.path())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
