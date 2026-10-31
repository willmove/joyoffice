//! Bounded, versioned per-user state. File locking also coordinates independent app instances.
use crate::document::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub schema: u32,
    pub generation: u64,
    pub recent: Vec<PathBuf>,
    pub zoom: f32,
    pub inspector: bool,
    pub notes: bool,
    pub width: u32,
    pub height: u32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema: 2,
            generation: 0,
            recent: vec![],
            zoom: 1.,
            inspector: false,
            notes: false,
            width: 1600,
            height: 900,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        if self.schema != 2
            || self.recent.len() > 12
            || !self.zoom.is_finite()
            || !(0.25..=3.).contains(&self.zoom)
            || !(800..=2560).contains(&self.width)
            || !(600..=1600).contains(&self.height)
            || self
                .recent
                .iter()
                .any(|p| !p.is_absolute() || p.as_os_str().len() > 4096)
        {
            return Err("偏好设置版本或范围无效".into());
        }
        Ok(())
    }
}
pub fn root() -> PathBuf {
    if let Some(p) = std::env::var_os("JOYDECK_STATE_DIR") {
        return p.into();
    }
    if std::env::args().any(|a| a.ends_with("-smoke") || a == "--smoke" || a == "--m4-bench") {
        return std::env::temp_dir().join(format!("joydeck-qa-state-{}", std::process::id()));
    }
    if cfg!(target_os = "windows") {
        return PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("JoyDeck");
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/JoyDeck")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("joydeck")
    }
}
fn lock(root: &Path) -> Result<File> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("preferences.lock"))
        .map_err(|e| e.to_string())?;
    file.lock().map_err(|e| e.to_string())?;
    Ok(file)
}
fn read(root: &Path) -> Result<Preferences> {
    let file = match File::open(root.join("preferences.json")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Preferences::default()),
        Err(e) => return Err(format!("preferences-io:{e}")),
    };
    let mut bytes = vec![];
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("preferences-io:{e}"))?;
    if bytes.len() > 65536 {
        return Err("偏好设置超过64KiB".into());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value["schema"].as_u64().is_some_and(|s| s > 2) {
        return Err("future-preferences-version".into());
    }
    if value["schema"] == 1 {
        value["schema"] = serde_json::json!(2);
    }
    let prefs: Preferences = serde_json::from_value(value).map_err(|e| e.to_string())?;
    prefs.validate()?;
    Ok(prefs)
}
fn load_locked(root: &Path) -> Result<(Preferences, Option<String>)> {
    match read(root) {
        Ok(p) => Ok((p, None)),
        Err(e) if e == "future-preferences-version" => {
            Err("偏好来自更新版本，已保留文件；本版本拒绝覆盖".into())
        }
        Err(e) if e.starts_with("preferences-io:") => Err(e),
        Err(_) => {
            let name = format!(
                "preferences-corrupt-{}.json",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            std::fs::rename(root.join("preferences.json"), root.join(name))
                .map_err(|e| e.to_string())?;
            Ok((
                Preferences::default(),
                Some("偏好文件损坏，原文件已保留；已恢复默认设置".into()),
            ))
        }
    }
}
pub fn load(root: &Path) -> Result<(Preferences, Option<String>)> {
    let _lock = lock(root)?;
    load_locked(root)
}
pub fn update(root: &Path, change: impl FnOnce(&mut Preferences)) -> Result<Preferences> {
    let _lock = lock(root)?;
    let (mut prefs, _) = load_locked(root)?;
    change(&mut prefs);
    prefs.generation = prefs.generation.checked_add(1).ok_or("偏好版本计数已满")?;
    prefs.validate()?;
    let path = root.join("preferences.json");
    let temp = root.join(format!(
        ".preferences-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(&prefs).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temp, &path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(prefs)
    })();
    let _ = std::fs::remove_file(temp);
    result
}
pub fn remember(root: &Path, path: &Path) -> Result<Preferences> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    update(root, |p| {
        p.recent.retain(|v| v != &path);
        p.recent.insert(0, path);
        p.recent.truncate(12);
    })
}
/// Local file events only. Decode percent escapes without applying form '+' conversion.
pub fn local_file_url(url: &str) -> Result<PathBuf> {
    if url.len() > 16384 {
        return Err("文件URL过长".into());
    }
    let tail = url.strip_prefix("file://").ok_or("只接受本地文件URL")?;
    let encoded = if tail.starts_with('/') {
        tail
    } else {
        tail.strip_prefix("localhost")
            .filter(|s| s.starts_with('/'))
            .ok_or("不接受远程文件主机")?
    };
    let mut bytes = vec![];
    let mut i = 0;
    let raw = encoded.as_bytes();
    while i < raw.len() {
        if raw[i] == b'%' {
            if i + 2 >= raw.len() {
                return Err("文件URL转义不完整".into());
            }
            let text = std::str::from_utf8(&raw[i + 1..i + 3]).map_err(|_| "非法文件URL")?;
            bytes.push(u8::from_str_radix(text, 16).map_err(|_| "非法文件URL转义")?);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    if bytes.contains(&0) {
        return Err("文件URL含空字节".into());
    }
    let value = String::from_utf8(bytes).map_err(|_| "文件URL不是UTF-8")?;
    #[cfg(target_os = "windows")]
    let value = value.strip_prefix('/').unwrap_or(&value).to_owned();
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("文件URL必须是绝对路径".into());
    }
    Ok(path)
}
