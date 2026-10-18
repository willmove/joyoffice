//! Application diagnostics (QUALITY-007, T211).
//!
//! Each launch creates a new log file. Older launches stay on disk until a
//! later launch has synced its own file and the retention count is exceeded.
//! A crash does not truncate or replace the previous file. Every file stops
//! accepting records at a byte cap; a record that would pass the cap is
//! refused whole.
//!
//! Records are redacted before they touch the file. Document body text,
//! tracked-change text, comment text and credential material are not
//! diagnostic fields. Callers pass event codes plus an optional untrusted
//! string; the untrusted string is scanned under a byte budget and rewritten.
//! The panic hook stores the source location and a backtrace, and runs the
//! same redaction on the payload and frames.
//!
//! `JOYWORD_PROFILE=test` selects a configuration root that is not the user
//! preferences directory. Preferences and logs both live under that root.
//! An unknown profile is an error, not a silent fall back to the user tree.
//!
//! No third-party logger. The hook is the standard library panic hook; the
//! backtrace is [`std::backtrace::Backtrace::force_capture`]. File creation
//! uses [`std::fs::File::create_new`] so an existing launch cannot be
//! truncated. Sources are the Rust std library docs for those APIs.

use std::fs::{self, File};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Bytes scanned from one untrusted payload or panic message.
pub const MAX_UNTRUSTED_BYTES: usize = 8 * 1024;
/// Default per-file cap, including the header.
pub const DEFAULT_LOG_CAP: u64 = 256 * 1024;
/// Active file plus earlier launches kept after a successful open.
pub const RETAINED_LOGS: usize = 5;
/// Matching names considered in one directory. Beyond this, open refuses
/// rather than guessing which file to delete.
const MAX_TRACKED_LOGS: usize = 256;
const MAX_WITHHELD: usize = 32;
const MAX_WITHHELD_BYTES: usize = 256;
const MIN_WITHHELD_BYTES: usize = 8;
const MAX_BACKTRACE_LINES: usize = 48;

const CREDENTIAL_KEYS: &[&str] = &[
    "authorization",
    "credentials",
    "credential",
    "password",
    "api_key",
    "apikey",
    "passwd",
    "secret",
    "session",
    "token",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
    Info,
}

impl Level {
    fn tag(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    User,
    Test,
}

impl Profile {
    pub fn tag(self) -> &'static str {
        match self {
            Profile::User => "user",
            Profile::Test => "test",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilePaths {
    pub profile: Profile,
    pub config_dir: PathBuf,
    pub log_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// `JOYWORD_PROFILE` was not `user` or `test`.
    Unknown,
    /// The user profile has no home or platform config directory.
    UserDirectoryUnavailable,
    /// The test root is the user configuration directory.
    TestProfileNotIsolated,
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ProfileError::Unknown => "unknown diagnostics profile",
            ProfileError::UserDirectoryUnavailable => "user configuration directory is unavailable",
            ProfileError::TestProfileNotIsolated => {
                "test profile is not isolated from user configuration"
            }
        })
    }
}

impl std::error::Error for ProfileError {}

/// Structured diagnostic. `event` and `kind` are caller-chosen codes, not
/// document text. Invalid codes are refused and nothing is written.
#[derive(Clone, Copy, Debug)]
pub struct Event<'a> {
    pub level: Level,
    pub event: &'a str,
    pub kind: Option<&'a str>,
    pub count: Option<u64>,
}

#[derive(Clone, Copy, Debug)]
pub struct PanicReport<'a> {
    pub file: Option<&'a str>,
    pub line: u32,
    pub column: u32,
    pub message: Option<&'a str>,
    pub backtrace: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WithholdError {
    TooShort,
    TooLong,
    Full,
}

impl std::fmt::Display for WithholdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            WithholdError::TooShort => "withheld diagnostic string is too short",
            WithholdError::TooLong => "withheld diagnostic string exceeds the budget",
            WithholdError::Full => "withheld diagnostic list is full",
        })
    }
}

impl std::error::Error for WithholdError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookError {
    AlreadyInstalled,
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("panic hook is already installed")
    }
}

impl std::error::Error for HookError {}

struct LogState {
    file: File,
    path: PathBuf,
    cap: u64,
    cap_noted: bool,
    withheld: Vec<String>,
}

/// Size-capped launch log. Cloning the handle shares the file.
pub struct AppLog {
    inner: Mutex<LogState>,
}

