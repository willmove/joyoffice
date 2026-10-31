//! PDF rasterization and an explicit native print dialog, outside the engine and UI threads.
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct PrintDocument {
    pub directory: PathBuf,
    pub pdf: PathBuf,
    pub pages: Vec<PathBuf>,
}
impl Drop for PrintDocument {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
pub fn directory() -> Result<PathBuf, String> {
    let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let dir = std::env::temp_dir().join(format!("joysheet-print-{}-{time}", std::process::id()));
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
pub fn render(directory: PathBuf) -> Result<PrintDocument, String> {
    let pdf = directory.join("document.pdf");
    let mut document = PrintDocument { directory, pdf, pages: Vec::new() };
    render_pages(&mut document)?;
    Ok(document)
}
#[cfg(any(target_os = "windows", test))]
fn preview_size(width: f32, height: f32) -> Result<(u32, u32), String> {
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return Err("Invalid PDF page dimensions".into());
    }
    let scale = (1200. / width).min(2000. / height);
    Ok(((width * scale).round().clamp(1., 1200.) as u32, (height * scale).round().clamp(1., 2000.) as u32))
}
#[cfg(target_os = "windows")]
fn render_pages(document: &mut PrintDocument) -> Result<(), String> {
    use windows::{
        Data::Pdf::{PdfDocument, PdfPageRenderOptions},
        Storage::{
            StorageFile,
            Streams::{DataReader, InMemoryRandomAccessStream},
        },
        core::HSTRING,
    };
    let run = || -> windows::core::Result<Vec<Vec<u8>>> {
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(document.pdf.to_string_lossy().as_ref()))?.join()?;
        let pdf = PdfDocument::LoadFromFileAsync(&file)?.join()?;
        let count = pdf.PageCount()?;
        if count > 32 {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80070057u32 as i32),
                "Preview supports at most 32 pages; narrow the print area or export PDF",
            ));
        }
        let mut result = Vec::new();
        let mut total = 0u64;
        for i in 0..count {
            let page = pdf.GetPage(i)?;
            let size = page.Size()?;
            let (width, height) =
                preview_size(size.Width, size.Height).map_err(|e| windows::core::Error::new(windows::core::HRESULT(0x80070057u32 as i32), e))?;
            let stream = InMemoryRandomAccessStream::new()?;
            let options = PdfPageRenderOptions::new()?;
            options.SetDestinationWidth(width)?;
            options.SetDestinationHeight(height)?;
            page.RenderWithOptionsToStreamAsync(&stream, &options)?.join()?;
            let length = stream.Size()?;
            total = total.saturating_add(length);
            if length > 32 * 1024 * 1024 || total > 128 * 1024 * 1024 {
                return Err(windows::core::Error::new(windows::core::HRESULT(0x8007000eu32 as i32), "Preview image exceeds memory budget"));
            }
            let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
            reader.LoadAsync(length as u32)?.join()?;
            let mut bytes = vec![0; length as usize];
            reader.ReadBytes(&mut bytes)?;
            result.push(bytes);
            page.Close()?;
        }
        Ok(result)
    };
    for (i, bytes) in run().map_err(|e| e.to_string())?.into_iter().enumerate() {
        let path = document.directory.join(format!("page-{}.png", i + 1));
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        document.pages.push(path);
    }
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn render_pages(document: &mut PrintDocument) -> Result<(), String> {
    let output = Command::new("pdftoppm")
        .args(["-png", "-scale-to", "1200", "-f", "1", "-l", "33"])
        .arg(&document.pdf)
        .arg(document.directory.join("page"))
        .output()
        .map_err(|e| format!("Install Poppler (pdftoppm) for PDF preview: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    document.pages = std::fs::read_dir(&document.directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    document
        .pages
        .sort_by_key(|p| p.file_stem().and_then(|s| s.to_str()).and_then(|s| s.rsplit('-').next()).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0));
    if document.pages.len() > 32 {
        return Err("Preview supports at most 32 pages; export PDF or narrow the print area".into());
    }
    Ok(())
}
pub fn print_dialog(document: &PrintDocument) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = document.directory.join("print.ps1");
        std::fs::write(&script, include_str!("../../../scripts/joysheet/native-print.ps1")).map_err(|e| e.to_string())?;
        let output = Command::new("pwsh.exe")
            .args(["-NoProfile", "-STA", "-File"])
            .arg(script)
            .arg("-Directory")
            .arg(&document.directory)
            .arg("-OwnerProcessId")
            .arg(std::process::id().to_string())
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("PowerShell 7 is required for the Windows print dialog: {e}"))?;
        if output.status.success() {
            let reply: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("Invalid print helper response: {e}"))?;
            match reply["outcome"].as_str() {
                Some("cancelled") => Ok("cancelled".into()),
                Some("submitted") => Ok("submitted to Windows print spooler".into()),
                _ => Err("Unknown print helper outcome".into()),
            }
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let status =
            Command::new(if cfg!(target_os = "macos") { "open" } else { "xdg-open" }).arg(&document.pdf).status().map_err(|e| e.to_string())?;
        if status.success() {
            Ok("PDF opened in the system viewer; use its print dialog".into())
        } else {
            Err("Could not open the system PDF viewer".into())
        }
    }
}
pub fn export(document: &PrintDocument, path: &Path) -> Result<(), String> {
    if path.canonicalize().ok().zip(document.pdf.canonicalize().ok()).is_some_and(|(a, b)| a == b) {
        return Err("Choose an export path outside the temporary source PDF".into());
    }
    std::fs::copy(&document.pdf, path).map(|_| ()).map_err(|e| e.to_string())
}
#[cfg(test)]
#[path = "tests/printing_tests.rs"]
mod tests;
