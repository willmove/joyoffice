//! File orchestration for the pure command engine. Save jobs own immutable snapshots.
use crate::{
    Cancellation, Document, native,
    storage::{self, Fingerprint, Recovery},
};
use joydeck_engine::{EditorSnapshot, Session, original_bindings};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
pub struct Editor {
    pub session: Session,
    pub source: Option<PathBuf>,
    pub fingerprint: Option<Fingerprint>,
    pub base: Arc<Document>,
    pub template: Arc<[u8]>,
    pub recovery: Recovery,
}
#[derive(Clone)]
pub struct SaveJob {
    pub snapshot: EditorSnapshot,
    pub revision: u64,
    pub source: Arc<Document>,
    pub template: Arc<[u8]>,
    pub destination: PathBuf,
    pub expected: Option<Fingerprint>,
    pub recovery: Recovery,
}
pub struct Saved {
    pub fingerprint: Fingerprint,
    pub content_id: u64,
    pub revision: u64,
    pub path: PathBuf,
    pub warning: Option<String>,
}
impl Drop for Editor {
    fn drop(&mut self) {
        if !self.session.dirty() {
            let _ = self.recovery.discard();
        }
    }
}
pub fn recovery_root() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_STATE_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".local/state")
        });
    base.join("joyoffice/joydeck/recovery-v2")
}
impl Editor {
    pub fn open(path: &Path, root: PathBuf) -> Result<Self, String> {
        let before = storage::fingerprint(path)?;
        let doc = Document::open(path, &Cancellation::default())?;
        if storage::fingerprint(path)? != before {
            return Err("file changed during open".into());
        }
        let mut editor = Self::from_document(doc, root)?;
        editor.source = Some(path.to_path_buf());
        editor.fingerprint = Some(before);
        Ok(editor)
    }
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let bytes = joydeck_format::save(&joydeck_model::Presentation::default())
            .map_err(|e| e.to_string())?;
        let doc = Document::from_bytes("Untitled.joydeck".into(), bytes, &Cancellation::default())?;
        let mut e = Self::from_document(doc, root)?;
        e.session = Session::new(
            e.base.presentation.clone(),
            original_bindings(&e.base.presentation),
            false,
        );
        Ok(e)
    }
    pub fn from_document(doc: Document, root: PathBuf) -> Result<Self, String> {
        let template = doc.original.clone();
        let (model, origins, base) = if doc.info.format == "joydeck" {
            let n = native::load(&doc.original, &Cancellation::default())?;
            let base =
                Document::from_bytes("original source".into(), n.source, &Cancellation::default())?;
            (n.model, n.manifest.origins, base)
        } else {
            (
                doc.presentation.clone(),
                original_bindings(&doc.presentation),
                doc,
            )
        };
        Ok(Self {
            session: Session::new(model, origins, true),
            source: None,
            fingerprint: None,
            base: Arc::new(base),
            template,
            recovery: Recovery::new(root),
        })
    }
    pub fn display(&self) -> Result<Document, String> {
        Document::from_model(
            self.source
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled.joydeck".into()),
            self.session.model().clone(),
        )
    }
    pub fn save_job(&self, path: PathBuf) -> Result<SaveJob, String> {
        let same = self.source.as_ref().is_some_and(|source| {
            source == &path
                || source
                    .canonicalize()
                    .ok()
                    .is_some_and(|s| path.canonicalize().ok() == Some(s))
        });
        let expected = if same { self.fingerprint.clone() } else { None };
        Ok(SaveJob {
            snapshot: self.session.snapshot(),
            revision: self.session.revision(),
            source: self.base.clone(),
            template: self.template.clone(),
            destination: path,
            expected,
            recovery: self.recovery.clone(),
        })
    }
    pub fn apply_saved(&mut self, saved: Saved) {
        self.session.mark_saved(saved.content_id);
        self.source = Some(saved.path);
        self.fingerprint = Some(saved.fingerprint);
    }
    pub fn checkpoint_job(&self) -> SaveJob {
        SaveJob {
            snapshot: self.session.snapshot(),
            revision: self.session.revision(),
            source: self.base.clone(),
            template: self.template.clone(),
            destination: self.source.clone().unwrap_or_default(),
            expected: self.fingerprint.clone(),
            recovery: self.recovery.clone(),
        }
    }
    pub fn restore(root: PathBuf, record: &storage::Record) -> Result<Self, String> {
        let recovery = Recovery::new(root.clone());
        let bytes = recovery.restore(record)?;
        let doc =
            Document::from_bytes("Recovered.joydeck".into(), bytes, &Cancellation::default())?;
        let mut e = Self::from_document(doc, root)?;
        e.session = Session::new(
            e.session.model().clone(),
            e.session.snapshot().origins,
            false,
        );
        // Restoring never overwrites a potentially changed source; user chooses a new destination.
        Ok(e)
    }
}
impl SaveJob {
    pub fn native_bytes(&self) -> Result<Vec<u8>, String> {
        native::encode(
            &self.snapshot,
            &self.source.original,
            &self.source.info.format,
            Some(&self.template),
        )
    }
    pub fn checkpoint(&self) -> Result<(), String> {
        self.recovery.write(
            self.revision,
            &self.native_bytes()?,
            (!self.destination.as_os_str().is_empty()).then(|| self.destination.clone()),
            self.expected.clone(),
        )
    }
    pub fn run(&self) -> Result<Saved, String> {
        let extension = self
            .destination
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let bytes = match extension.as_str() {
            "joydeck" => self.native_bytes()?,
            "pptx" => {
                if self.source.info.format == "pptx" {
                    let bindings = self
                        .snapshot
                        .origins
                        .iter()
                        .map(|(id, o)| joydeck_pptx::SlideBinding {
                            slide: *id,
                            source: o.slide,
                            shapes: o.shapes.clone(),
                        })
                        .collect::<Vec<_>>();
                    joydeck_pptx::export_preserving(
                        &self.source.original,
                        &self.source.presentation,
                        &self.snapshot.model,
                        &bindings,
                    )
                    .map_err(|e| e.to_string())?
                } else {
                    joydeck_pptx::export(&self.snapshot.model).map_err(|e| e.to_string())?
                }
            }
            _ => {
                return Err(
                    "save as .joydeck v2 or .pptx; legacy .deckcraft is read/import only".into(),
                );
            }
        };
        // Reopen under the reader budgets before publishing anything.
        let reopened = Document::from_bytes(
            "prepared output".into(),
            bytes.clone(),
            &Cancellation::default(),
        )?;
        if reopened.presentation.slides.len() != self.snapshot.model.slides.len() {
            return Err("prepared output page count mismatch".into());
        }
        for (index, (expected, actual)) in self
            .snapshot
            .model
            .slides
            .iter()
            .zip(&reopened.presentation.slides)
            .enumerate()
        {
            if expected.shapes.len() != actual.shapes.len() {
                return Err(format!(
                    "page {} cannot round-trip its object tree; save .joydeck instead",
                    index + 1
                ));
            }
            let before = joydeck_model::resolve::Ctx::for_slide(&self.snapshot.model, expected)
                .ok_or("missing source layout")?;
            let after = joydeck_model::resolve::Ctx::for_slide(&reopened.presentation, actual)
                .ok_or("missing output layout")?;
            for (a, b) in expected.shapes.iter().zip(&actual.shapes) {
                let x = joydeck_model::resolve::xfrm(&before, a);
                let y = joydeck_model::resolve::xfrm(&after, b);
                let same = [(x.x, y.x), (x.y, y.y), (x.w, y.w), (x.h, y.h)]
                    .iter()
                    .all(|(a, b)| (a - b).abs() < 0.001)
                    && ((x.rot - y.rot + 180.).rem_euclid(360.) - 180.).abs() < 0.001
                    && x.flip_h == y.flip_h
                    && x.flip_v == y.flip_v;
                if !same || a.text.as_ref().map(|t| t.text()) != b.text.as_ref().map(|t| t.text()) {
                    return Err(format!(
                        "page {} object {} cannot preserve edited geometry/text; save .joydeck instead",
                        index + 1,
                        a.id
                    ));
                }
            }
        }
        let fingerprint = storage::save(&self.destination, &bytes, self.expected.clone())?;
        let warning = self.recovery.clean(self.revision).err();
        Ok(Saved {
            fingerprint,
            content_id: self.snapshot.content_id,
            revision: self.revision,
            path: self.destination.clone(),
            warning,
        })
    }
}