impl std::fmt::Debug for AppLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.lock();
        f.debug_struct("AppLog")
            .field("path", &state.path)
            .field("cap", &state.cap)
            .finish()
    }
}

/// Reads `JOYWORD_PROFILE` and `JOYWORD_PROFILE_DIR`.
pub fn resolve() -> Result<ProfilePaths, ProfileError> {
    let spec = std::env::var("JOYWORD_PROFILE").ok();
    let test_root = std::env::var_os("JOYWORD_PROFILE_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    resolve_from(
        spec.as_deref(),
        test_root.as_deref(),
        crate::prefs::default_config_dir().as_deref(),
    )
}

/// Pure profile selection. `spec` is the profile name, `test_root` is the
/// isolated directory used for `test`, and `user_dir` is the real preferences
/// directory. The test profile never returns `user_dir`.
pub fn resolve_from(
    spec: Option<&str>,
    test_root: Option<&Path>,
    user_dir: Option<&Path>,
) -> Result<ProfilePaths, ProfileError> {
    let profile = match spec.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("user") => Profile::User,
        Some("test") => Profile::Test,
        Some(_) => return Err(ProfileError::Unknown),
    };
    let config_dir = match profile {
        Profile::User => user_dir
            .ok_or(ProfileError::UserDirectoryUnavailable)?
            .to_path_buf(),
        Profile::Test => {
            let root = test_root
                .filter(|path| !path.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(isolated_test_root);
            if user_dir.is_some_and(|user| user == root) {
                return Err(ProfileError::TestProfileNotIsolated);
            }
            root
        }
    };
    let log_dir = config_dir.join("logs");
    Ok(ProfilePaths {
        profile,
        config_dir,
        log_dir,
    })
}

fn isolated_test_root() -> PathBuf {
    std::env::temp_dir().join(format!("joyword-test-profile-{}", std::process::id()))
}

impl AppLog {
    pub fn open(dir: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_capped(dir, DEFAULT_LOG_CAP)
    }

