use crate::desktop::{After, Desktop, Pending};
use gpui_base::Disableable;
use gpui_kit::{prelude::*, *};
use joysheet_platform::printing::{self, PrintDocument};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
pub struct PrintJob {
    directory: PathBuf,
    doc: u64,
    sheet: usize,
    revision: u64,
    generation: u64,
    print: bool,
}
pub struct Preview {
    pub focus: FocusHandle,
    pub document: Arc<PrintDocument>,
    pub page: usize,
    pub doc: u64,
    pub sheet: usize,
    pub revision: u64,
}
impl Desktop {
    pub fn close_print_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.print_generation = self.print_generation.saturating_add(1);
        self.preview = None;
        if self.print_dialog_generation.is_none() {
            self.print_status = "closed".into();
        }
        self.focus_grid(window, cx);
        cx.notify();
    }
    pub fn preview_current(&self) -> bool {
        self.preview
            .as_ref()
            .is_some_and(|p| self.draft.is_none() && self.view().is_some_and(|v| v.doc == p.doc && v.sheet == p.sheet && v.revision == p.revision))
    }
    pub fn begin_print(&mut self, mut params: Value, print: bool, cx: &mut Context<Self>) {
        self.error = false;
        let Some(view) = self.view() else { return };
        let (doc, sheet, revision) = (view.doc, view.sheet, view.revision);
        let directory = match printing::directory() {
            Ok(p) => p,
            Err(e) => {
                self.alert(e);
                return;
            }
        };
        params["path"] = json!(directory.join("document.pdf"));
        if let Some(m) = params.as_object_mut() {
            m.remove("page");
        }
        self.print_generation += 1;
        let job = PrintJob { directory, doc, sheet, revision, generation: self.print_generation, print };
        match self.engine.submit_guarded("file.exportPdf", params, self.target(), Some(revision)) {
            Ok(ticket) => {
                self.print_jobs.insert(ticket.id, job);
                self.pending.push(Pending { ticket, after: After::None, commit: false, clipboard: false });
                self.print_status = "Preparing PDF".into();
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(job.directory);
                self.alert(e);
            }
        }
        cx.notify();
    }
    pub fn finish_print_export(&mut self, job: PrintJob, result: Result<Value, String>, window: &mut Window, cx: &mut Context<Self>) {
        if self.print_generation != job.generation
            || !self.view().is_some_and(|v| v.doc == job.doc && v.sheet == job.sheet && v.revision == job.revision)
        {
            let _ = std::fs::remove_dir_all(job.directory);
            return;
        }
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(job.directory);
            self.print_status = "failed".into();
            self.alert(e);
            return;
        }
        let directory = job.directory.clone();
        self.print_rendering += 1;
        let task = cx.background_executor().spawn(async move { printing::render(directory) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.print_rendering = this.print_rendering.saturating_sub(1);
                if this.print_generation != job.generation
                    || !this.view().is_some_and(|v| v.doc == job.doc && v.sheet == job.sheet && v.revision == job.revision)
                {
                    return;
                }
                match result {
                    Ok(document) => {
                        let focus = cx.focus_handle();
                        window.focus(&focus, cx);
                        this.preview =
                            Some(Preview { focus, document: Arc::new(document), page: 0, doc: job.doc, sheet: job.sheet, revision: job.revision });
                        this.print_status = "preview ready".into();
                        if job.print {
                            this.system_print(cx);
                        }
                    }
                    Err(e) => {
                        this.print_status = "failed".into();
                        this.alert(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn system_print(&mut self, cx: &mut Context<Self>) {
        if self.print_dialog_generation.is_some() {
            return;
        }
        if !self.preview_current() {
            self.alert("预览已过期或存在草稿，请重新预览 / Preview changed; regenerate it before printing");
            cx.notify();
            return;
        }
        let Some(p) = &self.preview else { return };
        let document = p.document.clone();
        let generation = self.print_generation;
        self.print_dialog_generation = Some(generation);
        self.print_status = "dialog open".into();
        let task = cx.background_executor().spawn(async move { printing::print_dialog(&document) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.print_dialog_generation == Some(generation) {
                    this.print_dialog_generation = None;
                }
                if this.print_generation != generation || !this.preview_current() {
                    if this.print_status == "dialog open" {
                        this.print_status = "dialog finished; preview changed".into();
                    }
                    cx.notify();
                    return;
                }
                match result {
                    Ok(s) => {
                        this.print_status = s.clone();
                        this.message = s;
                    }
                    Err(e) => {
                        this.print_status = "failed".into();
                        this.alert(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub fn export_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preview_current() {
            self.alert("预览已过期，请重新预览 / Regenerate the preview before export");
            cx.notify();
            return;
        }
        let Some(p) = &self.preview else { return };
        let document = p.document.clone();
        cx.spawn_in(window, async move |this, cx| {
            if let Some(mut path) = joysheet_platform::pick_save("Joysheet.pdf").await {
                path.set_extension("pdf");
                let task = cx.background_executor().spawn(async move { printing::export(&document, &path) });
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    match result {
                        Ok(()) => this.message = "PDF 已导出 / PDF exported".into(),
                        Err(e) => this.alert(e),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub fn render_preview(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.preview.as_ref();
        let current = p.map_or(0, |p| p.page);
        let count = p.map_or(0, |p| p.document.pages.len());
        let image = p.and_then(|p| p.document.pages.get(current)).cloned();
        let stale = !self.preview_current();
        let mut panel = div()
            .occlude()
            .id("print-preview-dialog")
            .role(Role::Dialog)
            .aria_label("打印预览 / Print preview")
            .when_some(p, |panel, p| panel.track_focus(&p.focus))
            .absolute()
            .inset_0()
            .bg(rgb(0xe5e9e6))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(format!("打印预览 / Print preview  {}/{}{}", current + 1, count, if stale { " · 文档已更改，请重新预览" } else { "" }))
                    .child(gpui_kit::component::button::Button::new("preview-prev").label("←").on_click(cx.listener(|this, _, _, cx| {
                        if let Some(p) = &mut this.preview {
                            p.page = p.page.saturating_sub(1);
                        }
                        cx.notify();
                    })))
                    .child(gpui_kit::component::button::Button::new("preview-next").label("→").on_click(cx.listener(|this, _, _, cx| {
                        if let Some(p) = &mut this.preview {
                            p.page = (p.page + 1).min(p.document.pages.len().saturating_sub(1));
                        }
                        cx.notify();
                    })))
                    .child(
                        gpui_kit::component::button::Button::new("preview-print")
                            .label("系统打印… / Print…")
                            .disabled(stale || self.print_dialog_generation.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.system_print(cx))),
                    )
                    .child(
                        gpui_kit::component::button::Button::new("preview-export")
                            .label("导出 PDF…")
                            .disabled(stale)
                            .on_click(cx.listener(|this, _, w, cx| this.export_preview(w, cx))),
                    )
                    .child(gpui_kit::component::button::Button::new("preview-close").label("关闭 ×").on_click(cx.listener(|this, _, window, cx| {
                        this.close_print_preview(window, cx);
                    }))),
            );
        if let Some(path) = image {
            panel = panel
                .child(div().id("pdf-page").flex_1().min_h_0().overflow_y_scroll().flex().justify_center().child(img(path).w(px(760.)).h_auto()));
        }
        panel
    }
}
