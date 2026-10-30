use crate::*;
use joyword_shell::epub::{self, Metadata};
use std::sync::atomic::{AtomicBool, Ordering};

impl JoyWord {
    pub(crate) fn begin_epub(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.epub_cancel {
            cancel.store(true, Ordering::Release);
            return;
        }
        let Some(editor) = self
            .editor
            .as_ref()
            .filter(|e| !e.read_only() && !e.is_composing())
        else {
            return;
        };
        let document = editor.view().snapshot().clone();
        let title = self
            .document_title()
            .and_then(|v| {
                std::path::Path::new(&v)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "JoyWord".into());
        let directory = editor
            .source_path()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let suggested = format!("{title}.epub");
        let epoch = self.copy_epoch;
        let version = editor.version();
        let locale = self.locale;
        let cancel = Arc::new(AtomicBool::new(false));
        self.epub_cancel = Some(cancel.clone());
        cx.spawn_in(window, async move |view, cx| {
            let result = async {
                let plan = cx
                    .background_spawn(async move {
                        let modified = epub::modified_now().map_err(|e| e.to_string())?;
                        epub::export(
                            &document,
                            &Metadata {
                                title: &title,
                                language: "und",
                                modified: &modified,
                            },
                        )
                        .map_err(|e| e.to_string())
                    })
                    .await?;
                if cancel.load(Ordering::Acquire) {
                    return Ok(None);
                }
                let mut losses = plan
                    .losses
                    .iter()
                    .take(20)
                    .map(|loss| format!("{}: {}", loss.location, loss.detail))
                    .collect::<Vec<_>>()
                    .join("\n");
                if plan.losses.len() > 20 {
                    losses.push_str(&format!("\n… {}", plan.losses.len()));
                }
                let confirmation = view
                    .update_in(cx, |v, window, cx| {
                        window.prompt(
                            gpui::PromptLevel::Warning,
                            &v.label("epub.review"),
                            Some(&losses),
                            &[
                                gpui::PromptButton::cancel(v.label("dialog.cancel")),
                                gpui::PromptButton::new(v.label("epub.continue")),
                            ],
                            cx,
                        )
                    })
                    .map_err(|_| "EPUB window closed".to_owned())?;
                if confirmation.await.ok() != Some(1) || cancel.load(Ordering::Acquire) {
                    return Ok(None);
                }
                let picker = view
                    .update_in(cx, |_, _, cx| {
                        cx.prompt_for_new_path(&directory, Some(&suggested))
                    })
                    .map_err(|_| "EPUB window closed".to_owned())?;
                let picked = picker
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;
                let Some(path) = picked else {
                    return Ok(None);
                };
                if cancel.load(Ordering::Acquire) {
                    return Ok(None);
                }
                let token = cancel.clone();
                cx.background_spawn(async move {
                    epub::write_new(&plan, &path, &token)
                        .map(Some)
                        .map_err(|e| e.to_string())
                })
                .await
            }
            .await;
            let completion = view.update_in(cx, |v, window, cx| {
                v.epub_cancel = None;
                let mut completion = None;
                match result {
                    Ok(Some(path)) => {
                        let earlier = v.copy_epoch != epoch
                            || v.editor.as_ref().is_none_or(|e| e.version() != version);
                        let message = Catalog::shipped(locale).format(
                            if earlier {
                                "epub.saved_earlier"
                            } else {
                                "epub.saved"
                            },
                            &[("path", &path.display().to_string())],
                        );
                        v.copy_note = Some(v.label("epub.completed"));
                        completion = Some(window.prompt(
                            gpui::PromptLevel::Info,
                            &v.label("command.file.export_epub"),
                            Some(&message),
                            &[gpui::PromptButton::ok(v.label("dialog.ok"))],
                            cx,
                        ));
                    }
                    Ok(None) => v.copy_note = Some(v.label("epub.cancelled")),
                    Err(error) => v.report_input_error(error),
                }
                window.focus(&v.focus_handle, cx);
                cx.notify();
                completion
            });
            if let Ok(Some(completion)) = completion {
                let _ = completion.await;
            }
        })
        .detach();
    }
}
