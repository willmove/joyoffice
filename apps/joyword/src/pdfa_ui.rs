use crate::*;

impl JoyWord {
    pub(crate) fn begin_pdfa(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pdf.is_some() || self.ready_layout().is_none() {
            return;
        }
        let year = match joyword_shell::epub::modified_now()
            .ok()
            .and_then(|stamp| stamp.get(..4)?.parse::<u16>().ok())
        {
            Some(year @ 1..=9999) => year,
            _ => {
                self.report_input_error("archive year unavailable".to_owned());
                return;
            }
        };
        let epoch = self.copy_epoch;
        let version = self.editor.as_ref().map(|e| e.version());
        self.pdf = Some(input_ui::PdfPhase::Picking(epoch));
        let directory = self
            .editor
            .as_ref()
            .and_then(|e| e.source_path())
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let description = Catalog::shipped(self.locale)
            .format("pdfa.description", &[("year", &year.to_string())]);
        let confirmation = window.prompt(
            gpui::PromptLevel::Warning,
            &self.label("pdfa.review"),
            Some(&description),
            &[
                gpui::PromptButton::cancel(self.label("dialog.cancel")),
                gpui::PromptButton::new(self.label("epub.continue")),
            ],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let result = async {
                if confirmation.await.ok() != Some(1) {
                    return None;
                }
                let picker = view
                    .update_in(cx, |_, _, cx| {
                        cx.prompt_for_new_path(&directory, Some("JoyWord-archive.pdf"))
                    })
                    .ok()?;
                picker.await.ok()?.ok()?
            }
            .await;
            let _ = view.update_in(cx, |v, window, cx| {
                if v.pdf == Some(input_ui::PdfPhase::Picking(epoch)) {
                    v.pdf = None;
                }
                // A different document or a changed version must not be exported
                // under the confirmation given for the earlier snapshot.
                if v.copy_epoch == epoch
                    && v.editor.as_ref().map(|e| e.version()) == version
                    && let Some(destination) = result
                    && let Err(ExecuteError::Failed(error)) = v.queue_pdf(
                        input_ui::PdfOrder {
                            destination,
                            print: false,
                            copies: None,
                            range: None,
                            marks: None,
                            archive_year: Some(year),
                        },
                        window,
                        cx,
                    )
                {
                    v.report_input_error(error);
                }
                window.focus(&v.focus_handle, cx);
                cx.notify();
            });
        })
        .detach();
    }
}
