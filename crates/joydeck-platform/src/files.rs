use joydeck_model::document::{MAX_FILE, Result};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static IDS: AtomicU64 = AtomicU64::new(1);
/// Random names prevent concurrent allocations from colliding on coarse or adjusted wall clocks.
pub fn private_directory(label: &str) -> Result<PathBuf> {
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

pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    f.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("文件超过读取预算".into());
    }
    Ok(bytes)
}
pub fn publish(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    if bytes.len() > MAX_FILE as usize {
        return Err("文件超过写入预算".into());
    }
    let temp = path.with_file_name(format!(
        ".joydeck-{}-{}.tmp",
        std::process::id(),
        IDS.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        if replace {
            std::fs::rename(&temp, path).map_err(|e| e.to_string())?
        } else {
            std::fs::hard_link(&temp, path).map_err(|e| e.to_string())?
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(temp);
    result
}
