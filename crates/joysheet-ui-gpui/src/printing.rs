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

    fn print_choice(&self, id: &str, value: String, items: Vec<(&str, &str, Value)>, cx: &mut Context<Self>) -> AnyElement {
        let owner = cx.weak_entity();
        let options = items.iter().map(|(k, l, _)| (k.to_string(), l.to_string())).collect();
        let values = items.into_iter().map(|(k, _, v)| (k.to_string(), v)).collect::<std::collections::HashMap<_, _>>();
        crate::excel_workflows::choice(
            id.into(),
            value,
            options,
            std::rc::Rc::new(move |key, _, cx| {
                let _ = owner.update(cx, |v, cx| {
                    if let Some(params) = values.get(&key) {
                        v.issue("pageLayout.pageSetup", params.clone(), After::RefreshPrint, cx);
                    }
                });
            }),
            cx.weak_entity(),
        )
    }
    pub fn render_preview(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        use gpui_kit::component::button::{Button, ButtonVariants};
        let p = self.preview.as_ref();
        let current = p.map_or(0, |p| p.page);
        let count = p.map_or(0, |p| p.document.pages.len());
        let image = p.and_then(|p| p.document.pages.get(current)).cloned();
        let stale = !self.preview_current();
        let settings = self.view().and_then(|v| v.sheet()).map(|s| s.print.clone()).unwrap_or_default();
        let mut controls = div()
            .id("print-settings")
            .w(px(300.))
            .flex_shrink_0()
            .p_6()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(self.palette().surface))
            .child(div().text_size(px(28.)).child(self.label("打印", "Print")))
            .child(
                Button::new("preview-print")
                    .relative()
                    .child(self.advanced_probe("preview-print".into(), cx))
                    .label(self.label("打印…", "Print…"))
                    .disabled(stale || self.print_dialog_generation.is_some())
                    .on_click(cx.listener(|v, _, _, cx| v.system_print(cx))),
            )
            .child(self.label("打印机", "Printer"))
            .child(self.label("使用系统打印对话框选择打印机和份数", "Select printer and copies in the system print dialog"))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(self.label("设置", "Settings")));
        let mut scope = div().flex().flex_col().gap_1();
        for (key, zh, en) in [
            ("active", "打印活动工作表", "Print Active Sheets"),
            ("workbook", "打印整个工作簿", "Print Entire Workbook"),
            ("selection", "打印选定区域", "Print Selection"),
        ] {
            scope = scope.child(Button::new(key).label(self.label(zh, en)).on_click(cx.listener(move |v, _, _, cx| {
                v.excel_ui.print_params["sheets"] = json!(if key == "workbook" { "all" } else { "active" });
                if key == "selection" {
                    v.excel_ui.print_params["range"] = json!(v.selection().a1());
                } else {
                    v.excel_ui.print_params.as_object_mut().map(|m| m.remove("range"));
                }
                v.begin_print(v.excel_ui.print_params.clone(), false, cx);
            })));
        }
        controls = controls.child(scope);
        controls = controls.child(self.print_choice(
            "print-orientation",
            if settings.orientation == joysheet_engine::model::Orientation::Landscape { "landscape" } else { "portrait" }.into(),
            vec![
                ("portrait", "纵向 / Portrait Orientation", json!({"orientation":"portrait"})),
                ("landscape", "横向 / Landscape Orientation", json!({"orientation":"landscape"})),
            ],
            cx,
        ));
        controls = controls.child(self.print_choice(
            "print-paper",
            settings.paper.clone(),
            vec![
                ("A4", "A4", json!({"paper":"A4"})),
                ("A3", "A3", json!({"paper":"A3"})),
                ("Letter", "Letter", json!({"paper":"Letter"})),
                ("Legal", "Legal", json!({"paper":"Legal"})),
            ],
            cx,
        ));
        controls = controls.child(self.print_choice(
            "print-margins",
            "custom".into(),
            vec![
                ("custom", "页边距 / Margins", json!({})),
                ("normal", "普通 / Normal", json!({"margins":{"left":0.7,"right":0.7,"top":0.75,"bottom":0.75}})),
                ("wide", "宽 / Wide", json!({"margins":{"left":1.,"right":1.,"top":1.,"bottom":1.}})),
                ("narrow", "窄 / Narrow", json!({"margins":{"left":0.25,"right":0.25,"top":0.75,"bottom":0.75}})),
            ],
            cx,
        ));
        controls = controls.child(self.print_choice(
            "print-scaling",
            if settings.fit_to.is_some() { "fit" } else { "actual" }.into(),
            vec![
                ("actual", "无缩放 / No Scaling", json!({"scale":100})),
                ("fit", "将工作表调整为一页 / Fit Sheet on One Page", json!({"fitWidth":1,"fitHeight":1})),
                ("width", "将所有列调整为一页 / Fit All Columns on One Page", json!({"fitWidth":1,"fitHeight":0})),
                ("height", "将所有行调整为一页 / Fit All Rows on One Page", json!({"fitWidth":0,"fitHeight":1})),
            ],
            cx,
        ));
        controls = controls.child(
            Button::new("preview-export")
                .relative()
                .child(self.advanced_probe("preview-export".into(), cx))
                .label(self.label("导出 PDF…", "Export PDF…"))
                .disabled(stale)
                .on_click(cx.listener(|v, _, w, cx| v.export_preview(w, cx))),
        );
        let nav = div()
            .w(px(120.))
            .h_full()
            .bg(rgb(self.palette().header))
            .p_3()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                Button::new("preview-close")
                    .relative()
                    .child(self.advanced_probe("preview-close".into(), cx))
                    .ghost()
                    .label(self.label("← 返回", "← Back"))
                    .on_click(cx.listener(|v, _, w, cx| v.close_print_preview(w, cx))),
            )
            .child(self.label("打印", "Print"));
        let mut preview = div().flex_1().min_w_0().p_4().flex().flex_col().gap_3();
        preview = preview.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    Button::new("preview-prev")
                        .relative()
                        .child(self.advanced_probe("preview-prev".into(), cx))
                        .label("←")
                        .disabled(current == 0)
                        .on_click(cx.listener(|v, _, _, cx| {
                            if let Some(p) = &mut v.preview {
                                p.page = p.page.saturating_sub(1);
                            }
                            cx.notify();
                        })),
                )
                .child(format!("{} / {}", current + 1, count))
                .child(
                    Button::new("preview-next")
                        .relative()
                        .child(self.advanced_probe("preview-next".into(), cx))
                        .label("→")
                        .disabled(current + 1 >= count)
                        .on_click(cx.listener(|v, _, _, cx| {
                            if let Some(p) = &mut v.preview {
                                p.page = (p.page + 1).min(p.document.pages.len().saturating_sub(1));
                            }
                            cx.notify();
                        })),
                )
                .when(stale, |el| el.child(self.label("正在更新预览…", "Updating preview…"))),
        );
        if let Some(path) = image {
            preview = preview
                .child(div().id("pdf-page").flex_1().min_h_0().overflow_y_scroll().flex().justify_center().child(img(path).w(px(650.)).h_auto()));
        }
        div()
            .occlude()
            .id("print-preview-dialog")
            .role(Role::Dialog)
            .aria_label(self.label("打印", "Print"))
            .when_some(p, |el, p| el.track_focus(&p.focus))
            .absolute()
            .inset_0()
            .bg(rgb(0xe5e5e5))
            .flex()
            .child(nav)
            .child(controls)
            .child(preview)
    }
}
