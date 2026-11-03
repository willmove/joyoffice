//! Owned helper discovery and scratch lifetime, adapted from JoyOffice platform helpers/playback.
use std::{path::PathBuf, process::Command};

pub fn find(kind: &str) -> Result<PathBuf, String> {
    if !cfg!(target_os = "windows") {
        return Err("this build has Windows print/media adapters only".into());
    }
    if !["print", "media", "video"].contains(&kind) {
        return Err("unknown helper".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = exe
        .parent()
        .ok_or("executable directory missing")?
        .join(format!("joydeck-{kind}-windows.exe"));
    if !path.is_file() {
        return Err(format!(
            "missing adapter {}: build Windows helpers or use the complete development package",
            path.display()
        ));
    }
    Ok(path)
}
pub fn command(path: impl AsRef<std::ffi::OsStr>) -> Command {
    let command = Command::new(path);
    #[cfg(windows)]
    let command = {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x08000000);
        command
    };
    command
}
/// .NET Framework/WinRT media APIs reject Rust's canonical `\\?\` file prefix.
/// These paths already refer to our owned scratch directory; retain that canonical path for cleanup.
pub fn native_path(path: &std::path::Path) -> Result<PathBuf, String> {
    let text = path
        .to_str()
        .ok_or("native adapter requires a Unicode path")?;
    Ok(if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") {
        PathBuf::from(format!("\\\\{unc}"))
    } else {
        PathBuf::from(text.strip_prefix("\\\\?\\").unwrap_or(text))
    })
}
pub struct Scratch(pub PathBuf);
impl Scratch {
    pub fn new() -> Result<Self, String> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
        let suffix = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let parent = std::env::temp_dir()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let path = parent.join(format!("joydeck-{}-{suffix}", std::process::id()));
        std::fs::create_dir(&path).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        // Fresh random directory; refuse cleanup after a link/parent substitution.
        if let (Ok(parent), Ok(target)) =
            (std::env::temp_dir().canonicalize(), self.0.canonicalize())
            && target.parent() == Some(parent.as_path())
            && target == self.0
        {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
