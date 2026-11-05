use crate::document::MAX_INPUT as MAX_FILE;
type Result<T> = std::result::Result<T, String>;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static IDS: AtomicU64 = AtomicU64::new(1);

/// A process-independent identifier for ephemeral document capabilities.
pub fn random_token() -> Result<[u8; 16]> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    Ok(nonce)
}

/// Read and normalize a local raster picture under the same byte/pixel budgets as the editor.
pub fn picture_command(path: &Path, replace: bool) -> Result<joydeck_engine::Command> {
    let bytes = read_bounded(path, 16 << 20)?;
    picture_bytes_command(bytes, replace)
}
/// Clipboard image bytes use the same limits and PNG normalization as file import.
pub fn picture_bytes_command(bytes: Vec<u8>, replace: bool) -> Result<joydeck_engine::Command> {
    if bytes.len() > 16 << 20 {
        return Err("picture exceeds 16 MiB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 << 20);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?;
    let (w, h) = (image.width(), image.height());
    if u64::from(w) * u64::from(h) > 16_000_000 {
        return Err("picture exceeds 16 MP".into());
    }
    let mut out = std::io::Cursor::new(vec![]);
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(joydeck_engine::Command::InsertPicture {
        png: out.into_inner(),
        x: 72.,
        y: 150.,
        width: 240.,
        height: 240. * f64::from(h) / f64::from(w),
        replace,
    })
}
/// Random names prevent concurrent allocations from colliding on coarse or adjusted wall clocks.
pub fn private_directory(label: &str) -> Result<PathBuf> {
    for _ in 0..8 {
        let nonce = random_token()?;
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
            replace_file(&temp, path)?
        } else {
            std::fs::hard_link(&temp, path).map_err(|e| e.to_string())?
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(temp);
    result
}
/// Windows indexers can briefly open a newly written file without delete sharing.
/// Retry the atomic replacement; never remove the destination to make it succeed.
pub(crate) fn replace_file(source: &Path, destination: &Path) -> Result<()> {
    for attempt in 0..8 {
        match std::fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if cfg!(windows)
                    && error.kind() == std::io::ErrorKind::PermissionDenied
                    && attempt < 7
                {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                    continue;
                }
                return Err(format!("atomic replacement: {error}"));
            }
        }
    }
    Err("atomic replacement did not complete".into())
}
