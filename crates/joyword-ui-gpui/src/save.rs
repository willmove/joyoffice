//! Save/close continuation wiring follows JoyOffice's version-bound asynchronous jobs.
use super::*;
use gpui::base::FocusTrapElement as _;
use std::path::PathBuf;

fn run_desktop_save(
    job: joyword_shell::editing::SaveJob,
) -> Result<joyword_shell::editing::SaveReceipt, String> {
    // Saving verifies the result with the same nested DOCX reader used by open.
    // macOS executor workers have too little stack for that reader in debug builds.
    std::thread::Builder::new()
        .name("joyword-desktop-save".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || job.run())
        .map_err(|error| format!("Cannot start desktop document save: {error}"))?
        .join()
        .map_err(|_| "Desktop document save worker failed".to_string())?
}
impl JoyWord {
    pub(super) fn render_save_changes_dialog(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let replacing = self.replace_confirm.is_some();
        let message = if replacing {
            self.shell.tr(
                "替换原文件：当前组件无法验证文件所有者、ACL 与全部扩展属性。建议另存副本。",
                "Replace original: ownership, ACLs and all extended attributes cannot be verified. Save a copy is recommended.",
            )
        } else if matches!(self.pending_document, Some(DocumentChange::Open(_))) {
            self.shell.tr(
                "继续前可以先保存此文档。",
                "Save this document before continuing.",
            )
        } else {
            self.shell.tr(
                "关闭文档前可以先保存。",
                "Save this document before closing.",
            )
        };
        // Match JoySheet's close dialog while using JoyWord's active theme.
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(gpui::rgba(0x0e1e1855))
            .flex()
            .items_center()
            .justify_center()
            .on_key_down(cx.listener(|v, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    v.execute(
                        if v.replace_confirm.is_some() {
                            "file.cancel_replace"
                        } else {
                            "file.keep"
                        },
                        window,
                        cx,
                    );
                } else if event.keystroke.key == "enter"
                    && v.discard_focus.is_focused(window)
                    && !v.saving
                {
                    if v.replace_confirm.is_some() {
                        v.confirm_original(cx);
                    } else {
                        v.save_document(false, cx);
                    }
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .id("save-changes-dialog")
                    .debug_selector(|| "save-changes-dialog".into())
                    .track_focus(&self.discard_focus)
                    .role(gpui::Role::Dialog)
                    .aria_label(self.shell.tr("保存更改？", "Save changes?"))
                    .w(px(450.))
                    .max_w_full()
                    .m_4()
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(self.palette().surface))
                    .shadow_xl()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.shell.tr("保存更改？", "Save changes?")),
                    )
                    .child(div().whitespace_normal().child(message))
                    .when_some(self.shell.notice.as_ref(), |el, note| {
                        el.child(
                            div()
                                .id("save-changes-status")
                                .debug_selector(|| "save-changes-status".into())
                                .role(gpui::Role::Status)
                                .text_sm()
                                .whitespace_normal()
                                .child(note.clone()),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("confirm-keep")
                                    .debug_selector(|| "confirm-keep".into())
                                    .disabled(self.saving)
                                    .label(self.shell.tr("取消", "Cancel"))
                                    .on_click(cx.listener(move |v, _, window, cx| {
                                        v.execute(
                                            if replacing {
                                                "file.cancel_replace"
                                            } else {
                                                "file.keep"
                                            },
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("confirm-discard")
                                    .debug_selector(|| "confirm-discard".into())
                                    .disabled(self.saving)
                                    .label(if replacing {
                                        self.shell.tr("另存副本", "Save a copy")
                                    } else {
                                        self.shell.tr("不保存", "Discard")
                                    })
                                    .on_click(cx.listener(move |v, _, window, cx| {
                                        v.execute(
                                            if replacing {
                                                "file.save_as"
                                            } else {
                                                "file.discard"
                                            },
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("confirm-save")
                                    .debug_selector(|| "confirm-save".into())
                                    .primary()
                                    .disabled(self.saving)
                                    .label(if self.saving {
                                        self.shell.tr("保存中…", "Saving…")
                                    } else if replacing {
                                        self.shell.tr("确认替换原文件", "Confirm replace original")
                                    } else {
                                        self.shell.tr("保存", "Save")
                                    })
                                    .on_click(cx.listener(move |v, _, window, cx| {
                                        v.execute(
                                            if replacing {
                                                "file.confirm_original"
                                            } else {
                                                "file.save"
                                            },
                                            window,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .focus_trap("save-changes-focus", &self.discard_focus),
            )
    }

    pub(super) fn save_available(&self) -> bool {
        !self.saving
            && !self.canvas.loading
            && self
                .canvas
                .reader
                .as_ref()
                .is_some_and(|r| !r.is_composing())
    }

    pub(super) fn request_close(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.confirm_discard {
            return;
        }
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
            self.replace_confirm = None;
            self.close_pending = true;
            self.pending_document = None;
            self.shell.notice = None;
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
        } else if let Some(change) = self.pending_document.take() {
            self.apply_document_change(change, cx);
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
        let work = cx
            .background_executor()
            .spawn(async move { run_desktop_save(job) });
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
                                } else if let Some(change) = v.pending_document.take() {
                                    v.apply_document_change(change, cx);
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Modifiers, TestAppContext};
    use joyword_shell::reader::Reader;
    use std::sync::atomic::AtomicBool;

    struct OutputDirectory(PathBuf);
    impl Drop for OutputDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[gpui::test]
    fn closing_dirty_document_can_be_cancelled_or_discarded(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        cx.update(|window, cx| {
            view.update(cx, |v, cx| {
                let mut reader = Reader::blank(&AtomicBool::new(false)).unwrap();
                reader.replace_text(None, "Unsaved changes").unwrap();
                let document_id = reader.engine().document_id();
                v.canvas.reader = Some(reader);
                v.open_file_page(window, cx);

                v.begin_document_change(DocumentChange::Close, cx);
                assert!(v.confirm_discard);
                v.execute("file.keep", window, cx);
                assert_eq!(
                    v.canvas.reader.as_ref().unwrap().engine().document_id(),
                    document_id
                );
                assert!(v.pending_document.is_none());

                v.begin_document_change(DocumentChange::Close, cx);
                v.discard_pending(window, cx);
                assert!(!v.confirm_discard);
                assert!(v.canvas.reader.is_none());
                assert!(v.file_page);
            });
        });
    }

    #[gpui::test]
    fn save_changes_dialog_owns_focus_and_blocks_background_actions(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut editor = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| JoyWord::new(window, cx));
            editor = Some(view.clone());
            gpui::base::Root::new(view, window, cx)
        });
        let view = editor.unwrap();
        view.update(cx, |v, cx| {
            let mut reader = Reader::blank(&AtomicBool::new(false)).unwrap();
            reader.replace_text(None, "Keep these changes").unwrap();
            let id = reader.engine().document_id();
            v.canvas.reader = Some(reader);
            v.install_input(id, cx);
        });
        for file_page in [false, true] {
            view.update(cx, |v, cx| {
                v.file_page = file_page;
                v.begin_document_change(DocumentChange::Close, cx);
            });
            cx.update(|w, cx| w.draw(cx).clear(cx));
            let panel = cx.debug_bounds("save-changes-dialog").unwrap();
            cx.update(|w, cx| {
                assert!(
                    (f32::from(panel.center().x) - f32::from(w.viewport_size().width) / 2.).abs()
                        < 1.
                );
                assert!(
                    (f32::from(panel.center().y) - f32::from(w.viewport_size().height) / 2.).abs()
                        < 1.
                );
                view.read_with(cx, |v, cx| assert!(v.discard_focus.contains_focused(w, cx)));
                // Native menu actions cannot replace the pending close decision.
                w.dispatch_action(Box::new(NewDocument), cx);
            });
            cx.simulate_click(gpui::point(px(10.), px(10.)), Modifiers::default());
            for _ in 0..6 {
                cx.simulate_keystrokes("tab");
                cx.update(|w, cx| {
                    view.read_with(cx, |v, cx| assert!(v.discard_focus.contains_focused(w, cx)));
                });
            }
            cx.simulate_keystrokes("escape");
            cx.update(|w, cx| {
                view.read_with(cx, |v, _| {
                    assert!(!v.confirm_discard);
                    assert!(v.pending_document.is_none());
                    assert!(v.focus.is_focused(w));
                    assert_eq!(
                        v.canvas.reader.as_ref().unwrap().body_text(),
                        "Keep these changes"
                    );
                });
            });
        }
        view.update(cx, |v, cx| {
            v.begin_document_change(DocumentChange::Close, cx)
        });
        cx.update(|w, cx| w.draw(cx).clear(cx));
        let discard = cx.debug_bounds("confirm-discard").unwrap();
        cx.simulate_click(discard.center(), Modifiers::default());
        view.read_with(cx, |v, _| {
            assert!(!v.confirm_discard);
            assert!(v.canvas.reader.is_none());
        });
    }

    #[gpui::test]
    fn save_changes_dialog_retains_failed_save_and_closes_after_success(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = OutputDirectory(
            std::env::temp_dir().join(format!("joyword-close-{}-{suffix}", std::process::id())),
        );
        std::fs::create_dir(&output.0).unwrap();
        let path = output.0.join("saved.docx");
        let mut reader = Reader::blank(&AtomicBool::new(false)).unwrap();
        reader.replace_text(None, "Original").unwrap();
        reader.apply_save(run_desktop_save(reader.save_job(path.clone(), true).unwrap()).unwrap());
        let original = std::fs::read(&path).unwrap();
        reader.replace_text(None, " and unsaved changes").unwrap();
        let expected = reader.body_text();
        view.update(cx, |v, cx| {
            v.canvas.reader = Some(reader);
            v.begin_document_change(DocumentChange::Close, cx);
            // A Save As collision must leave the pending close and edits intact.
            v.start_save(path.clone(), true, false, cx);
        });
        cx.update(|w, cx| {
            view.update(cx, |v, cx| {
                v.execute("file.keep", w, cx);
                v.discard_pending(w, cx);
                assert!(v.confirm_discard);
            });
        });
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(!v.saving && v.confirm_discard);
            assert!(v.canvas.reader.as_ref().unwrap().is_dirty());
            assert_eq!(v.canvas.reader.as_ref().unwrap().body_text(), expected);
            assert!(v.shell.notice.as_ref().unwrap().contains("保存失败"));
        });
        assert_eq!(std::fs::read(&path).unwrap(), original);
        cx.update(|w, cx| w.draw(cx).clear(cx));
        assert!(cx.debug_bounds("save-changes-status").is_some());
        let save = cx.debug_bounds("confirm-save").unwrap();
        cx.simulate_click(save.center(), Modifiers::default());
        cx.update(|w, cx| w.draw(cx).clear(cx));
        if view.read_with(cx, |v, _| v.replace_confirm.is_some()) {
            // The existing original-file confirmation remains reachable in the modal.
            let save = cx.debug_bounds("confirm-save").unwrap();
            cx.simulate_click(save.center(), Modifiers::default());
        }
        cx.run_until_parked();
        view.read_with(cx, |v, _| {
            assert!(!v.confirm_discard && !v.saving);
            assert!(v.canvas.reader.is_none());
            assert!(v.file_page);
        });
        let reopened = Reader::open(&path, &AtomicBool::new(false)).unwrap();
        assert_eq!(reopened.body_text(), expected);
    }

    #[test]
    fn table_save_and_readback_from_small_executor_stack() -> Result<(), Box<dyn std::error::Error>>
    {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("joyword-save-{}-{suffix}", std::process::id()));
        std::fs::create_dir(&directory)?;
        let output = OutputDirectory(directory);
        let path = output.0.join("table.docx");
        let cancel = AtomicBool::new(false);
        let mut reader = Reader::blank(&cancel)?;
        reader.execute("insert.table", &serde_json::json!({"rows":2,"cols":3}))?;
        reader.replace_text(None, "Small-stack table save 中文")?;
        let job = reader.save_job(path.clone(), true)?;
        let receipt = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(move || run_desktop_save(job))?
            .join()
            .map_err(|_| "Small-stack caller failed")??;
        reader.apply_save(receipt);
        assert!(!reader.is_dirty());
        let reopened = Reader::open(&path, &cancel)?;
        assert!(reopened.body_text().contains("Small-stack table save 中文"));
        let original = std::fs::read(&path)?;
        assert!(run_desktop_save(reader.save_job(path.clone(), true)?).is_err());
        assert_eq!(std::fs::read(path)?, original);
        Ok(())
    }
}
