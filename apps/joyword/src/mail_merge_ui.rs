//! Native file-picker / mapping / preview workflow over the shared local job.
use crate::*;
use joyword_shell::mail_merge::{CsvData, Limits, MergeError, MergeTemplate, read_local};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

fn path_options(prompt: String, directory: bool) -> gpui::PathPromptOptions {
    gpui::PathPromptOptions {
        files: !directory,
        directories: directory,
        multiple: false,
        prompt: Some(prompt.into()),
    }
}

impl JoyWord {
    pub(crate) fn begin_mail_merge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.merge_cancel {
            cancel.store(true, Ordering::Release);
            self.copy_note = Some(self.label("merge.cancelling"));
            cx.notify();
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.merge_cancel = Some(cancel.clone());
        let locale = self.locale;
        let first = cx.prompt_for_paths(path_options(self.label("merge.choose_template"), false));
        cx.spawn_in(window, async move |view, cx| {
            let operation = async {
                let template_path = first
                    .await
                    .map_err(|_| MergeError::Cancelled)?
                    .map_err(|e| MergeError::Document(e.to_string()))?
                    .and_then(|mut paths| paths.pop())
                    .ok_or(MergeError::Cancelled)?;
                let second = view
                    .update_in(cx, |v, _, cx| {
                        cx.prompt_for_paths(path_options(v.label("merge.choose_data"), false))
                    })
                    .map_err(|_| MergeError::Cancelled)?;
                let data_path = second
                    .await
                    .map_err(|_| MergeError::Cancelled)?
                    .map_err(|e| MergeError::Document(e.to_string()))?
                    .and_then(|mut paths| paths.pop())
                    .ok_or(MergeError::Cancelled)?;
                let (template, data) = cx
                    .background_spawn(async move {
                        let limits = Limits::default();
                        let template = MergeTemplate::parse(
                            &read_local(&template_path, limits.template_bytes)?,
                            &limits,
                        )?;
                        let data =
                            CsvData::parse(&read_local(&data_path, limits.csv_bytes)?, &limits)?;
                        Ok::<_, MergeError>((template, data))
                    })
                    .await?;
                let mut mapping = BTreeMap::new();
                for field in template.fields() {
                    let mut column = data
                        .headers()
                        .iter()
                        .position(|name| name == field)
                        .unwrap_or(0);
                    loop {
                        if cancel.load(Ordering::Acquire) {
                            return Err(MergeError::Cancelled);
                        }
                        let name: String = data.headers()[column].chars().take(128).collect();
                        let number = (column + 1).to_string();
                        let receiver = view
                            .update_in(cx, |v, window, cx| {
                                let message = Catalog::shipped(locale).format(
                                    "merge.mapping",
                                    &[("field", field), ("column", &name), ("number", &number)],
                                );
                                window.prompt(
                                    gpui::PromptLevel::Info,
                                    &v.label("merge.title"),
                                    Some(&message),
                                    &[
                                        gpui::PromptButton::ok(v.label("merge.use_column")),
                                        gpui::PromptButton::new(v.label("merge.next_column")),
                                        gpui::PromptButton::cancel(v.label("dialog.cancel")),
                                    ],
                                    cx,
                                )
                            })
                            .map_err(|_| MergeError::Cancelled)?;
                        match receiver.await.ok() {
                            Some(0) => {
                                mapping.insert(field.clone(), data.headers()[column].clone());
                                break;
                            }
                            Some(1) => column = (column + 1) % data.headers().len(),
                            _ => return Err(MergeError::Cancelled),
                        }
                    }
                }
                let job = Arc::new(template.bind(data, &mapping)?);
                let mut row = 0;
                loop {
                    if cancel.load(Ordering::Acquire) {
                        return Err(MergeError::Cancelled);
                    }
                    let preview_job = job.clone();
                    let preview = cx
                        .background_spawn(async move { preview_job.preview(row) })
                        .await?;
                    let mut text: String = preview.text.chars().take(3000).collect();
                    if preview.text.chars().count() > 3000 {
                        text.push_str(&Catalog::shipped(locale).text("merge.truncated"));
                    }
                    let number = (row + 1).to_string();
                    let count = job.rows().to_string();
                    let receiver = view
                        .update_in(cx, |v, window, cx| {
                            let message = Catalog::shipped(locale).format(
                                "merge.preview",
                                &[("row", &number), ("count", &count), ("text", &text)],
                            );
                            window.prompt(
                                gpui::PromptLevel::Info,
                                &v.label("merge.title"),
                                Some(&message),
                                &[
                                    gpui::PromptButton::ok(v.label("merge.next_row")),
                                    gpui::PromptButton::new(v.label("merge.generate")),
                                    gpui::PromptButton::cancel(v.label("dialog.cancel")),
                                ],
                                cx,
                            )
                        })
                        .map_err(|_| MergeError::Cancelled)?;
                    match receiver.await.ok() {
                        Some(0) => row = (row + 1) % job.rows(),
                        Some(1) => break,
                        _ => return Err(MergeError::Cancelled),
                    }
                }
                let destination = view
                    .update_in(cx, |v, _, cx| {
                        cx.prompt_for_paths(path_options(v.label("merge.choose_parent"), true))
                    })
                    .map_err(|_| MergeError::Cancelled)?;
                let parent = destination
                    .await
                    .map_err(|_| MergeError::Cancelled)?
                    .map_err(|e| MergeError::Document(e.to_string()))?
                    .and_then(|mut paths| paths.pop())
                    .ok_or(MergeError::Cancelled)?;
                view.update_in(cx, |v, _, cx| {
                    v.copy_note = Some(v.label("merge.running"));
                    cx.notify();
                })
                .map_err(|_| MergeError::Cancelled)?;
                let token = cancel.clone();
                cx.background_spawn(async move { job.write_batch(&parent, &token, |_| {}) })
                    .await
            }
            .await;
            let completion = view.update_in(cx, |v, window, cx| {
                let mut completion = None;
                v.merge_cancel = None;
                match operation {
                    Ok(receipt) => {
                        let message = Catalog::shipped(locale).format(
                            "merge.saved",
                            &[
                                ("count", &receipt.files.len().to_string()),
                                ("path", &receipt.directory.display().to_string()),
                            ],
                        );
                        v.copy_note = Some(Catalog::shipped(locale).format(
                            "merge.saved_count",
                            &[("count", &receipt.files.len().to_string())],
                        ));
                        completion = Some(window.prompt(
                            gpui::PromptLevel::Info,
                            &v.label("merge.title"),
                            Some(&message),
                            &[gpui::PromptButton::ok(v.label("dialog.ok"))],
                            cx,
                        ));
                        v.input_error = None;
                    }
                    Err(MergeError::Cancelled) => v.copy_note = Some(v.label("merge.cancelled")),
                    Err(error) => {
                        v.input_error = Some(
                            Catalog::shipped(locale)
                                .format("merge.error", &[("reason", &error.to_string())]),
                        )
                    }
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
