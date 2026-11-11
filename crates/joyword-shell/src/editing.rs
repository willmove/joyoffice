//! GPUI-independent input adapter. UTF-16 conversion and transaction rules follow JoyOffice.
use crate::{
    reader::{Reader, guarded},
    storage::{self, ExportPlan, SourceStamp},
};
pub use joyword_engine::doc::Pos as DocPos;
pub use joyword_engine::doc::StoryRef::Body as BodyStory;
use joyword_engine::{
    EditSnapshot, Selection,
    doc::{Pos, ime_text::VisibleParagraph},
};
pub use joyword_geom::Rect as PageRect;
use serde_json::json;
use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) struct Composition {
    snapshot: EditSnapshot,
    start: Pos,
    end: Pos,
    text: String,
}
#[derive(Clone, Debug)]
pub struct InputContext {
    pub text: String,
    pub selection: Range<usize>,
    pub reversed: bool,
    at: Pos,
    visible_start: usize,
}

/// Exact UTF-16 boundaries only. Shared host rule: never split a surrogate pair.
pub fn utf16_bytes(text: &str, range: Range<usize>) -> Result<Range<usize>, String> {
    if range.start > range.end {
        return Err("Invalid text range".into());
    }
    let byte = |wanted| -> Result<usize, String> {
        let mut n = 0;
        for (i, ch) in text.char_indices() {
            if n == wanted {
                return Ok(i);
            }
            if n + ch.len_utf16() > wanted {
                return Err("Range splits a UTF-16 surrogate pair".into());
            }
            n += ch.len_utf16();
        }
        if n == wanted {
            Ok(text.len())
        } else {
            Err("Text range is outside the current context".into())
        }
    };
    Ok(byte(range.start)?..byte(range.end)?)
}
fn boundary(text: &str, at: usize) -> bool {
    at == text.len() || text.grapheme_indices(true).any(|(i, _)| i == at)
}
impl Reader {
    pub fn input_context(&self) -> Option<InputContext> {
        let sel = &self.session.sel;
        let w = self
            .session
            .doc
            .ime_surrounding(&sel.focus, &sel.anchor, 4000)?;
        let view = VisibleParagraph::new(self.session.doc.para_at(&sel.focus)?);
        let visible = view.visible_offset(sel.focus.off)?;
        let anchor = w.text.get(..w.anchor)?.encode_utf16().count();
        let focus = w.text.get(..w.cursor)?.encode_utf16().count();
        Some(InputContext {
            text: w.text,
            selection: anchor.min(focus)..anchor.max(focus),
            reversed: focus < anchor,
            at: sel.focus.clone(),
            visible_start: visible.checked_sub(w.cursor)?,
        })
    }
    fn context_positions(&self, range: Range<usize>) -> Result<(Pos, Pos), String> {
        let context = self
            .input_context()
            .ok_or("Current selection has no surrounding-text mapping")?;
        let bytes = utf16_bytes(&context.text, range)?;
        if !boundary(&context.text, bytes.start) || !boundary(&context.text, bytes.end) {
            return Err("Text range splits a grapheme cluster".into());
        }
        let view = VisibleParagraph::new(
            self.session
                .doc
                .para_at(&context.at)
                .ok_or("Paragraph is unavailable")?,
        );
        let mut a = context.at.clone();
        let mut b = a.clone();
        a.off = view
            .stored_offset_after(context.visible_start + bytes.start)
            .ok_or("Unrepresentable range start")?;
        b.off = view
            .stored_offset_before(context.visible_start + bytes.end)
            .ok_or("Unrepresentable range end")?;
        if bytes.is_empty() {
            b = a.clone();
        }
        Ok((a, b))
    }
    fn positions_range(&self, a: &Pos, b: &Pos) -> Option<Range<usize>> {
        let c = self.input_context()?;
        if c.at.story != a.story || c.at.path != a.path || a.path != b.path {
            return None;
        }
        let view = VisibleParagraph::new(self.session.doc.para_at(a)?);
        let x = view.visible_offset(a.off)?.checked_sub(c.visible_start)?;
        let y = view.visible_offset(b.off)?.checked_sub(c.visible_start)?;
        Some(c.text.get(..x)?.encode_utf16().count()..c.text.get(..y)?.encode_utf16().count())
    }
    pub fn marked_range(&self) -> Option<Range<usize>> {
        let c = self.composition.as_ref()?;
        self.positions_range(&c.start, &c.end)
    }
    pub fn native_positions(&self, range: Range<usize>) -> Result<(Pos, Pos), String> {
        self.context_positions(range)
    }
    pub fn native_index(&self, pos: &Pos) -> Option<usize> {
        self.positions_range(pos, pos).map(|r| r.start)
    }
    pub fn is_composing(&self) -> bool {
        self.composition.is_some()
    }
    pub fn is_dirty(&self) -> bool {
        self.composition
            .as_ref()
            .map_or(self.session.dirty, |c| c.snapshot.dirty())
    }
    pub fn enable_desktop_recovery(&mut self) -> Result<(), String> {
        if let Some(root) = crate::prefs::default_config_dir() {
            self.session
                .enable_recovery(root.join("migration-preview/r3-recovery"), Arc::new(|| {}))?;
        }
        Ok(())
    }
    pub fn enable_desktop_recovery_with_interval(
        &mut self,
        interval: Option<std::time::Duration>,
    ) -> Result<(), String> {
        if let Some(root) = crate::prefs::default_config_dir() {
            self.session.enable_recovery_with_interval(
                root.join("migration-preview/r3-recovery"),
                Arc::new(|| {}),
                interval,
            )?;
        }
        Ok(())
    }
    pub fn recovery_tick(&mut self) {
        if self.is_composing() {
            return;
        }
        let before = self.session.rev();
        let identity = self.session.document_id();
        self.session.recovery_tick();
        let replaced = identity != self.session.document_id();
        if replaced {
            // Revisions are local to a document: a restored draft can start
            // at the same revision as the document it replaces or parks.
            self.recovered_package = None;
            self.recovery_notice = None;
            self.summary.name = "Untitled".into();
        }
        if replaced || before != self.session.rev() {
            self.refresh();
            self.source = None;
        }
    }
    pub fn finish_recovery(&mut self, discard: bool) {
        self.cancel_composition();
        if discard {
            self.session.discard_recovery_copy();
        }
        self.session.finish_recovery();
    }
    pub fn recovery_first(&self) -> Option<String> {
        self.session
            .recovery_documents()
            .first()
            .map(|d| d.id.clone())
    }
    pub fn can_undo(&self) -> bool {
        !self.session.view.read_mode && self.session.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        !self.session.view.read_mode && self.session.can_redo()
    }
    pub fn source_path(&self) -> Option<&Path> {
        self.source.as_ref().map(SourceStamp::path)
    }
    pub fn select_positions(&mut self, a: Pos, b: Pos) -> Result<(), String> {
        self.cancel_composition();
        self.session.close_typing();
        self.session
            .run("select.range", &json!({"anchor":a,"focus":b}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn composition_text(&self, range: Option<Range<usize>>, text: &str) -> Result<String, String> {
        let Some(c) = self.composition.as_ref() else {
            return Ok(text.into());
        };
        let Some(range) = range else {
            return Ok(text.into());
        };
        let marked = self.marked_range().ok_or("Composition context expired")?;
        if range.start < marked.start || range.end > marked.end {
            return Err("Composition replacement is outside marked text".into());
        }
        let bytes = utf16_bytes(
            &c.text,
            range.start - marked.start..range.end - marked.start,
        )?;
        let mut replacement = c.text.clone();
        replacement.replace_range(bytes, text);
        Ok(replacement)
    }
    pub fn replace_text(&mut self, range: Option<Range<usize>>, text: &str) -> Result<(), String> {
        if self.session.view.read_mode {
            return Err("Switch to Print Layout to edit".into());
        }
        if text.len() > 64 * 1024 {
            return Err("Text input exceeds 64 KiB".into());
        }
        let composing = self.is_composing();
        let text = self.composition_text(range.clone(), text)?;
        if composing {
            self.restore_preedit();
            self.session.close_typing();
        } else if let Some(range) = range {
            // A cross-paragraph selection has no paragraph surrounding context; its empty
            // native replacement range means replace the engine selection, not the first byte.
            if !(range.is_empty() && self.input_context().is_none()) {
                let (a, b) = self.context_positions(range)?;
                self.session.sel = Selection {
                    anchor: a,
                    focus: b,
                };
            }
        }
        let inserted = guarded(|| {
            self.session
                .run_ime(
                    "text.insert",
                    &json!({"text":text,"raw":true,"paragraphs":text.contains('\n')}),
                )
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
        if inserted.is_err() && composing {
            self.refresh();
        }
        inserted?;
        if composing {
            self.session.close_typing();
        }
        self.refresh();
        Ok(())
    }
    pub fn mark_text(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
    ) -> Result<(), String> {
        if self.session.view.read_mode {
            return Err("Switch to Print Layout to edit".into());
        }
        if text.is_empty() {
            self.cancel_composition();
            return Ok(());
        }
        let composing = self.is_composing();
        let prefix = if composing {
            range
                .as_ref()
                .and_then(|r| self.marked_range().map(|m| r.start.saturating_sub(m.start)))
                .unwrap_or(0)
        } else {
            0
        };
        let text = self.composition_text(range.clone(), text)?;
        if text.len() > 1024 || text.contains(['\n', '\r']) {
            return Err("IME preedit exceeds the supported paragraph window".into());
        }
        let selection = selected
            .map(|r| r.start + prefix..r.end + prefix)
            .unwrap_or_else(|| text.encode_utf16().count()..text.encode_utf16().count());
        let selected = utf16_bytes(&text, selection)?;
        if composing {
            self.restore_preedit();
        } else if let Some(range) = range
            && !(range.is_empty() && self.input_context().is_none())
        {
            let (a, b) = self.context_positions(range)?;
            self.session.sel = Selection {
                anchor: a,
                focus: b,
            };
        }
        self.session.close_typing();
        let snapshot = self.session.edit_snapshot();
        let start = self.session.sel.ordered().0;
        let result = self
            .session
            .run_ime("text.insert", &json!({"text":text,"raw":true}))
            .map_err(|e| e.to_string());
        if let Err(e) = result {
            self.session.restore(snapshot);
            self.refresh();
            return Err(e);
        }
        let end = self.session.sel.focus.clone();
        if start.path != end.path || end.off < text.len() {
            self.session.restore(snapshot);
            self.refresh();
            return Err("IME preedit cannot span paragraphs".into());
        }
        let mut begin = end.clone();
        begin.off -= text.len();
        let mut a = begin.clone();
        a.off += selected.start;
        let mut b = begin.clone();
        b.off += selected.end;
        self.session.sel = Selection {
            anchor: a,
            focus: b,
        };
        self.composition = Some(Composition {
            snapshot,
            start: begin,
            end,
            text,
        });
        self.refresh();
        Ok(())
    }
    pub fn cancel_composition(&mut self) -> bool {
        if self.restore_preedit() {
            self.refresh();
            true
        } else {
            false
        }
    }
    fn restore_preedit(&mut self) -> bool {
        if let Some(c) = self.composition.take() {
            self.session.restore(c.snapshot);
            true
        } else {
            false
        }
    }
    pub fn commit_composition(&mut self) -> Result<(), String> {
        if self.composition.take().is_some() {
            self.session.close_typing();
        }
        Ok(())
    }
    pub fn edit_command(&mut self, id: &str, params: &serde_json::Value) -> Result<(), String> {
        self.execute(id, params).map(|_| ())
    }
    pub fn paste_clipboard_image(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Clipboard image exceeds 16 MiB".into());
        }
        let data = joyword_engine::cmd::insert::base64_encode(bytes);
        self.edit_command("insert.picture", &serde_json::json!({"data": data}))
    }
    pub fn save_job(&self, destination: PathBuf, new_file: bool) -> Result<SaveJob, String> {
        self.require_editable_source()?;
        if self.is_composing() {
            return Err("Finish or cancel the input-method composition before saving".into());
        }
        if !destination
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("docx"))
        {
            return Err("Save requires a DOCX path".into());
        }
        if !new_file
            && (self.source.is_none()
                || self
                    .source_path()
                    .and_then(|p| std::fs::canonicalize(p).ok())
                    != std::fs::canonicalize(&destination).ok())
        {
            return Err("Original save path does not match the opened source".into());
        }
        Ok(SaveJob {
            document: self.session.document_id(),
            doc: self.session.doc.clone(),
            version: self.session.rev(),
            source: self.source.clone(),
            recovered_package: self
                .recovered_package
                .as_ref()
                .filter(|(revision, _)| *revision == self.session.rev())
                .map(|(_, bytes)| bytes.clone()),
            destination,
            new_file,
            confirmed: false,
        })
    }
    pub fn apply_save(&mut self, receipt: SaveReceipt) {
        if self.session.document_id() != receipt.document {
            return;
        }
        if self.session.rev() == receipt.version && !self.is_composing() {
            self.session.dirty = false;
        }
        self.summary.name = receipt
            .stamp
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.session.path = Some(receipt.stamp.path().to_path_buf());
        self.source = Some(receipt.stamp);
        self.recovered_package = None;
        self.recovery_notice = None;
    }
}
pub struct SaveJob {
    document: u64,
    doc: joyword_engine::doc::Document,
    version: u64,
    source: Option<SourceStamp>,
    recovered_package: Option<Arc<[u8]>>,
    pub destination: PathBuf,
    new_file: bool,
    confirmed: bool,
}
pub struct SaveReceipt {
    document: u64,
    version: u64,
    stamp: SourceStamp,
}
impl SaveJob {
    /// Only after an explicit caller decision about the listed metadata limitations.
    pub fn confirm_original(mut self) -> Self {
        self.confirmed = true;
        self
    }
    pub fn run(self) -> Result<SaveReceipt, String> {
        guarded(|| {
            let origin = self.source.as_ref().map(|s| Arc::from(s.bytes()));
            let plan = if let Some(bytes) = self.recovered_package {
                ExportPlan::from_recovered_package(&bytes, self.version)?
            } else {
                ExportPlan::build(&self.doc, self.version, origin)?
            };
            if self.new_file {
                storage::save_export_copy(&plan, &self.destination).map_err(|e| e.to_string())?;
            } else {
                storage::save_original_with_choice(
                    &plan,
                    self.source.as_ref().ok_or("Original file is unavailable")?,
                    None,
                    if self.confirmed {
                        storage::OriginalChoice::Confirm
                    } else {
                        storage::OriginalChoice::Decline
                    },
                )
                .map_err(|e| e.to_string())?;
            }
            let stamp = storage::open_source(&self.destination, &Default::default())
                .map_err(|e| e.to_string())?
                .into_stamp();
            if stamp.bytes() != plan.bytes() {
                return Err(
                    "Saved file changed before read-back; current edits remain unsaved".into(),
                );
            }
            Ok(SaveReceipt {
                document: self.document,
                version: self.version,
                stamp,
            })
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
mod tests {
    use super::*;
    use crate::reader::{Reader, WELCOME};
    use std::sync::atomic::AtomicBool;
    fn blank() -> Reader {
        Reader::blank(&AtomicBool::new(false)).unwrap()
    }
    #[test]
    fn ime_updates_cancel_commit_and_one_undo() {
        let mut r = blank();
        r.replace_text(None, "A 😀e\u{301}Z").unwrap();
        r.session.close_typing();
        r.select_positions(Pos::body(0, 2), Pos::body(0, 6))
            .unwrap();
        let before = r.body_text();
        let depth = r.session.undo_depth();
        r.mark_text(None, "ni", None).unwrap();
        r.mark_text(None, "nihao", None).unwrap();
        assert_eq!(r.body_text(), "A nihaoe\u{301}Z");
        assert!(r.is_composing());
        assert!(r.save_job(PathBuf::from("x.docx"), true).is_err());
        assert!(r.cancel_composition());
        assert_eq!(r.body_text(), before);
        assert_eq!(r.session.undo_depth(), depth);
        r.mark_text(None, "zhong", None).unwrap();
        r.replace_text(None, "中文").unwrap();
        assert_eq!(r.body_text(), "A 中文e\u{301}Z");
        assert_eq!(r.session.undo_depth(), depth + 1);
        r.edit_command("edit.undo", &json!({})).unwrap();
        assert_eq!(r.body_text(), before);
        r.edit_command("edit.redo", &json!({})).unwrap();
        assert_eq!(r.body_text(), "A 中文e\u{301}Z");
    }
    #[test]
    fn utf16_clusters_and_table_positions_never_split_characters() {
        assert!(utf16_bytes("a😀b", 2..3).is_err());
        let mut r = blank();
        r.replace_text(None, "a😀e\u{301}👨‍👩‍👧‍👦z").unwrap();
        assert!(r.replace_text(Some(4..5), "x").is_err()); // combining sequence
        assert!(r.replace_text(Some(1..2), "x").is_err()); // surrogate
        r.replace_text(Some(1..3), "中").unwrap();
        assert!(r.body_text().starts_with("a中e\u{301}"));
        r.edit_command("select.all", &json!({})).unwrap();
        r.replace_text(None, "中文\nSecond paragraph").unwrap();
        r.edit_command("select.all", &json!({})).unwrap();
        r.replace_text(Some(0..0), "跨段替换").unwrap();
        assert_eq!(r.body_text(), "跨段替换");
    }
    #[test]
    fn original_save_roundtrip_conflicts_locks_and_late_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("原件.docx");
        std::fs::write(&path, WELCOME).unwrap();
        let mut r = Reader::open(&path, &AtomicBool::new(false)).unwrap();
        r.edit_command("select.all", &json!({})).unwrap();
        r.replace_text(None, "中文保存 😀").unwrap();
        #[cfg(windows)]
        assert!(r.save_job(path.clone(), false).unwrap().run().is_err());
        let saved = r
            .save_job(path.clone(), false)
            .unwrap()
            .confirm_original()
            .run()
            .unwrap();
        r.apply_save(saved);
        assert!(!r.is_dirty());
        let back = Reader::open(&path, &AtomicBool::new(false)).unwrap();
        assert_eq!(back.body_text(), "中文保存 😀");
        r.replace_text(None, "A").unwrap();
        let job = r.save_job(path.clone(), false).unwrap();
        r.replace_text(None, "B").unwrap();
        r.apply_save(job.confirm_original().run().unwrap());
        assert!(r.is_dirty());
        assert!(r.body_text().ends_with("AB"));
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"external changed").unwrap();
        assert!(r.save_job(path.clone(), false).unwrap().run().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"external changed");
        assert!(r.is_dirty());
        std::fs::write(&path, original).unwrap();
        let r = Reader::open(&path, &AtomicBool::new(false)).unwrap();
        std::fs::write(dir.path().join("~$原件.docx"), b"other editor").unwrap();
        assert!(r.save_job(path.clone(), false).unwrap().run().is_err());
    }
    #[test]
    fn copy_save_is_verified_new_only_and_failure_keeps_dirty() {
        let mut r = blank();
        r.replace_text(None, "新文件\n保存后重开").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("新 文档.docx");
        let receipt = r.save_job(path.clone(), true).unwrap().run().unwrap();
        r.apply_save(receipt);
        assert!(!r.is_dirty());
        let bytes = std::fs::read(&path).unwrap();
        r.replace_text(None, " modified").unwrap();
        assert!(r.save_job(path.clone(), true).unwrap().run().is_err());
        assert!(r.is_dirty());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let missing = dir.path().join("missing/no.docx");
        assert!(r.save_job(missing.clone(), true).unwrap().run().is_err());
        assert!(!missing.exists());
        assert!(r.is_dirty());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
mod additional_input_tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn settle_recovery(reader: &mut Reader) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            reader.recovery_tick();
            if reader.session.recovery_ready() && !reader.session.recovery_busy() {
                assert!(reader.session.recovery_error().is_none());
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "recovery worker timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    fn restore_another_document(reader: &mut Reader, root: &Path) {
        let id = "0123456789abcdef0123456789abcdef";
        let directory = root.join(id);
        std::fs::create_dir_all(&directory).unwrap();
        let doc = joyword_engine::doc::Document::from_text("Recovered draft, not the old document");
        let snapshot = json!({"version":1, "generation":1,
            "info":{"id":id, "name":"Draft", "savedAt":"2026-11-06T00:00:00Z"},
            "document":doc});
        std::fs::write(
            directory.join("snapshot-1.json"),
            serde_json::to_vec(&snapshot).unwrap(),
        )
        .unwrap();
        reader
            .session
            .enable_recovery_with_interval(root.to_owned(), Arc::new(|| {}), None)
            .unwrap();
        settle_recovery(reader);
        let identity = reader.session.document_id();
        let revision = reader.session.rev();
        reader
            .execute("file.recover", &json!({"restore":id}))
            .unwrap();
        settle_recovery(reader);
        assert_ne!(reader.session.document_id(), identity);
        assert_eq!(
            reader.session.rev(),
            revision,
            "exercise equal revisions across documents"
        );
        assert_eq!(reader.body_text(), "Recovered draft, not the old document");
    }

    #[test]
    fn async_restore_cannot_reuse_a_previous_documents_source() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original.docx");
        let bytes = joyword_docx::write(&joyword_engine::doc::Document::from_text(
            "Original document",
        ))
        .unwrap();
        std::fs::write(&original, &bytes).unwrap();
        let mut reader = Reader::open(&original, &AtomicBool::new(false)).unwrap();
        let presentation = reader.presentation_revision();
        restore_another_document(&mut reader, &dir.path().join("recovery"));
        assert!(reader.source_path().is_none());
        assert!(reader.save_job(original.clone(), false).is_err());
        assert!(reader.presentation_revision() > presentation);
        assert_eq!(reader.summary.name, "Untitled");
        assert_eq!(std::fs::read(original).unwrap(), bytes);
    }

    #[test]
    fn async_restore_cannot_save_a_previous_legacy_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let original_text = "Legacy checkpoint";
        let bytes =
            joyword_docx::write(&joyword_engine::doc::Document::from_text(original_text)).unwrap();
        let mut record = b"JWREC003".to_vec();
        record.extend_from_slice(&1_u64.to_le_bytes());
        record.extend_from_slice(&[0; 9]);
        for part in [bytes.as_slice(), original_text.as_bytes(), bytes.as_slice()] {
            record.extend_from_slice(&(part.len() as u64).to_le_bytes());
            record.extend_from_slice(part);
        }
        record.extend_from_slice(&crc32fast::hash(&record).to_le_bytes());
        let slot = dir.path().join("recovery-a.jwr");
        std::fs::write(&slot, &record).unwrap();
        let mut reader = Reader::recover_legacy(&slot, &AtomicBool::new(false)).unwrap();
        assert!(reader.recovered_package.is_some());
        restore_another_document(&mut reader, &dir.path().join("recovery"));
        let saved = dir.path().join("restored.docx");
        let receipt = reader.save_job(saved.clone(), true).unwrap().run().unwrap();
        let reopened = Reader::open(&saved, &AtomicBool::new(false)).unwrap();
        assert_eq!(reopened.body_text(), reader.body_text());
        assert!(reader.recovered_package.is_none());
        assert!(reader.recovery_notice.is_none());
        reader.apply_save(receipt);
        assert!(!reader.is_dirty());
        assert_eq!(std::fs::read(slot).unwrap(), record);
    }

    #[test]
    fn empty_mark_cancels_and_cross_paragraph_composition_replaces_atomically() {
        let mut r = Reader::blank(&AtomicBool::new(false)).unwrap();
        r.replace_text(None, "原文\nSecond").unwrap();
        r.edit_command("select.all", &json!({})).unwrap();
        let original = r.body_text();
        r.mark_text(Some(0..0), "ni", None).unwrap();
        r.mark_text(None, "", None).unwrap();
        assert_eq!(r.body_text(), original);
        r.mark_text(Some(0..0), "ni", None).unwrap();
        r.replace_text(None, "你").unwrap();
        assert_eq!(r.body_text(), "你");
        r.edit_command("edit.undo", &json!({})).unwrap();
        assert_eq!(r.body_text(), original);
    }
    #[test]
    fn save_receipt_cannot_rebind_a_replacement_document() {
        let dir = tempfile::tempdir().unwrap();
        let mut old = Reader::blank(&AtomicBool::new(false)).unwrap();
        old.replace_text(None, "old").unwrap();
        let job = old.save_job(dir.path().join("old.docx"), true).unwrap();
        let mut new = Reader::blank(&AtomicBool::new(false)).unwrap();
        new.replace_text(None, "new").unwrap();
        new.apply_save(job.run().unwrap());
        assert!(new.source_path().is_none());
        assert!(new.is_dirty());
        assert_eq!(new.body_text(), "new");
    }
}
