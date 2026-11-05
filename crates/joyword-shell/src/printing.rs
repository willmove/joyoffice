//! Print selected pages from the current presentation snapshot.
use crate::reader::{RasterScene, Reader};
use serde_json::json;

pub struct PrintJob {
    scene: RasterScene,
    pages: Vec<usize>,
}

impl Reader {
    pub fn print_job(&self, pages: &str) -> Result<PrintJob, String> {
        if self.is_composing() {
            return Err("Finish or cancel the composition before printing".into());
        }
        let scene = self.scene();
        let plan =
            joyword_engine::print::plan(&json!({"pages":pages}), scene.layout.pages.len(), (0, 0))
                .map_err(|e| e.to_string())?;
        if plan.chosen > 32 {
            return Err("Select at most 32 pages per system print job".into());
        }
        Ok(PrintJob {
            scene,
            pages: plan.pages,
        })
    }
}

impl PrintJob {
    pub fn pdf(&self, links: bool) -> Result<Vec<u8>, String> {
        let bytes = joyword_pdf::export_layout(
            &self.scene.doc,
            &self.scene.layout,
            &joyword_pdf::PdfOptions {
                pages: Some(self.pages.clone()),
                include_markup: self.scene.options.display.markup,
                links,
                ..Default::default()
            },
        )
        .map_err(|error| error.to_string())?;
        if bytes.len() > 256 << 20 {
            return Err("Print PDF exceeds 256 MiB".into());
        }
        Ok(bytes)
    }

    #[cfg(windows)]
    pub fn dialog(&self) -> Result<bool, String> {
        use std::{fs, os::windows::process::CommandExt, process::Command};
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let pdf = dir.path().join("document.pdf");
        fs::write(&pdf, self.pdf(false)?).map_err(|e| e.to_string())?;
        render_windows_pdf(&pdf, self.pages.len() as u32).map_err(|e| e.to_string())?;
        let script = dir.path().join("print.ps1");
        fs::write(
            &script,
            include_str!("../../../scripts/joysheet/native-print.ps1"),
        )
        .map_err(|e| e.to_string())?;
        let output = Command::new("pwsh.exe")
            .env_remove("JOYWORD_CONTROL_KEY")
            .args(["-NoProfile", "-STA", "-File"])
            .arg(script)
            .arg("-Directory")
            .arg(dir.path())
            .args([
                "-OwnerProcessId",
                &std::process::id().to_string(),
                "-JobName",
                "JoyWord",
            ])
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("PowerShell 7 print dialog: {e}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr)
                .trim()
                .chars()
                .take(2000)
                .collect());
        }
        let reply: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        match (reply["outcome"].as_str(), reply["queueSubmitted"].as_bool()) {
            (Some("submitted"), Some(true)) => Ok(true),
            (Some("cancelled"), Some(false)) => Ok(false),
            _ => Err("Invalid print dialog result".into()),
        }
    }
}

#[cfg(windows)]
fn render_windows_pdf(path: &std::path::Path, count: u32) -> windows::core::Result<()> {
    use windows::{
        Data::Pdf::{PdfDocument, PdfPageRenderOptions},
        Storage::{
            StorageFile,
            Streams::{DataReader, InMemoryRandomAccessStream},
        },
        core::{Error, HRESULT, HSTRING},
    };
    let invalid = |message| Error::new(HRESULT(0x80070057u32 as i32), message);
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))?.join()?;
    let pdf = PdfDocument::LoadFromFileAsync(&file)?.join()?;
    if pdf.PageCount()? != count || !(1..=32).contains(&count) {
        return Err(invalid("Unexpected print PDF page count"));
    }
    let mut total = 0;
    for i in 0..count {
        let page = pdf.GetPage(i)?;
        let size = page.Size()?;
        if !size.Width.is_finite()
            || !size.Height.is_finite()
            || size.Width <= 0.0
            || size.Height <= 0.0
        {
            return Err(invalid("Invalid PDF page geometry"));
        }
        let scale = (1500.0 / size.Width).min(2400.0 / size.Height);
        let render = |scale: f32| -> windows::core::Result<Option<Vec<u8>>> {
            let options = PdfPageRenderOptions::new()?;
            options.SetDestinationWidth((size.Width * scale).round().max(1.0) as u32)?;
            options.SetDestinationHeight((size.Height * scale).round().max(1.0) as u32)?;
            let stream = InMemoryRandomAccessStream::new()?;
            page.RenderWithOptionsToStreamAsync(&stream, &options)?
                .join()?;
            let length = stream.Size()?;
            if length > 16 << 20 {
                return Ok(None);
            }
            let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
            reader.LoadAsync(length as u32)?.join()?;
            let mut bytes = vec![0; length as usize];
            reader.ReadBytes(&mut bytes)?;
            // WinRT takes DIPs. Check encoded pixel geometry before handing the
            // bitmap to the native paginator, including on scaled displays.
            let image = joyword_geom::picture::probe(&bytes)
                .map_err(|_| invalid("Invalid print bitmap"))?;
            Ok((u64::from(image.width) * u64::from(image.height) <= 4_000_000).then_some(bytes))
        };
        let bytes = match render(scale)? {
            Some(bytes) => bytes,
            None => render(scale * 0.5)?.ok_or_else(|| invalid("Print bitmap budget exceeded"))?,
        };
        total += bytes.len();
        if total > 128 << 20 {
            return Err(invalid("Print bitmap budget exceeded"));
        }
        std::fs::write(path.with_file_name(format!("page-{}.png", i + 1)), bytes)?;
        page.Close()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn print_range_uses_unsaved_pages_and_keeps_the_edit_revision() -> Result<(), String> {
        let mut reader = Reader::blank(&AtomicBool::new(false))?;
        reader.execute("text.insert", &json!({"text":"First page"}))?;
        reader.execute("text.pageBreak", &json!({}))?;
        reader.execute("text.insert", &json!({"text":"Second page"}))?;
        let version = reader.engine().rev();
        assert!(reader.print_job("3").is_err());
        let job = reader.print_job("2")?;
        assert_eq!(job.pages, [1]);
        assert!(job.pdf(true)?.starts_with(b"%PDF-"));
        assert_eq!(reader.engine().rev(), version);
        Ok(())
    }

    #[cfg(windows)]
    #[test]
    fn windows_print_pipeline_validates_the_selected_page_without_a_queue() -> Result<(), String> {
        use std::{fs, process::Command};
        let mut reader = Reader::blank(&AtomicBool::new(false))?;
        reader.execute("text.insert", &json!({"text":"First page"}))?;
        reader.execute("text.pageBreak", &json!({}))?;
        reader.execute("text.insert", &json!({"text":"Second page"}))?;
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let pdf = dir.path().join("document.pdf");
        fs::write(&pdf, reader.print_job("2")?.pdf(false)?).map_err(|e| e.to_string())?;
        render_windows_pdf(&pdf, 1).map_err(|e| e.to_string())?;
        let script = dir.path().join("print.ps1");
        fs::write(
            &script,
            include_str!("../../../scripts/joysheet/native-print.ps1"),
        )
        .map_err(|e| e.to_string())?;
        let run = |page: &str| {
            Command::new("pwsh.exe")
                .args(["-NoProfile", "-STA", "-File"])
                .arg(&script)
                .arg("-Directory")
                .arg(dir.path())
                .args(["-Validate", "-FromPage", page])
                .output()
                .map_err(|e| e.to_string())
        };
        let output = run("1")?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into());
        }
        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
        assert_eq!(report["pages"], 1);
        assert_eq!(report["queueSubmitted"], false);
        assert!(!run("2")?.status.success());
        Ok(())
    }
}
