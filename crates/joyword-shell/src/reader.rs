//! Read-only desktop/CLI host over the single Wordcraft-derived engine.
use joyword_engine::{
    Session,
    doc::{Document, StoryRef},
};
use joyword_layout::DocLayout;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_INPUT_BYTES: u64 = 64 << 20;
pub const MAX_CACHE_PIXELS: u64 = 16_777_216;
pub const MAX_PAGE_PIXELS: u64 = 4_194_304;
pub const WELCOME: &[u8] = include_bytes!("../../../assets/samples/welcome.docx");

/// Apply the desktop host's untrusted Word container limits before import.
pub fn preflight_word_package(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 > MAX_INPUT_BYTES {
        return Err("DOCX exceeds 64 MiB".into());
    }
    let budget = joyoffice_opc::Budget {
        max_part_bytes: 64 << 20,
        max_total_bytes: 256 << 20,
        max_xml_nodes: 4_000_000,
        ..Default::default()
    };
    joyoffice_opc::Package::read(bytes, &budget)
        .map(|_| ())
        .map_err(|e| format!("DOCX container: {e}"))
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Summary {
    pub name: String,
    pub fingerprint: String,
    pub pages: Vec<PageSize>,
    pub command_count: usize,
    pub layout_ms: f64,
}

#[derive(serde::Serialize)]
pub struct Inspection<'a> {
    #[serde(flatten)]
    pub summary: &'a Summary,
    pub text: String,
}

pub struct Reader {
    pub(crate) session: Session,
    pub(crate) layout: Arc<DocLayout>,
    pub(crate) composition: Option<crate::editing::Composition>,
    pub(crate) source: Option<crate::storage::SourceStamp>,
    pub(crate) recovered_package: Option<(u64, Arc<[u8]>)>,
    pub recovery_notice: Option<&'static str>,
    pub summary: Summary,
    pub(crate) presentation: u64,
}

pub fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("Document operation cancelled".into())
    } else {
        Ok(())
    }
}
pub fn guarded<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).map_err(|_| {
        "Document processing failed internally; the previous document is retained".to_string()
    })?
}
fn visual_command(id: &str) -> bool {
    matches!(
        id,
        "review.markup"
            | "review.trackingOptions"
            | "mailings.preview"
            | "mailings.next"
            | "mailings.previous"
            | "mailings.first"
            | "mailings.last"
            | "mailings.goToRecord"
            | "mailings.findRecipient"
            | "mailings.recipients"
            | "mailings.filterRecipients"
            | "mailings.sortRecipients"
            | "view.fieldCodes"
            | "view.marks"
    )
}
impl Reader {
    pub fn set_review_author(&mut self, author: &str) {
        if !author.is_empty() {
            self.session.author = author.to_owned();
        }
    }

