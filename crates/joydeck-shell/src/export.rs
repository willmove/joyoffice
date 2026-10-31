use crate::document::{Deck, Result, check_cancel};
pub use joydeck_export::export::*;
use std::{
    io::Write,
    path::Path,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
static IDS: AtomicU64 = AtomicU64::new(1);
pub fn atomic(
    path: &Path,
    bytes: &[u8],
    expected: Option<crate::storage::Fingerprint>,
    cancel: &AtomicBool,
) -> Result<()> {
    check_cancel(cancel)?;
    if bytes.len() > crate::document::MAX_FILE as usize {
        return Err("输出超过64 MiB预算".into());
    }
    let temp = path.with_file_name(format!(
        ".joydeck-output-{}-{}",
        std::process::id(),
        IDS.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        check_cancel(cancel)?;
        if let Some(fp) = expected {
            if crate::storage::fingerprint(path)? != fp {
                return Err("输出目标已被外部修改，拒绝覆盖".into());
            }
            std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        } else {
            std::fs::hard_link(&temp, path)
                .map_err(|e| format!("无法新建输出（目标可能已存在）：{e}"))?;
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(temp);
    result
}
pub fn save_parts(
    path: &Path,
    parts: &std::collections::BTreeMap<String, Vec<u8>>,
    cancel: &AtomicBool,
) -> Result<()> {
    crate::interop::validate_graph(parts)?;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(vec![]));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in parts {
        check_cancel(cancel)?;
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(data).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    atomic(path, &bytes, None, cancel)
}
pub fn images(
    path: &Path,
    deck: &Deck,
    options: &Options,
    format: ImageFormat,
    cancel: &AtomicBool,
) -> Result<()> {
    options.validate(deck)?;
    if format == ImageFormat::Jpeg && options.transparent {
        return Err("JPEG不支持透明背景，请使用PNG或关闭透明".into());
    }
    let staging = path.with_file_name(format!(
        ".joydeck-images-{}-{}",
        std::process::id(),
        IDS.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let mut names = vec![];
    let mut published = vec![];
    let mut created = false;
    let result = (|| {
        let mut total = 0;
        for page in &options.pages {
            check_cancel(cancel)?;
            let d = page_deck(deck, *page, options.notes)?;
            let raster = raster(&d, *page, options.dpi, options.transparent, cancel)?;
            let bytes = encoded(&raster, format, options.dpi)?;
            total += bytes.len();
            if total > 64 * 1024 * 1024 {
                return Err("图片输出总预算超过64 MiB".into());
            }
            let name = format!(
                "slide-{:03}.{}",
                page + 1,
                if format == ImageFormat::Png {
                    "png"
                } else {
                    "jpg"
                }
            );
            std::fs::write(staging.join(&name), bytes).map_err(|e| e.to_string())?;
            names.push(name);
        }
        check_cancel(cancel)?;
        std::fs::create_dir(path).map_err(|e| format!("导出文件夹必须不存在：{e}"))?;
        created = true;
        for name in &names {
            check_cancel(cancel)?;
            std::fs::hard_link(staging.join(name), path.join(name)).map_err(|e| e.to_string())?;
            published.push(name.clone());
        }
        Ok(())
    })();
    if result.is_err() && created {
        for name in published {
            let _ = std::fs::remove_file(path.join(name));
        }
        let _ = std::fs::remove_dir(path);
    }
    let _ = std::fs::remove_dir_all(staging);
    result
}
pub fn pdf(path: &Path, deck: &Deck, options: &Options, cancel: &AtomicBool) -> Result<()> {
    let bytes = pdf_bytes(deck, options, cancel)?;
    atomic(path, &bytes, None, cancel)
}
