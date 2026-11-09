//! Version-bound review and no-overwrite publication for the File export list.
use super::*;
use gpui::component::WindowExt;
use joyword_shell::{
    advanced::{ArchivalPdf, OutputJob, PreparedCopy},
    epub,
    reader::{self, RasterScene},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default)]
pub(super) struct CheckedExport {
    cancel: Option<Arc<AtomicBool>>,
    task: Option<gpui::Task<()>>,
}

impl Drop for CheckedExport {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Release);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Format {
    Copy(&'static str),
    Word2003,
    Png(usize),
    Epub,
    Archive,
}

impl Format {
    pub(super) fn suffix(self) -> &'static str {
        match self {
            Self::Copy(ext) => ext,
            Self::Word2003 => "xml",
            Self::Png(_) => "png",
            Self::Epub => "epub",
            Self::Archive => "pdf",
        }
    }
}

enum Output {
    Copy(PreparedCopy),
    Epub(epub::Plan),
    Archive(ArchivalPdf),
}

impl Output {
    fn publish(&self, path: &Path, cancel: &AtomicBool) -> Result<(), String> {
        match self {
            Self::Copy(copy) => copy.publish(path, cancel),
            Self::Epub(plan) => plan.publish(path, cancel),
            Self::Archive(archive) => archive.publish(path, cancel),
        }
    }
}

enum Preparation<D> {
    Copy(Box<OutputJob>),
    Page(RasterScene, usize),
    Epub(D, Option<joyword_shell::storage::SourceStamp>),
    Archive(D),
}

struct Request {
    label: &'static str,
    target: (u64, u64, u64),
    directory: PathBuf,
    suggestion: String,
    cancel: Arc<AtomicBool>,
}

impl Request {
    fn valid(&self, v: &JoyWord) -> bool {
        !self.cancel.load(Ordering::Acquire)
            && !v.canvas.loading
            && v.canvas.reader.as_ref().is_some_and(|r| {
                (
                    r.engine().document_id(),
                    r.engine().rev(),
                    r.presentation_revision(),
                ) == self.target
                    && !r.is_composing()
            })
    }
}

