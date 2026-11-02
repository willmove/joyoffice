//! Desktop text-format export through the same plan/publisher as the CLI.
use crate::*;
use joyword_shell::exchange::{self, Format};
use joyword_shell::exchange_output;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn source_is_current(epoch: u64, version: u64, current: Option<(u64, u64)>) -> bool {
    current == Some((epoch, version))
}

impl JoyWord {
    pub(crate) fn begin_text_exchange(
        &mut self,
        format: Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(cancel) = &self.exchange_cancel {
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
        let version = editor.version();
        let epoch = self.copy_epoch;
        if document.version().get() != version {
            self.report_input_error(self.label("exchange.stale"));
            cx.notify();
            return;
        }
        let directory = editor
            .source_path()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let title = self
            .document_title()
            .and_then(|s| {
                std::path::Path::new(&s)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "JoyWord".into());
        let cancel = Arc::new(AtomicBool::new(false));
        self.exchange_cancel = Some(cancel.clone());
        cx.spawn_in(window, async move |view, cx| {
            let result = async {
                let plan = cx
                    .background_spawn(async move {
                        exchange::plan_document(&document, format)
                            .map_err(|_| "exchange.unsupported")
                    })
                    .await?;
                if cancel.load(Ordering::Acquire) {
                    return Ok(None);
                }
                let confirmation = view
                    .update_in(cx, |v, window, cx| {
                        if !source_is_current(
                            epoch,
                            version,
                            v.editor.as_ref().map(|e| (v.copy_epoch, e.version())),
                        ) {
                            return None;
                        }
                        Some(window.prompt(
                            gpui::PromptLevel::Warning,
                            &v.label("exchange.review"),
                            Some(&v.label(if format == Format::Txt {
                                "exchange.plain_losses"
                            } else {
                                "exchange.losses"
                            })),
                            &[
                                gpui::PromptButton::cancel(v.label("dialog.cancel")),
                                gpui::PromptButton::new(v.label("exchange.continue")),
                            ],
                            cx,
                        ))
                    })
                    .map_err(|_| "exchange.closed")?;
                let Some(confirmation) = confirmation else {
                    return Err("exchange.stale");
                };
                if confirmation.await.ok() != Some(1) || cancel.load(Ordering::Acquire) {
                    return Ok(None);
                }
                let suggested = format!(
                    "{title}.{}",
                    if format == Format::Markdown {
                        "md"
                    } else {
                        format.name()
                    }
                );
                let picker = view
                    .update_in(cx, |v, _, cx| {
                        if !source_is_current(
                            epoch,
                            version,
                            v.editor.as_ref().map(|e| (v.copy_epoch, e.version())),
                        ) {
                            return None;
                        }
                        Some(cx.prompt_for_new_path(&directory, Some(&suggested)))
                    })
                    .map_err(|_| "exchange.closed")?;
                let Some(picker) = picker else {
                    return Err("exchange.stale");
                };
                let Some(path) = picker
                    .await
                    .map_err(|_| "exchange.path_error")?
                    .map_err(|_| "exchange.path_error")?
                else {
                    return Ok(None);
                };
                let current = view
                    .update(cx, |v, _| {
                        source_is_current(
                            epoch,
                            version,
                            v.editor.as_ref().map(|e| (v.copy_epoch, e.version())),
                        )
                    })
                    .map_err(|_| "exchange.closed")?;
                if !current {
                    return Err("exchange.stale");
                }
                if cancel.load(Ordering::Acquire) {
                    return Ok(None);
                }
                cx.background_spawn(async move {
                    exchange_output::write_new(&plan, &path, true, &cancel)
                        .map(Some)
                        .map_err(|error| match error {
                            exchange_output::WriteError::InvalidExtension => {
                                "exchange.extension_error"
                            }
                            exchange_output::WriteError::Cancelled => "exchange.cancelled",
                            exchange_output::WriteError::Published { .. } => {
                                "exchange.published_error"
                            }
                            _ => "exchange.write_error",
                        })
                })
                .await
            }
            .await;
            let _ = view.update_in(cx, |v, window, cx| {
                v.exchange_cancel = None;
                match result {
                    Ok(Some(path)) => {
                        let earlier = !source_is_current(
                            epoch,
                            version,
                            v.editor.as_ref().map(|e| (v.copy_epoch, e.version())),
                        );
                        v.copy_note = Some(Catalog::shipped(v.locale).format(
                            if earlier {
                                "exchange.saved_earlier"
                            } else {
                                "exchange.saved"
                            },
                            &[("path", &path.display().to_string())],
                        ));
                        v.input_error = None;
                    }
                    Ok(None) => v.copy_note = Some(v.label("exchange.cancelled")),
                    Err(key) => v.report_input_error(v.label(key)),
                }
                window.focus(&v.focus_handle, cx);
                cx.notify();
            });
        })
        .detach();
    }
}