    pub fn open_capped(dir: impl AsRef<Path>, cap: u64) -> io::Result<Self> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir)?;
        let existing = existing_logs(dir)?;
        let header = format_line(Level::Info, "log.opened", Some("cap"), Some(cap), None);
        if (header.len() as u64).saturating_add(1) > cap {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "log cap is smaller than the header",
            ));
        }
        let (path, file) = create_launch_file(dir, &existing, cap, &header)?;
        let failed = retain_older(&existing);
        let log = Self {
            inner: Mutex::new(LogState {
                file,
                path,
                cap,
                cap_noted: false,
                withheld: Vec::new(),
            }),
        };
        if failed > 0 {
            let _ = log.record(Event {
                level: Level::Error,
                event: "log.retention",
                kind: Some("failed"),
                count: Some(failed),
            });
        }
        Ok(log)
    }

    pub fn path(&self) -> PathBuf {
        self.lock().path.clone()
    }

    /// Remembers an exact string that must not appear in later records or
    /// panic output. The string itself is not written to the log.
    pub fn withhold(&self, text: &str) -> Result<(), WithholdError> {
        if text.len() < MIN_WITHHELD_BYTES {
            return Err(WithholdError::TooShort);
        }
        if text.len() > MAX_WITHHELD_BYTES {
            return Err(WithholdError::TooLong);
        }
        let mut state = self.lock();
        if state.withheld.iter().any(|kept| kept == text) {
            return Ok(());
        }
        if state.withheld.len() >= MAX_WITHHELD {
            return Err(WithholdError::Full);
        }
        state.withheld.push(text.to_owned());
        Ok(())
    }

    pub fn record(&self, event: Event<'_>) -> io::Result<()> {
        self.write(event, None)
    }

    /// Writes `event` and a redacted form of `untrusted`. The original
    /// `untrusted` bytes are not written.
    pub fn record_untrusted(&self, level: Level, event: &str, untrusted: &str) -> io::Result<()> {
        let state = self.lock();
        let detail = redact_with(&state.withheld, untrusted);
        drop(state);
        self.write(
            Event {
                level,
                event,
                kind: None,
                count: None,
            },
            Some(detail),
        )
    }

    pub fn record_panic_report(&self, report: PanicReport<'_>) -> io::Result<()> {
        let state = self.lock();
        let file = report.file.map(|file| redact_with(&state.withheld, file));
        let message = report
            .message
            .map(|message| redact_with(&state.withheld, message));
        let mut frames = Vec::new();
        for line in bound(report.backtrace).0.lines() {
            if frames.len() == MAX_BACKTRACE_LINES {
                break;
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let redacted = redact_with(&state.withheld, line);
            if !redacted.is_empty() {
                frames.push(redacted);
            }
        }
        drop(state);
        self.write(
            Event {
                level: Level::Error,
                event: "panic",
                kind: Some("location"),
                count: Some(u64::from(report.line)),
            },
            None,
        )?;
        let mut location = format!("column={}", report.column);
        if let Some(file) = file {
            location = format!("file={file} {location}");
        }
        self.write(
            Event {
                level: Level::Error,
                event: "panic.location",
                kind: Some("source"),
                count: Some(u64::from(report.column)),
            },
            Some(location),
        )?;
        match message {
            Some(message) => self.write(
                Event {
                    level: Level::Error,
                    event: "panic.message",
                    kind: None,
                    count: None,
                },
                Some(message),
            )?,
            None => self.write(
                Event {
                    level: Level::Error,
                    event: "panic.message",
                    kind: Some("non_string"),
                    count: None,
                },
                None,
            )?,
        }
        for (index, frame) in frames.iter().enumerate() {
            self.write(
                Event {
                    level: Level::Error,
                    event: "panic.frame",
                    kind: Some("trace"),
                    count: Some((index as u64).saturating_add(1)),
                },
                Some(frame.clone()),
            )?;
        }
        Ok(())
    }

    fn write(&self, event: Event<'_>, detail: Option<String>) -> io::Result<()> {
        if !valid_code(event.event) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid diagnostic event",
            ));
        }
        if event.kind.is_some_and(|kind| !valid_kind(kind)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid diagnostic kind",
            ));
        }
        let line = format_line(
            event.level,
            event.event,
            event.kind,
            event.count,
            detail.as_deref(),
        );
        let mut state = self.lock();
        let cap = state.cap;
        match write_line(&mut state.file, cap, &line) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::StorageFull => {
                if !state.cap_noted {
                    state.cap_noted = true;
                    let note =
                        format_line(Level::Error, "log.capped", Some("cap"), Some(cap), None);
                    let _ = write_line(&mut state.file, cap, &note);
                }
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    fn lock(&self) -> MutexGuard<'_, LogState> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

/// Installs the process panic hook. A second call returns
/// [`HookError::AlreadyInstalled`] and leaves the first hook in place.
pub fn install_panic_hook(log: Arc<AppLog>) -> Result<(), HookError> {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    if INSTALLED.set(()).is_err() {
        return Err(HookError::AlreadyInstalled);
    }
    std::panic::set_hook(Box::new(move |info| {
        let message = panic_payload(info);
        let backtrace = std::backtrace::Backtrace::force_capture();
        let (file, line, column) = match info.location() {
            Some(location) => (Some(location.file()), location.line(), location.column()),
            None => (None, 0, 0),
        };
        let _ = log.record_panic_report(PanicReport {
            file,
            line,
            column,
            message: message.as_deref(),
            backtrace: &backtrace.to_string(),
        });
        eprintln!("JoyWord panic; see the application log");
    }));
    Ok(())
}

fn panic_payload(info: &std::panic::PanicHookInfo) -> Option<String> {
    let text = if let Some(message) = info.payload().downcast_ref::<&str>() {
        *message
    } else {
        info.payload().downcast_ref::<String>()?.as_str()
    };
    Some(bound(text).0)
}

/// Rewrites `text` so document prose, quoted text and credential values are
/// not preserved. Safe diagnostic tokens (codes, numbers, source paths and
/// symbols) can remain.
pub fn redact(text: &str) -> String {
    redact_with(&[], text)
}

fn redact_with(withheld: &[String], text: &str) -> String {
    let (mut limited, truncated) = bound(text);
    for needle in withheld_longest_first(withheld) {
        if needle.is_empty() {
            continue;
        }
        limited = limited.replace(needle, "<redacted>");
    }
    let mut out = redact_credentials(&limited);
    out = redact_quotes(&out);
    out = redact_prose(&out);
    if truncated {
        out.push_str(" <truncated>");
    }
    out
}

fn bound(text: &str) -> (String, bool) {
    if text.len() <= MAX_UNTRUSTED_BYTES {
        return (text.to_owned(), false);
    }
    let mut end = MAX_UNTRUSTED_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

fn withheld_longest_first(withheld: &[String]) -> Vec<&str> {
    let mut needles: Vec<&str> = withheld.iter().map(String::as_str).collect();
    needles.sort_by_key(|needle| std::cmp::Reverse(needle.len()));
    needles
}

fn redact_credentials(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < text.len() {
        if !text.is_char_boundary(index) {
            index += 1;
            continue;
        }
        if let Some((key_end, value_end)) = credential_span(text, bytes, index) {
            out.push_str(&text[index..key_end]);
            out.push_str("=<redacted>");
            index = value_end;
            continue;
        }
        if let Some(userinfo_end) = url_userinfo_end(text, index) {
            out.push_str("<redacted>");
            index = userinfo_end;
            continue;
        }
        let next = text[index..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
        out.push_str(&text[index..index + next]);
        index += next;
    }
    out
}

fn credential_span(text: &str, lower: &[u8], index: usize) -> Option<(usize, usize)> {
    if index > 0 {
        let previous = text[..index].chars().next_back()?;
        if previous.is_ascii_alphanumeric() || previous == '_' {
            return None;
        }
    }
    let key = CREDENTIAL_KEYS
        .iter()
        .find(|key| lower.get(index..index + key.len()) == Some(key.as_bytes()))?;
    let mut cursor = index + key.len();
    if text
        .as_bytes()
        .get(cursor)
        .is_some_and(u8::is_ascii_alphanumeric)
    {
        return None;
    }
    cursor = skip_ascii_space(text, cursor);
    let separator = *text.as_bytes().get(cursor)?;
    if separator != b'=' && separator != b':' {
        return None;
    }
    cursor = skip_ascii_space(text, cursor + 1);
    let mut value_end = cursor;
    while value_end < text.len() && !text.as_bytes()[value_end].is_ascii_whitespace() {
        value_end += 1;
    }
    let value = &text[cursor..value_end];
    if value.eq_ignore_ascii_case("bearer") || value.eq_ignore_ascii_case("basic") {
        let after = skip_ascii_space(text, value_end);
        value_end = after;
        while value_end < text.len() && !text.as_bytes()[value_end].is_ascii_whitespace() {
            value_end += 1;
        }
    }
    Some((index + key.len(), value_end))
}

fn url_userinfo_end(text: &str, index: usize) -> Option<usize> {
    if !text[index..].starts_with("://") {
        return None;
    }
    let start = index + 3;
    let rest = &text[start..];
    let end = rest
        .find(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '/' | '?' | '#'))
        .map(|offset| start + offset)
        .unwrap_or(text.len());
    let authority = &text[start..end];
    let at = authority.find('@')?;
    if !authority[..at].contains(':') {
        return None;
    }
    Some(start + at)
}

fn skip_ascii_space(text: &str, mut index: usize) -> usize {
    while text
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    index
}

fn redact_quotes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let quoted = ch == '"' || (ch == '\'' && quote_starts(text, index));
        if !quoted {
            out.push(ch);
            continue;
        }
        let mut taken = 0usize;
        let mut closed = false;
        for (_, next) in chars.by_ref() {
            taken += 1;
            if next == ch || next == '\n' || taken == 512 {
                closed = next == ch;
                break;
            }
        }
        let _ = closed;
        out.push_str("<redacted>");
    }
    out
}

