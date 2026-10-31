use joydeck_model::document::Result;
use std::path::PathBuf;
pub fn directory() -> PathBuf {
    PathBuf::from(env!("OUT_DIR"))
}
pub fn find(kind: &str) -> Result<PathBuf> {
    if !matches!(kind, "media" | "print" | "video") {
        return Err("未知原生适配器".into());
    }
    let name = format!("joydeck-{kind}-macos");
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = exe.parent().ok_or("无法定位程序目录")?;
    let mut candidates = vec![parent.join(&name), directory().join(&name)];
    if let Some(p) = parent.parent() {
        candidates.push(p.join(&name))
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| format!("缺少原生适配器 {name}"))
}
