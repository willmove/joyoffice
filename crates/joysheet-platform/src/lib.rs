//! Safe native services and Joysheet-specific settings storage.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub mod printing;
pub mod windows;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub dark: bool,
    pub language: String,
    pub recent: Vec<String>,
    pub autosave: bool,
    pub formula_bar: bool,
    pub autocomplete: bool,
    pub recovery: bool,
    pub animations: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            dark: false,
            language: "zh".into(),
            recent: Vec::new(),
            autosave: false,
            formula_bar: true,
            autocomplete: true,
            recovery: true,
            animations: false,
        }
    }
}
pub fn config_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("JOYSHEET_CONFIG_DIR").filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    if cfg!(target_os = "windows")
        && let Some(path) = std::env::var_os("LOCALAPPDATA").filter(|p| !p.is_empty())
    {
        PathBuf::from(path).join("Joysheet")
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Library/Application Support/Joysheet")
    } else if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()) {
        PathBuf::from(path).join("joysheet")
    } else {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join(".config").join("joysheet")
    }
}

/// An explicit allowlist prevents dropping executable files or folders into the document loader.
pub fn workbook_paths(paths: impl IntoIterator<Item = PathBuf>) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    for path in paths.into_iter().take(33) {
        if result.len() == 32 {
            return Err("Open at most 32 workbooks at once".into());
        }
        if !path.is_file()
            || !path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                ["xlsx", "xlsm", "xltx", "xltm", "xls", "xlsb", "ods", "csv", "tsv", "txt", "json"].iter().any(|v| e.eq_ignore_ascii_case(v))
            })
        {
            return Err(format!("Unsupported workbook: {}", path.display()));
        }
        result.push(path.to_string_lossy().into_owned());
    }
    if result.is_empty() {
        return Err("No workbook files supplied".into());
    }
    Ok(result)
}
pub fn local_open_path(value: &str) -> Result<PathBuf, String> {
    if value.starts_with("file:") {
        url::Url::parse(value).map_err(|e| e.to_string())?.to_file_path().map_err(|_| "Invalid file URL".into())
    } else if value.contains("://") {
        Err("Only local workbook file URLs are supported".into())
    } else {
        Ok(PathBuf::from(value))
    }
}
impl Preferences {
    pub fn load() -> Self {
        std::fs::read(config_dir().join("preferences.json")).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }
    pub fn save(&self) -> Result<(), String> {
        let dir = config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let temporary = dir.join(format!("preferences-{}.tmp", std::process::id()));
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, dir.join("preferences.json")).map_err(|e| e.to_string())
    }
    pub fn remember(&mut self, path: String) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(20);
    }
}
pub async fn pick_open() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Joysheet — 打开工作簿 / Open workbook")
        .add_filter("Workbooks", &["xlsx", "xlsm", "xls", "xlsb", "ods", "csv", "tsv", "json"])
        .pick_file()
        .await
        .map(|file| file.path().to_path_buf())
}
pub async fn pick_save(name: &str) -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Joysheet — 另存为 / Save as")
        .set_file_name(name)
        .add_filter("Excel workbook", &["xlsx"])
        .add_filter("CSV", &["csv"])
        .add_filter("PDF", &["pdf"])
        .save_file()
        .await
        .map(|file| file.path().to_path_buf())
}
#[derive(Default)]
pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
}
// Windows on the same engine share its rich clipboard, even with separate native handles.
static LAST_WRITTEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
impl Clipboard {
    fn native(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.inner.is_none() {
            self.inner = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        self.inner.as_mut().ok_or_else(|| "clipboard unavailable".into())
    }
    pub fn write(&mut self, text: &str, html: Option<&str>) -> Result<(), String> {
        match html {
            Some(html) => self.native()?.set_html(html, Some(text)).map_err(|e| e.to_string())?,
            None => self.native()?.set_text(text).map_err(|e| e.to_string())?,
        }
        *LAST_WRITTEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(text.into());
        Ok(())
    }
    pub fn read_external(&mut self) -> Option<String> {
        let text = self.native().ok()?.get_text().ok()?;
        // The engine's richer clipboard is authoritative while the OS still holds our copy.
        if LAST_WRITTEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref() == Some(&text) { None } else { Some(text) }
    }
}