impl JoyWord {
    pub(super) fn start_checked_export(
        &mut self,
        format: Format,
        label: &'static str,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(r) = self.canvas.reader.as_ref() else {
            return;
        };
        if r.is_composing() {
            self.shell.notice = Some(
                self.shell
                    .tr(
                        "请先完成或取消输入法组合。",
                        "Finish or cancel input-method composition before exporting.",
                    )
                    .into(),
            );
            cx.notify();
            return;
        }
        let preparation = match format {
            Format::Copy(ext) => r
                .export_job(ext, false)
                .map(Box::new)
                .map(Preparation::Copy),
            Format::Word2003 => r.word2003_export_job().map(Box::new).map(Preparation::Copy),
            Format::Png(page) => Ok(Preparation::Page(r.scene(), page)),
            Format::Epub => r
                .export_snapshot()
                .map(|doc| Preparation::Epub(doc, r.epub_export_source())),
            Format::Archive => r.export_snapshot().map(Preparation::Archive),
        };
        let preparation = match preparation {
            Ok(preparation) => preparation,
            Err(error) => {
                self.shell.notice = Some(error);
                cx.notify();
                return;
            }
        };
        let title = Path::new(&r.summary.name)
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("Untitled")
            .to_owned();
        let request = Arc::new(Request {
            label,
            target: (
                r.engine().document_id(),
                r.engine().rev(),
                r.presentation_revision(),
            ),
            directory: r
                .source_path()
                .and_then(Path::parent)
                .unwrap_or(Path::new("."))
                .to_owned(),
            suggestion: match format {
                Format::Png(page) => format!("{title}-page-{}.png", page + 1),
                Format::Word2003 => format!("{title}-word2003.xml"),
                Format::Archive => format!("{title}-archive.pdf"),
                _ => format!("{title}-export.{}", format.suffix()),
            },
            cancel: Arc::new(AtomicBool::new(false)),
        });
        w.focus(&self.focus, cx);
        self.advanced.checked_export = CheckedExport {
            cancel: Some(request.cancel.clone()),
            task: None,
        };
        let preparing = self.shell.tr("正在准备", "Preparing");
        let exchange_note = self.shell.tr(
            "转换可能改变版式，并省略目标格式无法表达的对象、批注或修订。",
            "Conversion may change the layout and omit objects, comments or revisions the destination format cannot represent.",
        );
        let page_note = self
            .shell
            .tr("输出当前可见页", "Exporting the current visible page");
        self.shell.notice = Some(format!("{preparing} {label}…"));
        cx.notify();
        let cancel = request.cancel.clone();
        let prepare = move || {
            reader::guarded(|| {
                if cancel.load(Ordering::Acquire) {
                    return Err("Export cancelled".into());
                }
                match preparation {
                    Preparation::Copy(job) => {
                        let notes = if matches!(format, Format::Copy("docx" | "pdf")) {
                            Vec::new()
                        } else {
                            vec![exchange_note.into()]
                        };
                        Ok((Output::Copy(job.prepare()?), notes))
                    }
                    Preparation::Page(scene, page) => Ok((
                        Output::Copy(scene.prepare_page_image(page, &cancel)?),
                        vec![format!("{page_note} ({})", page + 1)],
                    )),
                    Preparation::Epub(doc, epub_source) => {
                        if let Some(source) = epub_source {
                            epub::validate_docx_source(source.bytes())?;
                        }
                        let now = epub::modified_now()?;
                        let plan = epub::export(&doc, &title, "und", &now)?;
                        let mut counts = BTreeMap::new();
                        for loss in &plan.losses {
                            *counts.entry(loss.detail).or_insert(0usize) += 1;
                        }
                        let mut notes = vec![format!(
                            "{} 段，{} 张图片。请确认以下转换差异。",
                            plan.paragraphs, plan.images
                        )];
                        notes.extend(
                            counts
                                .into_iter()
                                .map(|(detail, count)| format!("{detail} ({count})")),
                        );
                        Ok((Output::Epub(plan), notes))
                    }
                    Preparation::Archive(doc) => {
                        let now = epub::modified_now()?;
                        let year = now
                            .get(..4)
                            .and_then(|s| s.parse().ok())
                            .ok_or("无法确定归档年份")?;
                        let output = ArchivalPdf::prepare(&doc, year)?;
                        Ok((
                            Output::Archive(output),
                            vec![
                                format!(
                                    "归档年份为 {year}（UTC）。请确认你有权嵌入文档使用的字体。"
                                ),
                                "如需确认归档合规性，请使用独立校验器检查输出。".into(),
                            ],
                        ))
                    }
                }
            })
        };
        let task = cx.spawn_in(w, async move |view, cx| {
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    // Match the desktop open/save stack for complex layouts and DOCX verification.
                    std::thread::Builder::new()
                        .name("joyword-desktop-export".into())
                        .stack_size(8 * 1024 * 1024)
                        .spawn(prepare)
                        .map_err(|e| e.to_string())?
                        .join()
                        .map_err(|_| "Export worker failed".to_owned())?
                })
                .await;
            let _ = view.update_in(cx, |v, w, cx| {
                if request.valid(v) {
                    match prepared {
                        Ok((output, notes)) => {
                            v.shell.notice = None;
                            let output = Arc::new(output);
                            if notes.is_empty() {
                                v.publish_checked_export(request, output, cx);
                            } else {
                                v.review_checked_export(request, output, notes, w, cx);
                            }
                        }
                        Err(error) => v.shell.notice = Some(format!("输出失败：{error}")),
                    }
                } else if !request.cancel.load(Ordering::Acquire) {
                    v.shell.notice = Some("文档已变化，请重新导出".into());
                }
                cx.notify();
            });
        });
        self.advanced.checked_export.task = Some(task);
    }

    fn review_checked_export(
        &mut self,
        request: Arc<Request>,
        output: Arc<Output>,
        notes: Vec<String>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity().downgrade();
        let title = format!("{} {}", self.shell.tr("导出", "Export"), request.label);
        let cancel_label = self.shell.tr("取消", "Cancel");
        let save_label = self.shell.tr("选择保存位置…", "Choose where to save…");
        w.open_dialog(cx, move |dialog, _, _| {
            let body = div()
                .flex()
                .flex_col()
                .gap_2()
                .children(notes.iter().cloned().map(|note| div().child(note)));
            let (view, request, output) = (view.clone(), request.clone(), output.clone());
            dialog
                .title(title.clone())
                .width(px(560.))
                .child(body)
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-export-review")
                                .ghost()
                                .label(cancel_label)
                                .on_click(|_, w, cx| w.close_dialog(cx)),
                        )
                        .child(
                            Button::new("confirm-export-review")
                                .primary()
                                .label(save_label)
                                .on_click(move |_, w, cx| {
                                    w.close_dialog(cx);
                                    let _ = view.update(cx, |v, cx| {
                                        w.focus(&v.focus, cx);
                                        v.publish_checked_export(
                                            request.clone(),
                                            output.clone(),
                                            cx,
                                        );
                                    });
                                }),
                        ),
                )
        });
    }

    fn publish_checked_export(
        &mut self,
        request: Arc<Request>,
        output: Arc<Output>,
        cx: &mut Context<Self>,
    ) {
        if !request.valid(self) {
            self.shell.notice = Some("文档已变化，请重新导出".into());
            cx.notify();
            return;
        }
        let prompt = cx.prompt_for_new_path(&request.directory, Some(&request.suggestion));
        self.advanced.checked_export.task = Some(cx.spawn(async move |view, cx| {
            let path = match prompt.await {
                Ok(Ok(Some(path))) => path,
                _ => return,
            };
            let valid = view
                .update(cx, |v, cx| {
                    let valid = request.valid(v);
                    if !valid {
                        v.shell.notice = Some("文档已变化，请重新导出".into());
                        cx.notify();
                    }
                    valid
                })
                .unwrap_or(false);
            if !valid {
                return;
            }
            let cancel = request.cancel.clone();
            let result = cx
                .background_executor()
                .spawn(async move { output.publish(&path, &cancel).map(|()| path) })
                .await;
            let _ = view.update(cx, |v, cx| {
                if !request.cancel.load(Ordering::Acquire) {
                    v.shell.notice = Some(match result {
                        Ok(path) => format!("已输出副本：{}", path.display()),
                        Err(error) => format!("输出失败：{error}"),
                    });
                    cx.notify();
                }
            });
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use serde_json::json;

    #[gpui::test]
    fn checked_export_request_expires_on_edit_or_cancellation(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(JoyWord::new);
        view.update(cx, |v, _| {
            let cancel = Arc::new(AtomicBool::new(false));
            let mut reader = reader::Reader::blank(&cancel).unwrap();
            let request = Request {
                label: "PDF/A-2b",
                target: (
                    reader.engine().document_id(),
                    reader.engine().rev(),
                    reader.presentation_revision(),
                ),
                directory: PathBuf::new(),
                suggestion: String::new(),
                cancel: cancel.clone(),
            };
            reader
                .execute("text.insert", &json!({"text":"changed"}))
                .unwrap();
            v.canvas.reader = Some(reader);
            assert!(!request.valid(v));
            let reader = v.canvas.reader.as_ref().unwrap();
            let current = Request {
                target: (
                    reader.engine().document_id(),
                    reader.engine().rev(),
                    reader.presentation_revision(),
                ),
                cancel: cancel.clone(),
                ..request
            };
            assert!(current.valid(v));
            cancel.store(true, Ordering::Release);
            assert!(!current.valid(v));
        });
    }
}
