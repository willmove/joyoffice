//! Save/close continuation wiring follows JoyOffice's version-bound asynchronous jobs.
use super::*;
use std::path::PathBuf;
impl JoyWord {
    pub(super) fn request_close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            self.shell.notice = Some(
                self.shell
                    .tr("正在保存，请等待完成。", "Saving; wait for completion.")
                    .into(),
            );
            cx.notify();
            return;
        }
        if self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.is_dirty() || r.is_composing())
        {
            self.confirm_discard = true;
            self.close_pending = true;
            self.pending_open = None;
            cx.notify();
        } else {
            if let Some(r) = self.canvas.reader.as_mut() {
                r.finish_recovery(false);
            }
            cx.quit();
        }
    }
    pub(super) fn discard_pending(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if let Some(reader) = self.canvas.reader.as_mut() {
            reader.finish_recovery(true);
        }
        self.confirm_discard = false;
        if self.close_pending {
            self.close_pending = false;
            cx.quit();
        } else if let Some(source) = self.pending_open.take() {
            self.open_unchecked(source, cx);
        }
    }
    pub(super) fn save_document(&mut self, save_as: bool, cx: &mut Context<Self>) {
        if self.saving || self.canvas.loading {
            return;
        }
        let Some(reader) = &self.canvas.reader else {
            return;
        };
        let save_as = save_as
            || reader
                .source_path()
                .is_some_and(|p| joyword_shell::advanced::extension(p) != "docx");
        if reader.is_composing() {
            self.shell.notice = Some(
                self.shell
                    .tr(
                        "请先完成或取消中文输入，再保存。",
                        "Finish or cancel the composition before saving.",
                    )
                    .into(),
            );
            cx.notify();
            return;
        }
        if !save_as && let Some(path) = reader.source_path() {
            self.start_save(path.to_path_buf(), false, false, cx);
            return;
        }
        let document = reader.engine().document_id();
        let directory = reader
            .source_path()
            .and_then(|p| p.parent())
            .map(std::path::Path::to_path_buf)
            .or_else(|| {
                std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                    .map(PathBuf::from)
            })
            .unwrap_or_else(|| PathBuf::from("."));
        let suggestion = if reader.summary.name == "Untitled" {
            "Untitled.docx".to_owned()
        } else {
            format!(
                "{}-copy.docx",
                reader.summary.name.trim_end_matches(".docx")
            )
        };
        let receiver = cx.prompt_for_new_path(&directory, Some(&suggestion));
        self.canvas.dialog_task = Some(cx.spawn(async move |view, cx| match receiver.await {
            Ok(Ok(Some(path))) => {
                let _ = view.update(cx, |v, cx| {
                    if v.canvas
                        .reader
                        .as_ref()
                        .is_some_and(|r| r.engine().document_id() == document)
                    {
                        v.start_save(path, true, false, cx);
                    }
                });
            }
            Ok(Ok(None)) => {}
            _ => {
                let _ = view.update(cx, |v, cx| {
                    v.shell.notice = Some("Save dialog failed; document remains unsaved".into());
                    cx.notify();
                });
            }
        }));
    }
    pub(super) fn confirm_original(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self.replace_confirm.take() else {
            return;
        };
        if let Some(r) = &self.canvas.reader
            && (r.engine().document_id(), r.engine().rev()) == token
            && let Some(path) = r.source_path()
        {
            self.start_save(path.to_path_buf(), false, true, cx);
            return;
        }
        self.shell.notice = Some("Document changed; request Save again".into());
        cx.notify();
    }
    fn start_save(
        &mut self,
        path: PathBuf,
        new_file: bool,
        confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(reader) = &self.canvas.reader else {
            return;
        };
        let document = reader.engine().document_id();
        if !new_file && !confirmed {
            let cap = joyword_shell::storage::original_replacement_capabilities();
            use joyword_shell::storage::MetadataCapability::Preservable;
            if [
                cap.ownership,
                cap.acl,
                cap.extended_attributes,
                cap.creation_time,
                cap.provenance_marks,
            ]
            .iter()
            .any(|c| *c != Preservable)
            {
                self.replace_confirm = Some((document, reader.engine().rev()));
                cx.notify();
                return;
            }
        }
        let job = match reader.save_job(path, new_file) {
            Ok(job) => job,
            Err(e) => {
                self.shell.notice = Some(e);
                cx.notify();
                return;
            }
        };
        let job = if confirmed {
            job.confirm_original()
        } else {
            job
        };
        self.replace_confirm = None;
        self.saving = true;
        self.shell.notice = Some(
            self.shell
                .tr("正在校验并保存 DOCX…", "Verifying and saving DOCX…")
                .into(),
        );
        let work = cx.background_executor().spawn(async move { job.run() });
        self.save_task = Some(cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |v, cx| {
                v.saving = false;
                if let Some(reader) = v
                    .canvas
                    .reader
                    .as_mut()
                    .filter(|r| r.engine().document_id() == document)
                {
                    match result {
                        Ok(receipt) => {
                            reader.apply_save(receipt);
                            v.shell.notice = Some(
                                v.shell
                                    .tr(
                                        "DOCX 已保存并通过读回校验。",
                                        "DOCX saved and read-back verified.",
                                    )
                                    .into(),
                            );
                            if let Some(path) = reader.source_path()
                                && let Err(error) = v.shell.remember_document(path)
                            {
                                v.shell.notice = Some(format!(
                                    "{} {error}",
                                    v.shell.tr("文档已保存。", "Document saved.")
                                ));
                            }
                            if !reader.is_dirty() && v.confirm_discard {
                                v.confirm_discard = false;
                                if v.close_pending {
                                    if let Some(r) = v.canvas.reader.as_mut() {
                                        r.finish_recovery(false);
                                    }
                                    cx.quit();
                                } else if let Some(source) = v.pending_open.take() {
                                    v.open_unchecked(source, cx);
                                }
                            }
                        }
                        Err(error) => {
                            v.shell.notice = Some(format!(
                                "{} {error}",
                                v.shell
                                    .tr("保存失败，修改仍保留。", "Save failed; changes retained.")
                            ))
                        }
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