fn quote_starts(text: &str, index: usize) -> bool {
    let Some(previous) = text[..index].chars().next_back() else {
        return true;
    };
    previous.is_whitespace()
}

fn redact_prose(text: &str) -> String {
    let mut out = Vec::new();
    let mut run = Vec::new();
    for word in text.split_whitespace() {
        if word == "<redacted>" || word == "<truncated>" || is_safe_token(word) {
            run.push(word);
        } else {
            flush_run(&mut run, &mut out);
            out.push("<redacted>");
        }
    }
    flush_run(&mut run, &mut out);
    out.join(" ")
}

fn flush_run<'a>(run: &mut Vec<&'a str>, out: &mut Vec<&'a str>) {
    if run.len() >= 5 {
        out.push("<redacted>");
    } else {
        out.extend(run.iter().copied());
    }
    run.clear();
}

fn is_safe_token(word: &str) -> bool {
    if word.is_empty() || word.len() > 512 {
        return false;
    }
    if word.contains('/') || word.contains('\\') {
        return word
            .split(['/', '\\'])
            .all(|part| part.is_empty() || is_safe_component(part));
    }
    if !is_safe_component(word) {
        return false;
    }
    word.len() <= 24 || word.contains("::") || word.contains(".rs")
}

fn is_safe_component(word: &str) -> bool {
    if word.is_empty() || word.len() > 96 {
        return false;
    }
    if !word.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | ':' | '+' | '#' | '=' | '-')
    }) {
        return false;
    }
    word.len() <= 24 || word.contains('.') || word.contains(':')
}