    pub fn presentation_revision(&self) -> u64 {
        self.presentation
    }
    fn display_options(&self) -> joyword_render::RenderOptions {
        joyword_render::RenderOptions {
            display: joyword_layout::display::DisplayOptions {
                markup: self.session.view.show_markup,
                marks: self.session.view.marks,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    pub fn open(path: &Path, cancel: &AtomicBool) -> Result<Self, String> {
        guarded(|| {
            check_cancel(cancel)?;
            if crate::advanced::extension(path) == "jwr" {
                return Self::recover_legacy(path, cancel);
            }
            if !crate::advanced::IMPORTS.contains(&crate::advanced::extension(path).as_str()) {
                return Err("此格式尚未接入桌面导入".into());
            }
            let opened = crate::storage::open_source(
                path,
                &joyoffice_opc::Budget {
                    max_total_bytes: MAX_INPUT_BYTES,
                    ..Default::default()
                },
            )
            .map_err(|e| e.to_string())?;
            let mut result =
                Self::load_bytes(&path.to_string_lossy(), opened.bytes(), Some(path), cancel)?;
            result.source = Some(opened.into_stamp());
            Ok(result)
        })
    }
    /// Check the exact captured source before command editing or DOCX regeneration.
    /// This does not re-read a path that may have changed after opening.
    pub fn require_editable_source(&self) -> Result<(), String> {
        if let Some(source) = &self.source
            && joyword_engine::io::is_word_package(&crate::advanced::extension(source.path()))
            && joyword_docx::legacy_inspection::project(source.bytes(), 0)?.read_only
        {
            return Err("Source DOCX requires read-only handling; command editing and regenerated saves are refused".into());
        }
        Ok(())
    }

    pub fn from_bytes(name: &str, bytes: &[u8], cancel: &AtomicBool) -> Result<Self, String> {
        Self::load_bytes(name, bytes, None, cancel)
    }
    fn load_bytes(
        name: &str,
        bytes: &[u8],
        path: Option<&Path>,
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        guarded(|| {
            check_cancel(cancel)?;
            if bytes.len() as u64 > MAX_INPUT_BYTES {
                return Err("DOCX exceeds 64 MiB".into());
            }
            // Host OPC validates the untrusted container; Word-specific semantics stay in the engine.
            let ext = crate::advanced::extension(Path::new(name));
            if joyword_engine::io::is_word_package(&ext)
                || (ext.is_empty() && bytes.starts_with(b"PK\x03\x04"))
            {
                preflight_word_package(bytes)?;
            }
            check_cancel(cancel)?;
            let doc = joyword_engine::io::open_bytes(name, bytes)?;
            Self::with_document(
                name,
                doc,
                format!("{:x}", Sha256::digest(bytes)),
                path,
                cancel,
            )
        })
    }
    pub fn blank(cancel: &AtomicBool) -> Result<Self, String> {
        Self::blank_with_preset(None, cancel)
    }
    pub fn blank_with_preset(
        preset: Option<crate::prefs::PagePreset>,
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        let region = joyword_geom::Region {
            letter: preset.map(|paper| paper == crate::prefs::PagePreset::Letter),
            ..Default::default()
        };
        Self::with_document(
            "Untitled",
            joyword_engine::blank_document(&region),
            "blank".into(),
            None,
            cancel,
        )
    }
    fn with_document(
        name: &str,
        doc: Document,
        fingerprint: String,
        path: Option<&Path>,
        cancel: &AtomicBool,
    ) -> Result<Self, String> {
        check_cancel(cancel)?;
        let mut session = Session::new(doc);
        // FILENAME fields and the paragraph cache both depend on the source path.
        session.path = path.map(Path::to_path_buf);
        let layout = session.layout();
        check_cancel(cancel)?;
        if layout.pages.is_empty() || layout.pages.len() > 10_000 {
            return Err("Document page count is outside preview limits".into());
        }
        if layout
            .pages
            .iter()
            .any(|p| !p.w.is_finite() || !p.h.is_finite() || p.w <= 0.0 || p.h <= 0.0)
        {
            return Err("Document contains invalid page dimensions".into());
        }
        let summary = Summary {
            name: PathBuf::from(name)
                .file_name()
                .map_or_else(|| name.to_owned(), |p| p.to_string_lossy().into_owned()),
            fingerprint,
            pages: layout
                .pages
                .iter()
                .map(|p| PageSize {
                    width: p.w,
                    height: p.h,
                })
                .collect(),
            command_count: session.registry.all().len(),
            layout_ms: layout.ms,
        };
        Ok(Self {
            session,
            layout,
            composition: None,
            source: None,
            recovered_package: None,
            recovery_notice: None,
            summary,
            presentation: 0,
        })
    }
    pub fn raster(
        &self,
        page: usize,
        scale: f32,
        cancel: &AtomicBool,
    ) -> Result<joyword_render::Rendered, String> {
        guarded(|| {
            check_cancel(cancel)?;
            let page = self.layout.pages.get(page).ok_or("Page is out of range")?;
            if !scale.is_finite() || !(0.1..=8.0).contains(&scale) {
                return Err("Invalid raster scale".into());
            }
            let budget_scale =
                ((MAX_PAGE_PIXELS as f64) / (f64::from(page.w) * f64::from(page.h))).sqrt() as f32;
            let scale = scale.min(budget_scale * 0.99);
            let result = joyword_render::render_page(
                &self.session.doc,
                page,
                scale,
                &self.display_options(),
            );
            check_cancel(cancel)?;
            if result.width == 0
                || result.height == 0
                || u64::from(result.width) * u64::from(result.height) > MAX_CACHE_PIXELS
            {
                return Err("Page raster exceeds the preview pixel budget".into());
            }
            Ok(result)
        })
    }
    pub fn pdf(&self) -> Result<Vec<u8>, String> {
        joyword_engine::io::save_bytes("preview.pdf", &self.session.doc)
    }
    pub fn engine(&self) -> &Session {
        &self.session
    }
    pub fn body_text(&self) -> String {
        self.session.doc.plain_text(StoryRef::Body)
    }
    pub fn inspection(&self) -> Inspection<'_> {
        Inspection {
            summary: &self.summary,
            text: self.body_text(),
        }
    }
    pub fn execute(
        &mut self,
        command: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.commit_composition()?;
        if command == "tools.macros" {
            self.validate_macro_run(params)?;
        }
        let rollback = (command == "tools.macros" && params["run"].is_string())
            .then(|| self.session.edit_snapshot());
        let before = self.session.rev();
        let identity = self.session.document_id();
        let result = match self.session.run(command, params) {
            Ok(value) => value,
            Err(e) => {
                if let Some(snapshot) = rollback {
                    self.session.restore(snapshot);
                    self.refresh();
                }
                return Err(e.to_string());
            }
        };
        if self.session.document_id() != identity {
            // Commands creating a new active document cannot inherit an old source stamp.
            self.source = None;
            self.recovered_package = None;
            self.recovery_notice = None;
            self.summary.name = "Untitled".into();
        }
        if self.session.rev() != before
            || self.session.document_id() != identity
            || visual_command(command)
        {
            self.refresh();
        }
        Ok(result)
    }
    pub(crate) fn refresh(&mut self) {
        self.presentation = self.presentation.saturating_add(1);
        self.layout = self.session.layout();
        self.summary.pages = self
            .layout
            .pages
            .iter()
            .map(|p| PageSize {
                width: p.w,
                height: p.h,
            })
            .collect();
        self.summary.layout_ms = self.layout.ms;
    }
}

/// Immutable render input; the live editable Session remains on the UI thread.
#[derive(Clone)]
pub struct RasterScene {
    pub doc: Arc<Document>,
    pub layout: Arc<DocLayout>,
    pub options: joyword_render::RenderOptions,
}
impl Reader {
    pub fn scene(&self) -> RasterScene {
        RasterScene {
            doc: Arc::new(self.session.doc.clone()),
            layout: self.layout.clone(),
            options: self.display_options(),
        }
    }
    pub fn geometry(&self) -> &DocLayout {
        &self.layout
    }
    pub fn changed_page_from(&self, scene: &RasterScene) -> Option<usize> {
        self.layout.changed_page_from(&scene.layout)
    }
}
impl RasterScene {
    pub fn raster(&self, page: usize, scale: f32) -> Result<joyword_render::Rendered, String> {
        self.raster_cancellable(page, scale, &AtomicBool::new(false))
    }

    pub fn raster_cancellable(
        &self,
        page: usize,
        scale: f32,
        cancel: &AtomicBool,
    ) -> Result<joyword_render::Rendered, String> {
        guarded(|| {
            let page = self.layout.pages.get(page).ok_or("Page is out of range")?;
            if !scale.is_finite() || !(0.1..=8.0).contains(&scale) {
                return Err("Invalid raster scale".into());
            }
            let limit =
                ((MAX_PAGE_PIXELS as f64) / (f64::from(page.w) * f64::from(page.h))).sqrt() as f32;
            joyword_render::render_page_cancellable(
                &self.doc,
                page,
                scale.min(limit * 0.99),
                &self.options,
                || cancel.load(Ordering::Relaxed),
            )
            .ok_or_else(|| "Page rendering cancelled".into())
        })
    }
}

/// New output only. The host tempfile pipeline synchronizes before no-clobber publication.
pub fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    publish(path, bytes, None)
}

pub(crate) fn publish_verified(
    path: &Path,
    bytes: &[u8],
    cancel: &AtomicBool,
) -> Result<(), String> {
    check_cancel(cancel)?;
    publish(path, bytes, Some(cancel))
}

fn publish(path: &Path, bytes: &[u8], verify: Option<&AtomicBool>) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(bytes).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    if let Some(cancel) = verify {
        crate::storage::verify_bytes(temporary.as_file_mut(), bytes).map_err(|e| e.to_string())?;
        check_cancel(cancel)?;
    }
    temporary
        .persist_noclobber(path)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Publish encoded pages after checking for collisions across the batch.
pub fn publish_rasters(directory: &Path, artifacts: &[(PathBuf, Vec<u8>)]) -> Result<(), String> {
    if artifacts
        .iter()
        .any(|(path, _)| path.symlink_metadata().is_ok())
    {
        return Err("output already exists".into());
    }
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    for (path, bytes) in artifacts {
        publish_new(path, bytes)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
mod tests {
    use super::*;
    #[test]
    fn file_name_field_uses_source_on_initial_layout() -> Result<(), Box<dyn std::error::Error>> {
        use joyword_engine::doc::{Paragraph, para::InlineObject, para_block};
        let mut doc = Document::new();
        let mut paragraph = Paragraph::new();
        paragraph.insert_object(
            0,
            InlineObject::Field {
                instr: "FILENAME".into(),
                result: String::new(),
                locked: false,
                dirty: false,
                code: false,
                form: String::new(),
            },
            &Default::default(),
        )?;
        doc.body = vec![para_block(paragraph)];
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("source-name.docx");
        std::fs::write(
            &path,
            joyword_engine::io::save_bytes("source-name.docx", &doc)?,
        )?;
        let reader = Reader::open(&path, &AtomicBool::new(false))?;
        assert!(reader.geometry().pages[0].items.iter().any(|item| {
            matches!(item, joyword_layout::Placed::Lines { para, .. }
                if para.shown.iter().any(|(_, value, _)| value == "source-name.docx"))
        }));
        Ok(())
    }
    #[test]
    fn real_docx_uses_engine_layout_and_nonempty_raster() -> Result<(), String> {
        let cancel = AtomicBool::new(false);
        let reader = Reader::from_bytes("welcome.docx", WELCOME, &cancel)?;
        assert!(!reader.body_text().is_empty());
        assert!(reader.summary.command_count > 400);
        let page = reader.raster(0, 1.0, &cancel)?;
        assert!(page.pixels.as_chunks::<4>().0.iter().any(|p| p[0] < 180));
        assert!(reader.pdf()?.starts_with(b"%PDF"));
        Ok(())
    }
    #[test]
    fn mixed_media_docx_keeps_text_and_image_colors() -> Result<(), String> {
        let cancel = AtomicBool::new(false);
        let reader = Reader::from_bytes(
            "mixed-media.docx",
            include_bytes!("../../../assets/samples/mixed-media.docx"),
            &cancel,
        )?;
        assert!(reader.body_text().contains("中文与 English"));
        let page = reader.raster(0, 1.0, &cancel)?;
        for color in [[26, 91, 180], [32, 150, 110], [220, 80, 45]] {
            assert!(
                page.pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[..3] == color)
                    .count()
                    > 100
            );
        }
        Ok(())
    }
    #[test]
    fn malformed_cancelled_and_existing_output_are_rejected() {
        let cancel = AtomicBool::new(true);
        assert!(Reader::from_bytes("bad.docx", b"not a zip", &cancel).is_err());
        cancel.store(false, Ordering::Release);
        assert!(Reader::from_bytes("bad.docx", b"not a zip", &cancel).is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("original.docx");
        std::fs::write(&path, "original").unwrap();
        assert!(publish_new(&path, b"new").is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"original");
    }
}
