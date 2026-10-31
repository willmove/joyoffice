use crate::document::{Deck, Result};
use std::{path::Path, sync::atomic::AtomicBool};
pub fn vector_pdf(path: &Path, d: &Deck, pages: &[usize], cancel: &AtomicBool) -> Result<()> {
    let bytes = joydeck_export::output::vector_pdf_bytes(d, pages, cancel)?;
    crate::export::atomic(path, &bytes, None, cancel)
}
pub fn presentation(path: &Path, d: &Deck, kind: &str, cancel: &AtomicBool) -> Result<()> {
    let mut parts = crate::pptx_write::generate(d)?;
    let ct = parts
        .get_mut("[Content_Types].xml")
        .ok_or("缺少content types")?;
    let original =
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
    let target = match kind {
        "pptx" => original,
        "potx" => "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml",
        "ppsx" => "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml",
        _ => return Err("未知演示格式".into()),
    };
    *ct = String::from_utf8(ct.clone())
        .map_err(|e| e.to_string())?
        .replace(original, target)
        .into_bytes();
    crate::export::save_parts(path, &parts, cancel)
}
pub fn other(
    path: &Path,
    d: &Deck,
    pages: &[usize],
    kind: &str,
    cancel: &AtomicBool,
) -> Result<()> {
    let bytes = joydeck_export::output::other_bytes(d, pages, kind, cancel)?;
    crate::export::atomic(path, &bytes, None, cancel)
}
pub fn print(d: &Deck, pages: &[usize], cancel: &AtomicBool) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err("该平台请先导出PDF，再使用系统打印".into());
    }
    let helper = joydeck_platform::helpers::find("print")?;
    let path = std::env::temp_dir().join(format!(
        "joydeck-print-{}-{}.pdf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    vector_pdf(&path, d, pages, cancel)?;
    let result = std::process::Command::new(helper)
        .arg(&path)
        .status()
        .map_err(|e| e.to_string());
    let _ = std::fs::remove_file(path);
    match result {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err("系统打印失败".into()),
        Err(e) => Err(e),
    }
}