fn valid_code(event: &str) -> bool {
    !event.is_empty()
        && event.len() <= 64
        && event.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_'
        })
}

fn valid_kind(kind: &str) -> bool {
    !kind.is_empty()
        && kind.len() <= 64
        && kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
}

fn format_line(
    level: Level,
    event: &str,
    kind: Option<&str>,
    count: Option<u64>,
    detail: Option<&str>,
) -> String {
    let mut line = format!(
        "ts={} level={} event={event} pid={}",
        unix_secs(),
        level.tag(),
        std::process::id()
    );
    if let Some(kind) = kind {
        line.push_str(" kind=");
        line.push_str(kind);
    }
    if let Some(count) = count {
        line.push_str(" count=");
        line.push_str(&count.to_string());
    }
    if let Some(detail) = detail {
        line.push_str(" detail=");
        line.push_str(detail);
    }
    line
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn write_line(file: &mut File, cap: u64, line: &str) -> io::Result<()> {
    let current = file.metadata()?.len();
    let extra = (line.len() as u64).saturating_add(1);
    if current.checked_add(extra).is_none_or(|next| next > cap) {
        return Err(io::Error::new(io::ErrorKind::StorageFull, "log file cap"));
    }
    file.seek(SeekFrom::End(0))?;
    if let Err(error) = file
        .write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
    {
        let _ = file.set_len(current);
        return Err(error);
    }
    if let Err(error) = file.sync_all() {
        let _ = file.set_len(current);
        return Err(error);
    }
    Ok(())
}

struct ExistingLog {
    index: u32,
    path: PathBuf,
}

fn existing_logs(dir: &Path) -> io::Result<Vec<ExistingLog>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(index) = log_index(&name) else {
            continue;
        };
        if found.len() == MAX_TRACKED_LOGS {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "too many log files",
            ));
        }
        found.push(ExistingLog {
            index,
            path: entry.path(),
        });
    }
    found.sort_by_key(|item| item.index);
    Ok(found)
}

fn log_index(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("joyword-")?.strip_suffix(".log")?;
    if rest.len() != 6 || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

fn create_launch_file(
    dir: &Path,
    existing: &[ExistingLog],
    cap: u64,
    header: &str,
) -> io::Result<(PathBuf, File)> {
    let mut index = next_index(existing)?;
    let mut tries = 0u32;
    loop {
        let path = dir.join(format!("joyword-{index:06}.log"));
        match File::create_new(&path) {
            Ok(mut file) => {
                if let Err(error) = write_line(&mut file, cap, header) {
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
                return Ok((path, file));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                tries += 1;
                if tries > 8 {
                    return Err(error);
                }
                index = index.saturating_add(1);
                if index == 0 || index > 999_999 {
                    return Err(io::Error::new(
                        io::ErrorKind::StorageFull,
                        "log index exhausted",
                    ));
                }
            }
            Err(error) => return Err(error),
        }
    }
}

fn next_index(existing: &[ExistingLog]) -> io::Result<u32> {
    let mut index = existing
        .last()
        .map(|item| item.index.saturating_add(1))
        .unwrap_or(1);
    if index == 0 || index > 999_999 {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "log index exhausted",
        ));
    }
    let mut tries = 0u32;
    while existing.iter().any(|item| item.index == index) || index > 999_999 {
        index = index.saturating_add(1);
        tries += 1;
        if tries > 8 || index > 999_999 {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "log index exhausted",
            ));
        }
    }
    Ok(index)
}

fn retain_older(existing: &[ExistingLog]) -> u64 {
    if existing.len() < RETAINED_LOGS {
        return 0;
    }
    let delete_count = existing.len() + 1 - RETAINED_LOGS;
    let mut failed = 0u64;
    for item in existing.iter().take(delete_count) {
        if fs::remove_file(&item.path).is_err() {
            failed += 1;
        }
    }
    failed
}
